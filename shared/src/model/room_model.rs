#[derive(Debug, Clone)]
pub struct RoomInfo {
    pub room_id: u16,
    pub room_name: String,
    pub max_player_num: u16,
    pub current_player_num: u16,
    pub create_at: String,
}
