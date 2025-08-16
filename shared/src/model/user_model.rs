#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user_id: i32,
    pub nick_name: String,
    pub tcp_ip: String,
    pub tcp_port: i32,
    pub udp_ip: String,
    pub udp_port: i32,
    pub access_token: String,
}
