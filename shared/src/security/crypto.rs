//! 암호화 및 해싱 유틸리티
//! 
//! - bcrypt 비밀번호 해싱
//! - AES 데이터 암호화
//! - 보안 랜덤 생성

use crate::security::{SecurityConfig, SecurityError};
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};

/// 암호화 관리자
pub struct CryptoManager {
    config: SecurityConfig,
}

impl CryptoManager {
    /// 새 암호화 관리자 생성
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }
    
    /// 비밀번호 해싱 (bcrypt)
    pub fn hash_password(&self, password: &str) -> Result<String, SecurityError> {
        bcrypt::hash(password, self.config.bcrypt_rounds)
            .map_err(|e| SecurityError::EncryptionFailed(format!("Password hashing failed: {}", e)))
    }
    
    /// 비밀번호 검증
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, SecurityError> {
        bcrypt::verify(password, hash)
            .map_err(|e| SecurityError::EncryptionFailed(format!("Password verification failed: {}", e)))
    }
    
    /// SHA-256 해싱
    pub fn sha256_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
    
    /// 보안 랜덤 바이트 생성
    pub fn generate_random_bytes(&self, length: usize) -> Vec<u8> {
        let mut rng = thread_rng();
        (0..length).map(|_| rng.gen()).collect()
    }
    
    /// 보안 토큰 생성 (hex)
    pub fn generate_secure_token(&self, length: usize) -> String {
        let bytes = self.generate_random_bytes(length);
        hex::encode(bytes)
    }
    
    /// API 키 생성
    pub fn generate_api_key(&self) -> String {
        format!("pk_{}", self.generate_secure_token(32))
    }
    
    /// 세션 ID 생성
    pub fn generate_session_id(&self) -> String {
        format!("sess_{}", self.generate_secure_token(16))
    }
}

impl Default for CryptoManager {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_password_hashing() {
        let crypto = CryptoManager::default();
        let password = "test_password_123";
        
        let hash = crypto.hash_password(password).unwrap();
        assert!(crypto.verify_password(password, &hash).unwrap());
        assert!(!crypto.verify_password("wrong_password", &hash).unwrap());
    }
    
    #[test]
    fn test_secure_token_generation() {
        let crypto = CryptoManager::default();
        
        let token1 = crypto.generate_secure_token(16);
        let token2 = crypto.generate_secure_token(16);
        
        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 32); // 16 bytes = 32 hex chars
    }
}