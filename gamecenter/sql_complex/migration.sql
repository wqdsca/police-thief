-- ======================================================
-- Redis to MariaDB Migration Script
-- ======================================================

-- 1. Redis 데이터 마이그레이션을 위한 임시 테이블
CREATE TEMPORARY TABLE IF NOT EXISTS temp_redis_users (
    redis_key VARCHAR(255),
    user_data JSON,
    imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TEMPORARY TABLE IF NOT EXISTS temp_redis_rooms (
    redis_key VARCHAR(255),
    room_data JSON,
    imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- ======================================================
-- Migration Stored Procedures
-- ======================================================

DELIMITER $$

-- Redis 사용자 데이터 마이그레이션
CREATE PROCEDURE sp_migrate_redis_users()
BEGIN
    DECLARE done INT DEFAULT FALSE;
    DECLARE v_user_data JSON;
    DECLARE v_user_id VARCHAR(50);
    DECLARE v_username VARCHAR(100);
    DECLARE v_nickname VARCHAR(100);
    
    DECLARE cur CURSOR FOR SELECT user_data FROM temp_redis_users;
    DECLARE CONTINUE HANDLER FOR NOT FOUND SET done = TRUE;
    
    OPEN cur;
    
    read_loop: LOOP
        FETCH cur INTO v_user_data;
        IF done THEN
            LEAVE read_loop;
        END IF;
        
        -- JSON에서 데이터 추출
        SET v_user_id = JSON_UNQUOTE(JSON_EXTRACT(v_user_data, '$.user_id'));
        SET v_username = JSON_UNQUOTE(JSON_EXTRACT(v_user_data, '$.username'));
        SET v_nickname = JSON_UNQUOTE(JSON_EXTRACT(v_user_data, '$.nickname'));
        
        -- users 테이블에 삽입
        INSERT IGNORE INTO users (
            username,
            nickname,
            password_hash,
            created_at
        ) VALUES (
            COALESCE(v_username, CONCAT('user_', v_user_id)),
            COALESCE(v_nickname, CONCAT('Player_', v_user_id)),
            'migration_password_reset_required',
            NOW()
        );
        
        -- user_profiles 테이블에 삽입
        INSERT IGNORE INTO user_profiles (user_id)
        SELECT user_id FROM users WHERE username = v_username;
        
    END LOOP;
    
    CLOSE cur;
END$$

-- Redis 룸 데이터 마이그레이션
CREATE PROCEDURE sp_migrate_redis_rooms()
BEGIN
    DECLARE done INT DEFAULT FALSE;
    DECLARE v_room_data JSON;
    DECLARE v_room_id VARCHAR(50);
    DECLARE v_room_name VARCHAR(100);
    DECLARE v_owner_id BIGINT;
    DECLARE v_max_players INT;
    
    DECLARE cur CURSOR FOR SELECT room_data FROM temp_redis_rooms;
    DECLARE CONTINUE HANDLER FOR NOT FOUND SET done = TRUE;
    
    OPEN cur;
    
    read_loop: LOOP
        FETCH cur INTO v_room_data;
        IF done THEN
            LEAVE read_loop;
        END IF;
        
        -- JSON에서 데이터 추출
        SET v_room_id = JSON_UNQUOTE(JSON_EXTRACT(v_room_data, '$.room_id'));
        SET v_room_name = JSON_UNQUOTE(JSON_EXTRACT(v_room_data, '$.room_name'));
        SET v_max_players = JSON_EXTRACT(v_room_data, '$.max_players');
        
        -- game_rooms 테이블에 삽입
        INSERT IGNORE INTO game_rooms (
            room_code,
            room_name,
            owner_id,
            max_players,
            created_at
        ) VALUES (
            UPPER(SUBSTRING(MD5(v_room_id), 1, 6)),
            COALESCE(v_room_name, CONCAT('Room_', v_room_id)),
            1, -- 기본 owner_id, 실제 마이그레이션 시 매핑 필요
            COALESCE(v_max_players, 8),
            NOW()
        );
        
    END LOOP;
    
    CLOSE cur;
END$$

DELIMITER ;

-- ======================================================
-- Data Transformation Scripts
-- ======================================================

-- Redis 키 패턴을 SQL 레코드로 변환
-- user:{id} -> users 테이블
-- room:info:{id} -> game_rooms 테이블
-- room:list:time:index -> 정렬된 룸 목록

-- ======================================================
-- Verification Queries
-- ======================================================

-- 마이그레이션 후 데이터 검증
CREATE VIEW v_migration_status AS
SELECT 
    'users' as table_name,
    COUNT(*) as record_count,
    MAX(created_at) as last_migration
FROM users
WHERE password_hash = 'migration_password_reset_required'
UNION ALL
SELECT 
    'game_rooms',
    COUNT(*),
    MAX(created_at)
FROM game_rooms
WHERE room_status = 'closed';

-- ======================================================
-- Rollback Procedures
-- ======================================================

DELIMITER $$

CREATE PROCEDURE sp_rollback_migration()
BEGIN
    -- 마이그레이션된 데이터 삭제
    DELETE FROM users WHERE password_hash = 'migration_password_reset_required';
    DELETE FROM game_rooms WHERE created_at >= DATE_SUB(NOW(), INTERVAL 1 DAY);
    
    -- 시퀀스 리셋
    ALTER TABLE users AUTO_INCREMENT = 1;
    ALTER TABLE game_rooms AUTO_INCREMENT = 1;
END$$

DELIMITER ;

-- ======================================================
-- Redis to SQL Mapping Reference
-- ======================================================

/*
Redis Key Pattern -> SQL Table Mapping:

1. user:{id} -> users + user_profiles
   - user_id -> users.user_id
   - username -> users.username
   - nickname -> users.nickname
   - level -> user_profiles.level
   - exp -> user_profiles.experience

2. room:info:{id} -> game_rooms
   - room_id -> game_rooms.room_id
   - room_name -> game_rooms.room_name
   - max_players -> game_rooms.max_players
   - current_players -> game_rooms.current_players
   - owner_id -> game_rooms.owner_id

3. room:list:time:index -> Sorted by created_at DESC
   - ZADD score -> game_rooms.created_at
   - member -> game_rooms.room_id

4. user:session:{token} -> user_tokens
   - token -> user_tokens.token_hash
   - user_id -> user_tokens.user_id
   - expires -> user_tokens.expires_at

5. room_counter:id -> AUTO_INCREMENT
   - Redis INCR -> MariaDB AUTO_INCREMENT

6. recycle_room_id:index -> Not needed with AUTO_INCREMENT

7. ban:user:{id} -> user_bans
   - reason -> user_bans.reason
   - ban_time -> user_bans.ban_start
   - ban_duration -> user_bans.ban_end

8. event:{id} -> events + user_events
   - event_name -> events.event_name
   - start_time -> events.start_time
   - end_time -> events.end_time
   - participants -> user_events
*/

-- ======================================================
-- Performance Comparison Baseline
-- ======================================================

-- Redis 성능 기준 (from CLAUDE.md):
-- - 12,991+ msg/sec throughput
-- - <1ms p99 latency
-- - 22KB per connection

-- SQL 성능 목표:
-- - Connection Pool: 100-500 connections
-- - Query Cache: 80%+ hit rate
-- - Index Usage: 100% for critical queries
-- - Response Time: <5ms for cached, <20ms for complex

-- ======================================================
-- Monitoring Queries
-- ======================================================

-- 실시간 성능 모니터링
SELECT 
    table_name,
    index_name,
    cardinality,
    ROUND(data_length/1024/1024, 2) as data_mb,
    ROUND(index_length/1024/1024, 2) as index_mb
FROM information_schema.statistics
WHERE table_schema = 'police_thief_game'
GROUP BY table_name, index_name
ORDER BY data_mb DESC;