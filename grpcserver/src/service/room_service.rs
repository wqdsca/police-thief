// src/service/room_service.rs
pub struct RoomService;

impl RoomService {
    pub fn new() -> Self { Self }

    pub async fn make_room(
        &self,
        _user_id: i32,
        _nick_name: String,
        _room_name: String,
        _max_player_num: i32,
    ) -> anyhow::Result<i32> {
        // TODO: 실제 DB/Redis 로직
        Ok(42)
    }

    pub async fn get_room_list(
        &self,
        _last_room_id: i32,
    ) -> anyhow::Result<Vec<crate::room::RoomInfo>> {
        // TODO: 실제 DB/Redis 조회
        Ok(vec![])
    }
}
