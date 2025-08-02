use std::format;

// redis 키타입 정의

//user => user{user_id}
//room_info => room_info{room_id}
//room_user_list => room_user_list{room_id}
//room_list_by_time => room_list_by_time{room_id}


pub enum KeyType {
    User,
    RoomInfo,
    RoomUserList,
    RoomListByTime,
}

impl KeyType {
    pub fn get_key(&self, id: &u16) -> String {
        match self {
            KeyType::User => format!("user:{}", id),
            KeyType::RoomInfo => format!("room_info:{}", id),
            KeyType::RoomUserList => format!("room_user_list:{}", id),
            KeyType::RoomListByTime => format!("room_list_by_time:{}", id),
        }
    }
}

