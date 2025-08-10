//! 게임 메시지 프로토콜 정의
//!
//! RUDP를 통해 전송되는 모든 게임 메시지의 정의와 직렬화/역직렬화 구현
//! 연결(Connect), 이동(Move), 공격(Attack), 사망(Die) 등 핵심 게임 액션을 포함
//!
//! # 프로토콜 설계 원칙
//! - **최소 오버헤드**: 바이너리 직렬화 (bincode) 사용
//! - **타입 안전성**: 강타입 열거형으로 메시지 구분
//! - **버전 호환성**: 향후 확장을 위한 예약 필드 포함
//! - **검증 가능**: 모든 입력 데이터 유효성 검사 지원

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 플레이어 ID 타입 정의
pub type PlayerId = u32;

/// 게임 메시지 최상위 열거형
///
/// 클라이언트와 서버 간 모든 통신에 사용되는 메시지 타입을 정의합니다.
/// 각 메시지는 바이너리로 직렬화되어 RUDP 패킷으로 전송됩니다.
///
/// # 예시
/// ```rust
/// use rudpserver::game::GameMessage;
/// let connect_msg = GameMessage::Connect {
///     player_name: "Player1".to_string(),
///     auth_token: "abc123".to_string(),
///     client_version: "1.0.0".to_string(),
/// };
/// let serialized = bincode::serialize(&connect_msg)?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameMessage {
    // === 연결 관리 메시지 ===
    /// 클라이언트 연결 요청
    ///
    /// 새로운 플레이어가 게임 서버에 접속할 때 전송하는 메시지입니다.
    /// 인증 토큰을 통해 플레이어를 검증하고 게임 월드에 스폰시킵니다.
    Connect {
        /// 플레이어 이름 (3-20자, 영문/숫자만)
        player_name: String,
        /// JWT 인증 토큰
        auth_token: String,
        /// 클라이언트 버전 (호환성 검사용)
        client_version: String,
    },

    /// 서버 연결 응답
    ///
    /// Connect 메시지에 대한 서버의 응답으로, 연결 성공/실패와
    /// 플레이어의 초기 게임 상태 정보를 포함합니다.
    ConnectResponse {
        /// 연결 성공 여부
        success: bool,
        /// 할당된 플레이어 ID (성공시)
        player_id: Option<PlayerId>,
        /// 초기 스폰 위치 (성공시)
        spawn_position: Option<Position>,
        /// 초기 플레이어 상태 (성공시)
        initial_state: Option<PlayerState>,
        /// 응답 메시지 (오류시 이유 포함)
        message: String,
        /// 서버 설정 정보
        server_config: Option<ServerConfig>,
    },

    /// 연결 해제 요청
    ///
    /// 클라이언트가 정상적으로 게임을 종료할 때 전송하는 메시지입니다.
    /// 서버는 플레이어 데이터를 저장하고 다른 플레이어들에게 알립니다.
    Disconnect {
        /// 연결 해제 사유
        reason: DisconnectReason,
    },

    // === 이동 관련 메시지 ===
    /// 플레이어 이동 요청
    ///
    /// 클라이언트에서 플레이어의 이동 입력이 발생할 때 전송하는 메시지입니다.
    /// 높은 빈도로 전송되므로 RUDP의 비신뢰성 채널을 사용합니다.
    Move {
        /// 목표 위치
        target_position: Position,
        /// 이동 방향 벡터 (-1.0 ~ 1.0)
        direction: Direction,
        /// 이동 속도 (0.0 ~ 1.0, 달리기/걷기)
        speed_multiplier: f32,
        /// 클라이언트 예측 시간 (지연 보상용)
        client_timestamp: u64,
    },

    /// 플레이어 이동 브로드캐스트
    ///
    /// 서버가 모든 클라이언트에게 플레이어 위치 업데이트를 알리는 메시지입니다.
    /// 관심 영역(AOI) 내의 플레이어들에게만 전송됩니다.
    MoveUpdate {
        /// 이동한 플레이어 ID
        player_id: PlayerId,
        /// 현재 위치
        current_position: Position,
        /// 이동 속도
        velocity: Velocity,
        /// 서버 타임스탬프
        server_timestamp: u64,
    },

    // === 전투 관련 메시지 ===
    /// 공격 요청
    ///
    /// 플레이어가 다른 플레이어나 오브젝트를 공격할 때 전송하는 메시지입니다.
    /// 중요한 게임 로직이므로 RUDP의 신뢰성 채널을 사용합니다.
    Attack {
        /// 공격 대상 (플레이어 ID 또는 좌표)
        target: AttackTarget,
        /// 공격 타입 (근접, 원거리, 스킬 등)
        attack_type: AttackType,
        /// 사용된 무기/스킬 ID
        weapon_id: Option<u32>,
        /// 공격 방향
        attack_direction: Direction,
        /// 클라이언트 예측 데미지
        predicted_damage: u32,
    },

    /// 공격 결과 브로드캐스트
    ///
    /// 서버가 공격 결과를 모든 관련 플레이어에게 알리는 메시지입니다.
    /// 데미지, 상태 효과, 애니메이션 정보를 포함합니다.
    AttackResult {
        /// 공격자 플레이어 ID
        attacker_id: PlayerId,
        /// 공격 대상 정보
        target: AttackTarget,
        /// 공격 성공 여부
        hit: bool,
        /// 실제 입힌 데미지
        damage_dealt: u32,
        /// 치명타 여부
        critical_hit: bool,
        /// 대상의 남은 체력 (공격 성공시)
        target_health: Option<u32>,
        /// 서버 타임스탬프
        server_timestamp: u64,
    },

    // === 생존/사망 관련 메시지 ===
    /// 플레이어 사망 알림
    ///
    /// 플레이어가 사망했을 때 서버가 모든 클라이언트에게 알리는 메시지입니다.
    /// 사망 원인, 킬러 정보, 드롭 아이템 등을 포함합니다.
    Die {
        /// 사망한 플레이어 ID
        player_id: PlayerId,
        /// 사망 원인
        death_cause: DeathCause,
        /// 킬러 플레이어 ID (PvP 사망시)
        killer_id: Option<PlayerId>,
        /// 사망 위치
        death_position: Position,
        /// 드롭된 아이템들
        dropped_items: Vec<DroppedItem>,
        /// 리스폰 쿨타임 (초)
        respawn_cooldown: u32,
        /// 경험치/골드 페널티
        death_penalty: DeathPenalty,
    },

    /// 리스폰 요청
    ///
    /// 사망한 플레이어가 다시 살아나고 싶을 때 전송하는 메시지입니다.
    Respawn,

    /// 리스폰 완료 알림
    ///
    /// 플레이어가 성공적으로 리스폰되었음을 알리는 메시지입니다.
    RespawnComplete {
        /// 리스폰된 플레이어 ID
        player_id: PlayerId,
        /// 새로운 스폰 위치
        spawn_position: Position,
        /// 복구된 플레이어 상태
        restored_state: PlayerState,
        /// 서버 타임스탬프
        server_timestamp: u64,
    },

    // === 상태 동기화 메시지 ===
    /// 플레이어 상태 업데이트
    ///
    /// 체력, 마나, 경험치 등 플레이어 상태 변화를 알리는 메시지입니다.
    StateUpdate {
        /// 대상 플레이어 ID
        player_id: PlayerId,
        /// 변경된 상태 정보
        state_changes: HashMap<String, StateValue>,
        /// 서버 타임스탬프
        server_timestamp: u64,
    },

    // === 에러 및 시스템 메시지 ===
    /// 에러 메시지
    ///
    /// 서버에서 발생한 에러를 클라이언트에게 알리는 메시지입니다.
    Error {
        /// 에러 코드
        error_code: String,
        /// 에러 메시지
        error_message: String,
        /// 에러 카테고리
        category: ErrorCategory,
        /// 복구 가능 여부
        recoverable: bool,
    },

    /// 서버 상태 알림
    ///
    /// 서버 점검, 재시작 등 시스템 메시지를 전달합니다.
    ServerNotice {
        /// 알림 타입
        notice_type: NoticeType,
        /// 알림 내용
        message: String,
        /// 긴급도 레벨
        priority: Priority,
        /// 만료 시간 (옵션)
        expires_at: Option<u64>,
    },
}

// === 데이터 구조체 정의 ===

/// 3D 위치 좌표
///
/// Unity 게임 월드 내의 3D 위치를 나타내는 구조체입니다.
/// 부동소수점을 사용하여 정밀한 위치 표현이 가능합니다.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    /// X 좌표 (가로축)
    pub x: f32,
    /// Y 좌표 (높이축) - Unity의 Y-up 좌표계
    pub y: f32,
    /// Z 좌표 (세로축)
    pub z: f32,
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// 3D 위치 유효성 검사
    ///
    /// # Arguments
    /// * `world_bounds` - (width, height, depth) 월드 경계
    ///
    /// # Returns
    /// 유효한 위치인지 여부
    pub fn is_valid(&self, world_bounds: (f32, f32, f32)) -> bool {
        let (width, height, depth) = world_bounds;
        self.x >= -width / 2.0
            && self.x <= width / 2.0
            && self.y >= 0.0
            && self.y <= height
            && self.z >= -depth / 2.0
            && self.z <= depth / 2.0
    }
}

impl Default for Position {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// 3D 방향 벡터
///
/// Unity에서 이동 방향이나 공격 방향을 나타내는 정규화된 벡터입니다.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Direction {
    /// X 방향 성분 (-1.0 ~ 1.0)
    pub x: f32,
    /// Y 방향 성분 (-1.0 ~ 1.0)
    pub y: f32,
    /// Z 방향 성분 (-1.0 ~ 1.0)
    pub z: f32,
}

impl Direction {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        let magnitude = (x * x + y * y + z * z).sqrt();
        if magnitude > 0.0 {
            Self {
                x: x / magnitude,
                y: y / magnitude,
                z: z / magnitude,
            }
        } else {
            Self {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }
        }
    }

    /// 3D 방향의 크기 (길이) 계산
    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// 3D 속도 벡터
///
/// Unity에서 플레이어나 오브젝트의 이동 속도를 나타냅니다.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Velocity {
    /// X 방향 속도 (단위/초)
    pub x: f32,
    /// Y 방향 속도 (단위/초)
    pub y: f32,
    /// Z 방향 속도 (단위/초)
    pub z: f32,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Velocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// 벡터의 크기 계산
    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// 정규화된 방향 벡터 반환
    pub fn normalize(self) -> Self {
        let magnitude = self.magnitude();
        if magnitude > 0.0 {
            Self {
                x: self.x / magnitude,
                y: self.y / magnitude,
                z: self.z / magnitude,
            }
        } else {
            Self::default()
        }
    }
}

/// 플레이어 상태 정보
///
/// Unity 클라이언트를 위한 플레이어의 현재 게임 상태를 나타내는 구조체입니다.
/// 체력, 마나, 장비 등 필수 플레이어 속성을 포함합니다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerState {
    /// 현재 체력
    pub health: u32,
    /// 최대 체력
    pub max_health: u32,
    /// 현재 마나
    pub mana: u32,
    /// 최대 마나
    pub max_mana: u32,
    /// 현재 위치 (3D)
    pub position: Position,
    /// 이동 속도
    pub movement_speed: f32,
    /// 공격력
    pub attack_power: u32,
    /// 방어력
    pub defense: u32,
    /// 인벤토리 아이템 수
    pub inventory_count: u32,
    /// 플레이어 상태 (생존, 사망, 전투 등)
    pub player_status: PlayerStatus,
}

/// 공격 대상 열거형
///
/// 공격의 대상이 특정 플레이어인지 좌표 기반 공격인지를 구분합니다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AttackTarget {
    /// 특정 플레이어 공격
    Player(PlayerId),
    /// 좌표 기반 공격 (AoE 스킬 등)
    Position(Position),
    /// NPC/몬스터 공격
    Npc(u32),
}

/// 공격 타입 열거형
///
/// 다양한 공격 방식을 구분하여 각각 다른 로직을 적용할 수 있습니다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AttackType {
    /// 기본 근접 공격
    MeleeBasic,
    /// 강한 근접 공격 (차지 어택)
    MeleeHeavy,
    /// 원거리 공격
    Ranged,
    /// 마법 공격
    Magic,
    /// 범위 공격 (AoE)
    AreaOfEffect,
    /// 특수 스킬
    Skill { skill_id: u32 },
}

/// 사망 원인 열거형
///
/// 플레이어가 사망한 원인을 분류하여 적절한 페널티와 UI를 적용할 수 있습니다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeathCause {
    /// 다른 플레이어에 의한 사망 (PvP)
    PlayerKill(PlayerId),
    /// NPC/몬스터에 의한 사망
    NpcKill(u32),
    /// 환경 데미지 (낙사, 독 등)
    Environmental,
    /// 자살 (명령어 등)
    Suicide,
    /// 시간 초과 (AFK 등)
    Timeout,
    /// 기타
    Other(String),
}

/// 드롭된 아이템 정보
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DroppedItem {
    /// 아이템 ID
    pub item_id: u32,
    /// 아이템 수량
    pub quantity: u32,
    /// 드롭 위치
    pub position: Position,
    /// 만료 시간
    pub expires_at: u64,
}

/// 사망 페널티 정보
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeathPenalty {
    /// 잃은 골드
    pub gold_lost: u32,
    /// 내구도 감소
    pub durability_loss: f32,
}

/// 서버 설정 정보 (Unity 클라이언트용)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    /// 틱 레이트 (TPS)
    pub tick_rate: u32,
    /// 최대 동시 접속자 수
    pub max_players: u32,
    /// PvP 활성화 여부
    pub pvp_enabled: bool,
    /// 골드 드롭 배율
    pub gold_multiplier: f32,
    /// 월드 경계 (Unity 좌표계)
    pub world_bounds: (f32, f32, f32),
}

/// 연결 해제 사유
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DisconnectReason {
    /// 정상 종료
    Normal,
    /// 타임아웃
    Timeout,
    /// 킥 (관리자)
    Kicked,
    /// 밴 (영구 차단)
    Banned,
    /// 네트워크 오류
    NetworkError,
    /// 클라이언트 오류
    ClientError,
}

/// 상태 값 열거형 (동적 타입)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
}

/// 에러 카테고리
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorCategory {
    /// 네트워크 오류
    Network,
    /// 인증 오류
    Authentication,
    /// 권한 오류
    Authorization,
    /// 게임 로직 오류
    GameLogic,
    /// 데이터베이스 오류
    Database,
    /// 시스템 오류
    System,
}

/// 알림 타입
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NoticeType {
    /// 서버 점검
    Maintenance,
    /// 업데이트
    Update,
    /// 이벤트
    Event,
    /// 긴급 알림
    Emergency,
    /// 일반 공지
    General,
}

/// 우선순위 레벨
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// 플레이어 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlayerStatus {
    /// 생존 상태
    Alive,
    /// 사망 상태
    Dead,
    /// 전투 상태
    InCombat,
    /// AFK 상태
    Away,
    /// 거래 중
    Trading,
}

/// 게임 메시지의 크기 추정
///
/// 네트워크 대역폭 최적화를 위해 메시지 크기를 추정합니다.
///
/// # Examples
/// ```
/// let msg = GameMessage::Move { ... };
/// let estimated_size = estimate_message_size(&msg);
/// ```
pub fn estimate_message_size(message: &GameMessage) -> usize {
    match message {
        GameMessage::Connect { .. } => 100, // ~100 bytes
        GameMessage::Move { .. } => 32,     // ~32 bytes
        GameMessage::Attack { .. } => 48,   // ~48 bytes
        GameMessage::Die { .. } => 80,      // ~80 bytes
        _ => 64,                            // 기본값
    }
}

/// 메시지 우선순위 결정
///
/// RUDP 전송시 우선순위를 결정하여 중요한 메시지를 먼저 처리합니다.
///
/// # Arguments
/// * `message` - 우선순위를 확인할 메시지
///
/// # Returns
/// 우선순위 (0: 최고, 255: 최저)
pub fn get_message_priority(message: &GameMessage) -> u8 {
    match message {
        GameMessage::Die { .. } => 0,    // 최고 우선순위
        GameMessage::Attack { .. } => 1, // 높은 우선순위
        GameMessage::ConnectResponse { .. } => 2,
        GameMessage::StateUpdate { .. } => 3,
        GameMessage::Move { .. } => 100, // 낮은 우선순위 (빈번함)
        GameMessage::MoveUpdate { .. } => 101,
        _ => 50, // 중간 우선순위
    }
}

/// 메시지 신뢰성 요구사항
///
/// RUDP에서 해당 메시지가 신뢰성 있는 전송을 필요로 하는지 결정합니다.
///
/// # Arguments
/// * `message` - 신뢰성을 확인할 메시지
///
/// # Returns
/// true: 신뢰성 필요, false: 비신뢰성 허용
pub fn requires_reliable_delivery(message: &GameMessage) -> bool {
    match message {
        GameMessage::Connect { .. }
        | GameMessage::ConnectResponse { .. }
        | GameMessage::Attack { .. }
        | GameMessage::AttackResult { .. }
        | GameMessage::Die { .. }
        | GameMessage::Respawn { .. }
        | GameMessage::RespawnComplete { .. }
        | GameMessage::Error { .. } => true,

        GameMessage::Move { .. } | GameMessage::MoveUpdate { .. } => false,

        _ => true, // 기본적으로 신뢰성 요구
    }
}
