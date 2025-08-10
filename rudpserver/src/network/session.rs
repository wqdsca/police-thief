//! RUDP 세션 관리 시스템
//!
//! 2000명 동시접속을 지원하는 고성능 세션 관리
//! - 연결 상태 추적 및 관리
//! - 세션 타임아웃 및 정리
//! - 부하 분산 및 성능 최적화
//! - 실시간 모니터링 및 통계

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

use crate::game::player::{PlayerId, PlayerManager};
use crate::protocol::rudp::RudpConnection;

// Shared library imports
use shared::security::SecurityMiddleware;
use shared::tool::high_performance::{
    atomic_stats::AtomicStats, dashmap_optimizer::DashMapOptimizer, redis_optimizer::RedisOptimizer,
};

// 세션 관리 상수
const DEFAULT_AVERAGE_SESSION_DURATION_SECS: u64 = 300; // 5분
const DEFAULT_PACKET_LOSS_RATE: f64 = 0.01; // 1%
const DEFAULT_SESSION_MEMORY_SIZE_BYTES: f64 = 1024.0;

/// 세션 ID 타입
pub type SessionId = u64;

/// 세션 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// 연결 시도 중
    Connecting,
    /// 인증 대기 중
    Authenticating,
    /// 활성 상태
    Active,
    /// 유휴 상태
    Idle,
    /// 연결 해제 중
    Disconnecting,
    /// 연결 해제됨
    Disconnected,
    /// 타임아웃
    Timeout,
    /// 에러 상태
    Error,
}

/// 세션 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionMetadata {
    /// 세션 ID
    pub session_id: SessionId,
    /// 클라이언트 IP 주소
    pub remote_addr: SocketAddr,
    /// 세션 상태
    pub state: SessionState,
    /// 연결된 플레이어 ID (인증 후)
    pub player_id: Option<PlayerId>,
    /// 클라이언트 정보
    pub client_info: ClientInfo,
    /// 세션 생성 시간
    #[serde(skip)]
    pub created_at: Instant,
    /// 마지막 활동 시간
    #[serde(skip)]
    pub last_activity: Instant,
    /// 연결 지속 시간 (초)
    pub uptime_seconds: u64,
    /// 인증 토큰
    pub auth_token: Option<String>,
    /// 세션 우선순위 (VIP 등)
    pub priority: SessionPriority,
    /// 세션 태그 (디버깅/분석용)
    pub tags: Vec<String>,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            session_id: 0,
            remote_addr: "127.0.0.1:0".parse().unwrap(),
            state: SessionState::Connecting,
            player_id: None,
            client_info: ClientInfo::default(),
            created_at: now,
            last_activity: now,
            uptime_seconds: 0,
            auth_token: None,
            priority: SessionPriority::Normal,
            tags: Vec::new(),
        }
    }
}

impl SessionMetadata {
    pub fn new(session_id: SessionId, remote_addr: SocketAddr, client_info: ClientInfo) -> Self {
        let now = Instant::now();

        Self {
            session_id,
            remote_addr,
            state: SessionState::Connecting,
            player_id: None,
            client_info,
            created_at: now,
            last_activity: now,
            uptime_seconds: 0,
            auth_token: None,
            priority: SessionPriority::Normal,
            tags: Vec::new(),
        }
    }

    /// 세션 활성화 업데이트
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
        self.uptime_seconds = self.created_at.elapsed().as_secs();
    }

    /// 세션 타임아웃 확인
    pub fn is_timeout(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// 세션 활성 여부
    pub fn is_active(&self) -> bool {
        matches!(self.state, SessionState::Active | SessionState::Idle)
    }
}

/// 클라이언트 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// 클라이언트 버전
    pub version: String,
    /// 플랫폼 (Windows, Mac, Linux 등)
    pub platform: String,
    /// 사용자 에이전트
    pub user_agent: String,
    /// 언어 설정
    pub language: String,
    /// 화면 해상도
    pub screen_resolution: Option<(u32, u32)>,
    /// 연결 품질 정보
    pub connection_quality: ConnectionQuality,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            platform: "Unknown".to_string(),
            user_agent: "GameClient/1.0".to_string(),
            language: "en".to_string(),
            screen_resolution: None,
            connection_quality: ConnectionQuality::Unknown,
        }
    }
}

/// 연결 품질
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionQuality {
    /// 매우 좋음 (<30ms RTT)
    Excellent,
    /// 좋음 (30-60ms RTT)
    Good,
    /// 보통 (60-120ms RTT)
    Fair,
    /// 나쁨 (120-300ms RTT)
    Poor,
    /// 매우 나쁨 (>300ms RTT)
    VeryPoor,
    /// 알 수 없음
    Unknown,
}

impl ConnectionQuality {
    pub fn from_rtt(rtt: Duration) -> Self {
        let rtt_ms = rtt.as_millis() as u32;
        match rtt_ms {
            0..=30 => Self::Excellent,
            31..=60 => Self::Good,
            61..=120 => Self::Fair,
            121..=300 => Self::Poor,
            _ => Self::VeryPoor,
        }
    }

    pub fn to_multiplier(&self) -> f32 {
        match self {
            Self::Excellent => 1.0,
            Self::Good => 0.9,
            Self::Fair => 0.8,
            Self::Poor => 0.7,
            Self::VeryPoor => 0.6,
            Self::Unknown => 0.8,
        }
    }
}

/// 세션 우선순위
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SessionPriority {
    /// 낮음
    Low = 0,
    /// 보통
    Normal = 1,
    /// 높음
    High = 2,
    /// VIP
    Vip = 3,
    /// 관리자
    Admin = 4,
}

/// 세션 이벤트
#[derive(Debug, Clone, Serialize)]
pub enum SessionEvent {
    /// 세션 생성
    Created {
        session_id: SessionId,
        remote_addr: SocketAddr,
        client_info: ClientInfo,
    },
    /// 세션 인증 완료
    Authenticated {
        session_id: SessionId,
        player_id: PlayerId,
        auth_method: String,
    },
    /// 세션 상태 변경
    StateChanged {
        session_id: SessionId,
        old_state: SessionState,
        new_state: SessionState,
    },
    /// 세션 타임아웃
    Timeout {
        session_id: SessionId,
        idle_duration: Duration,
    },
    /// 세션 종료
    Terminated {
        session_id: SessionId,
        reason: SessionTerminationReason,
        uptime: Duration,
    },
    /// 연결 품질 변경
    QualityChanged {
        session_id: SessionId,
        old_quality: ConnectionQuality,
        new_quality: ConnectionQuality,
        rtt: Duration,
    },
}

/// 세션 종료 이유
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionTerminationReason {
    /// 클라이언트 요청
    ClientRequest,
    /// 서버 종료
    ServerShutdown,
    /// 타임아웃
    Timeout,
    /// 네트워크 에러
    NetworkError(String),
    /// 인증 실패
    AuthenticationFailed,
    /// 중복 로그인
    DuplicateLogin,
    /// 서버 과부하
    ServerOverload,
    /// 관리자 킥
    AdminKick(String),
    /// 기타
    Other(String),
}

/// 세션 통계
#[derive(Debug, Clone, Serialize)]
pub struct SessionStats {
    /// 총 세션 수
    pub total_sessions: u64,
    /// 현재 활성 세션 수
    pub active_sessions: u32,
    /// 대기 중 세션 수
    pub pending_sessions: u32,
    /// 평균 세션 지속 시간
    pub avg_session_duration: Duration,
    /// 최대 동시 세션 수
    pub peak_concurrent_sessions: u32,
    /// 세션 생성률 (초당)
    pub session_creation_rate: f64,
    /// 세션 종료률 (초당)
    pub session_termination_rate: f64,
    /// 평균 RTT
    pub avg_rtt: Duration,
    /// 패킷 손실률
    pub packet_loss_rate: f64,
    /// 메모리 사용량
    pub memory_usage_mb: f64,
}

/// 세션 풀 (메모리 최적화)
#[derive(Debug)]
pub struct SessionPool {
    /// 사용 가능한 세션 메타데이터 풀
    available: VecDeque<SessionMetadata>,
    /// 풀 크기 제한
    max_size: usize,
    /// 생성된 총 객체 수
    total_created: u64,
    /// 재사용된 객체 수
    total_reused: u64,
}

impl SessionPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            available: VecDeque::with_capacity(max_size),
            max_size,
            total_created: 0,
            total_reused: 0,
        }
    }

    /// 세션 메타데이터 가져오기 (재사용 우선)
    pub fn acquire(
        &mut self,
        session_id: SessionId,
        remote_addr: SocketAddr,
        client_info: ClientInfo,
    ) -> SessionMetadata {
        if let Some(mut metadata) = self.available.pop_front() {
            // 기존 객체 재사용
            metadata.session_id = session_id;
            metadata.remote_addr = remote_addr;
            metadata.state = SessionState::Connecting;
            metadata.player_id = None;
            metadata.client_info = client_info;
            metadata.created_at = Instant::now();
            metadata.last_activity = Instant::now();
            metadata.uptime_seconds = 0;
            metadata.auth_token = None;
            metadata.priority = SessionPriority::Normal;
            metadata.tags.clear();

            self.total_reused += 1;
            metadata
        } else {
            // 새 객체 생성
            self.total_created += 1;
            SessionMetadata::new(session_id, remote_addr, client_info)
        }
    }

    /// 세션 메타데이터 반환 (풀에 저장)
    pub fn release(&mut self, metadata: SessionMetadata) {
        if self.available.len() < self.max_size {
            self.available.push_back(metadata);
        }
    }

    /// 풀 효율성 (재사용률)
    pub fn efficiency(&self) -> f64 {
        if self.total_created + self.total_reused == 0 {
            0.0
        } else {
            self.total_reused as f64 / (self.total_created + self.total_reused) as f64
        }
    }
}

/// 세션 관리자 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManagerConfig {
    /// 최대 동시 세션 수
    pub max_sessions: usize,
    /// 세션 타임아웃 (초)
    pub session_timeout_secs: u64,
    /// 유휴 세션 타임아웃 (초)
    pub idle_timeout_secs: u64,
    /// 인증 타임아웃 (초)
    pub auth_timeout_secs: u64,
    /// 세션 정리 간격 (초)
    pub cleanup_interval_secs: u64,
    /// 통계 업데이트 간격 (초)
    pub stats_interval_secs: u64,
    /// 세션 풀 크기
    pub session_pool_size: usize,
    /// 연결 품질 모니터링 간격 (초)
    pub quality_check_interval_secs: u64,
    /// 부하 제한 활성화
    pub enable_load_limiting: bool,
    /// VIP 세션 우선순위
    pub vip_priority_enabled: bool,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            max_sessions: 2000,
            session_timeout_secs: 300,       // 5분
            idle_timeout_secs: 600,          // 10분
            auth_timeout_secs: 30,           // 30초
            cleanup_interval_secs: 60,       // 1분
            stats_interval_secs: 30,         // 30초
            session_pool_size: 500,          // 500개 재사용
            quality_check_interval_secs: 10, // 10초
            enable_load_limiting: true,
            vip_priority_enabled: true,
        }
    }
}

/// 세션 관리자
pub struct SessionManager {
    /// 설정
    config: SessionManagerConfig,
    /// 모든 세션들 (SessionID -> SessionMetadata)
    sessions: Arc<DashMap<SessionId, Arc<Mutex<SessionMetadata>>>>,
    /// RUDP 연결들 (SessionID -> RudpConnection)  
    connections: Arc<RwLock<HashMap<SessionId, Arc<Mutex<RudpConnection>>>>>,
    /// 주소별 세션 매핑 (IP -> SessionID)
    addr_to_session: Arc<RwLock<HashMap<SocketAddr, SessionId>>>,
    /// 플레이어별 세션 매핑 (PlayerID -> SessionID)
    player_to_session: Arc<RwLock<HashMap<PlayerId, SessionId>>>,
    /// 세션 풀 (메모리 최적화)
    session_pool: Arc<Mutex<SessionPool>>,
    /// 보안 미들웨어
    security: Arc<SecurityMiddleware>,
    /// Redis 최적화기
    redis: Arc<RedisOptimizer>,
    /// 플레이어 관리자
    player_manager: Arc<PlayerManager>,
    /// 세션 통계
    stats: Arc<AtomicStats>,
    /// 이벤트 리스너들
    event_listeners: Arc<RwLock<Vec<Arc<dyn SessionEventListener>>>>,
    /// 실행 중 플래그
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl SessionManager {
    /// 새로운 세션 관리자 생성
    pub async fn new(
        config: SessionManagerConfig,
        security: Arc<SecurityMiddleware>,
        redis: Arc<RedisOptimizer>,
        player_manager: Arc<PlayerManager>,
    ) -> Result<Self> {
        let sessions_map = Arc::new(DashMap::new());

        let session_pool = Arc::new(Mutex::new(SessionPool::new(config.session_pool_size)));
        let stats = Arc::new(AtomicStats::new());

        info!(
            max_sessions = %config.max_sessions,
            session_timeout = %config.session_timeout_secs,
            "Session manager created"
        );

        Ok(Self {
            config,
            sessions: sessions_map,
            connections: Arc::new(RwLock::new(HashMap::new())),
            addr_to_session: Arc::new(RwLock::new(HashMap::new())),
            player_to_session: Arc::new(RwLock::new(HashMap::new())),
            session_pool,
            security,
            redis,
            player_manager,
            stats,
            event_listeners: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// 세션 관리자 시작
    pub async fn start(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        info!("Starting session manager...");

        // 세션 정리 태스크
        let cleanup_task = self.start_cleanup_loop();

        // 통계 업데이트 태스크
        let stats_task = self.start_stats_loop();

        // 연결 품질 모니터링 태스크
        let quality_task = self.start_quality_monitoring_loop();

        // 모든 태스크 실행
        tokio::select! {
            result = cleanup_task => {
                error!("Cleanup loop ended: {:?}", result);
                result
            }
            result = stats_task => {
                error!("Stats loop ended: {:?}", result);
                result
            }
            result = quality_task => {
                error!("Quality monitoring loop ended: {:?}", result);
                result
            }
        }
    }

    /// 새로운 세션 생성
    pub async fn create_session(
        &self,
        session_id: SessionId,
        remote_addr: SocketAddr,
        connection: Arc<Mutex<RudpConnection>>,
        client_info: ClientInfo,
    ) -> Result<()> {
        // 최대 세션 수 확인
        if self.get_active_session_count().await >= self.config.max_sessions {
            return Err(anyhow!("Maximum session limit reached"));
        }

        // 중복 세션 확인
        if self.sessions.contains_key(&session_id) {
            return Err(anyhow!("Session already exists"));
        }

        // IP별 중복 연결 확인
        {
            let addr_map = self.addr_to_session.read().await;
            if addr_map.contains_key(&remote_addr) {
                warn!(
                    addr = %remote_addr,
                    "Duplicate connection from same IP"
                );
                // 기존 연결을 종료하고 새 연결을 허용할지 결정
                // 여기서는 새 연결을 허용
            }
        }

        // 세션 메타데이터 생성
        let metadata = {
            let mut pool = self.session_pool.lock().await;
            pool.acquire(session_id, remote_addr, client_info.clone())
        };

        // 세션 저장
        self.sessions
            .insert(session_id, Arc::new(Mutex::new(metadata)));

        // 연결 정보 저장
        {
            let mut connections = self.connections.write().await;
            connections.insert(session_id, connection);
        }

        // 주소 매핑 저장
        {
            let mut addr_map = self.addr_to_session.write().await;
            addr_map.insert(remote_addr, session_id);
        }

        // 통계 업데이트
        self.stats
            .total_connections
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats
            .active_connections
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 이벤트 발생
        let event = SessionEvent::Created {
            session_id,
            remote_addr,
            client_info,
        };
        self.emit_event(event).await;

        info!(
            session_id = %session_id,
            remote_addr = %remote_addr,
            "Session created"
        );

        Ok(())
    }

    /// 세션 인증
    pub async fn authenticate_session(
        &self,
        session_id: SessionId,
        player_id: PlayerId,
        auth_token: String,
        auth_method: String,
    ) -> Result<()> {
        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| anyhow!("Session not found"))?;

        let mut session_lock = session.lock().await;

        // 인증 상태 확인
        if session_lock.state != SessionState::Connecting
            && session_lock.state != SessionState::Authenticating
        {
            return Err(anyhow!("Session not in authenticating state"));
        }

        // 중복 플레이어 로그인 확인
        {
            let player_map = self.player_to_session.read().await;
            if let Some(existing_session_id) = player_map.get(&player_id) {
                if *existing_session_id != session_id {
                    // 기존 세션 종료
                    warn!(
                        player_id = %player_id,
                        existing_session = %existing_session_id,
                        new_session = %session_id,
                        "Duplicate login detected, terminating existing session"
                    );

                    drop(session_lock); // 락 해제
                    self.terminate_session(
                        *existing_session_id,
                        SessionTerminationReason::DuplicateLogin,
                    )
                    .await?;
                    session_lock = session.lock().await;
                }
            }
        }

        // 세션 업데이트
        session_lock.player_id = Some(player_id);
        session_lock.auth_token = Some(auth_token);
        session_lock.state = SessionState::Active;
        session_lock.update_activity();

        // 플레이어 매핑 저장
        {
            let mut player_map = self.player_to_session.write().await;
            player_map.insert(player_id, session_id);
        }

        // 통계 업데이트
        // 인증 완료 통계 (별도 필드 없으므로 주석처리)
        // self.stats.sessions_authenticated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 이벤트 발생
        let event = SessionEvent::Authenticated {
            session_id,
            player_id,
            auth_method,
        };
        self.emit_event(event).await;

        info!(
            session_id = %session_id,
            player_id = %player_id,
            "Session authenticated"
        );

        Ok(())
    }

    /// 세션 상태 변경
    pub async fn change_session_state(
        &self,
        session_id: SessionId,
        new_state: SessionState,
    ) -> Result<()> {
        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| anyhow!("Session not found"))?;

        let old_state = {
            let mut session_lock = session.lock().await;
            let old_state = session_lock.state;
            session_lock.state = new_state;
            session_lock.update_activity();
            old_state
        };

        // 상태 변경 이벤트 발생
        if old_state != new_state {
            let event = SessionEvent::StateChanged {
                session_id,
                old_state,
                new_state,
            };
            self.emit_event(event).await;

            debug!(
                session_id = %session_id,
                old_state = ?old_state,
                new_state = ?new_state,
                "Session state changed"
            );
        }

        Ok(())
    }

    /// 세션 종료
    pub async fn terminate_session(
        &self,
        session_id: SessionId,
        reason: SessionTerminationReason,
    ) -> Result<()> {
        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| anyhow!("Session not found"))?;

        let (remote_addr, player_id, uptime) = {
            let session_lock = session.lock().await;
            let uptime_duration = session_lock.created_at.elapsed();
            (
                session_lock.remote_addr,
                session_lock.player_id,
                uptime_duration,
            )
        };

        // 연결 정보 제거
        {
            let mut connections = self.connections.write().await;
            connections.remove(&session_id);
        }

        // 주소 매핑 제거
        {
            let mut addr_map = self.addr_to_session.write().await;
            addr_map.remove(&remote_addr);
        }

        // 플레이어 매핑 제거
        if let Some(player_id) = player_id {
            let mut player_map = self.player_to_session.write().await;
            player_map.remove(&player_id);

            // 플레이어 오프라인 상태로 변경
            if let Some(player) = self.player_manager.get_player(player_id) {
                // Player가 더 이상 Mutex로 래핑되지 않으므로 직접 업데이트는 불가
                // PlayerManager를 통해 업데이트해야 함
                // TODO: PlayerManager에 update_online_status 메서드 추가 필요
            }
        }

        // 세션 메타데이터를 풀에 반환
        if let Some((_, session_arc)) = self.sessions.remove(&session_id) {
            let metadata = Arc::try_unwrap(session_arc)
                .map_err(|_| anyhow!("Failed to unwrap session metadata"))?
                .into_inner();

            let mut pool = self.session_pool.lock().await;
            pool.release(metadata);
        }

        // 통계 업데이트
        self.stats
            .active_connections
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        // sessions_terminated 필드가 없으므로 주석처리

        // 이벤트 발생
        let event = SessionEvent::Terminated {
            session_id,
            reason: reason.clone(),
            uptime,
        };
        self.emit_event(event).await;

        info!(
            session_id = %session_id,
            reason = ?reason,
            uptime_secs = %uptime.as_secs(),
            "Session terminated"
        );

        Ok(())
    }

    /// 세션 가져오기
    pub async fn get_session(&self, session_id: SessionId) -> Option<Arc<Mutex<SessionMetadata>>> {
        self.sessions
            .get(&session_id)
            .map(|entry| entry.value().clone())
    }

    /// 플레이어 ID로 세션 가져오기
    pub async fn get_session_by_player(&self, player_id: PlayerId) -> Option<SessionId> {
        let player_map = self.player_to_session.read().await;
        player_map.get(&player_id).copied()
    }

    /// 주소로 세션 가져오기
    pub async fn get_session_by_addr(&self, addr: SocketAddr) -> Option<SessionId> {
        let addr_map = self.addr_to_session.read().await;
        addr_map.get(&addr).copied()
    }

    /// RUDP 연결 가져오기
    pub async fn get_connection(
        &self,
        session_id: SessionId,
    ) -> Option<Arc<Mutex<RudpConnection>>> {
        let connections = self.connections.read().await;
        connections.get(&session_id).cloned()
    }

    /// 활성 세션 수
    pub async fn get_active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 세션 활동 업데이트
    pub async fn update_session_activity(&self, session_id: SessionId) -> Result<()> {
        if let Some(session) = self.get_session(session_id).await {
            let mut session_lock = session.lock().await;
            session_lock.update_activity();

            // 유휴 상태에서 활성 상태로 변경
            if session_lock.state == SessionState::Idle {
                session_lock.state = SessionState::Active;
            }
        }

        Ok(())
    }

    /// 연결 품질 업데이트
    pub async fn update_connection_quality(
        &self,
        session_id: SessionId,
        rtt: Duration,
    ) -> Result<()> {
        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| anyhow!("Session not found"))?;

        let (old_quality, new_quality) = {
            let mut session_lock = session.lock().await;
            let old_quality = session_lock.client_info.connection_quality;
            let new_quality = ConnectionQuality::from_rtt(rtt);
            session_lock.client_info.connection_quality = new_quality;
            (old_quality, new_quality)
        };

        // 품질 변경 이벤트 발생
        if old_quality != new_quality {
            let event = SessionEvent::QualityChanged {
                session_id,
                old_quality,
                new_quality,
                rtt,
            };
            self.emit_event(event).await;

            debug!(
                session_id = %session_id,
                old_quality = ?old_quality,
                new_quality = ?new_quality,
                rtt_ms = %rtt.as_millis(),
                "Connection quality updated"
            );
        }

        Ok(())
    }

    /// 비활성 세션 정리 (public 메서드 추가)
    pub async fn cleanup_inactive_sessions(&self) -> usize {
        let session_timeout = Duration::from_secs(self.config.session_timeout_secs);
        let idle_timeout = Duration::from_secs(self.config.idle_timeout_secs);
        let auth_timeout = Duration::from_secs(self.config.auth_timeout_secs);

        let mut expired_sessions = Vec::new();
        let mut idle_sessions = Vec::new();

        // 만료된 세션 찾기
        for entry in self.sessions.iter() {
            let session_id = *entry.key();
            let session = entry.value();
            let session_lock = session.lock().await;

            match session_lock.state {
                SessionState::Connecting | SessionState::Authenticating => {
                    if session_lock.is_timeout(auth_timeout) {
                        expired_sessions.push((session_id, SessionTerminationReason::Timeout));
                    }
                }
                SessionState::Active => {
                    if session_lock.is_timeout(idle_timeout) {
                        idle_sessions.push(session_id);
                    }
                }
                SessionState::Idle => {
                    if session_lock.is_timeout(session_timeout) {
                        expired_sessions.push((session_id, SessionTerminationReason::Timeout));
                    }
                }
                SessionState::Error => {
                    expired_sessions.push((
                        session_id,
                        SessionTerminationReason::Other("Error state".to_string()),
                    ));
                }
                _ => {}
            }
        }

        // 유휴 세션을 Idle 상태로 변경
        for session_id in idle_sessions {
            let _ = self
                .change_session_state(session_id, SessionState::Idle)
                .await;
        }

        let cleaned_count = expired_sessions.len();

        // 만료된 세션 정리
        for (session_id, reason) in expired_sessions {
            let _ = self.terminate_session(session_id, reason).await;
        }

        cleaned_count
    }

    /// 세션 정리 루프
    async fn start_cleanup_loop(&self) -> Result<()> {
        let cleanup_interval = Duration::from_secs(self.config.cleanup_interval_secs);
        let session_timeout = Duration::from_secs(self.config.session_timeout_secs);
        let idle_timeout = Duration::from_secs(self.config.idle_timeout_secs);
        let auth_timeout = Duration::from_secs(self.config.auth_timeout_secs);

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(cleanup_interval).await;

            let mut expired_sessions = Vec::new();
            let mut idle_sessions = Vec::new();

            // 만료된 세션 찾기
            for entry in self.sessions.iter() {
                let session_id = *entry.key();
                let session = entry.value();
                let session_lock = session.lock().await;

                match session_lock.state {
                    SessionState::Connecting | SessionState::Authenticating => {
                        if session_lock.is_timeout(auth_timeout) {
                            expired_sessions.push((session_id, SessionTerminationReason::Timeout));
                        }
                    }
                    SessionState::Active => {
                        if session_lock.is_timeout(idle_timeout) {
                            idle_sessions.push(session_id);
                        }
                    }
                    SessionState::Idle => {
                        if session_lock.is_timeout(session_timeout) {
                            expired_sessions.push((session_id, SessionTerminationReason::Timeout));
                        }
                    }
                    SessionState::Error => {
                        expired_sessions.push((
                            session_id,
                            SessionTerminationReason::Other("Error state".to_string()),
                        ));
                    }
                    _ => {}
                }
            }

            // 유휴 세션을 Idle 상태로 변경
            for session_id in idle_sessions {
                let _ = self
                    .change_session_state(session_id, SessionState::Idle)
                    .await;
            }

            // 만료된 세션 정리
            for (session_id, reason) in expired_sessions {
                let _ = self.terminate_session(session_id, reason).await;
            }

            // 통계 업데이트
            let active_count = self.get_active_session_count().await;
            self.stats
                .active_connections
                .store(active_count as u64, std::sync::atomic::Ordering::Relaxed);

            trace!("Session cleanup completed");
        }

        Ok(())
    }

    /// 통계 업데이트 루프
    async fn start_stats_loop(&self) -> Result<()> {
        let stats_interval = Duration::from_secs(self.config.stats_interval_secs);

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(stats_interval).await;

            // 세션 풀 효율성 업데이트
            {
                let pool = self.session_pool.lock().await;
                let efficiency = pool.efficiency();
                // 세션 풀 효율성을 별도로 저장할 필요 없으면 주석처리
                // self.stats에는 미리 정의된 필드만 있음
            }

            // 메모리 사용량 추정 (대략적)
            let _memory_usage = self.estimate_memory_usage().await;
            // 메모리 사용량은 별도의 필드가 없으므로 주석처리

            trace!("Session statistics updated");
        }

        Ok(())
    }

    /// 연결 품질 모니터링 루프
    async fn start_quality_monitoring_loop(&self) -> Result<()> {
        let quality_interval = Duration::from_secs(self.config.quality_check_interval_secs);

        while self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(quality_interval).await;

            let connections = self.connections.read().await.clone();

            for (session_id, connection) in connections {
                let conn_lock = connection.lock().await;
                let rtt = conn_lock.rtt;
                drop(conn_lock);

                let _ = self.update_connection_quality(session_id, rtt).await;
            }

            trace!("Connection quality monitoring completed");
        }

        Ok(())
    }

    /// 메모리 사용량 추정
    async fn estimate_memory_usage(&self) -> f64 {
        let session_count = self.get_active_session_count().await;
        (session_count as f64 * DEFAULT_SESSION_MEMORY_SIZE_BYTES) / 1024.0 // KB 단위
    }

    /// 이벤트 리스너 추가
    pub async fn add_event_listener(&self, listener: Arc<dyn SessionEventListener>) {
        let mut listeners = self.event_listeners.write().await;
        listeners.push(listener);
    }

    /// 이벤트 발생
    async fn emit_event(&self, event: SessionEvent) {
        let listeners = self.event_listeners.read().await;
        for listener in listeners.iter() {
            listener.on_session_event(&event).await;
        }
    }

    /// 세션 통계 가져오기
    pub async fn get_stats(&self) -> SessionStats {
        let active_sessions = self.get_active_session_count().await as u32;

        // 연결들의 평균 RTT 계산
        let connections = self.connections.read().await;
        let mut total_rtt = Duration::ZERO;
        let mut rtt_count = 0;

        for connection in connections.values() {
            let conn_lock = connection.lock().await;
            total_rtt += conn_lock.rtt;
            rtt_count += 1;
        }

        let avg_rtt = if rtt_count > 0 {
            total_rtt / rtt_count as u32
        } else {
            Duration::ZERO
        };

        SessionStats {
            total_sessions: self
                .stats
                .total_connections
                .load(std::sync::atomic::Ordering::Relaxed),
            active_sessions,
            pending_sessions: 0, // TODO: 계산 로직 추가
            avg_session_duration: Duration::from_secs(DEFAULT_AVERAGE_SESSION_DURATION_SECS),
            peak_concurrent_sessions: active_sessions, // TODO: 최대값 추적
            session_creation_rate: 0.0,                // TODO: 계산 로직 추가
            session_termination_rate: 0.0,             // TODO: 계산 로직 추가
            avg_rtt,
            packet_loss_rate: DEFAULT_PACKET_LOSS_RATE,
            memory_usage_mb: self.estimate_memory_usage().await / 1024.0,
        }
    }

    /// 세션 관리자 종료
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down session manager...");

        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // 모든 세션 종료
        let session_ids: Vec<SessionId> = self.sessions.iter().map(|entry| *entry.key()).collect();

        for session_id in session_ids {
            let _ = self
                .terminate_session(session_id, SessionTerminationReason::ServerShutdown)
                .await;
        }

        info!("Session manager shutdown complete");
        Ok(())
    }
}

/// 세션 이벤트 리스너 트레이트
#[async_trait::async_trait]
pub trait SessionEventListener: Send + Sync {
    async fn on_session_event(&self, event: &SessionEvent);
}

/// 기본 세션 이벤트 리스너 (로깅용)
pub struct DefaultSessionEventListener;

#[async_trait::async_trait]
impl SessionEventListener for DefaultSessionEventListener {
    async fn on_session_event(&self, event: &SessionEvent) {
        match event {
            SessionEvent::Created {
                session_id,
                remote_addr,
                ..
            } => {
                info!(
                    session_id = %session_id,
                    remote_addr = %remote_addr,
                    "Session created"
                );
            }
            SessionEvent::Authenticated {
                session_id,
                player_id,
                ..
            } => {
                info!(
                    session_id = %session_id,
                    player_id = %player_id,
                    "Session authenticated"
                );
            }
            SessionEvent::Terminated {
                session_id,
                reason,
                uptime,
            } => {
                info!(
                    session_id = %session_id,
                    reason = ?reason,
                    uptime_secs = %uptime.as_secs(),
                    "Session terminated"
                );
            }
            _ => {
                debug!("Session event: {:?}", event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_quality() {
        assert_eq!(
            ConnectionQuality::from_rtt(Duration::from_millis(25)),
            ConnectionQuality::Excellent
        );
        assert_eq!(
            ConnectionQuality::from_rtt(Duration::from_millis(45)),
            ConnectionQuality::Good
        );
        assert_eq!(
            ConnectionQuality::from_rtt(Duration::from_millis(90)),
            ConnectionQuality::Fair
        );
        assert_eq!(
            ConnectionQuality::from_rtt(Duration::from_millis(200)),
            ConnectionQuality::Poor
        );
        assert_eq!(
            ConnectionQuality::from_rtt(Duration::from_millis(400)),
            ConnectionQuality::VeryPoor
        );
    }

    #[test]
    fn test_session_priority() {
        assert!(SessionPriority::Admin > SessionPriority::Vip);
        assert!(SessionPriority::Vip > SessionPriority::High);
        assert!(SessionPriority::High > SessionPriority::Normal);
        assert!(SessionPriority::Normal > SessionPriority::Low);
    }

    #[tokio::test]
    async fn test_session_metadata() {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let client_info = ClientInfo::default();
        let mut metadata = SessionMetadata::new(12345, addr, client_info);

        assert_eq!(metadata.session_id, 12345);
        assert_eq!(metadata.remote_addr, addr);
        assert_eq!(metadata.state, SessionState::Connecting);

        metadata.update_activity();
        assert!(metadata.uptime_seconds > 0);
        assert!(!metadata.is_timeout(Duration::from_secs(1)));

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(metadata.is_timeout(Duration::from_millis(50)));
    }

    #[tokio::test]
    async fn test_session_pool() {
        let mut pool = SessionPool::new(2);
        let addr = "127.0.0.1:8080".parse().unwrap();
        let client_info = ClientInfo::default();

        // 첫 번째 세션 획득 (새로 생성)
        let session1 = pool.acquire(1, addr, client_info.clone());
        assert_eq!(pool.total_created, 1);
        assert_eq!(pool.total_reused, 0);

        // 세션 반환
        pool.release(session1);

        // 두 번째 세션 획득 (재사용)
        let _session2 = pool.acquire(2, addr, client_info);
        assert_eq!(pool.total_created, 1);
        assert_eq!(pool.total_reused, 1);

        // 효율성 확인
        assert_eq!(pool.efficiency(), 0.5); // 1 reused / (1 created + 1 reused)
    }
}
