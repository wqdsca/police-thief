//! JWT Key Rotation System
//! 
//! Provides automatic key rotation for enhanced security.
//! Keys are rotated periodically and old keys are kept for grace period.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tracing::{info, warn};

/// Key rotation configuration
#[derive(Debug, Clone)]
pub struct KeyRotationConfig {
    /// How often to rotate keys (in hours)
    pub rotation_interval_hours: i64,
    /// How many old keys to keep for validation
    pub max_old_keys: usize,
    /// Grace period for old keys (in hours)
    pub grace_period_hours: i64,
}

impl Default for KeyRotationConfig {
    fn default() -> Self {
        Self {
            rotation_interval_hours: 24 * 7, // Weekly rotation
            max_old_keys: 3,
            grace_period_hours: 24, // 1 day grace period
        }
    }
}

/// A signing key with metadata
#[derive(Debug, Clone)]
struct SigningKey {
    pub key: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub key_id: String,
}

/// JWT Key Rotation Manager
pub struct KeyRotationManager {
    current_key: Arc<RwLock<SigningKey>>,
    old_keys: Arc<RwLock<VecDeque<SigningKey>>>,
    config: KeyRotationConfig,
}

impl KeyRotationManager {
    /// Create a new key rotation manager
    pub fn new(initial_key: String, config: KeyRotationConfig) -> Self {
        let now = Utc::now();
        let initial_signing_key = SigningKey {
            key: initial_key,
            created_at: now,
            expires_at: now + Duration::hours(config.rotation_interval_hours),
            key_id: generate_key_id(),
        };
        
        Self {
            current_key: Arc::new(RwLock::new(initial_signing_key)),
            old_keys: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_old_keys))),
            config,
        }
    }
    
    /// Rotate to a new key
    pub fn rotate_key(&self) -> Result<()> {
        let new_key = generate_secure_key();
        let now = Utc::now();
        
        let new_signing_key = SigningKey {
            key: new_key,
            created_at: now,
            expires_at: now + Duration::hours(self.config.rotation_interval_hours),
            key_id: generate_key_id(),
        };
        
        // Move current key to old keys
        let mut current = self.current_key.write();
        let mut old = self.old_keys.write();
        
        old.push_front(current.clone());
        
        // Remove expired old keys
        while old.len() > self.config.max_old_keys {
            old.pop_back();
        }
        
        // Remove keys past grace period
        let grace_cutoff = now - Duration::hours(self.config.grace_period_hours);
        old.retain(|key| key.created_at > grace_cutoff);
        
        // Set new current key
        *current = new_signing_key;
        
        info!(
            "Key rotated successfully. New key ID: {}, Old keys count: {}",
            current.key_id,
            old.len()
        );
        
        Ok(())
    }
    
    /// Get the current encoding key for signing
    pub fn get_encoding_key(&self) -> EncodingKey {
        let current = self.current_key.read();
        EncodingKey::from_secret(current.key.as_bytes())
    }
    
    /// Get all valid decoding keys (current + old within grace period)
    pub fn get_decoding_keys(&self) -> Vec<DecodingKey> {
        let mut keys = Vec::new();
        
        // Add current key
        let current = self.current_key.read();
        keys.push(DecodingKey::from_secret(current.key.as_bytes()));
        
        // Add old keys
        let old = self.old_keys.read();
        for key in old.iter() {
            keys.push(DecodingKey::from_secret(key.key.as_bytes()));
        }
        
        keys
    }
    
    /// Check if rotation is needed
    pub fn needs_rotation(&self) -> bool {
        let current = self.current_key.read();
        Utc::now() > current.expires_at
    }
    
    /// Get current key ID for inclusion in JWT header
    pub fn get_current_key_id(&self) -> String {
        self.current_key.read().key_id.clone()
    }
    
    /// Validate a token with automatic key selection
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let keys = self.get_decoding_keys();
        
        // Try each key until one works
        for (i, key) in keys.iter().enumerate() {
            match jsonwebtoken::decode::<Claims>(
                token,
                key,
                &Validation::default()
            ) {
                Ok(token_data) => {
                    if i > 0 {
                        warn!("Token validated with old key #{}", i);
                    }
                    return Ok(token_data.claims);
                }
                Err(_) => continue,
            }
        }
        
        Err(anyhow::anyhow!("Token validation failed with all keys"))
    }
    
    /// Generate a new token with the current key
    pub fn generate_token(&self, user_id: i32) -> Result<String> {
        let claims = Claims {
            sub: user_id,
            exp: (Utc::now() + Duration::hours(24)).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            kid: self.get_current_key_id(),
        };
        
        let mut header = Header::default();
        header.kid = Some(self.get_current_key_id());
        
        let token = jsonwebtoken::encode(
            &header,
            &claims,
            &self.get_encoding_key()
        )?;
        
        Ok(token)
    }
    
    /// Start automatic rotation task
    pub fn start_auto_rotation(self: Arc<Self>) {
        tokio::spawn(async move {
            let check_interval = Duration::hours(1);
            let mut interval = tokio::time::interval(check_interval.to_std()?);
            
            loop {
                interval.tick().await;
                
                if self.needs_rotation() {
                    match self.rotate_key() {
                        Ok(_) => info!("Automatic key rotation completed"),
                        Err(e) => warn!("Automatic key rotation failed: {}", e),
                    }
                }
            }
        });
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i32,      // User ID
    pub exp: usize,    // Expiration time
    pub iat: usize,    // Issued at
    pub kid: String,   // Key ID
}

/// Generate a secure random key
fn generate_secure_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let key_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    base64::encode(key_bytes)
}

/// Generate a unique key ID
fn generate_key_id() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_rotation() {
        let config = KeyRotationConfig {
            rotation_interval_hours: 1,
            max_old_keys: 2,
            grace_period_hours: 1,
        };
        
        let manager = KeyRotationManager::new("initial_key".to_string(), config);
        
        // Initial state
        assert!(!manager.needs_rotation());
        
        // Rotate key
        manager.rotate_key().expect("Failed to rotate key in test");
        
        // Check old keys
        let old_keys = manager.old_keys.read();
        assert_eq!(old_keys.len(), 1);
    }
    
    #[tokio::test]
    async fn test_token_generation_and_validation() {
        let manager = KeyRotationManager::new(
            "test_secret_key_256_bits_minimum".to_string(),
            KeyRotationConfig::default()
        );
        
        // Generate token
        let token = manager.generate_token(123)?;
        assert!(!token.is_empty());
        
        // Validate token
        let claims = manager.validate_token(&token)?;
        assert_eq!(claims.sub, 123);
    }
    
    #[test]
    fn test_key_rotation_with_grace_period() {
        let config = KeyRotationConfig {
            rotation_interval_hours: 1,
            max_old_keys: 3,
            grace_period_hours: 2,
        };
        
        let manager = KeyRotationManager::new("key1".to_string(), config);
        
        // Rotate multiple times
        for i in 0..5 {
            manager.rotate_key().expect(&format!("Failed to rotate key on iteration {}", i));
            let old_keys = manager.old_keys.read();
            assert!(old_keys.len() <= 3, "Rotation {}: old keys count exceeded max", i);
        }
    }
}