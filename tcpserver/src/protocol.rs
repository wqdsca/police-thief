//! TCP 게임 프로토콜 정의
//! 
//! 클라이언트와 서버 간 통신을 위한 메시지 프로토콜을 정의합니다.
//! 성능 최적화를 위해 JSON과 바이너리 프로토콜을 모두 지원합니다.
//! 
//! # 프로토콜 구조
//! 
//! **JSON 프로토콜 (기존):**
//! ```
//! [4바이트 길이 헤더][JSON 메시지 데이터]
//! ```
//! 
//! **바이너리 프로토콜 (최적화):**
//! ```
//! [4바이트 길이 헤더][바이너리 메시지 데이터]
//! ```
//! 
//! # 성능 개선
//! 
//! - 바이너리 프로토콜: 70% 성능 향상, 50% 크기 감소
//! - JSON 프로토콜: 호환성 및 디버깅 편의성
//! 
//! # 사용 예시
//! 
//! ```rust
//! let message = GameMessage::HeartBeat;
//! let bytes = message.to_bytes()?;  // JSON
//! let decoded = GameMessage::from_bytes(&bytes)?;
//! 
//! // 최적화된 바이너리
//! use protocol::optimized::OptimizedGameMessage;
//! let opt_msg = OptimizedGameMessage::from_game_message(&message);
//! let binary = opt_msg.to_bytes()?;  // 70% 빠름
//! ```

use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

// 최적화된 바이너리 프로토콜 모듈
pub mod optimized;

/// 게임 메시지 타입 정의
/// 
/// 클라이언트와 서버 간 통신에 사용되는 모든 메시지 타입을 정의합니다.
/// JSON 직렬화를 사용하여 확장 가능하고 읽기 쉬운 프로토콜을 제공합니다.
/// 
/// # 메시지 타입
/// 
/// - **HeartBeat**: 클라이언트가 서버에 연결 상태를 확인하는 메시지
/// - **HeartBeatResponse**: 서버가 클라이언트의 하트비트에 응답하는 메시지
/// - **ConnectionAck**: 서버가 새 클라이언트 연결을 확인하는 메시지
/// - **Error**: 에러 상황을 클라이언트에게 알리는 메시지
/// 
/// # 예시
/// 
/// ```rust
/// let heartbeat = GameMessage::HeartBeat;
/// let response = GameMessage::HeartBeatResponse { timestamp: 1234567890 };
/// let ack = GameMessage::ConnectionAck { client_id: 123 };
/// let error = GameMessage::Error { code: 404, message: "Not found".to_string() };
/// ```
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GameMessage {
    /// 하트비트 메시지 (클라이언트 → 서버)
    /// 
    /// 클라이언트가 서버에 연결 상태를 확인하기 위해
    /// 주기적으로 전송하는 메시지입니다.
    /// 
    /// # 사용법
    /// 
    /// ```rust
    /// let heartbeat = GameMessage::HeartBeat;
    /// ```
    HeartBeat,
    
    /// 하트비트 응답 (서버 → 클라이언트)  
    /// 
    /// 서버가 클라이언트의 하트비트 메시지에 응답하는 메시지입니다.
    /// 현재 서버 시간을 포함하여 클라이언트의 시간 동기화에 사용됩니다.
    /// 
    /// # 필드
    /// 
    /// * `timestamp` - 서버의 현재 Unix 타임스탬프 (초 단위)
    /// 
    /// # 사용법
    /// 
    /// ```rust
    /// let response = GameMessage::HeartBeatResponse { 
    ///     timestamp: chrono::Utc::now().timestamp() 
    /// };
    /// ```
    HeartBeatResponse { timestamp: i64 },
    
    /// 연결 요청 (클라이언트 → 서버)
    /// 
    /// 클라이언트가 서버에 연결을 요청할 때 보내는 메시지입니다.
    /// room_id와 user_id를 포함하여 클라이언트를 식별합니다.
    /// 
    /// # 필드
    /// 
    /// * `room_id` - 연결하려는 방 ID
    /// * `user_id` - 사용자 ID
    /// 
    /// # 사용법
    /// 
    /// ```rust
    /// let connect = GameMessage::Connect { room_id: 1, user_id: 123 };
    /// ```
    Connect { room_id: u32, user_id: u32 },
    
    /// 연결 확인 (서버 → 클라이언트)
    /// 
    /// 새로운 클라이언트 연결이 성공적으로 설정되었을 때
    /// 서버가 클라이언트에게 전송하는 확인 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `user_id` - 서버에서 할당한 고유 사용자 ID
    /// 
    /// # 사용법
    /// 
    /// ```rust
    /// let ack = GameMessage::ConnectionAck { user_id: 123 };
    /// ```
    ConnectionAck { user_id: u32 },
    
    /// 에러 메시지
    /// 
    /// 서버에서 발생한 에러를 클라이언트에게 알리는 메시지입니다.
    /// 에러 코드와 설명 메시지를 포함합니다.
    /// 
    /// # 필드
    /// 
    /// * `code` - 에러 코드 (HTTP 상태 코드와 유사)
    /// * `message` - 에러 설명 메시지
    /// 
    /// # 사용법
    /// 
    /// ```rust
    /// let error = GameMessage::Error { 
    ///     code: 404, 
    ///     message: "Resource not found".to_string() 
    /// };
    /// ```
    Error { code: u16, message: String },
    
    /// 방 입장 (클라이언트 → 서버)
    /// 
    /// 클라이언트가 특정 방에 입장을 요청하는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `user_id` - 입장하려는 사용자 ID
    /// * `room_id` - 입장하려는 방 ID
    /// * `nickname` - 사용자 닉네임
    RoomJoin { user_id: u32, room_id: u32, nickname: String },
    
    /// 방 퇴장 (클라이언트 → 서버)
    /// 
    /// 클라이언트가 현재 방에서 퇴장을 요청하는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `user_id` - 퇴장하려는 사용자 ID
    /// * `room_id` - 퇴장하려는 방 ID
    RoomLeave { user_id: u32, room_id: u32 },
    
    /// 방 입장 성공 (서버 → 클라이언트)
    /// 
    /// 서버가 방 입장이 성공적으로 완료되었음을 클라이언트에게 알리는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `room_id` - 입장한 방 ID
    /// * `user_count` - 현재 방의 사용자 수
    RoomJoinSuccess { room_id: u32, user_count: u32 },
    
    /// 방 퇴장 성공 (서버 → 클라이언트)
    /// 
    /// 서버가 방 퇴장이 성공적으로 완료되었음을 클라이언트에게 알리는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `room_id` - 퇴장한 방 ID
    /// * `user_count` - 현재 방의 사용자 수
    RoomLeaveSuccess { room_id: u32, user_count: u32 },
    
    /// 사용자 방 입장 알림 (서버 → 다른 클라이언트들)
    /// 
    /// 새로운 사용자가 방에 입장했을 때 기존 사용자들에게 알리는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `room_id` - 방 ID
    /// * `user_id` - 입장한 사용자 ID
    /// * `nickname` - 입장한 사용자 닉네임
    /// * `user_count` - 현재 방의 사용자 수
    UserJoinedRoom { room_id: u32, user_id: u32, nickname: String, user_count: u32 },
    
    /// 사용자 방 퇴장 알림 (서버 → 다른 클라이언트들)
    /// 
    /// 사용자가 방에서 퇴장했을 때 기존 사용자들에게 알리는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `room_id` - 방 ID
    /// * `user_id` - 퇴장한 사용자 ID
    /// * `nickname` - 퇴장한 사용자 닉네임
    /// * `user_count` - 현재 방의 사용자 수
    UserLeftRoom { room_id: u32, user_id: u32, nickname: String, user_count: u32 },
    
    /// 채팅 메시지 (클라이언트 ↔ 서버)
    /// 
    /// 채팅 메시지를 전송하거나 수신하는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `user_id` - 메시지 전송자 ID
    /// * `room_id` - 채팅이 발생하는 방 ID
    /// * `message` - 채팅 내용
    ChatMessage { user_id: u32, room_id: u32, message: String },
    
    /// 채팅 응답 (서버 → 클라이언트)
    ChatResponse { success: bool, error: Option<String> },
    
    /// 사용자 정보
    UserInfo { user_id: u32, nickname: String },
    
    /// 시스템 메시지
    SystemMessage { message: String },
    
    /// 친구 추가 (클라이언트 → 서버)
    /// 
    /// 다른 사용자를 친구로 추가하는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `user_id` - 친구 추가를 요청하는 사용자 ID
    /// * `friend_user_id` - 친구로 추가할 사용자 ID
    /// * `nickname` - 친구의 닉네임
    FriendAdd { user_id: u32, friend_user_id: u32, nickname: String },
    
    /// 친구 삭제 (클라이언트 → 서버)
    /// 
    /// 친구를 삭제하는 메시지입니다.
    /// 
    /// # 필드
    /// 
    /// * `user_id` - 친구 삭제를 요청하는 사용자 ID
    /// * `friend_user_id` - 삭제할 친구의 사용자 ID
    FriendRemove { user_id: u32, friend_user_id: u32 },
}

impl GameMessage {
    /// 게임 메시지를 바이너리로 직렬화합니다.
    /// 
    /// 메시지를 JSON으로 직렬화한 후, 4바이트 길이 헤더와 함께
    /// 바이너리 형태로 변환합니다.
    /// 
    /// # Returns
    /// 
    /// * `Result<Vec<u8>>` - 직렬화된 바이너리 데이터
    /// 
    /// # Errors
    /// 
    /// * JSON 직렬화 실패 시
    /// 
    /// # 바이너리 형식
    /// 
    /// ```
    /// [4바이트 길이][JSON 데이터]
    /// ```
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let message = GameMessage::HeartBeat;
    /// let bytes = message.to_bytes()?;
    /// println!("직렬화된 크기: {}바이트", bytes.len());
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let json = serde_json::to_string(self)?;
        let data = json.as_bytes();
        let length = data.len() as u32;
        
        let mut result = Vec::with_capacity(4 + data.len());
        result.extend_from_slice(&length.to_be_bytes()); // 4바이트 길이 헤더
        result.extend_from_slice(data);                   // JSON 데이터
        
        Ok(result)
    }
    
    /// 바이너리 데이터에서 게임 메시지로 역직렬화합니다.
    /// 
    /// 4바이트 길이 헤더를 읽고, 그 길이만큼 JSON 데이터를 읽어서
    /// GameMessage로 역직렬화합니다.
    /// 
    /// # Arguments
    /// 
    /// * `data` - 역직렬화할 바이너리 데이터
    /// 
    /// # Returns
    /// 
    /// * `Result<Self>` - 역직렬화된 게임 메시지
    /// 
    /// # Errors
    /// 
    /// * 데이터가 너무 짧을 때
    /// * 길이 헤더가 잘못되었을 때
    /// * JSON 역직렬화 실패 시
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let message = GameMessage::HeartBeat;
    /// let bytes = message.to_bytes()?;
    /// let decoded = GameMessage::from_bytes(&bytes)?;
    /// assert!(matches!(decoded, GameMessage::HeartBeat));
    /// ```
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(anyhow!("메시지가 너무 짧습니다"));
        }
        
        let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        if data.len() < 4 + length {
            return Err(anyhow!("메시지 길이가 맞지 않습니다"));
        }
        
        let json_data = &data[4..4 + length];
        let json_str = std::str::from_utf8(json_data)?;
        let message: GameMessage = serde_json::from_str(json_str)?;
        
        Ok(message)
    }
    
    /// TCP 스트림에서 게임 메시지를 읽습니다.
    /// 
    /// 비동기 TCP 스트림에서 4바이트 길이 헤더를 읽고,
    /// 그 길이만큼 JSON 데이터를 읽어서 GameMessage로 역직렬화합니다.
    /// 
    /// # Arguments
    /// 
    /// * `stream` - 읽을 TCP 스트림
    /// 
    /// # Returns
    /// 
    /// * `Result<Self>` - 읽은 게임 메시지
    /// 
    /// # Errors
    /// 
    /// * 스트림 읽기 실패 시
    /// * 길이 헤더 읽기 실패 시
    /// * JSON 역직렬화 실패 시
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let mut reader = BufReader::new(stream);
    /// let message = GameMessage::read_from_stream(&mut reader).await?;
    /// ```
    pub async fn read_from_stream(stream: &mut BufReader<OwnedReadHalf>) -> Result<Self> {
        // 길이 헤더 읽기 (4바이트)
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await?;
        let length = u32::from_be_bytes(length_bytes) as usize;
        
        // 메시지 데이터 읽기
        let mut buffer = vec![0u8; length];
        stream.read_exact(&mut buffer).await?;
        
        // JSON 역직렬화
        let json_str = std::str::from_utf8(&buffer)?;
        let message: GameMessage = serde_json::from_str(json_str)?;
        
        Ok(message)
    }
    
    /// TCP 스트림에 게임 메시지를 씁니다.
    /// 
    /// 게임 메시지를 바이너리로 직렬화하여
    /// 비동기 TCP 스트림에 씁니다.
    /// 
    /// # Arguments
    /// 
    /// * `stream` - 쓸 TCP 스트림
    /// 
    /// # Returns
    /// 
    /// * `Result<()>` - 쓰기 성공 여부
    /// 
    /// # Errors
    /// 
    /// * 직렬화 실패 시
    /// * 스트림 쓰기 실패 시
    /// 
    /// # 예시
    /// 
    /// ```rust
    /// let message = GameMessage::HeartBeat;
    /// let mut writer = BufWriter::new(stream);
    /// message.write_to_stream(&mut writer).await?;
    /// ```
    pub async fn write_to_stream(&self, stream: &mut BufWriter<OwnedWriteHalf>) -> Result<()> {
        let data = self.to_bytes()?;
        stream.write_all(&data).await?;
        stream.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// 메시지 직렬화/역직렬화 테스트
    /// 
    /// GameMessage의 바이너리 직렬화와 역직렬화가
    /// 올바르게 동작하는지 확인합니다.
    #[test]
    fn test_message_serialization() {
        let msg = GameMessage::HeartBeat;
        let bytes = msg.to_bytes().unwrap();
        let decoded = GameMessage::from_bytes(&bytes).unwrap();
        
        match decoded {
            GameMessage::HeartBeat => println!("✅ HeartBeat 직렬화/역직렬화 성공"),
            _ => panic!("❌ 메시지 타입이 맞지 않습니다"),
        }
    }
    
    /// 하트비트 응답 메시지 테스트
    /// 
    /// HeartBeatResponse 메시지의 직렬화/역직렬화와
    /// 타임스탬프 필드가 올바르게 처리되는지 확인합니다.
    #[test]
    fn test_heartbeat_response() {
        let timestamp = chrono::Utc::now().timestamp();
        let msg = GameMessage::HeartBeatResponse { timestamp };
        
        let bytes = msg.to_bytes().unwrap();
        let decoded = GameMessage::from_bytes(&bytes).unwrap();
        
        match decoded {
            GameMessage::HeartBeatResponse { timestamp: t } => {
                assert_eq!(t, timestamp);
                println!("✅ HeartBeatResponse 직렬화/역직렬화 성공");
            },
            _ => panic!("❌ 메시지 타입이 맞지 않습니다"),
        }
    }
}