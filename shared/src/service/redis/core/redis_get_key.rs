use std::format;
use anyhow::Result;

// redis 키타입 정의

//user => user{user_id}
//room_info => room_info{room_id}
//room_user_list => room_user_list{room_id}
//room_list_by_time => room_list_by_time{room_id}

#[derive(Clone)]
pub enum KeyType {
    User,
    RoomInfo,
    RoomUserList,
    RoomListByTime,
    Custom(String),
}

impl KeyType {
    pub fn get_key(&self, id: &u16) -> String {
        match self {
            KeyType::User => format!("user:{}", id),
            KeyType::RoomInfo => format!("room:info:{}", id),
            KeyType::RoomUserList => format!("room:users:{}", id),
            KeyType::RoomListByTime => format!("room:list:time:{}", id),
            KeyType::Custom(prefix) => format!("{}:{}", prefix, id),
        }
    }

    pub fn get_index_key(&self) -> String {
        match self {
            KeyType::User => "user:index".to_string(),
            KeyType::RoomInfo => "room:info:index".to_string(),
            KeyType::RoomUserList => "room:users:index".to_string(),
            KeyType::RoomListByTime => "room:list:time:index".to_string(),
            KeyType::Custom(prefix) => format!("{}:index", prefix),
        }
    }
}

pub fn get_key(key_type: &KeyType, id: &u16) -> String {
    key_type.get_key(id)
}

pub fn try_get_key(key_type: &KeyType, id: Option<&u16>) -> Result<String> {
    match id {
        Some(id) => Ok(key_type.get_key(id)),
        None => Err(anyhow::anyhow!("ID is required for key generation")),
    }
}

