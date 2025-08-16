//! WebSocket 실시간 업데이트 핸들러

use actix::prelude::*;
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// WebSocket 연결당 하트비트 간격
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// 하트비트 타임아웃
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// WebSocket 메시지 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// 서버 상태 업데이트
    ServerStatus {
        cpu_usage: f32,
        memory_mb: f64,
        connected_clients: usize,
    },
    /// 유저 벤 알림
    UserBanned { user_id: String, reason: String },
    /// 유저 벤 해제 알림
    UserUnbanned { user_id: String },
    /// 이벤트 시작 알림
    EventStarted {
        event_id: String,
        event_name: String,
        reward_type: String,
        reward_amount: i32,
    },
    /// 이벤트 종료 알림
    EventEnded {
        event_id: String,
        event_name: String,
    },
    /// 하트비트
    Heartbeat,
}

/// WebSocket 연결 액터
pub struct WsConnection {
    /// 하트비트 타이머
    hb: Instant,
    /// 연결 ID
    id: String,
    /// 브로드캐스트 수신기
    broadcast_rx: Option<Arc<RwLock<tokio::sync::broadcast::Receiver<WsMessage>>>>,
}

impl WsConnection {
    pub fn new(id: String) -> Self {
        Self {
            hb: Instant::now(),
            id,
            broadcast_rx: None,
        }
    }

    /// 브로드캐스트 수신기 설정
    pub fn set_broadcast_receiver(
        &mut self,
        rx: Arc<RwLock<tokio::sync::broadcast::Receiver<WsMessage>>>,
    ) {
        self.broadcast_rx = Some(rx);
    }

    /// 하트비트 시작
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("WebSocket Client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }
            match serde_json::to_string(&WsMessage::Heartbeat) {
                Ok(msg) => ctx.text(msg),
                Err(e) => {
                    tracing::error!("Failed to serialize heartbeat message: {}", e);
                    ctx.stop();
                }
            }
        });
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        // 브로드캐스트 메시지 수신 시작
        if let Some(ref rx) = self.broadcast_rx {
            let rx_clone = rx.clone();
            let addr = ctx.address();

            ctx.spawn(
                async move {
                    loop {
                        let mut receiver = rx_clone.write().await;
                        match receiver.recv().await {
                            Ok(msg) => {
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    addr.do_send(BroadcastMessage(json));
                                }
                            }
                            Err(_) => {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }
                .into_actor(self),
            );
        }

        println!("WebSocket connection started: {}", self.id);
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        println!("WebSocket connection stopped: {}", self.id);
    }
}

/// 브로드캐스트 메시지
#[derive(Message)]
#[rtype(result = "()")]
struct BroadcastMessage(String);

impl Handler<BroadcastMessage> for WsConnection {
    type Result = ();

    fn handle(&mut self, msg: BroadcastMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket 메시지 핸들러
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // 클라이언트 메시지 처리
                if text.trim() == "ping" {
                    ctx.text("pong");
                } else {
                    println!("Received message from {}: {}", self.id, text);
                }
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

/// WebSocket 브로드캐스터
pub struct WsBroadcaster {
    tx: tokio::sync::broadcast::Sender<WsMessage>,
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(100);
        Self { tx }
    }

    /// 브로드캐스트 수신기 생성
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }

    /// 서버 상태 브로드캐스트
    pub async fn broadcast_server_status(
        &self,
        cpu_usage: f32,
        memory_mb: f64,
        connected_clients: usize,
    ) {
        let msg = WsMessage::ServerStatus {
            cpu_usage,
            memory_mb,
            connected_clients,
        };
        let _ = self.tx.send(msg);
    }

    /// 유저 벤 알림 브로드캐스트
    pub async fn broadcast_user_banned(&self, user_id: String, reason: String) {
        let msg = WsMessage::UserBanned { user_id, reason };
        let _ = self.tx.send(msg);
    }

    /// 유저 벤 해제 알림 브로드캐스트
    pub async fn broadcast_user_unbanned(&self, user_id: String) {
        let msg = WsMessage::UserUnbanned { user_id };
        let _ = self.tx.send(msg);
    }

    /// 이벤트 시작 알림 브로드캐스트
    pub async fn broadcast_event_started(
        &self,
        event_id: String,
        event_name: String,
        reward_type: String,
        reward_amount: i32,
    ) {
        let msg = WsMessage::EventStarted {
            event_id,
            event_name,
            reward_type,
            reward_amount,
        };
        let _ = self.tx.send(msg);
    }

    /// 이벤트 종료 알림 브로드캐스트
    pub async fn broadcast_event_ended(&self, event_id: String, event_name: String) {
        let msg = WsMessage::EventEnded {
            event_id,
            event_name,
        };
        let _ = self.tx.send(msg);
    }
}

/// WebSocket 연결 핸들러
pub async fn ws_handler(
    req: actix_web::HttpRequest,
    stream: actix_web::web::Payload,
    broadcaster: actix_web::web::Data<Arc<WsBroadcaster>>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let conn_id = uuid::Uuid::new_v4().to_string();
    let mut ws_conn = WsConnection::new(conn_id);

    // 브로드캐스트 수신기 설정
    let rx = broadcaster.subscribe();
    ws_conn.set_broadcast_receiver(Arc::new(RwLock::new(rx)));

    ws::start(ws_conn, &req, stream)
}
