-- 소셜 로그인 계정 정보 테이블
CREATE TABLE IF NOT EXISTS social_accounts (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    provider ENUM('Kakao', 'Google', 'Apple') NOT NULL,
    provider_id VARCHAR(255) NOT NULL,
    email VARCHAR(255),
    profile_image TEXT,
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    UNIQUE KEY unique_provider_account (provider, provider_id),
    INDEX idx_user_id (user_id),
    
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 기존 users 테이블에 소셜 로그인 관련 컬럼 추가 (이미 없다면)
ALTER TABLE users 
ADD COLUMN IF NOT EXISTS login_type ENUM('normal', 'social') DEFAULT 'normal',
ADD COLUMN IF NOT EXISTS email_verified BOOLEAN DEFAULT FALSE;