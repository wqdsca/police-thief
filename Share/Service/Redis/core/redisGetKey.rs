//! redisGetKey.rs
//! Redis 키 네이밍을 타입 안전하게 관리한다.
//!
//! - 기존 API: `get_key(KeyType, Option<u32>) -> String` (하위 호환 유지)
//! - 권장 API: `try_get_key(...) -> AppResult<String>` (panic 방지)
//! - 보조: `item_namespace`, `list_namespace`, `item_key`, `list_key`

use std::fmt;

use crate::Share::Comman::error::{AppError, AppResult};

/// Redis 키 타입 정의
///
/// - `User`            => "user:{id}"
/// - `RoomInfo`        => "room:list:{id}"
/// - `RoomUserList`    => "room:user:{id}"
/// - `RoomListByTime`  => "room:list:time" (id 불필요)
/// - `Custom(String)`  => "{custom}" 또는 "{custom}:{id}"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyType {
    User,
    RoomInfo,
    RoomUserList,
    RoomListByTime,
    Custom(String), // 기타 용도
}

impl KeyType {
    /// 문자열에서 KeyType으로 변환 (간단 매핑)
    pub fn from_str(s: &str) -> Self {
        match s {
            "user" => KeyType::User,
            "roomInfo" => KeyType::RoomInfo,
            "roomUserList" => KeyType::RoomUserList,
            "roomListByTime" => KeyType::RoomListByTime,
            other => KeyType::Custom(other.to_string()),
        }
    }

    /// "아이템 키"의 네임스페이스(프리픽스)를 반환한다.
    /// 예) User -> "user", RoomInfo -> "room:list"
    /// 일부 타입(RoomListByTime)은 id를 사용하지 않음 -> None
    pub fn item_namespace(&self) -> Option<&'static str> {
        match self {
            KeyType::User => Some("user"),
            KeyType::RoomInfo => Some("room:list"),
            KeyType::RoomUserList => Some("room:user"),
            KeyType::RoomListByTime => None,
            KeyType::Custom(_) => None, // Custom은 용도에 따라 달라서 고정 불가
        }
    }

    /// "리스트 키"의 네임스페이스(프리픽스)를 반환한다.
    /// 예) RoomInfo -> "room:list", RoomUserList -> "room:user"
    pub fn list_namespace(&self) -> Option<&'static str> {
        match self {
            KeyType::User => None,
            KeyType::RoomInfo => Some("room:list"),
            KeyType::RoomUserList => Some("room:user"),
            KeyType::RoomListByTime => Some("room:list:time"), // 전체 키가 네임스페이스로 간주
            KeyType::Custom(_) => None, // Custom은 호출부에서 직접 관리 권장
        }
    }
}

impl fmt::Display for KeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyType::User => write!(f, "user"),
            KeyType::RoomInfo => write!(f, "roomInfo"),
            KeyType::RoomUserList => write!(f, "roomUserList"),
            KeyType::RoomListByTime => write!(f, "roomListByTime"),
            KeyType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// 권장: panic 없는 안전한 키 생성기
///
/// - id가 필요한 타입에 `None`을 주면 `Err`
/// - id가 필요 없는 타입에 `Some(_)`을 주면 `Err`
pub fn try_get_key(key_type: KeyType, id: Option<u32>) -> AppResult<String> {
    let s = match key_type {
        KeyType::User => match id {
            Some(i) => format!("user:{}", i),
            None => return Err(AppError::business("User key requires an id", Some("KEY_VALIDATION"))),
        },
        KeyType::RoomInfo => match id {
            Some(i) => format!("room:list:{}", i),
            None => return Err(AppError::business("RoomInfo key requires an id", Some("KEY_VALIDATION"))),
        },
        KeyType::RoomUserList => match id {
            Some(i) => format!("room:user:{}", i),
            None => return Err(AppError::business("RoomUserList key requires an id", Some("KEY_VALIDATION"))),
        },
        KeyType::RoomListByTime => match id {
            None => "room:list:time".to_string(),
            Some(_) => return Err(AppError::business("RoomListByTime does not take an id", Some("KEY_VALIDATION"))),
        },
        KeyType::Custom(prefix) => match id {
            Some(i) => format!("{}:{}", prefix, i),
            None => prefix,
        },
    };
    Ok(s)
}

/// 하위 호환: 기존 시그니처 유지 (id 누락 시 panic)
/// 새 코드에서는 `try_get_key` 사용 권장.
pub fn get_key(key_type: KeyType, id: Option<u32>) -> String {
    try_get_key(key_type, id).expect("invalid key_type/id combination")
}

/// 편의 함수: 아이템 키 (항상 id 필요)
pub fn item_key(key_type: KeyType, id: u32) -> AppResult<String> {
    try_get_key(key_type, Some(id))
}

/// 편의 함수: 리스트 키 (id 불필요)
/// 예) RoomInfo -> "room:list", RoomUserList -> "room:user", RoomListByTime -> "room:list:time"
pub fn list_key(key_type: KeyType) -> AppResult<String> {
    match key_type.list_namespace() {
        Some("room:list:time") => Ok("room:list:time".to_string()),
        Some(ns) => Ok(ns.to_string()),
        None => Err(AppError::business(format!("list_key not defined for {}", key_type), Some("KEY_VALIDATION"))),
    }
}
