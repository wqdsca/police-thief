-- ======================================================
-- High-Performance Query Templates
-- Optimized for Police Thief Game Server
-- ======================================================

-- ======================================================
-- 1. USER AUTHENTICATION & SESSION
-- ======================================================

-- 로그인 쿼리 (인덱스: idx_users_login)
PREPARE stmt_user_login FROM '
SELECT 
    u.user_id,
    u.username,
    u.nickname,
    u.avatar_url,
    u.status,
    p.level,
    p.experience,
    p.currency_gold,
    p.currency_gem
FROM users u
INNER JOIN user_profiles p ON u.user_id = p.user_id
WHERE u.username = ? 
    AND u.password_hash = ?
    AND u.status = "active"';

-- JWT 토큰 검증 (인덱스: idx_user_token)
PREPARE stmt_verify_token FROM '
SELECT 
    t.user_id,
    t.expires_at,
    u.status
FROM user_tokens t
INNER JOIN users u ON t.user_id = u.user_id
WHERE t.token_hash = ?
    AND t.token_type = "access"
    AND t.expires_at > NOW()
    AND t.revoked_at IS NULL
    AND u.status = "active"';

-- ======================================================
-- 2. ROOM MANAGEMENT
-- ======================================================

-- 활성 룸 목록 조회 (인덱스: idx_rooms_list)
PREPARE stmt_get_active_rooms FROM '
SELECT SQL_CACHE
    r.room_id,
    r.room_code,
    r.room_name,
    r.game_mode,
    r.current_players,
    r.max_players,
    r.is_private,
    u.nickname as owner_nickname,
    r.created_at
FROM game_rooms r
INNER JOIN users u ON r.owner_id = u.user_id
WHERE r.room_status = ?
    AND r.is_private = FALSE
    AND r.current_players < r.max_players
ORDER BY r.created_at DESC
LIMIT ?';

-- 룸 생성 (트랜잭션)
DELIMITER $$
CREATE PROCEDURE sp_quick_create_room(
    IN p_room_name VARCHAR(100),
    IN p_owner_id BIGINT,
    OUT p_room_code VARCHAR(10)
)
BEGIN
    DECLARE v_room_id BIGINT;
    
    -- 빠른 룸 코드 생성
    SET p_room_code = UPPER(SUBSTRING(MD5(CONCAT(p_owner_id, NOW(6))), 1, 6));
    
    -- 룸 생성
    INSERT INTO game_rooms (room_code, room_name, owner_id, current_players)
    VALUES (p_room_code, p_room_name, p_owner_id, 1);
    
    SET v_room_id = LAST_INSERT_ID();
    
    -- 방장 추가 (배치 인서트)
    INSERT INTO room_participants (room_id, user_id) VALUES (v_room_id, p_owner_id);
END$$
DELIMITER ;

-- 룸 참가 (동시성 제어)
PREPARE stmt_join_room FROM '
UPDATE game_rooms 
SET current_players = current_players + 1
WHERE room_code = ?
    AND room_status = "waiting"
    AND current_players < max_players
    AND (is_private = FALSE OR password_hash = ?)';

-- ======================================================
-- 3. GAME SESSION & RESULTS
-- ======================================================

-- 게임 시작 (배치 처리)
DELIMITER $$
CREATE PROCEDURE sp_start_game_batch(
    IN p_room_id BIGINT
)
BEGIN
    DECLARE v_session_id BIGINT;
    
    -- 세션 생성
    INSERT INTO game_sessions (room_id, started_at)
    VALUES (p_room_id, NOW());
    
    SET v_session_id = LAST_INSERT_ID();
    
    -- 참가자 결과 레코드 배치 생성
    INSERT INTO game_results (session_id, user_id, team)
    SELECT v_session_id, user_id, team
    FROM room_participants
    WHERE room_id = p_room_id AND connection_status = 'connected';
    
    -- 룸 상태 업데이트
    UPDATE game_rooms 
    SET room_status = 'playing', started_at = NOW()
    WHERE room_id = p_room_id;
END$$
DELIMITER ;

-- 게임 결과 조회 (커버링 인덱스)
PREPARE stmt_get_game_results FROM '
SELECT 
    r.user_id,
    u.nickname,
    r.team,
    r.score,
    r.kills,
    r.deaths,
    r.assists,
    r.is_winner
FROM game_results r USE INDEX (idx_session_team)
INNER JOIN users u ON r.user_id = u.user_id
WHERE r.session_id = ?
ORDER BY r.score DESC';

-- ======================================================
-- 4. RANKING & STATISTICS
-- ======================================================

-- TOP 100 랭킹 (매터리얼라이즈드 뷰 활용)
CREATE TABLE IF NOT EXISTS cached_rankings (
    rank_type ENUM('winrate', 'level', 'score'),
    user_id BIGINT,
    nickname VARCHAR(50),
    avatar_url VARCHAR(500),
    metric_value DECIMAL(10,2),
    rank_position INT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (rank_type, rank_position),
    KEY idx_user_rank (user_id, rank_type)
) ENGINE=MEMORY;

-- 랭킹 캐시 갱신 (5분마다 실행)
DELIMITER $$
CREATE EVENT IF NOT EXISTS update_rankings_cache
ON SCHEDULE EVERY 5 MINUTE
DO
BEGIN
    -- Win Rate 랭킹
    REPLACE INTO cached_rankings (rank_type, user_id, nickname, avatar_url, metric_value, rank_position)
    SELECT 
        'winrate',
        u.user_id,
        u.nickname,
        u.avatar_url,
        p.win_rate,
        @rank := @rank + 1
    FROM users u
    INNER JOIN user_profiles p ON u.user_id = p.user_id
    CROSS JOIN (SELECT @rank := 0) r
    WHERE u.status = 'active' AND p.total_games >= 10
    ORDER BY p.win_rate DESC, p.total_games DESC
    LIMIT 100;
END$$
DELIMITER ;

-- 빠른 랭킹 조회
PREPARE stmt_get_rankings FROM '
SELECT SQL_CACHE
    rank_position,
    user_id,
    nickname,
    avatar_url,
    metric_value
FROM cached_rankings
WHERE rank_type = ?
ORDER BY rank_position
LIMIT ?';

-- ======================================================
-- 5. REAL-TIME FEATURES
-- ======================================================

-- 채팅 메시지 전송 (최소 지연)
PREPARE stmt_send_chat FROM '
INSERT INTO chat_messages (room_id, sender_id, message_type, message_content)
VALUES (?, ?, ?, ?)';

-- 최근 채팅 조회 (파티션 활용)
PREPARE stmt_get_recent_chat FROM '
SELECT 
    m.message_id,
    m.sender_id,
    u.nickname,
    m.message_content,
    m.created_at
FROM chat_messages m PARTITION (p_2024_03)
INNER JOIN users u ON m.sender_id = u.user_id
WHERE m.room_id = ?
    AND m.is_deleted = FALSE
ORDER BY m.created_at DESC
LIMIT 50';

-- 친구 온라인 상태 (Redis 대체)
CREATE TABLE online_status (
    user_id BIGINT PRIMARY KEY,
    status ENUM('online', 'away', 'busy', 'offline') DEFAULT 'online',
    last_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    current_room_id BIGINT NULL,
    KEY idx_last_seen (last_seen)
) ENGINE=MEMORY;

-- 온라인 친구 조회
PREPARE stmt_get_online_friends FROM '
SELECT 
    f.friend_id,
    u.nickname,
    u.avatar_url,
    o.status,
    o.current_room_id
FROM friendships f
INNER JOIN users u ON f.friend_id = u.user_id
LEFT JOIN online_status o ON f.friend_id = o.user_id
WHERE f.user_id = ?
    AND f.status = "accepted"
    AND (o.status IN ("online", "away", "busy") OR o.last_seen > DATE_SUB(NOW(), INTERVAL 5 MINUTE))';

-- ======================================================
-- 6. BATCH OPERATIONS
-- ======================================================

-- 일일 통계 집계 (새벽 실행)
DELIMITER $$
CREATE PROCEDURE sp_daily_statistics()
BEGIN
    INSERT INTO daily_statistics (
        stat_date,
        total_users,
        active_users,
        new_users,
        total_games,
        avg_game_duration
    )
    SELECT 
        CURDATE() - INTERVAL 1 DAY,
        (SELECT COUNT(*) FROM users WHERE status = 'active'),
        (SELECT COUNT(DISTINCT user_id) FROM activity_logs 
         WHERE DATE(created_at) = CURDATE() - INTERVAL 1 DAY),
        (SELECT COUNT(*) FROM users 
         WHERE DATE(created_at) = CURDATE() - INTERVAL 1 DAY),
        (SELECT COUNT(*) FROM game_sessions 
         WHERE DATE(started_at) = CURDATE() - INTERVAL 1 DAY),
        (SELECT AVG(game_duration) FROM game_sessions 
         WHERE DATE(started_at) = CURDATE() - INTERVAL 1 DAY)
    ON DUPLICATE KEY UPDATE
        total_users = VALUES(total_users),
        active_users = VALUES(active_users),
        new_users = VALUES(new_users),
        total_games = VALUES(total_games),
        avg_game_duration = VALUES(avg_game_duration);
END$$
DELIMITER ;

-- ======================================================
-- 7. MAINTENANCE QUERIES
-- ======================================================

-- 만료된 토큰 정리 (매시간)
CREATE EVENT IF NOT EXISTS cleanup_expired_tokens
ON SCHEDULE EVERY 1 HOUR
DO
    DELETE FROM user_tokens 
    WHERE expires_at < NOW() 
        OR revoked_at IS NOT NULL
    LIMIT 1000;

-- 오래된 로그 파티션 삭제 (매월)
CREATE EVENT IF NOT EXISTS cleanup_old_partitions
ON SCHEDULE EVERY 1 MONTH
DO
BEGIN
    ALTER TABLE chat_messages DROP PARTITION p_2024_01;
    ALTER TABLE activity_logs DROP PARTITION p_logs_2024_01;
END;

-- ======================================================
-- 8. CONNECTION POOL RECOMMENDATIONS
-- ======================================================

/*
Rust sqlx 연결 풀 설정:

let pool = MySqlPoolOptions::new()
    .max_connections(100)           // 최대 연결 수
    .min_connections(10)            // 최소 연결 수
    .connect_timeout(Duration::from_secs(3))
    .idle_timeout(Duration::from_secs(300))
    .max_lifetime(Duration::from_secs(1800))
    .connect(&database_url)
    .await?;

// 읽기 전용 복제본 연결
let read_pool = MySqlPoolOptions::new()
    .max_connections(50)
    .connect(&read_replica_url)
    .await?;
*/

-- ======================================================
-- 9. MONITORING QUERIES
-- ======================================================

-- 실시간 성능 모니터링
SELECT 
    SUBSTRING_INDEX(info, ':', 1) as query_type,
    COUNT(*) as count,
    AVG(time) as avg_time_ms,
    MAX(time) as max_time_ms
FROM information_schema.processlist
WHERE command != 'Sleep'
GROUP BY query_type
ORDER BY count DESC;

-- 슬로우 쿼리 분석
SELECT 
    query_time,
    lock_time,
    rows_sent,
    rows_examined,
    sql_text
FROM mysql.slow_log
WHERE query_time > 0.1
ORDER BY query_time DESC
LIMIT 10;