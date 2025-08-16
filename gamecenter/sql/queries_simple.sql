-- ======================================================
-- Simplified High-Performance Query Templates
-- Features: Login, Friends, Chat Only
-- ======================================================

-- ======================================================
-- 1. LOGIN & AUTHENTICATION QUERIES
-- ======================================================

-- 로그인 쿼리 (모든 통계 포함, NULL 허용)
PREPARE stmt_user_login FROM '
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
WHERE username = ? 
    AND password_hash = ?
    AND status = "active"
    AND deleted_at IS NULL';

-- 사용자 정보 업데이트 (로그인 시)
PREPARE stmt_update_login FROM '
UPDATE users 
SET last_login_at = NOW(),
    last_login_ip = ?
WHERE user_id = ?';

-- JWT 토큰 생성
PREPARE stmt_create_token FROM '
INSERT INTO user_tokens (user_id, token_type, token_hash, client_info, expires_at)
VALUES (?, ?, ?, ?, ?)';

-- JWT 토큰 검증
PREPARE stmt_verify_token FROM '
SELECT 
    t.user_id,
    t.expires_at,
    u.status,
    u.nickname
FROM user_tokens t
INNER JOIN users u ON t.user_id = u.user_id
WHERE t.token_hash = ?
    AND t.token_type = "access"
    AND t.expires_at > NOW()
    AND t.revoked_at IS NULL
    AND u.status = "active"';

-- 사용자 통계 조회 (NULL 처리 포함)
PREPARE stmt_get_user_stats FROM '
SELECT 
    user_id,
    username,
    nickname,
    avatar_url,
    COALESCE(level, 1) as level,
    COALESCE(experience, 0) as experience,
    COALESCE(total_games, 0) as total_games,
    COALESCE(win_count, 0) as win_count,
    COALESCE(lose_count, 0) as lose_count,
    COALESCE(win_rate, 0.00) as win_rate,
    created_at,
    last_login_at
FROM users
WHERE user_id = ?
    AND status = "active"
    AND deleted_at IS NULL';

-- ======================================================
-- 2. FRIENDS SYSTEM QUERIES
-- ======================================================

-- 친구 요청 보내기 (중복 체크 포함)
PREPARE stmt_check_friendship_exists FROM '
SELECT COUNT(*) as count
FROM friendships
WHERE (requester_id = ? AND addressee_id = ?)
   OR (requester_id = ? AND addressee_id = ?)';

-- 친구 요청 목록 (받은 요청)
PREPARE stmt_get_friend_requests FROM '
SELECT 
    f.friendship_id,
    f.requester_id,
    u.nickname as requester_nickname,
    u.avatar_url as requester_avatar,
    COALESCE(u.level, 1) as requester_level,
    f.created_at
FROM friendships f
INNER JOIN users u ON f.requester_id = u.user_id
WHERE f.addressee_id = ?
    AND f.status = "pending"
ORDER BY f.created_at DESC';

-- 친구 목록 (온라인 상태 포함)
PREPARE stmt_get_friends_list FROM '
SELECT 
    f.friend_id,
    u.nickname as friend_nickname,
    u.avatar_url as friend_avatar,
    COALESCE(u.level, 1) as friend_level,
    COALESCE(u.win_rate, 0.00) as friend_win_rate,
    COALESCE(os.status, "offline") as online_status,
    os.current_activity,
    os.last_seen
FROM v_friendships f
INNER JOIN users u ON f.friend_id = u.user_id
LEFT JOIN online_status os ON f.friend_id = os.user_id
WHERE f.user_id = ?
    AND u.status = "active"
ORDER BY 
    CASE os.status 
        WHEN "online" THEN 1
        WHEN "away" THEN 2
        WHEN "busy" THEN 3
        ELSE 4
    END,
    u.nickname';

-- 친구 요청 승인/거절
PREPARE stmt_respond_friend_request FROM '
UPDATE friendships
SET status = ?,
    updated_at = NOW()
WHERE friendship_id = ?
    AND addressee_id = ?
    AND status = "pending"';

-- 친구 검색 (닉네임으로)
PREPARE stmt_search_users FROM '
SELECT 
    user_id,
    username,
    nickname,
    avatar_url,
    COALESCE(level, 1) as level,
    COALESCE(win_rate, 0.00) as win_rate
FROM users
WHERE nickname LIKE CONCAT("%", ?, "%")
    AND status = "active"
    AND deleted_at IS NULL
    AND user_id != ?
ORDER BY nickname
LIMIT 20';

-- ======================================================
-- 3. CHAT SYSTEM QUERIES
-- ======================================================

-- 1:1 채팅방 생성/찾기는 stored procedure 사용: sp_get_or_create_private_chat

-- 사용자의 채팅방 목록
PREPARE stmt_get_user_chat_rooms FROM '
SELECT 
    cr.room_id,
    cr.room_type,
    cr.room_name,
    cr.created_at,
    (SELECT message_content FROM chat_messages 
     WHERE room_id = cr.room_id 
     ORDER BY created_at DESC LIMIT 1) as last_message,
    (SELECT created_at FROM chat_messages 
     WHERE room_id = cr.room_id 
     ORDER BY created_at DESC LIMIT 1) as last_message_time,
    (SELECT COUNT(*) FROM chat_messages 
     WHERE room_id = cr.room_id 
     AND created_at > COALESCE(cp.last_read_at, "1970-01-01")
     AND sender_id != ?) as unread_count,
    -- 1:1 채팅인 경우 상대방 정보
    (SELECT u.nickname FROM chat_participants cp2 
     INNER JOIN users u ON cp2.user_id = u.user_id
     WHERE cp2.room_id = cr.room_id 
     AND cp2.user_id != ? 
     AND cr.room_type = "private" LIMIT 1) as other_user_nickname,
    (SELECT u.avatar_url FROM chat_participants cp2 
     INNER JOIN users u ON cp2.user_id = u.user_id
     WHERE cp2.room_id = cr.room_id 
     AND cp2.user_id != ? 
     AND cr.room_type = "private" LIMIT 1) as other_user_avatar
FROM chat_rooms cr
INNER JOIN chat_participants cp ON cr.room_id = cp.room_id
WHERE cp.user_id = ?
    AND cp.is_active = TRUE
ORDER BY last_message_time DESC';

-- 채팅방 메시지 조회
PREPARE stmt_get_chat_messages FROM '
SELECT 
    cm.message_id,
    cm.sender_id,
    u.nickname as sender_nickname,
    u.avatar_url as sender_avatar,
    cm.message_type,
    cm.message_content,
    cm.reply_to_message_id,
    cm.is_edited,
    cm.edited_at,
    cm.created_at
FROM chat_messages cm
INNER JOIN users u ON cm.sender_id = u.user_id
WHERE cm.room_id = ?
    AND cm.is_deleted = FALSE
ORDER BY cm.created_at DESC
LIMIT ? OFFSET ?';

-- 메시지 전송
PREPARE stmt_send_message FROM '
INSERT INTO chat_messages (room_id, sender_id, message_type, message_content, reply_to_message_id)
VALUES (?, ?, ?, ?, ?)';

-- 메시지 읽음 표시
PREPARE stmt_mark_messages_read FROM '
UPDATE chat_participants
SET last_read_at = NOW()
WHERE room_id = ? AND user_id = ?';

-- 안읽은 메시지 수 조회
PREPARE stmt_get_unread_count FROM '
SELECT COUNT(*) as unread_count
FROM chat_messages cm
INNER JOIN chat_participants cp ON cm.room_id = cp.room_id
WHERE cp.user_id = ?
    AND cm.created_at > COALESCE(cp.last_read_at, "1970-01-01")
    AND cm.sender_id != ?
    AND cm.is_deleted = FALSE';

-- ======================================================
-- 4. ONLINE STATUS QUERIES
-- ======================================================

-- 온라인 상태 업데이트
PREPARE stmt_update_online_status FROM '
INSERT INTO online_status (user_id, status, current_activity)
VALUES (?, ?, ?)
ON DUPLICATE KEY UPDATE
    status = VALUES(status),
    current_activity = VALUES(current_activity),
    last_seen = NOW()';

-- 사용자 오프라인 처리
PREPARE stmt_set_offline FROM '
UPDATE online_status
SET status = "offline",
    current_activity = NULL
WHERE user_id = ?';

-- 온라인 친구 수 조회
PREPARE stmt_count_online_friends FROM '
SELECT COUNT(*) as online_friends_count
FROM v_friendships f
INNER JOIN online_status os ON f.friend_id = os.user_id
WHERE f.user_id = ?
    AND os.status IN ("online", "away", "busy")';

-- ======================================================
-- 5. GAME STATISTICS UPDATES
-- ======================================================

-- 게임 통계 업데이트는 stored procedure 사용: sp_update_user_stats

-- 레벨업 계산 (경험치 기반)
PREPARE stmt_check_level_up FROM '
SELECT 
    user_id,
    COALESCE(level, 1) as current_level,
    COALESCE(experience, 0) as current_exp,
    FLOOR(COALESCE(experience, 0) / 1000) + 1 as calculated_level
FROM users
WHERE user_id = ?';

-- 레벨 업데이트
PREPARE stmt_update_level FROM '
UPDATE users
SET level = ?
WHERE user_id = ? AND COALESCE(level, 1) < ?';

-- 랭킹 조회 (간단 버전)
PREPARE stmt_get_rankings FROM '
SELECT 
    user_id,
    nickname,
    avatar_url,
    COALESCE(level, 1) as level,
    COALESCE(win_rate, 0.00) as win_rate,
    COALESCE(total_games, 0) as total_games,
    @rank := @rank + 1 as rank_position
FROM users, (SELECT @rank := 0) r
WHERE status = "active"
    AND deleted_at IS NULL
    AND total_games IS NOT NULL
    AND total_games >= 5
ORDER BY win_rate DESC, total_games DESC
LIMIT ?';

-- ======================================================
-- 6. BATCH OPERATIONS & MAINTENANCE
-- ======================================================

-- 일괄 친구 추가 (그룹 게임 후)
DELIMITER $$
CREATE PROCEDURE sp_batch_friend_suggestions(
    IN p_user_ids TEXT
)
BEGIN
    DECLARE done INT DEFAULT FALSE;
    DECLARE user1_id, user2_id BIGINT;
    DECLARE cur CURSOR FOR 
        SELECT DISTINCT u1.user_id, u2.user_id
        FROM (SELECT user_id FROM users WHERE FIND_IN_SET(user_id, p_user_ids)) u1
        CROSS JOIN (SELECT user_id FROM users WHERE FIND_IN_SET(user_id, p_user_ids)) u2
        WHERE u1.user_id < u2.user_id;
    
    DECLARE CONTINUE HANDLER FOR NOT FOUND SET done = TRUE;
    
    OPEN cur;
    
    read_loop: LOOP
        FETCH cur INTO user1_id, user2_id;
        IF done THEN
            LEAVE read_loop;
        END IF;
        
        -- 이미 친구가 아닌 경우만 제안
        INSERT IGNORE INTO friendships (requester_id, addressee_id, status)
        SELECT user1_id, user2_id, 'pending'
        WHERE NOT EXISTS (
            SELECT 1 FROM friendships
            WHERE (requester_id = user1_id AND addressee_id = user2_id)
               OR (requester_id = user2_id AND addressee_id = user1_id)
        );
    END LOOP;
    
    CLOSE cur;
END$$
DELIMITER ;

-- 오래된 메시지 정리 (파티션 활용)
CREATE EVENT IF NOT EXISTS cleanup_old_messages
ON SCHEDULE EVERY 1 WEEK
DO
BEGIN
    -- 6개월 이상된 메시지 삭제
    DELETE FROM chat_messages 
    WHERE created_at < DATE_SUB(NOW(), INTERVAL 6 MONTH)
    LIMIT 10000;
END;

-- ======================================================
-- 7. DEVELOPMENT & TESTING QUERIES
-- ======================================================

-- 테스트 데이터 생성
DELIMITER $$
CREATE PROCEDURE sp_create_test_data()
BEGIN
    DECLARE i INT DEFAULT 1;
    
    -- 테스트 사용자 10명 생성
    WHILE i <= 10 DO
        INSERT IGNORE INTO users (
            username, 
            nickname, 
            password_hash, 
            level, 
            experience, 
            total_games, 
            win_count, 
            lose_count
        ) VALUES (
            CONCAT('testuser', i),
            CONCAT('TestPlayer', i),
            'test_password_hash',
            FLOOR(1 + RAND() * 50),
            FLOOR(RAND() * 50000),
            FLOOR(RAND() * 100),
            FLOOR(RAND() * 70),
            FLOOR(RAND() * 30)
        );
        
        SET i = i + 1;
    END WHILE;
    
    -- 테스트 친구 관계 생성
    INSERT IGNORE INTO friendships (requester_id, addressee_id, status)
    SELECT u1.user_id, u2.user_id, 'accepted'
    FROM users u1, users u2
    WHERE u1.username LIKE 'testuser%' 
        AND u2.username LIKE 'testuser%'
        AND u1.user_id < u2.user_id
        AND RAND() < 0.3;
END$$
DELIMITER ;

-- 통계 조회
SELECT 
    'users' as table_name,
    COUNT(*) as total_count,
    COUNT(CASE WHEN status = 'active' THEN 1 END) as active_count,
    COUNT(CASE WHEN level IS NOT NULL THEN 1 END) as with_stats_count
FROM users
UNION ALL
SELECT 
    'friendships',
    COUNT(*),
    COUNT(CASE WHEN status = 'accepted' THEN 1 END),
    COUNT(CASE WHEN status = 'pending' THEN 1 END)
FROM friendships
UNION ALL
SELECT 
    'chat_messages',
    COUNT(*),
    COUNT(CASE WHEN is_deleted = FALSE THEN 1 END),
    COUNT(CASE WHEN created_at > DATE_SUB(NOW(), INTERVAL 1 DAY) THEN 1 END)
FROM chat_messages;

-- 성능 모니터링
SELECT 
    table_name,
    ROUND(data_length/1024/1024, 2) as data_mb,
    ROUND(index_length/1024/1024, 2) as index_mb,
    table_rows
FROM information_schema.tables
WHERE table_schema = 'police_thief_simple'
ORDER BY data_length DESC;