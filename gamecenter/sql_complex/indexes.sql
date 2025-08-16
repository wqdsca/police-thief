-- ======================================================
-- Performance Optimization Indexes
-- ======================================================

-- 1. 사용자 검색 최적화
CREATE INDEX idx_users_search ON users(nickname, status);
CREATE INDEX idx_users_login ON users(username, password_hash, status);

-- 2. 룸 검색 최적화 (가장 자주 사용되는 쿼리)
CREATE INDEX idx_rooms_list ON game_rooms(
    room_status, 
    is_private, 
    current_players, 
    created_at DESC
);

-- 3. 실시간 매칭 최적화
CREATE INDEX idx_rooms_matching ON game_rooms(
    game_mode,
    room_status,
    current_players,
    max_players,
    is_private
);

-- 4. 사용자 게임 이력 조회
CREATE INDEX idx_game_history ON game_results(
    user_id,
    session_id DESC,
    is_winner
);

-- 5. 랭킹 시스템 최적화
CREATE INDEX idx_ranking_winrate ON user_profiles(
    win_rate DESC,
    total_games DESC
) WHERE total_games >= 10;

CREATE INDEX idx_ranking_level ON user_profiles(
    level DESC,
    experience DESC
);

CREATE INDEX idx_ranking_score ON user_profiles(
    highest_score DESC
);

-- 6. 친구 시스템 최적화
CREATE INDEX idx_friend_requests ON friendships(
    friend_id,
    status,
    created_at DESC
) WHERE status = 'pending';

-- 7. 채팅 메시지 조회 최적화
CREATE INDEX idx_chat_room_recent ON chat_messages(
    room_id,
    created_at DESC,
    is_deleted
) WHERE is_deleted = FALSE;

-- 8. 토큰 관리 최적화
CREATE INDEX idx_token_cleanup ON user_tokens(
    expires_at,
    revoked_at
) WHERE revoked_at IS NULL;

-- 9. 활동 로그 분석 최적화
CREATE INDEX idx_activity_analysis ON activity_logs(
    activity_type,
    created_at DESC,
    user_id
);

-- 10. 이벤트 관리 최적화
CREATE INDEX idx_active_events ON events(
    is_active,
    start_time,
    end_time
) WHERE is_active = TRUE;

CREATE INDEX idx_user_event_progress ON user_events(
    user_id,
    is_completed,
    rewards_claimed
);

-- ======================================================
-- Covering Indexes for Read-Heavy Queries
-- ======================================================

-- 룸 목록 조회용 커버링 인덱스
CREATE INDEX idx_room_list_covering ON game_rooms(
    room_status,
    is_private,
    created_at DESC
) INCLUDE (
    room_code,
    room_name,
    game_mode,
    current_players,
    max_players,
    owner_id
);

-- 사용자 프로필 조회용 커버링 인덱스
CREATE INDEX idx_user_profile_covering ON users(
    user_id,
    status
) INCLUDE (
    username,
    nickname,
    avatar_url,
    last_login_at
);

-- ======================================================
-- Full-Text Search Indexes
-- ======================================================

-- 룸 이름 검색
ALTER TABLE game_rooms ADD FULLTEXT idx_room_name_ft (room_name);

-- 사용자 닉네임 검색
ALTER TABLE users ADD FULLTEXT idx_nickname_ft (nickname);

-- 채팅 메시지 검색
ALTER TABLE chat_messages ADD FULLTEXT idx_message_ft (message_content);

-- ======================================================
-- Index Hints for Critical Queries
-- ======================================================

-- Example: 활성 룸 목록 조회
-- SELECT /*+ INDEX(game_rooms idx_rooms_list) */
--     room_id, room_code, room_name, current_players, max_players
-- FROM game_rooms
-- WHERE room_status = 'waiting' 
--     AND is_private = FALSE
-- ORDER BY created_at DESC
-- LIMIT 20;

-- Example: 사용자 랭킹 조회
-- SELECT /*+ INDEX(user_profiles idx_ranking_winrate) */
--     user_id, win_rate, total_games
-- FROM user_profiles
-- WHERE total_games >= 10
-- ORDER BY win_rate DESC
-- LIMIT 100;