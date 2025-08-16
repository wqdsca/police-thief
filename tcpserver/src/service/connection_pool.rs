//! 고급 연결 풀링 서비스
//!
//! TCP 연결을 효율적으로 관리하기 위한 고도화된 풀링 시스템입니다.
//! 연결 재사용, 상태 관리, 부하 분산, 자동 확장/축소 등을 제공합니다.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

/// 연결 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,     // 유휴 상태
    Active,   // 활성 상태
    Warming,  // 예열 중
    Draining, // 배수 중
    Failed,   // 실패
}

/// 연결 풀 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// 최소 연결 수
    pub min_connections: usize,
    /// 최대 연결 수
    pub max_connections: usize,
    /// 연결당 최대 요청 수
    pub max_requests_per_connection: usize,
    /// 연결 유휴 타임아웃 (초)
    pub idle_timeout_secs: u64,
    /// 연결 최대 수명 (초)
    pub max_lifetime_secs: u64,
    /// 연결 획득 타임아웃 (밀리초)
    pub acquire_timeout_ms: u64,
    /// 헬스체크 간격 (초)
    pub health_check_interval_secs: u64,
    /// 자동 확장 활성화
    pub enable_auto_scaling: bool,
    /// 확장 임계값 (사용률 %)
    pub scale_up_threshold: f64,
    /// 축소 임계값 (사용률 %)
    pub scale_down_threshold: f64,
    /// 연결 예열 활성화
    pub enable_connection_warming: bool,
    /// 연결 재시도 횟수
    pub max_retries: u32,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 100,
            max_requests_per_connection: 10000,
            idle_timeout_secs: 300,
            max_lifetime_secs: 3600,
            acquire_timeout_ms: 5000,
            health_check_interval_secs: 30,
            enable_auto_scaling: true,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.2,
            enable_connection_warming: true,
            max_retries: 3,
        }
    }
}

/// 풀링된 연결
pub struct PooledConnection {
    pub id: u64,
    pub stream: Arc<Mutex<TcpStream>>,
    pub addr: SocketAddr,
    pub state: Arc<RwLock<ConnectionState>>,
    pub created_at: Instant,
    pub last_used: Arc<RwLock<Instant>>,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub is_healthy: AtomicBool,
}

impl PooledConnection {
    pub async fn new(id: u64, addr: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;

        Ok(Self {
            id,
            stream: Arc::new(Mutex::new(stream)),
            addr,
            state: Arc::new(RwLock::new(ConnectionState::Idle)),
            created_at: Instant::now(),
            last_used: Arc::new(RwLock::new(Instant::now())),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            is_healthy: AtomicBool::new(true),
        })
    }

    /// 연결 사용
    pub async fn acquire(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state != ConnectionState::Idle {
            return Err(anyhow!("연결이 사용 가능하지 않음"));
        }
        *state = ConnectionState::Active;

        let mut last_used = self.last_used.write().await;
        *last_used = Instant::now();

        self.request_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 연결 해제
    pub async fn release(&self) {
        let mut state = self.state.write().await;
        *state = ConnectionState::Idle;
    }

    /// 수명 확인
    pub fn is_expired(&self, config: &ConnectionPoolConfig) -> bool {
        self.created_at.elapsed().as_secs() > config.max_lifetime_secs
            || self.request_count.load(Ordering::Relaxed)
                > config.max_requests_per_connection as u64
    }

    /// 유휴 시간 확인
    pub async fn is_idle_timeout(&self, config: &ConnectionPoolConfig) -> bool {
        let last_used = self.last_used.read().await;
        last_used.elapsed().as_secs() > config.idle_timeout_secs
    }

    /// 헬스체크
    pub async fn health_check(&self) -> bool {
        // 간단한 ping 테스트
        let mut stream = self.stream.lock().await;

        // TCP keep-alive 확인
        match timeout(Duration::from_secs(5), stream.write_all(b"")).await {
            Ok(Ok(_)) => {
                self.is_healthy.store(true, Ordering::Relaxed);
                true
            }
            _ => {
                self.is_healthy.store(false, Ordering::Relaxed);
                self.error_count.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// 예열
    pub async fn warm_up(&self) -> Result<()> {
        let mut state = self.state.write().await;
        *state = ConnectionState::Warming;

        // 더미 요청으로 연결 예열
        let mut stream = self.stream.lock().await;
        stream.write_all(b"PING\r\n").await?;

        let mut buf = [0u8; 1024];
        let _ = timeout(Duration::from_secs(1), stream.read(&mut buf)).await;

        *state = ConnectionState::Idle;
        Ok(())
    }
}

/// 연결 풀 통계
#[derive(Debug, Default)]
pub struct ConnectionPoolStats {
    pub total_connections: AtomicUsize,
    pub active_connections: AtomicUsize,
    pub idle_connections: AtomicUsize,
    pub failed_connections: AtomicU64,
    pub total_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub connections_created: AtomicU64,
    pub connections_destroyed: AtomicU64,
    pub wait_time_us: AtomicU64,
    pub scale_up_count: AtomicU64,
    pub scale_down_count: AtomicU64,
}

/// 부하 분산 전략
#[derive(Debug, Clone, Copy)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    LeastResponseTime,
    Random,
    WeightedRoundRobin,
}

/// 고급 연결 풀
pub struct AdvancedConnectionPool {
    config: ConnectionPoolConfig,
    connections: Arc<RwLock<HashMap<u64, Arc<PooledConnection>>>>,
    idle_queue: Arc<Mutex<VecDeque<u64>>>,
    stats: Arc<ConnectionPoolStats>,
    next_conn_id: AtomicU64,
    acquire_semaphore: Arc<Semaphore>,
    load_balancing: LoadBalancingStrategy,
    addresses: Vec<SocketAddr>,
    health_check_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    auto_scale_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    shutdown: Arc<AtomicBool>,
}

impl AdvancedConnectionPool {
    /// 새 연결 풀 생성
    pub async fn new(
        config: ConnectionPoolConfig,
        addresses: Vec<SocketAddr>,
        strategy: LoadBalancingStrategy,
    ) -> Result<Self> {
        if addresses.is_empty() {
            return Err(anyhow!("주소 목록이 비어있음"));
        }

        let pool = Self {
            config: config.clone(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            idle_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(ConnectionPoolStats::default()),
            next_conn_id: AtomicU64::new(1),
            acquire_semaphore: Arc::new(Semaphore::new(config.max_connections)),
            load_balancing: strategy,
            addresses,
            health_check_handle: Arc::new(Mutex::new(None)),
            auto_scale_handle: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
        };

        // 최소 연결 수 생성
        pool.warm_up_connections().await?;

        // 백그라운드 태스크 시작
        pool.start_background_tasks().await;

        Ok(pool)
    }

    /// 연결 획득
    pub async fn acquire(&self) -> Result<Arc<PooledConnection>> {
        let start = Instant::now();

        // 타임아웃 처리
        let result = timeout(
            Duration::from_millis(self.config.acquire_timeout_ms),
            self.acquire_internal(),
        )
        .await;

        let wait_time = start.elapsed();
        self.stats
            .wait_time_us
            .fetch_add(wait_time.as_micros() as u64, Ordering::Relaxed);

        match result {
            Ok(Ok(conn)) => {
                self.stats.total_requests.fetch_add(1, Ordering::Relaxed);
                Ok(conn)
            }
            Ok(Err(e)) => {
                self.stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
            Err(_) => {
                self.stats.failed_requests.fetch_add(1, Ordering::Relaxed);
                Err(anyhow!("연결 획득 타임아웃"))
            }
        }
    }

    /// 내부 연결 획득 로직
    async fn acquire_internal(&self) -> Result<Arc<PooledConnection>> {
        let _permit = self.acquire_semaphore.acquire().await?;

        // 재시도 로직
        for attempt in 0..=self.config.max_retries {
            // 유휴 연결 확인
            if let Some(conn) = self.get_idle_connection().await {
                if conn.is_healthy.load(Ordering::Relaxed) {
                    conn.acquire().await?;
                    self.update_stats_on_acquire().await;
                    return Ok(conn);
                }
            }

            // 새 연결 생성
            match self.create_connection().await {
                Ok(conn) => {
                    conn.acquire().await?;
                    self.update_stats_on_acquire().await;
                    return Ok(conn);
                }
                Err(e) if attempt < self.config.max_retries => {
                    warn!("연결 생성 실패 (시도 {}): {}", attempt + 1, e);
                    tokio::time::sleep(Duration::from_millis(100 * (attempt + 1) as u64)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(anyhow!("모든 재시도 실패"))
    }

    /// 유휴 연결 가져오기
    async fn get_idle_connection(&self) -> Option<Arc<PooledConnection>> {
        let mut idle_queue = self.idle_queue.lock().await;

        while let Some(conn_id) = idle_queue.pop_front() {
            let connections = self.connections.read().await;

            if let Some(conn) = connections.get(&conn_id) {
                // 만료/타임아웃 확인
                if !conn.is_expired(&self.config) && !conn.is_idle_timeout(&self.config).await {
                    return Some(conn.clone());
                }
            }
        }

        None
    }

    /// 연결 생성
    async fn create_connection(&self) -> Result<Arc<PooledConnection>> {
        let connections = self.connections.read().await;
        if connections.len() >= self.config.max_connections {
            return Err(anyhow!("최대 연결 수 도달"));
        }
        drop(connections);

        // 부하 분산 전략에 따라 주소 선택
        let addr = self.select_address().await;

        let conn_id = self.next_conn_id.fetch_add(1, Ordering::Relaxed);
        let conn = Arc::new(PooledConnection::new(conn_id, addr).await?);

        // 연결 예열
        if self.config.enable_connection_warming {
            conn.warm_up().await?;
        }

        let mut connections = self.connections.write().await;
        connections.insert(conn_id, conn.clone());

        self.stats
            .connections_created
            .fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_connections
            .store(connections.len(), Ordering::Relaxed);

        info!("새 연결 생성: ID={}, 주소={}", conn_id, addr);

        Ok(conn)
    }

    /// 주소 선택 (부하 분산)
    async fn select_address(&self) -> SocketAddr {
        match self.load_balancing {
            LoadBalancingStrategy::RoundRobin => {
                static COUNTER: AtomicUsize = AtomicUsize::new(0);
                let index = COUNTER.fetch_add(1, Ordering::Relaxed) % self.addresses.len();
                self.addresses[index]
            }
            LoadBalancingStrategy::Random => {
                use rand::Rng;
                let index = rand::thread_rng().gen_range(0..self.addresses.len());
                self.addresses[index]
            }
            _ => {
                // 기본: 첫 번째 주소
                self.addresses[0]
            }
        }
    }

    /// 연결 반환
    pub async fn release(&self, conn: Arc<PooledConnection>) {
        conn.release().await;

        let mut idle_queue = self.idle_queue.lock().await;
        idle_queue.push_back(conn.id);

        self.update_stats_on_release().await;
    }

    /// 연결 제거
    async fn remove_connection(&self, conn_id: u64) {
        let mut connections = self.connections.write().await;

        if connections.remove(&conn_id).is_some() {
            self.stats
                .connections_destroyed
                .fetch_add(1, Ordering::Relaxed);
            self.stats
                .total_connections
                .store(connections.len(), Ordering::Relaxed);

            debug!("연결 제거: ID={}", conn_id);
        }
    }

    /// 초기 연결 예열
    async fn warm_up_connections(&self) -> Result<()> {
        info!("연결 풀 예열 시작: {} 연결", self.config.min_connections);

        for _ in 0..self.config.min_connections {
            self.create_connection().await?;
        }

        info!("연결 풀 예열 완료");
        Ok(())
    }

    /// 백그라운드 태스크 시작
    async fn start_background_tasks(&self) {
        // 헬스체크 태스크
        let pool = self.clone_internals();
        let health_handle = tokio::spawn(async move {
            pool.health_check_loop().await;
        });
        *self.health_check_handle.lock().await = Some(health_handle);

        // 자동 스케일링 태스크
        if self.config.enable_auto_scaling {
            let pool = self.clone_internals();
            let scale_handle = tokio::spawn(async move {
                pool.auto_scale_loop().await;
            });
            *self.auto_scale_handle.lock().await = Some(scale_handle);
        }
    }

    /// 헬스체크 루프
    async fn health_check_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.health_check_interval_secs));

        while !self.shutdown.load(Ordering::Relaxed) {
            interval.tick().await;

            let connections = self.connections.read().await.clone();
            for (conn_id, conn) in connections {
                if !conn.health_check().await {
                    warn!("헬스체크 실패: 연결 ID={}", conn_id);
                    self.remove_connection(conn_id).await;
                }
            }
        }
    }

    /// 자동 스케일링 루프
    async fn auto_scale_loop(&self) {
        let mut interval = interval(Duration::from_secs(10));

        while !self.shutdown.load(Ordering::Relaxed) {
            interval.tick().await;

            let usage = self.calculate_usage().await;

            if usage > self.config.scale_up_threshold {
                self.scale_up().await;
            } else if usage < self.config.scale_down_threshold {
                self.scale_down().await;
            }
        }
    }

    /// 사용률 계산
    async fn calculate_usage(&self) -> f64 {
        let total = self.stats.total_connections.load(Ordering::Relaxed);
        let active = self.stats.active_connections.load(Ordering::Relaxed);

        if total > 0 {
            active as f64 / total as f64
        } else {
            0.0
        }
    }

    /// 스케일 업
    async fn scale_up(&self) {
        let connections = self.connections.read().await;
        let current_count = connections.len();

        if current_count >= self.config.max_connections {
            return;
        }

        let scale_amount = ((self.config.max_connections - current_count) / 4).max(1);
        drop(connections);

        info!("연결 풀 스케일 업: {} 연결 추가", scale_amount);

        for _ in 0..scale_amount {
            if let Err(e) = self.create_connection().await {
                warn!("스케일 업 중 연결 생성 실패: {}", e);
                break;
            }
        }

        self.stats.scale_up_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 스케일 다운
    async fn scale_down(&self) {
        let mut connections = self.connections.write().await;
        let current_count = connections.len();

        if current_count <= self.config.min_connections {
            return;
        }

        let scale_amount = ((current_count - self.config.min_connections) / 4).max(1);

        info!("연결 풀 스케일 다운: {} 연결 제거", scale_amount);

        // 유휴 연결부터 제거
        let mut idle_queue = self.idle_queue.lock().await;
        let mut removed = 0;

        while removed < scale_amount && !idle_queue.is_empty() {
            if let Some(conn_id) = idle_queue.pop_front() {
                connections.remove(&conn_id);
                removed += 1;
            }
        }

        self.stats.scale_down_count.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_connections
            .store(connections.len(), Ordering::Relaxed);
    }

    /// 통계 업데이트 (획득 시)
    async fn update_stats_on_acquire(&self) {
        self.stats
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
        self.stats.idle_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// 통계 업데이트 (반환 시)
    async fn update_stats_on_release(&self) {
        self.stats
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);
        self.stats.idle_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// 내부 복제 (백그라운드 태스크용)
    fn clone_internals(&self) -> Self {
        Self {
            config: self.config.clone(),
            connections: self.connections.clone(),
            idle_queue: self.idle_queue.clone(),
            stats: self.stats.clone(),
            next_conn_id: AtomicU64::new(self.next_conn_id.load(Ordering::Relaxed)),
            acquire_semaphore: self.acquire_semaphore.clone(),
            load_balancing: self.load_balancing,
            addresses: self.addresses.clone(),
            health_check_handle: Arc::new(Mutex::new(None)),
            auto_scale_handle: Arc::new(Mutex::new(None)),
            shutdown: self.shutdown.clone(),
        }
    }

    /// 종료
    pub async fn shutdown(&self) {
        info!("연결 풀 종료 시작");

        self.shutdown.store(true, Ordering::Relaxed);

        // 백그라운드 태스크 종료
        if let Some(handle) = self.health_check_handle.lock().await.take() {
            handle.abort();
        }

        if let Some(handle) = self.auto_scale_handle.lock().await.take() {
            handle.abort();
        }

        // 모든 연결 종료
        let connections = self.connections.write().await;
        info!("{}개 연결 종료", connections.len());
    }

    /// 성능 보고서 생성
    pub fn get_performance_report(&self) -> ConnectionPoolPerformanceReport {
        let stats = self.stats.clone();

        ConnectionPoolPerformanceReport {
            total_connections: stats.total_connections.load(Ordering::Relaxed),
            active_connections: stats.active_connections.load(Ordering::Relaxed),
            idle_connections: stats.idle_connections.load(Ordering::Relaxed),
            total_requests: stats.total_requests.load(Ordering::Relaxed),
            failed_requests: stats.failed_requests.load(Ordering::Relaxed),
            success_rate: {
                let total = stats.total_requests.load(Ordering::Relaxed) as f64;
                let failed = stats.failed_requests.load(Ordering::Relaxed) as f64;
                if total > 0.0 {
                    (total - failed) / total
                } else {
                    1.0
                }
            },
            avg_wait_time_us: {
                let total_time = stats.wait_time_us.load(Ordering::Relaxed) as f64;
                let total_requests = stats.total_requests.load(Ordering::Relaxed) as f64;
                if total_requests > 0.0 {
                    total_time / total_requests
                } else {
                    0.0
                }
            },
            connections_created: stats.connections_created.load(Ordering::Relaxed),
            connections_destroyed: stats.connections_destroyed.load(Ordering::Relaxed),
            scale_events: stats.scale_up_count.load(Ordering::Relaxed)
                + stats.scale_down_count.load(Ordering::Relaxed),
        }
    }
}

/// 연결 풀 성능 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolPerformanceReport {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub total_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub avg_wait_time_us: f64,
    pub connections_created: u64,
    pub connections_destroyed: u64,
    pub scale_events: u64,
}

impl ConnectionPoolPerformanceReport {
    /// 성능 점수 (0-100)
    pub fn performance_score(&self) -> f64 {
        let success_score = self.success_rate * 30.0;

        let utilization = if self.total_connections > 0 {
            self.active_connections as f64 / self.total_connections as f64
        } else {
            0.0
        };
        let utilization_score = (utilization * 0.7).min(1.0) * 25.0; // 70% 사용률이 최적

        let wait_score = (1000.0 / self.avg_wait_time_us.max(1.0)).min(1.0) * 25.0;

        let stability_score = if self.scale_events < 10 { 20.0 } else { 10.0 };

        success_score + utilization_score + wait_score + stability_score
    }
}

mod tests {

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let addr = "127.0.0.1:8080"
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .expect("Invalid socket address");
        let config = ConnectionPoolConfig {
            min_connections: 1,
            max_connections: 5,
            ..Default::default()
        };

        // 실제 서버가 없으므로 연결 실패 예상
        let result =
            AdvancedConnectionPool::new(config, vec![addr], LoadBalancingStrategy::RoundRobin)
                .await;

        assert!(result.is_err());
    }
}
