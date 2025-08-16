//! Game Logic Handler Interface
//!
//! 이 모듈은 게임 핵심 로직을 구현할 수 있는 인터페이스를 제공합니다.
//! 통신 최적화는 프레임워크에서 처리하고, 사용자는 게임 로직에만 집중할 수 있습니다.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 게임 메시지 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMessage {
    pub msg_type: String,
    pub payload: serde_json::Value,
    pub timestamp: i64,
    pub sequence: u64,
    pub player_id: Option<String>,
    pub room_id: Option<String>,
}

/// 게임 응답 메시지
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResponse {
    pub msg_type: String,
    pub payload: serde_json::Value,
    pub timestamp: i64,
    pub sequence: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// 플레이어 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: String,
    pub nickname: String,
    pub position: Option<Position>,
    pub health: Option<i32>,
    pub status: PlayerStatus,
}

/// 위치 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
    pub rotation: Option<f64>,
}

/// 플레이어 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerStatus {
    Online,
    InGame,
    Offline,
}

/// 게임 로직 처리를 위한 트레이트
/// 사용자는 이 트레이트를 구현하여 핵심 게임 로직을 작성합니다.
#[async_trait::async_trait]
pub trait GameLogicHandler: Send + Sync {
    /// 플레이어 로그인 처리
    async fn handle_login(&self, payload: serde_json::Value) -> Result<GameResponse>;

    /// 플레이어 로그아웃 처리
    async fn handle_logout(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 방 생성 처리
    async fn handle_create_room(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 방 참가 처리
    async fn handle_join_room(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 방 나가기 처리
    async fn handle_leave_room(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 플레이어 이동 처리 (핵심 로직)
    async fn handle_player_move(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 플레이어 공격 처리 (핵심 로직)
    async fn handle_player_attack(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 채팅 메시지 처리
    async fn handle_chat(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 컨트롤 메시지 처리 (QUIC specific)
    async fn process_control_message(&self, message: &[u8]) -> Result<Vec<u8>> {
        // Default implementation - convert message to string and process
        let msg_str = String::from_utf8_lossy(message);
        tracing::warn!("Control message processing not implemented: {}", msg_str);
        Ok(b"OK".to_vec())
    }

    /// 커스텀 메시지 처리 (확장 가능)
    async fn handle_custom(
        &self,
        msg_type: &str,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse>;

    /// 게임 상태 업데이트 알림 (브로드캐스트용)
    async fn on_game_state_changed(
        &self,
        room_id: &str,
        players: &[PlayerInfo],
    ) -> Result<Vec<GameMessage>>;

    /// 플레이어 연결 해제 처리
    async fn on_player_disconnected(&self, player_id: &str) -> Result<()>;
}

/// 기본 게임 로직 핸들러 (예제 구현)
/// 실제 게임에서는 이를 참고하여 구현하세요.
pub struct DefaultGameLogicHandler {
    // 실제 구현에서는 데이터베이스, Redis 등의 연결을 여기에 추가
    players: tokio::sync::RwLock<HashMap<String, PlayerInfo>>,
    rooms: tokio::sync::RwLock<HashMap<String, Vec<String>>>, // room_id -> player_ids
}

impl DefaultGameLogicHandler {
    pub fn new() -> Self {
        Self {
            players: tokio::sync::RwLock::new(HashMap::new()),
            rooms: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl GameLogicHandler for DefaultGameLogicHandler {
    async fn handle_login(&self, payload: serde_json::Value) -> Result<GameResponse> {
        let nickname = payload
            .get("nickname")
            .and_then(|v| v.as_str())
            .unwrap_or("Anonymous");

        let player_id = uuid::Uuid::new_v4().to_string();
        let player = PlayerInfo {
            id: player_id.clone(),
            nickname: nickname.to_string(),
            position: Some(Position {
                x: 0.0,
                y: 0.0,
                z: None,
                rotation: None,
            }),
            health: Some(100),
            status: PlayerStatus::Online,
        };

        self.players.write().await.insert(player_id.clone(), player);

        Ok(GameResponse {
            msg_type: "login_response".to_string(),
            payload: serde_json::json!({
                "player_id": player_id,
                "nickname": nickname,
                "session_token": format!("quic_token_{}", player_id)
            }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0, // 시퀀스는 외부에서 설정
            success: true,
            error_message: None,
        })
    }

    async fn handle_logout(
        &self,
        player_id: &str,
        _payload: serde_json::Value,
    ) -> Result<GameResponse> {
        self.players.write().await.remove(player_id);

        Ok(GameResponse {
            msg_type: "logout_response".to_string(),
            payload: serde_json::json!({ "success": true }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            success: true,
            error_message: None,
        })
    }

    async fn handle_create_room(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        let room_name = payload
            .get("room_name")
            .and_then(|v| v.as_str())
            .unwrap_or("New Room");

        let room_id = uuid::Uuid::new_v4().to_string();
        self.rooms
            .write()
            .await
            .insert(room_id.clone(), vec![player_id.to_string()]);

        Ok(GameResponse {
            msg_type: "create_room_response".to_string(),
            payload: serde_json::json!({
                "room_id": room_id,
                "room_name": room_name
            }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            success: true,
            error_message: None,
        })
    }

    async fn handle_join_room(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        let room_id = payload
            .get("room_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing room_id"))?;

        let mut rooms = self.rooms.write().await;
        if let Some(room_players) = rooms.get_mut(room_id) {
            if !room_players.contains(&player_id.to_string()) {
                room_players.push(player_id.to_string());
            }

            Ok(GameResponse {
                msg_type: "join_room_response".to_string(),
                payload: serde_json::json!({
                    "room_id": room_id,
                    "players_count": room_players.len()
                }),
                timestamp: chrono::Utc::now().timestamp_millis(),
                sequence: 0,
                success: true,
                error_message: None,
            })
        } else {
            Ok(GameResponse {
                msg_type: "join_room_response".to_string(),
                payload: serde_json::json!({}),
                timestamp: chrono::Utc::now().timestamp_millis(),
                sequence: 0,
                success: false,
                error_message: Some("Room not found".to_string()),
            })
        }
    }

    async fn handle_leave_room(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        let room_id = payload
            .get("room_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing room_id"))?;

        let mut rooms = self.rooms.write().await;
        if let Some(room_players) = rooms.get_mut(room_id) {
            room_players.retain(|id| id != player_id);

            // 방이 비었으면 삭제
            if room_players.is_empty() {
                rooms.remove(room_id);
            }
        }

        Ok(GameResponse {
            msg_type: "leave_room_response".to_string(),
            payload: serde_json::json!({ "success": true }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            success: true,
            error_message: None,
        })
    }

    /// 🎮 핵심 게임 로직: 플레이어 이동
    /// 여기에 실제 이동 로직을 구현하세요
    async fn handle_player_move(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        // 이동 데이터 파싱
        let x = payload.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = payload.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let rotation = payload.get("rotation").and_then(|v| v.as_f64());

        // 플레이어 위치 업데이트
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.position = Some(Position {
                x,
                y,
                z: None,
                rotation,
            });

            // TODO: 여기에 실제 게임 로직 추가
            // - 이동 가능한 범위 검증
            // - 충돌 검사
            // - 다른 플레이어들에게 브로드캐스트

            Ok(GameResponse {
                msg_type: "player_move_response".to_string(),
                payload: serde_json::json!({
                    "player_id": player_id,
                    "position": {
                        "x": x,
                        "y": y,
                        "rotation": rotation
                    }
                }),
                timestamp: chrono::Utc::now().timestamp_millis(),
                sequence: 0,
                success: true,
                error_message: None,
            })
        } else {
            Ok(GameResponse {
                msg_type: "player_move_response".to_string(),
                payload: serde_json::json!({}),
                timestamp: chrono::Utc::now().timestamp_millis(),
                sequence: 0,
                success: false,
                error_message: Some("Player not found".to_string()),
            })
        }
    }

    /// 🎮 핵심 게임 로직: 플레이어 공격
    /// 여기에 실제 공격 로직을 구현하세요
    async fn handle_player_attack(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        let target_id = payload.get("target_id").and_then(|v| v.as_str());
        let attack_type = payload
            .get("attack_type")
            .and_then(|v| v.as_str())
            .unwrap_or("basic");
        let damage = payload.get("damage").and_then(|v| v.as_i64()).unwrap_or(10) as i32;

        // TODO: 여기에 실제 공격 로직 추가
        // - 공격 범위 검증
        // - 데미지 계산
        // - 타겟 플레이어 체력 감소
        // - 결과를 모든 플레이어에게 브로드캐스트

        Ok(GameResponse {
            msg_type: "player_attack_response".to_string(),
            payload: serde_json::json!({
                "attacker_id": player_id,
                "target_id": target_id,
                "attack_type": attack_type,
                "damage": damage,
                "result": "hit" // hit, miss, critical 등
            }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            success: true,
            error_message: None,
        })
    }

    async fn handle_chat(
        &self,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        let message = payload
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let room_id = payload.get("room_id").and_then(|v| v.as_str());

        // TODO: 채팅 필터링, 스팸 방지 등 추가

        Ok(GameResponse {
            msg_type: "chat_response".to_string(),
            payload: serde_json::json!({
                "sender_id": player_id,
                "message": message,
                "room_id": room_id,
                "timestamp": chrono::Utc::now().timestamp_millis()
            }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            success: true,
            error_message: None,
        })
    }

    async fn handle_custom(
        &self,
        msg_type: &str,
        player_id: &str,
        payload: serde_json::Value,
    ) -> Result<GameResponse> {
        // 커스텀 메시지 타입 처리
        // 예: "skill_use", "item_pickup", "trade_request" 등

        Ok(GameResponse {
            msg_type: format!("{}_response", msg_type),
            payload: serde_json::json!({
                "player_id": player_id,
                "original_type": msg_type,
                "handled": false,
                "note": "Implement custom message handling"
            }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            success: false,
            error_message: Some(format!("Unhandled custom message type: {}", msg_type)),
        })
    }

    async fn on_game_state_changed(
        &self,
        room_id: &str,
        players: &[PlayerInfo],
    ) -> Result<Vec<GameMessage>> {
        // 게임 상태 변경 시 브로드캐스트할 메시지들 생성
        let state_message = GameMessage {
            msg_type: "game_state_update".to_string(),
            payload: serde_json::json!({
                "room_id": room_id,
                "players": players,
                "timestamp": chrono::Utc::now().timestamp_millis()
            }),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sequence: 0,
            player_id: None,
            room_id: Some(room_id.to_string()),
        };

        Ok(vec![state_message])
    }

    async fn on_player_disconnected(&self, player_id: &str) -> Result<()> {
        // 플레이어 연결 해제 시 정리 작업
        self.players.write().await.remove(player_id);

        // 모든 방에서 플레이어 제거
        let mut rooms = self.rooms.write().await;
        let mut empty_rooms = Vec::new();

        for (room_id, room_players) in rooms.iter_mut() {
            room_players.retain(|id| id != player_id);
            if room_players.is_empty() {
                empty_rooms.push(room_id.clone());
            }
        }

        // 빈 방들 삭제
        for room_id in empty_rooms {
            rooms.remove(&room_id);
        }

        Ok(())
    }
}

impl Default for DefaultGameLogicHandler {
    fn default() -> Self {
        Self::new()
    }
}
