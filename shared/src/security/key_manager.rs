use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::info;

/// JWT 키 로테이션 관리자
pub struct KeyManager {
    current_key: Arc<RwLock<KeyInfo>>,
    previous_keys: Arc<RwLock<Vec<KeyInfo>>>,
    rotation_interval: Duration,
    max_old_keys: usize,
}

/// 키 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key_id: String,
    pub key: String,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub algorithm: String,
}

impl KeyInfo {
    pub fn new(key: String, algorithm: String, lifetime_hours: Option<u64>) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Operation failed")
            .as_secs();

        let expires_at = lifetime_hours.map(|hours| created_at + (hours * 3600));

        // 키 ID 생성 (키의 해시)
        let mut hasher = Sha256::new();
        hasher.update(&key);
        hasher.update(created_at.to_be_bytes());
        let key_id = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

        Self {
            key_id: key_id[..16].to_string(), // 처음 16자만 사용
            key,
            created_at,
            expires_at,
            algorithm,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Operation failed")
                .as_secs();
            now >= expires_at
        } else {
            false
        }
    }
}

impl KeyManager {
    /// 새 키 매니저 생성
    pub fn new(initial_key: String, rotation_hours: u64) -> Self {
        let key_info = KeyInfo::new(
            initial_key,
            "HS256".to_string(),
            Some(rotation_hours * 2), // 키 수명은 로테이션 주기의 2배
        );

        info!("🔐 Key Manager initialized");
        info!("  └─ Key ID: {}", key_info.key_id);
        info!("  └─ Rotation: every {} hours", rotation_hours);

        let manager = Self {
            current_key: Arc::new(RwLock::new(key_info)),
            previous_keys: Arc::new(RwLock::new(Vec::new())),
            rotation_interval: Duration::from_secs(rotation_hours * 3600),
            max_old_keys: 3, // 최대 3개의 이전 키 보관
        };

        // 자동 로테이션 시작
        manager.start_auto_rotation();

        manager
    }

    /// 환경 변수에서 키 매니저 생성
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let key = std::env::var("JWT_SECRET_KEY").map_err(|_| "JWT_SECRET_KEY not set")?;

        let rotation_hours = std::env::var("JWT_KEY_ROTATION_HOURS")
            .unwrap_or_else(|_| "168".to_string()) // 기본값: 1주일
            .parse::<u64>()
            .unwrap_or(168);

        // 키 검증
        if key.len() < 32 {
            return Err("JWT_SECRET_KEY must be at least 32 characters".into());
        }

        Ok(Self::new(key, rotation_hours))
    }

    /// 현재 키 가져오기
    pub async fn get_current_key(&self) -> KeyInfo {
        self.current_key.read().await.clone()
    }

    /// 키 ID로 키 가져오기 (검증용)
    pub async fn get_key_by_id(&self, key_id: &str) -> Option<KeyInfo> {
        // 현재 키 확인
        let current = self.current_key.read().await;
        if current.key_id == key_id {
            return Some(current.clone());
        }
        drop(current);

        // 이전 키들 확인
        let previous = self.previous_keys.read().await;
        previous
            .iter()
            .find(|k| k.key_id == key_id && !k.is_expired())
            .cloned()
    }

    /// 모든 유효한 키 가져오기
    pub async fn get_all_valid_keys(&self) -> Vec<KeyInfo> {
        let mut keys = vec![self.current_key.read().await.clone()];

        let previous = self.previous_keys.read().await;
        for key in previous.iter() {
            if !key.is_expired() {
                keys.push(key.clone());
            }
        }

        keys
    }

    /// 키 로테이션
    pub async fn rotate_key(&self) -> Result<KeyInfo, Box<dyn std::error::Error>> {
        // 새 키 생성
        let new_key = self.generate_secure_key();
        let new_key_info = KeyInfo::new(
            new_key,
            "HS256".to_string(),
            Some((self.rotation_interval.as_secs() / 3600) * 2),
        );

        // 현재 키를 이전 키 목록으로 이동
        let old_key = {
            let mut current = self.current_key.write().await;
            let old = current.clone();
            *current = new_key_info.clone();
            old
        };

        // 이전 키 목록 업데이트
        {
            let mut previous = self.previous_keys.write().await;
            previous.insert(0, old_key);

            // 만료된 키와 초과 키 제거
            previous.retain(|k| !k.is_expired());
            if previous.len() > self.max_old_keys {
                previous.truncate(self.max_old_keys);
            }
        }

        info!("🔄 Key rotated successfully");
        info!("  └─ New Key ID: {}", new_key_info.key_id);
        info!(
            "  └─ Old keys retained: {}",
            self.previous_keys.read().await.len()
        );

        Ok(new_key_info)
    }

    /// 안전한 랜덤 키 생성
    fn generate_secure_key(&self) -> String {
        use rand::distributions::Alphanumeric;
        use rand::{thread_rng, Rng};

        let key: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        // 추가 엔트로피
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Operation failed")
            .as_nanos();

        let mut hasher = Sha256::new();
        hasher.update(&key);
        hasher.update(timestamp.to_be_bytes());

        general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
    }

    /// 자동 키 로테이션 시작
    fn start_auto_rotation(&self) {
        let current_key = self.current_key.clone();
        let previous_keys = self.previous_keys.clone();
        let rotation_interval = self.rotation_interval;
        let max_old_keys = self.max_old_keys;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(rotation_interval);

            loop {
                interval.tick().await;

                // 새 키 생성
                let new_key = Self::generate_secure_key_static();
                let new_key_info = KeyInfo::new(
                    new_key,
                    "HS256".to_string(),
                    Some((rotation_interval.as_secs() / 3600) * 2),
                );

                // 키 로테이션
                let old_key = {
                    let mut current = current_key.write().await;
                    let old = current.clone();
                    *current = new_key_info.clone();
                    old
                };

                // 이전 키 목록 업데이트
                {
                    let mut previous = previous_keys.write().await;
                    previous.insert(0, old_key);

                    // 정리
                    previous.retain(|k| !k.is_expired());
                    if previous.len() > max_old_keys {
                        previous.truncate(max_old_keys);
                    }
                }

                info!("🔄 Automatic key rotation completed");
                info!("  └─ New Key ID: {}", new_key_info.key_id);
            }
        });
    }

    /// 정적 키 생성 함수 (spawn 내부용)
    fn generate_secure_key_static() -> String {
        use rand::distributions::Alphanumeric;
        use rand::{thread_rng, Rng};

        let key: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Operation failed")
            .as_nanos();

        let mut hasher = Sha256::new();
        hasher.update(&key);
        hasher.update(timestamp.to_be_bytes());

        general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
    }

    /// 키 상태 정보
    pub async fn get_status(&self) -> KeyManagerStatus {
        let current = self.current_key.read().await;
        let previous = self.previous_keys.read().await;

        let valid_previous_keys = previous.iter().filter(|k| !k.is_expired()).count();

        let expired_previous_keys = previous.iter().filter(|k| k.is_expired()).count();

        KeyManagerStatus {
            current_key_id: current.key_id.clone(),
            current_key_age_hours: ((SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Operation failed")
                .as_secs()
                - current.created_at)
                / 3600) as u32,
            total_previous_keys: previous.len(),
            valid_previous_keys,
            expired_previous_keys,
            rotation_interval_hours: (self.rotation_interval.as_secs() / 3600) as u32,
        }
    }
}

/// 키 매니저 상태
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyManagerStatus {
    pub current_key_id: String,
    pub current_key_age_hours: u32,
    pub total_previous_keys: usize,
    pub valid_previous_keys: usize,
    pub expired_previous_keys: usize,
    pub rotation_interval_hours: u32,
}

mod tests {
    
    

    #[tokio::test]
    async fn test_key_rotation() {
        let manager = KeyManager::new("test_key".to_string(), 24);

        let initial_key = manager.get_current_key().await;
        assert_eq!(initial_key.algorithm, "HS256");

        // 키 로테이션
        let new_key = manager.rotate_key().await.expect("Failed to rotate key");
        assert_ne!(initial_key.key_id, new_key.key_id);

        // 이전 키도 여전히 유효해야 함
        let old_key = manager.get_key_by_id(&initial_key.key_id).await;
        assert!(old_key.is_some());
    }

    #[tokio::test]
    async fn test_key_expiration() {
        let key_info = KeyInfo::new(
            "test_key".to_string(),
            "HS256".to_string(),
            Some(0), // 즉시 만료
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(key_info.is_expired());
    }

    #[tokio::test]
    async fn test_multiple_keys() {
        let manager = KeyManager::new("test_key".to_string(), 24);

        // 여러 번 로테이션
        for _ in 0..5 {
            manager.rotate_key().await.expect("Failed to rotate key in loop");
        }

        let all_keys = manager.get_all_valid_keys().await;
        assert!(all_keys.len() <= 4); // 현재 키 + 최대 3개 이전 키
    }
}
