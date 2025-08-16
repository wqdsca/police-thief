-- ======================================================
-- Police Thief Game Database Schema
-- Database: MariaDB 10.6+
-- Character Set: utf8mb4
-- Collation: utf8mb4_unicode_ci
-- ======================================================

-- Database 생성
CREATE DATABASE IF NOT EXISTS police_thief_game
    CHARACTER SET utf8mb4
    COLLATE utf8mb4_unicode_ci;

USE police_thief_game;

-- ======================================================
-- 1. USER MANAGEMENT TABLES
-- ======================================================

-- 사용자 기본 정보
CREATE TABLE users (
    user_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    username VARCHAR(30) NOT NULL,
    email VARCHAR(255) NULL,
    password_hash VARCHAR(255) NOT NULL,
    nickname VARCHAR(50) NOT NULL,
    avatar_url VARCHAR(500) NULL,
    status ENUM('active', 'banned', 'suspended', 'deleted') DEFAULT 'active',
    last_login_at TIMESTAMP NULL,
    last_login_ip VARCHAR(45) NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP NULL,
    
    PRIMARY KEY (user_id),
    UNIQUE KEY uk_username (username),
    UNIQUE KEY uk_email (email),
    KEY idx_status (status),
    KEY idx_last_login (last_login_at),
    KEY idx_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 사용자 프로필 확장 정보
CREATE TABLE user_profiles (
    user_id BIGINT UNSIGNED NOT NULL,
    level INT UNSIGNED DEFAULT 1,
    experience BIGINT UNSIGNED DEFAULT 0,
    total_play_time INT UNSIGNED DEFAULT 0 COMMENT 'seconds',
    total_games INT UNSIGNED DEFAULT 0,
    win_count INT UNSIGNED DEFAULT 0,
    lose_count INT UNSIGNED DEFAULT 0,
    draw_count INT UNSIGNED DEFAULT 0,
    win_rate DECIMAL(5,2) GENERATED ALWAYS AS (
        CASE 
            WHEN total_games = 0 THEN 0
            ELSE (win_count * 100.0 / total_games)
        END
    ) STORED,
    highest_score INT UNSIGNED DEFAULT 0,
    achievement_points INT UNSIGNED DEFAULT 0,
    currency_gold INT UNSIGNED DEFAULT 0,
    currency_gem INT UNSIGNED DEFAULT 0,
    
    PRIMARY KEY (user_id),
    KEY idx_level (level),
    KEY idx_win_rate (win_rate),
    KEY idx_total_games (total_games),
    CONSTRAINT fk_profile_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 사용자 인증 토큰
CREATE TABLE user_tokens (
    token_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    token_type ENUM('access', 'refresh', 'reset_password') NOT NULL,
    token_hash VARCHAR(255) NOT NULL,
    client_info JSON NULL COMMENT 'device, browser, version info',
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
-- 2. ROOM MANAGEMENT TABLES
-- ======================================================

-- 게임 룸 정보
CREATE TABLE game_rooms (
    room_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_code VARCHAR(10) NOT NULL,
    room_name VARCHAR(100) NOT NULL,
    owner_id BIGINT UNSIGNED NOT NULL,
    game_mode ENUM('classic', 'ranked', 'custom', 'tournament') DEFAULT 'classic',
    max_players TINYINT UNSIGNED DEFAULT 8,
    current_players TINYINT UNSIGNED DEFAULT 0,
    room_status ENUM('waiting', 'playing', 'finished', 'closed') DEFAULT 'waiting',
    is_private BOOLEAN DEFAULT FALSE,
    password_hash VARCHAR(255) NULL,
    game_settings JSON NULL COMMENT 'map, time_limit, rules, etc',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    ended_at TIMESTAMP NULL,
    
    PRIMARY KEY (room_id),
    UNIQUE KEY uk_room_code (room_code),
    KEY idx_room_status (room_status, created_at),
    KEY idx_owner (owner_id),
    KEY idx_game_mode (game_mode),
    KEY idx_created_at (created_at),
    CONSTRAINT fk_room_owner FOREIGN KEY (owner_id) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 룸 참가자 정보
CREATE TABLE room_participants (
    participant_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    team ENUM('police', 'thief', 'spectator') NULL,
    player_number TINYINT UNSIGNED NULL,
    is_ready BOOLEAN DEFAULT FALSE,
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    left_at TIMESTAMP NULL,
    connection_status ENUM('connected', 'disconnected', 'reconnecting') DEFAULT 'connected',
    
    PRIMARY KEY (participant_id),
    UNIQUE KEY uk_room_user (room_id, user_id),
    KEY idx_user_rooms (user_id, left_at),
    KEY idx_room_players (room_id, connection_status),
    CONSTRAINT fk_participant_room FOREIGN KEY (room_id) REFERENCES game_rooms(room_id) ON DELETE CASCADE,
    CONSTRAINT fk_participant_user FOREIGN KEY (user_id) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 3. GAME SESSION TABLES
-- ======================================================

-- 게임 세션 기록
CREATE TABLE game_sessions (
    session_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_id BIGINT UNSIGNED NOT NULL,
    session_number INT UNSIGNED DEFAULT 1,
    winner_team ENUM('police', 'thief', 'draw') NULL,
    game_duration INT UNSIGNED NULL COMMENT 'seconds',
    total_score JSON NULL COMMENT '{police: 100, thief: 80}',
    mvp_user_id BIGINT UNSIGNED NULL,
    game_stats JSON NULL COMMENT 'detailed game statistics',
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    ended_at TIMESTAMP NULL,
    
    PRIMARY KEY (session_id),
    KEY idx_room_sessions (room_id, session_number),
    KEY idx_ended_at (ended_at),
    KEY idx_mvp (mvp_user_id),
    CONSTRAINT fk_session_room FOREIGN KEY (room_id) REFERENCES game_rooms(room_id),
    CONSTRAINT fk_session_mvp FOREIGN KEY (mvp_user_id) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 게임 세션 참가자 결과
CREATE TABLE game_results (
    result_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    session_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    team ENUM('police', 'thief') NOT NULL,
    is_winner BOOLEAN DEFAULT FALSE,
    score INT UNSIGNED DEFAULT 0,
    kills INT UNSIGNED DEFAULT 0,
    deaths INT UNSIGNED DEFAULT 0,
    assists INT UNSIGNED DEFAULT 0,
    experience_gained INT UNSIGNED DEFAULT 0,
    gold_earned INT UNSIGNED DEFAULT 0,
    performance_rating DECIMAL(3,2) NULL,
    
    PRIMARY KEY (result_id),
    UNIQUE KEY uk_session_user (session_id, user_id),
    KEY idx_user_results (user_id, is_winner),
    KEY idx_session_team (session_id, team),
    CONSTRAINT fk_result_session FOREIGN KEY (session_id) REFERENCES game_sessions(session_id),
    CONSTRAINT fk_result_user FOREIGN KEY (user_id) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 4. CHAT AND SOCIAL TABLES
-- ======================================================

-- 채팅 메시지
CREATE TABLE chat_messages (
    message_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_id BIGINT UNSIGNED NULL,
    sender_id BIGINT UNSIGNED NOT NULL,
    message_type ENUM('text', 'emoji', 'system', 'announcement') DEFAULT 'text',
    message_content TEXT NOT NULL,
    is_deleted BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    PRIMARY KEY (message_id),
    KEY idx_room_messages (room_id, created_at),
    KEY idx_sender (sender_id),
    CONSTRAINT fk_chat_room FOREIGN KEY (room_id) REFERENCES game_rooms(room_id) ON DELETE CASCADE,
    CONSTRAINT fk_chat_sender FOREIGN KEY (sender_id) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
PARTITION BY RANGE (UNIX_TIMESTAMP(created_at)) (
    PARTITION p_2024_01 VALUES LESS THAN (UNIX_TIMESTAMP('2024-02-01')),
    PARTITION p_2024_02 VALUES LESS THAN (UNIX_TIMESTAMP('2024-03-01')),
    PARTITION p_2024_03 VALUES LESS THAN (UNIX_TIMESTAMP('2024-04-01')),
    PARTITION p_future VALUES LESS THAN MAXVALUE
);

-- 친구 관계
CREATE TABLE friendships (
    friendship_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    friend_id BIGINT UNSIGNED NOT NULL,
    status ENUM('pending', 'accepted', 'blocked') DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    PRIMARY KEY (friendship_id),
    UNIQUE KEY uk_user_friend (user_id, friend_id),
    KEY idx_friend_user (friend_id, user_id),
    KEY idx_status (status),
    CONSTRAINT fk_friend_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    CONSTRAINT fk_friend_friend FOREIGN KEY (friend_id) REFERENCES users(user_id) ON DELETE CASCADE,
    CONSTRAINT chk_not_self_friend CHECK (user_id != friend_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 5. EVENT AND REWARD TABLES
-- ======================================================

-- 이벤트 정의
CREATE TABLE events (
    event_id INT UNSIGNED NOT NULL AUTO_INCREMENT,
    event_name VARCHAR(100) NOT NULL,
    event_type ENUM('daily', 'weekly', 'seasonal', 'special') NOT NULL,
    description TEXT NULL,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    event_config JSON NULL COMMENT 'rewards, conditions, rules',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    PRIMARY KEY (event_id),
    KEY idx_active_events (is_active, start_time, end_time),
    KEY idx_event_type (event_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 사용자 이벤트 참여
CREATE TABLE user_events (
    user_event_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    event_id INT UNSIGNED NOT NULL,
    progress JSON NULL COMMENT 'current progress data',
    is_completed BOOLEAN DEFAULT FALSE,
    rewards_claimed BOOLEAN DEFAULT FALSE,
    completed_at TIMESTAMP NULL,
    claimed_at TIMESTAMP NULL,
    
    PRIMARY KEY (user_event_id),
    UNIQUE KEY uk_user_event (user_id, event_id),
    KEY idx_event_users (event_id, is_completed),
    CONSTRAINT fk_user_event_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    CONSTRAINT fk_user_event_event FOREIGN KEY (event_id) REFERENCES events(event_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 6. BAN AND MODERATION TABLES
-- ======================================================

-- 사용자 제재 기록
CREATE TABLE user_bans (
    ban_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    ban_type ENUM('warning', 'temporary', 'permanent') NOT NULL,
    reason TEXT NOT NULL,
    banned_by BIGINT UNSIGNED NULL,
    ban_start TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    ban_end TIMESTAMP NULL,
    is_active BOOLEAN DEFAULT TRUE,
    unbanned_by BIGINT UNSIGNED NULL,
    unbanned_at TIMESTAMP NULL,
    unban_reason TEXT NULL,
    
    PRIMARY KEY (ban_id),
    KEY idx_user_bans (user_id, is_active),
    KEY idx_active_bans (is_active, ban_end),
    CONSTRAINT fk_ban_user FOREIGN KEY (user_id) REFERENCES users(user_id),
    CONSTRAINT fk_ban_admin FOREIGN KEY (banned_by) REFERENCES users(user_id),
    CONSTRAINT fk_unban_admin FOREIGN KEY (unbanned_by) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ======================================================
-- 7. STATISTICS AND ANALYTICS TABLES
-- ======================================================

-- 일별 통계 집계
CREATE TABLE daily_statistics (
    stat_date DATE NOT NULL,
    total_users INT UNSIGNED DEFAULT 0,
    active_users INT UNSIGNED DEFAULT 0,
    new_users INT UNSIGNED DEFAULT 0,
    total_games INT UNSIGNED DEFAULT 0,
    total_rooms INT UNSIGNED DEFAULT 0,
    avg_game_duration DECIMAL(10,2) DEFAULT 0,
    peak_concurrent_users INT UNSIGNED DEFAULT 0,
    revenue_gold BIGINT UNSIGNED DEFAULT 0,
    revenue_gem BIGINT UNSIGNED DEFAULT 0,
    
    PRIMARY KEY (stat_date),
    KEY idx_stat_date (stat_date)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 사용자 활동 로그
CREATE TABLE activity_logs (
    log_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NULL,
    activity_type VARCHAR(50) NOT NULL,
    activity_data JSON NULL,
    ip_address VARCHAR(45) NULL,
    user_agent TEXT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    PRIMARY KEY (log_id),
    KEY idx_user_activity (user_id, activity_type, created_at),
    KEY idx_created_at (created_at),
    CONSTRAINT fk_activity_user FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
PARTITION BY RANGE (UNIX_TIMESTAMP(created_at)) (
    PARTITION p_logs_2024_01 VALUES LESS THAN (UNIX_TIMESTAMP('2024-02-01')),
    PARTITION p_logs_2024_02 VALUES LESS THAN (UNIX_TIMESTAMP('2024-03-01')),
    PARTITION p_logs_future VALUES LESS THAN MAXVALUE
);

-- ======================================================
-- 8. VIEWS FOR COMMON QUERIES
-- ======================================================

-- 활성 룸 목록 뷰
CREATE OR REPLACE VIEW v_active_rooms AS
SELECT 
    r.room_id,
    r.room_code,
    r.room_name,
    r.game_mode,
    r.current_players,
    r.max_players,
    r.room_status,
    r.is_private,
    u.nickname as owner_nickname,
    r.created_at
FROM game_rooms r
INNER JOIN users u ON r.owner_id = u.user_id
WHERE r.room_status IN ('waiting', 'playing')
ORDER BY r.created_at DESC;

-- 사용자 랭킹 뷰
CREATE OR REPLACE VIEW v_user_rankings AS
SELECT 
    u.user_id,
    u.nickname,
    u.avatar_url,
    p.level,
    p.total_games,
    p.win_count,
    p.win_rate,
    p.highest_score,
    RANK() OVER (ORDER BY p.win_rate DESC, p.total_games DESC) as rank_by_winrate,
    RANK() OVER (ORDER BY p.level DESC, p.experience DESC) as rank_by_level,
    RANK() OVER (ORDER BY p.highest_score DESC) as rank_by_score
FROM users u
INNER JOIN user_profiles p ON u.user_id = p.user_id
WHERE u.status = 'active' AND p.total_games >= 10;

-- ======================================================
-- 9. STORED PROCEDURES
-- ======================================================

DELIMITER $$

-- 사용자 생성 프로시저
CREATE PROCEDURE sp_create_user(
    IN p_username VARCHAR(30),
    IN p_email VARCHAR(255),
    IN p_password_hash VARCHAR(255),
    IN p_nickname VARCHAR(50),
    OUT p_user_id BIGINT
)
BEGIN
    DECLARE EXIT HANDLER FOR SQLEXCEPTION
    BEGIN
        ROLLBACK;
        RESIGNAL;
    END;
    
    START TRANSACTION;
    
    -- 사용자 생성
    INSERT INTO users (username, email, password_hash, nickname)
    VALUES (p_username, p_email, p_password_hash, p_nickname);
    
    SET p_user_id = LAST_INSERT_ID();
    
    -- 프로필 생성
    INSERT INTO user_profiles (user_id) VALUES (p_user_id);
    
    COMMIT;
END$$

-- 룸 생성 프로시저
CREATE PROCEDURE sp_create_room(
    IN p_room_name VARCHAR(100),
    IN p_owner_id BIGINT,
    IN p_game_mode VARCHAR(20),
    IN p_max_players TINYINT,
    IN p_is_private BOOLEAN,
    IN p_password_hash VARCHAR(255),
    OUT p_room_id BIGINT,
    OUT p_room_code VARCHAR(10)
)
BEGIN
    DECLARE v_room_code VARCHAR(10);
    
    -- 유니크한 룸 코드 생성
    REPEAT
        SET v_room_code = UPPER(SUBSTRING(MD5(RAND()), 1, 6));
    UNTIL NOT EXISTS (SELECT 1 FROM game_rooms WHERE room_code = v_room_code) END REPEAT;
    
    -- 룸 생성
    INSERT INTO game_rooms (
        room_code, room_name, owner_id, game_mode, 
        max_players, is_private, password_hash
    ) VALUES (
        v_room_code, p_room_name, p_owner_id, p_game_mode,
        p_max_players, p_is_private, p_password_hash
    );
    
    SET p_room_id = LAST_INSERT_ID();
    SET p_room_code = v_room_code;
    
    -- 방장을 참가자로 추가
    INSERT INTO room_participants (room_id, user_id)
    VALUES (p_room_id, p_owner_id);
    
    -- 현재 인원 수 업데이트
    UPDATE game_rooms SET current_players = 1 WHERE room_id = p_room_id;
END$$

-- 게임 종료 처리 프로시저
CREATE PROCEDURE sp_end_game_session(
    IN p_session_id BIGINT,
    IN p_winner_team VARCHAR(10)
)
BEGIN
    DECLARE EXIT HANDLER FOR SQLEXCEPTION
    BEGIN
        ROLLBACK;
        RESIGNAL;
    END;
    
    START TRANSACTION;
    
    -- 세션 종료
    UPDATE game_sessions 
    SET winner_team = p_winner_team,
        ended_at = CURRENT_TIMESTAMP,
        game_duration = TIMESTAMPDIFF(SECOND, started_at, CURRENT_TIMESTAMP)
    WHERE session_id = p_session_id;
    
    -- 사용자 프로필 업데이트
    UPDATE user_profiles p
    INNER JOIN game_results r ON p.user_id = r.user_id
    SET p.total_games = p.total_games + 1,
        p.win_count = p.win_count + IF(r.is_winner, 1, 0),
        p.lose_count = p.lose_count + IF(NOT r.is_winner AND p_winner_team != 'draw', 1, 0),
        p.draw_count = p.draw_count + IF(p_winner_team = 'draw', 1, 0),
        p.experience = p.experience + r.experience_gained,
        p.currency_gold = p.currency_gold + r.gold_earned
    WHERE r.session_id = p_session_id;
    
    COMMIT;
END$$

DELIMITER ;

-- ======================================================
-- 10. INDEXES FOR PERFORMANCE
-- ======================================================

-- 복합 인덱스 추가
ALTER TABLE game_rooms ADD INDEX idx_active_rooms (room_status, is_private, created_at);
ALTER TABLE room_participants ADD INDEX idx_active_participants (room_id, connection_status, team);
ALTER TABLE game_sessions ADD INDEX idx_recent_games (ended_at DESC, room_id);
ALTER TABLE chat_messages ADD INDEX idx_recent_messages (room_id, created_at DESC);

-- ======================================================
-- 11. INITIAL DATA
-- ======================================================

-- 기본 이벤트 생성
INSERT INTO events (event_name, event_type, description, start_time, end_time) VALUES
('Daily Login Bonus', 'daily', 'Login daily to receive rewards', NOW(), DATE_ADD(NOW(), INTERVAL 1 YEAR)),
('Weekend Special', 'weekly', 'Double XP every weekend', NOW(), DATE_ADD(NOW(), INTERVAL 1 YEAR)),
('Season 1', 'seasonal', 'First competitive season', NOW(), DATE_ADD(NOW(), INTERVAL 3 MONTH));

-- ======================================================
-- 12. GRANTS AND PERMISSIONS
-- ======================================================

-- 애플리케이션 사용자 생성
CREATE USER IF NOT EXISTS 'game_app'@'%' IDENTIFIED BY 'strong_password_here';
GRANT SELECT, INSERT, UPDATE, DELETE, EXECUTE ON police_thief_game.* TO 'game_app'@'%';

-- 읽기 전용 사용자 생성 (분석용)
CREATE USER IF NOT EXISTS 'game_readonly'@'%' IDENTIFIED BY 'readonly_password_here';
GRANT SELECT ON police_thief_game.* TO 'game_readonly'@'%';

FLUSH PRIVILEGES;