-- ======================================================
-- Simplified Police Thief Game Database Schema
-- Features: Login, Friends, Chat Only
-- Database: MariaDB 10.6+
-- Character Set: utf8mb4
-- ======================================================

-- Database 생성
CREATE DATABASE IF NOT EXISTS police_thief
    CHARACTER SET utf8mb4
    COLLATE utf8mb4_unicode_ci;

USE police_thief;

-- ======================================================
-- 1. USER MANAGEMENT (로그인 기능)
-- ======================================================

-- 사용자 기본 정보
CREATE TABLE users (
    user_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    username VARCHAR(50) NOT NULL,
    nickname VARCHAR(100) NOT NULL,
    avatar_url VARCHAR(500) NULL,

    -- 소셜 로그인 정보
    social_provider VARCHAR(50) NULL COMMENT 'google, apple, kakao, etc.',
    social_id VARCHAR(255) NULL COMMENT 'Social provider user ID',

    -- 게임 통계 (NULL 허용)
    level INT UNSIGNED NULL DEFAULT NULL,
    experience BIGINT UNSIGNED NULL DEFAULT NULL,
    total_games INT UNSIGNED NULL DEFAULT NULL,
    win_count INT UNSIGNED NULL DEFAULT NULL,
    lose_count INT UNSIGNED NULL DEFAULT NULL,

    -- 계산된 승률 (Generated Column)
    win_rate DECIMAL(5,2) GENERATED ALWAYS AS (
        CASE
            WHEN total_games IS NULL OR total_games = 0 THEN NULL
            ELSE (win_count * 100.0 / total_games)
        END
    ) STORED,

    -- 상태 관리
    status ENUM('active', 'banned', 'suspended', 'deleted') DEFAULT 'active',
    last_login_at TIMESTAMP NULL,
    last_login_ip VARCHAR(45) NULL,

    -- 타임스탬프
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP NULL,

    PRIMARY KEY (user_id),
    UNIQUE KEY uk_username (username),
    UNIQUE KEY uk_social_login (social_provider, social_id),
    KEY idx_status (status),
    KEY idx_last_login (last_login_at),
    KEY idx_level (level),
    KEY idx_win_rate (win_rate),
    KEY idx_created_at (created_at),
    KEY idx_social_provider (social_provider)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 사용자 인증 토큰
CREATE TABLE user_tokens (
    token_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    token_type ENUM('access', 'refresh') NOT NULL DEFAULT 'access',
    token_hash VARCHAR(255) NOT NULL,
    client_info JSON NULL COMMENT 'device, browser info',
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    revoked_at TIMESTAMP NULL,

    PRIMARY KEY (token_id),
    UNIQUE KEY uk_token_hash (token_hash),
    KEY idx_user_token (user_id, token_type, expires_at),
    KEY idx_expires (expires_at),
    CONSTRAINT fk_token_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 2. FRIENDS SYSTEM (친구 기능)
-- ======================================================

-- 친구 관계
CREATE TABLE friendships (
    friendship_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    requester_id BIGINT UNSIGNED NOT NULL COMMENT '친구 요청한 사용자',
    addressee_id BIGINT UNSIGNED NOT NULL COMMENT '친구 요청받은 사용자',
    status ENUM('pending', 'accepted', 'blocked', 'declined') DEFAULT 'pending',

    -- 타임스탬프
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

    PRIMARY KEY (friendship_id),
    UNIQUE KEY uk_friendship (requester_id, addressee_id),
    KEY idx_addressee_status (addressee_id, status),
    KEY idx_requester_status (requester_id, status),
    KEY idx_status (status),
    CONSTRAINT fk_friend_requester FOREIGN KEY (requester_id) REFERENCES users(user_id) ON DELETE CASCADE,
    CONSTRAINT fk_friend_addressee FOREIGN KEY (addressee_id) REFERENCES users(user_id) ON DELETE CASCADE,
    CONSTRAINT chk_not_self_friend CHECK (requester_id != addressee_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 온라인 상태 (실시간 친구 상태용)
CREATE TABLE online_status (
    user_id BIGINT UNSIGNED NOT NULL,
    status ENUM('online', 'away', 'busy', 'offline') DEFAULT 'online',
    last_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    current_activity VARCHAR(100) NULL COMMENT '현재 활동 (게임중, 대기중 등)',

    PRIMARY KEY (user_id),
    KEY idx_last_seen (last_seen),
    KEY idx_status (status),
    CONSTRAINT fk_online_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 3. CHAT SYSTEM (채팅 기능)
-- ======================================================

-- 채팅 방 (1:1 채팅, 그룹 채팅)
CREATE TABLE chat_rooms (
    room_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_type ENUM('private', 'group', 'public') DEFAULT 'private',
    room_name VARCHAR(100) NULL COMMENT '그룹 채팅인 경우만 사용',
    created_by BIGINT UNSIGNED NOT NULL,

    -- 타임스탬프
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

    PRIMARY KEY (room_id),
    KEY idx_created_by (created_by),
    KEY idx_room_type (room_type),
    KEY idx_created_at (created_at),
    CONSTRAINT fk_chatroom_creator FOREIGN KEY (created_by) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 채팅 방 참가자
CREATE TABLE chat_participants (
    participant_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_read_at TIMESTAMP NULL COMMENT '마지막으로 읽은 시간',
    is_active BOOLEAN DEFAULT TRUE COMMENT '채팅방 활성 상태',

    PRIMARY KEY (participant_id),
    UNIQUE KEY uk_room_user (room_id, user_id),
    KEY idx_user_rooms (user_id, is_active),
    KEY idx_room_participants (room_id, is_active),
    CONSTRAINT fk_participant_room FOREIGN KEY (room_id) REFERENCES chat_rooms(room_id) ON DELETE CASCADE,
    CONSTRAINT fk_participant_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 채팅 메시지
CREATE TABLE chat_messages (
    message_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_id BIGINT UNSIGNED NOT NULL,
    sender_id BIGINT UNSIGNED NOT NULL,
    message_type ENUM('text', 'emoji', 'image', 'system') DEFAULT 'text',
    message_content TEXT NOT NULL,
    reply_to_message_id BIGINT UNSIGNED NULL COMMENT '답장 메시지 ID',

    -- 메시지 상태
    is_deleted BOOLEAN DEFAULT FALSE,
    is_edited BOOLEAN DEFAULT FALSE,
    edited_at TIMESTAMP NULL,

    -- 타임스탬프
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (message_id),
    KEY idx_room_messages (room_id, created_at DESC),
    KEY idx_sender (sender_id),
    KEY idx_reply_to (reply_to_message_id),
    CONSTRAINT fk_message_room FOREIGN KEY (room_id) REFERENCES chat_rooms(room_id) ON DELETE CASCADE,
    CONSTRAINT fk_message_sender FOREIGN KEY (sender_id) REFERENCES users(user_id),
    CONSTRAINT fk_message_reply FOREIGN KEY (reply_to_message_id) REFERENCES chat_messages(message_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
-- 파티셔닝으로 대용량 메시지 관리
PARTITION BY RANGE (UNIX_TIMESTAMP(created_at)) (
    PARTITION p_2024_08 VALUES LESS THAN (UNIX_TIMESTAMP('2024-09-01')),
    PARTITION p_2024_09 VALUES LESS THAN (UNIX_TIMESTAMP('2024-10-01')),
    PARTITION p_2024_10 VALUES LESS THAN (UNIX_TIMESTAMP('2024-11-01')),
    PARTITION p_future VALUES LESS THAN MAXVALUE
);

-- ======================================================
-- 4. VIEWS FOR COMMON QUERIES
-- ======================================================

-- 친구 목록 뷰 (양방향 친구 관계)
CREATE OR REPLACE VIEW v_friendships AS
SELECT
    requester_id as user_id,
    addressee_id as friend_id,
    status,
    created_at,
    updated_at
FROM friendships
WHERE status = 'accepted'

UNION ALL

SELECT
    addressee_id as user_id,
    requester_id as friend_id,
    status,
    created_at,
    updated_at
FROM friendships
WHERE status = 'accepted';

-- 온라인 친구 목록 뷰
CREATE OR REPLACE VIEW v_online_friends AS
SELECT
    f.user_id,
    f.friend_id,
    u.nickname as friend_nickname,
    u.avatar_url as friend_avatar,
    os.status as online_status,
    os.current_activity,
    os.last_seen
FROM v_friendships f
INNER JOIN users u ON f.friend_id = u.user_id
LEFT JOIN online_status os ON f.friend_id = os.user_id
WHERE u.status = 'active'
    AND (os.status IN ('online', 'away', 'busy') OR os.last_seen > DATE_SUB(NOW(), INTERVAL 10 MINUTE));

-- 사용자 통계 요약 뷰
CREATE OR REPLACE VIEW v_user_stats AS
SELECT
    user_id,
    username,
    nickname,
    avatar_url,
    level,
    experience,
    total_games,
    win_count,
    lose_count,
    win_rate,
    status,
    last_login_at
FROM users
WHERE deleted_at IS NULL;

-- ======================================================
-- 5. STORED PROCEDURES
-- ======================================================

DELIMITER $$

-- 사용자 생성 프로시저 (소셜 로그인만)
CREATE PROCEDURE sp_create_user(
    IN p_username VARCHAR(50),
    IN p_nickname VARCHAR(100),
    IN p_social_provider VARCHAR(50),
    IN p_social_id VARCHAR(255),
    OUT p_user_id BIGINT
)
BEGIN
    DECLARE EXIT HANDLER FOR SQLEXCEPTION
    BEGIN
        ROLLBACK;
        RESIGNAL;
    END;

    START TRANSACTION;

    -- 사용자 생성 (소셜 로그인 정보만)
    INSERT INTO users (username, nickname, social_provider, social_id)
    VALUES (p_username, p_nickname, p_social_provider, p_social_id);

    SET p_user_id = LAST_INSERT_ID();

    -- 온라인 상태 초기화
    INSERT INTO online_status (user_id, status)
    VALUES (p_user_id, 'offline');

    COMMIT;
END$$

-- 친구 요청 프로시저
CREATE PROCEDURE sp_send_friend_request(
    IN p_requester_id BIGINT,
    IN p_addressee_id BIGINT,
    OUT p_result VARCHAR(50)
)
BEGIN
    DECLARE v_existing_count INT DEFAULT 0;

    -- 기존 관계 확인
    SELECT COUNT(*) INTO v_existing_count
    FROM friendships
    WHERE (requester_id = p_requester_id AND addressee_id = p_addressee_id)
       OR (requester_id = p_addressee_id AND addressee_id = p_requester_id);

    IF v_existing_count > 0 THEN
        SET p_result = 'ALREADY_EXISTS';
    ELSE
        INSERT INTO friendships (requester_id, addressee_id, status)
        VALUES (p_requester_id, p_addressee_id, 'pending');
        SET p_result = 'SUCCESS';
    END IF;
END$$

-- 1:1 채팅방 생성 또는 찾기 프로시저
CREATE PROCEDURE sp_get_or_create_private_chat(
    IN p_user1_id BIGINT,
    IN p_user2_id BIGINT,
    OUT p_room_id BIGINT
)
BEGIN
    DECLARE v_room_id BIGINT DEFAULT NULL;

    -- 기존 1:1 채팅방 찾기
    SELECT cr.room_id INTO v_room_id
    FROM chat_rooms cr
    INNER JOIN chat_participants cp1 ON cr.room_id = cp1.room_id
    INNER JOIN chat_participants cp2 ON cr.room_id = cp2.room_id
    WHERE cr.room_type = 'private'
        AND cp1.user_id = p_user1_id
        AND cp2.user_id = p_user2_id
        AND cp1.is_active = TRUE
        AND cp2.is_active = TRUE
    LIMIT 1;

    -- 없으면 새로 생성
    IF v_room_id IS NULL THEN
        START TRANSACTION;

        -- 채팅방 생성
        INSERT INTO chat_rooms (room_type, created_by)
        VALUES ('private', p_user1_id);

        SET v_room_id = LAST_INSERT_ID();

        -- 참가자 추가
        INSERT INTO chat_participants (room_id, user_id)
        VALUES (v_room_id, p_user1_id), (v_room_id, p_user2_id);

        COMMIT;
    END IF;

    SET p_room_id = v_room_id;
END$$

-- 게임 통계 업데이트 프로시저
CREATE PROCEDURE sp_update_user_stats(
    IN p_user_id BIGINT,
    IN p_is_winner BOOLEAN,
    IN p_exp_gained INT
)
BEGIN
    UPDATE users
    SET level = COALESCE(level, 1),
        experience = COALESCE(experience, 0) + p_exp_gained,
        total_games = COALESCE(total_games, 0) + 1,
        win_count = COALESCE(win_count, 0) + IF(p_is_winner, 1, 0),
        lose_count = COALESCE(lose_count, 0) + IF(NOT p_is_winner, 1, 0),
        updated_at = NOW()
    WHERE user_id = p_user_id;
END$$

DELIMITER ;

-- ======================================================
-- 6. ESSENTIAL INDEXES
-- ======================================================

-- 로그인 최적화 (소셜 로그인만)
CREATE INDEX idx_social_login ON users(social_provider, social_id, status);

-- 친구 검색 최적화
CREATE INDEX idx_friend_requests ON friendships(addressee_id, status, created_at DESC) WHERE status = 'pending';
CREATE INDEX idx_mutual_friends ON friendships(requester_id, addressee_id, status);

-- 채팅 최적화
CREATE INDEX idx_user_chat_rooms ON chat_participants(user_id, is_active, room_id);
CREATE INDEX idx_recent_messages ON chat_messages(room_id, created_at DESC, is_deleted) WHERE is_deleted = FALSE;
CREATE INDEX idx_unread_messages ON chat_messages(room_id, created_at)
    WHERE created_at > (SELECT last_read_at FROM chat_participants WHERE room_id = chat_messages.room_id);

-- 온라인 상태 최적화
CREATE INDEX idx_online_users ON online_status(status, last_seen) WHERE status IN ('online', 'away', 'busy');

-- ======================================================
-- 7. INITIAL DATA & PERMISSIONS
-- ======================================================

-- 시스템 사용자 생성 (시스템 메시지용)
INSERT INTO users (username, nickname, status, created_at) VALUES
('system', 'System', 'active', NOW());

-- 애플리케이션 사용자 생성
CREATE USER IF NOT EXISTS 'game_simple'@'%' IDENTIFIED BY 'game_password_123';
GRANT SELECT, INSERT, UPDATE, DELETE, EXECUTE ON police_thief_simple.* TO 'game_simple'@'%';

-- 읽기 전용 사용자 생성
CREATE USER IF NOT EXISTS 'game_readonly'@'%' IDENTIFIED BY 'readonly_password_123';
GRANT SELECT ON police_thief_simple.* TO 'game_readonly'@'%';

FLUSH PRIVILEGES;

-- ======================================================
-- 8. CLEANUP EVENTS
-- ======================================================

-- 만료된 토큰 정리 (매시간)
CREATE EVENT IF NOT EXISTS cleanup_expired_tokens
ON SCHEDULE EVERY 1 HOUR
DO
    DELETE FROM user_tokens
    WHERE expires_at < NOW()
        OR revoked_at IS NOT NULL
    LIMIT 1000;

-- 오프라인 상태 업데이트 (5분마다)
CREATE EVENT IF NOT EXISTS update_offline_status
ON SCHEDULE EVERY 5 MINUTE
DO
    UPDATE online_status
    SET status = 'offline'
    WHERE status != 'offline'
        AND last_seen < DATE_SUB(NOW(), INTERVAL 10 MINUTE);