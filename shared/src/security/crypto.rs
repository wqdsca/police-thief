//! 암호화 및 해싱 유틸리티
//!
//! - bcrypt 비밀번호 해싱
//! - AES-256-GCM 데이터 암호화 (Redis 데이터 보호)
//! - 보안 랜덤 생성
//! - 키 파생 (PBKDF2)

use crate::security::{SecurityConfig, SecurityError};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use pbkdf2::pbkdf2_hmac_array;
use rand::{thread_rng, Rng};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 암호화된 데이터 구조체
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedData {
    /// Base64 인코딩된 암호화 데이터
    pub data: String,
    /// Base64 인코딩된 nonce
    pub nonce: String,
    /// 암호화에 사용된 키 ID
    pub key_id: String,
}

impl EncryptedData {
    /// Redis 저장용 JSON 문자열로 변환
    pub fn to_redis_string(&self) -> Result<String, SecurityError> {
        serde_json::to_string(self).map_err(|e| {
            SecurityError::EncryptionFailed(format!("Failed to serialize encrypted data: {e}"))
        })
    }

    /// Redis 문자열에서 복원
    pub fn from_redis_string(data: &str) -> Result<Self, SecurityError> {
        serde_json::from_str(data).map_err(|e| {
            SecurityError::EncryptionFailed(format!("Failed to deserialize encrypted data: {e}"))
        })
    }
}

/// 암호화 관리자
pub struct CryptoManager {
    config: SecurityConfig,
    /// 키 저장소 (key_id -> derived_key)
    keys: HashMap<String, Vec<u8>>,
    /// 현재 사용 중인 키 ID
    current_key_id: String,
}

impl CryptoManager {
    /// 새 암호화 관리자 생성
    pub fn new(config: SecurityConfig) -> Result<Self, SecurityError> {
        let mut manager = Self {
            config,
            keys: HashMap::new(),
            current_key_id: "default".to_string(),
        };

        // 기본 키 생성
        manager.initialize_default_key()?;
        
        Ok(manager)
    }

    /// 기본 암호화 키 초기화
    fn initialize_default_key(&mut self) -> Result<(), SecurityError> {
        let salt = b"police_thief_salt_v1"; // 프로덕션에서는 환경변수로
        let derived_key = self.derive_key(&self.config.jwt_secret, salt)?;
        
        self.keys.insert(self.current_key_id.clone(), derived_key);
        
        tracing::info!("🔐 Redis encryption initialized with key ID: {}", self.current_key_id);
        Ok(())
    }

    /// PBKDF2를 사용한 키 파생
    fn derive_key(&self, password: &str, salt: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let key: [u8; 32] = pbkdf2_hmac_array::<Sha256, 32>(password.as_bytes(), salt, 10000);
        Ok(key.to_vec())
    }

    /// 새 암호화 키 추가 (키 로테이션용)
    pub fn add_key(&mut self, key_id: String, password: &str, salt: &[u8]) -> Result<(), SecurityError> {
        let derived_key = self.derive_key(password, salt)?;
        self.keys.insert(key_id.clone(), derived_key);
        
        tracing::info!("🔄 New encryption key added: {}", key_id);
        Ok(())
    }

    /// 현재 키 변경 (새 데이터 암호화용)
    pub fn set_current_key(&mut self, key_id: String) -> Result<(), SecurityError> {
        if !self.keys.contains_key(&key_id) {
            return Err(SecurityError::InvalidToken(format!("Key ID not found: {}", key_id)));
        }
        
        self.current_key_id = key_id;
        tracing::info!("🔑 Current encryption key changed to: {}", self.current_key_id);
        Ok(())
    }

    /// Redis 데이터 암호화 (ChaCha20Poly1305)
    pub fn encrypt_redis_data(&self, plaintext: &str) -> Result<EncryptedData, SecurityError> {
        let key = self.keys.get(&self.current_key_id)
            .ok_or_else(|| SecurityError::InvalidToken("Current encryption key not found".to_string()))?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| SecurityError::EncryptionFailed(format!("ChaCha20 encryption failed: {e}")))?;

        Ok(EncryptedData {
            data: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(nonce),
            key_id: self.current_key_id.clone(),
        })
    }

    /// Redis 데이터 복호화 (ChaCha20Poly1305)
    pub fn decrypt_redis_data(&self, encrypted_data: &EncryptedData) -> Result<String, SecurityError> {
        let key = self.keys.get(&encrypted_data.key_id)
            .ok_or_else(|| SecurityError::InvalidToken(format!("Encryption key not found: {}", encrypted_data.key_id)))?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        
        let ciphertext = general_purpose::STANDARD
            .decode(&encrypted_data.data)
            .map_err(|e| SecurityError::EncryptionFailed(format!("Base64 decode failed: {e}")))?;
            
        let nonce_bytes = general_purpose::STANDARD
            .decode(&encrypted_data.nonce)
            .map_err(|e| SecurityError::EncryptionFailed(format!("Nonce decode failed: {e}")))?;
            
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| SecurityError::EncryptionFailed(format!("ChaCha20 decryption failed: {e}")))?;

        String::from_utf8(plaintext)
            .map_err(|e| SecurityError::EncryptionFailed(format!("UTF-8 conversion failed: {e}")))
    }

    /// Redis 데이터 암호화 (JSON 직렬화 포함)
    pub fn encrypt_for_redis<T: serde::Serialize>(&self, data: &T) -> Result<String, SecurityError> {
        let json_string = serde_json::to_string(data)
            .map_err(|e| SecurityError::EncryptionFailed(format!("JSON serialization failed: {e}")))?;
            
        let encrypted = self.encrypt_redis_data(&json_string)?;
        encrypted.to_redis_string()
    }

    /// Redis 데이터 복호화 (JSON 역직렬화 포함)
    pub fn decrypt_from_redis<T: serde::de::DeserializeOwned>(&self, encrypted_string: &str) -> Result<T, SecurityError> {
        let encrypted_data = EncryptedData::from_redis_string(encrypted_string)?;
        let json_string = self.decrypt_redis_data(&encrypted_data)?;
        
        serde_json::from_str(&json_string)
            .map_err(|e| SecurityError::EncryptionFailed(format!("JSON deserialization failed: {e}")))
    }

    /// 비밀번호 해싱 (bcrypt)
    pub fn hash_password(&self, password: &str) -> Result<String, SecurityError> {
        bcrypt::hash(password, self.config.bcrypt_rounds)
            .map_err(|e| SecurityError::EncryptionFailed(format!("Password hashing failed: {e}")))
    }

    /// 비밀번호 검증
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, SecurityError> {
        bcrypt::verify(password, hash).map_err(|e| {
            SecurityError::EncryptionFailed(format!("Password verification failed: {e}"))
        })
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
        Self::new(SecurityConfig::default()).expect("Failed to initialize default CryptoManager")
    }
}

mod tests {

    #[test]
    fn test_password_hashing() {
        let crypto = CryptoManager::default();
        let password = "test_password_123";

        let hash = crypto
            .hash_password(password)
            .expect("Test assertion failed");
        assert!(crypto
            .verify_password(password, &hash)
            .expect("Test assertion failed"));
        assert!(!crypto
            .verify_password("wrong_password", &hash)
            .expect("Test assertion failed"));
    }

    #[test]
    fn test_secure_token_generation() {
        let crypto = CryptoManager::default();

        let token1 = crypto.generate_secure_token(16);
        let token2 = crypto.generate_secure_token(16);

        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 32); // 16 bytes = 32 hex chars
    }

    #[test]
    fn test_redis_data_encryption() {
        let crypto = CryptoManager::default();
        let test_data = "sensitive_user_data_12345";

        // 암호화
        let encrypted = crypto.encrypt_redis_data(test_data).expect("Encryption failed");
        assert_ne!(encrypted.data, test_data);
        assert!(!encrypted.nonce.is_empty());
        assert_eq!(encrypted.key_id, "default");

        // 복호화
        let decrypted = crypto.decrypt_redis_data(&encrypted).expect("Decryption failed");
        assert_eq!(decrypted, test_data);
    }

    #[test]
    fn test_redis_json_encryption() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct TestData {
            user_id: u64,
            session_token: String,
            metadata: HashMap<String, String>,
        }

        let crypto = CryptoManager::default();
        let mut metadata = HashMap::new();
        metadata.insert("role".to_string(), "admin".to_string());
        
        let test_data = TestData {
            user_id: 12345,
            session_token: "secret_session_token".to_string(),
            metadata,
        };

        // JSON 암호화
        let encrypted_string = crypto.encrypt_for_redis(&test_data).expect("JSON encryption failed");
        assert!(!encrypted_string.contains("secret_session_token"));

        // JSON 복호화
        let decrypted_data: TestData = crypto.decrypt_from_redis(&encrypted_string).expect("JSON decryption failed");
        assert_eq!(decrypted_data, test_data);
    }

    #[test]
    fn test_key_rotation() {
        let mut crypto = CryptoManager::default();
        let test_data = "key_rotation_test_data";

        // 기본 키로 암호화
        let encrypted_v1 = crypto.encrypt_redis_data(test_data).expect("V1 encryption failed");
        assert_eq!(encrypted_v1.key_id, "default");

        // 새 키 추가
        crypto.add_key("v2".to_string(), "new_password_for_v2", b"new_salt_v2").expect("Add key failed");
        crypto.set_current_key("v2".to_string()).expect("Set key failed");

        // 새 키로 암호화
        let encrypted_v2 = crypto.encrypt_redis_data(test_data).expect("V2 encryption failed");
        assert_eq!(encrypted_v2.key_id, "v2");
        assert_ne!(encrypted_v1.data, encrypted_v2.data);

        // 두 버전 모두 복호화 가능
        let decrypted_v1 = crypto.decrypt_redis_data(&encrypted_v1).expect("V1 decryption failed");
        let decrypted_v2 = crypto.decrypt_redis_data(&encrypted_v2).expect("V2 decryption failed");
        
        assert_eq!(decrypted_v1, test_data);
        assert_eq!(decrypted_v2, test_data);
    }
}
