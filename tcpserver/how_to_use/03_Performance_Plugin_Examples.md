# TCP 서버 성능 플러그인 및 확장 예시

## 📋 목차
1. [성능 모니터링 플러그인](#성능-모니터링-플러그인)
2. [캐시 시스템 확장](#캐시-시스템-확장)
3. [보안 미들웨어](#보안-미들웨어)
4. [실시간 분석 시스템](#실시간-분석-시스템)

## 📊 성능 모니터링 플러그인

### 실시간 메트릭 수집기
```rust
// plugins/metrics_collector.rs
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMetrics {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub connections: u32,
    pub messages_per_second: f64,
    pub avg_response_time_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub error_rate_percent: f64,
    pub room_stats: HashMap<u32, RoomMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMetrics {
    pub room_id: u32,
    pub user_count: u32,
    pub messages_per_minute: f64,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub game_state: Option<String>,
}

pub struct RealTimeMetricsCollector {
    metrics: Arc<RwLock<ServerMetrics>>,
    message_count: Arc<std::sync::atomic::AtomicU64>,
    response_times: Arc<RwLock<Vec<Duration>>>,
    error_count: Arc<std::sync::atomic::AtomicU64>,
    room_service: Arc<RoomConnectionService>,
    collection_interval: Duration,
}

impl RealTimeMetricsCollector {
    pub fn new(room_service: Arc<RoomConnectionService>) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(ServerMetrics {
                timestamp: chrono::Utc::now(),
                connections: 0,
                messages_per_second: 0.0,
                avg_response_time_ms: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
                error_rate_percent: 0.0,
                room_stats: HashMap::new(),
            })),
            message_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            response_times: Arc::new(RwLock::new(Vec::with_capacity(1000))),
            error_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            room_service,
            collection_interval: Duration::from_secs(5),
        }
    }
    
    /// 메트릭 수집 시작
    pub async fn start_collection(&self) {
        let metrics = self.metrics.clone();
        let message_count = self.message_count.clone();
        let response_times = self.response_times.clone();
        let error_count = self.error_count.clone();
        let room_service = self.room_service.clone();
        let interval = self.collection_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            let mut last_message_count = 0u64;
            let mut last_error_count = 0u64;
            
            loop {
                interval_timer.tick().await;
                
                // 메시지 처리율 계산
                let current_message_count = message_count.load(std::sync::atomic::Ordering::Relaxed);
                let messages_per_second = (current_message_count - last_message_count) as f64 / interval.as_secs() as f64;
                last_message_count = current_message_count;
                
                // 평균 응답 시간 계산
                let avg_response_time = {
                    let mut times = response_times.write().await;
                    let avg = if !times.is_empty() {
                        times.iter().sum::<Duration>().as_millis() as f64 / times.len() as f64
                    } else {
                        0.0
                    };
                    times.clear(); // 다음 주기를 위해 클리어
                    avg
                };
                
                // 에러율 계산
                let current_error_count = error_count.load(std::sync::atomic::Ordering::Relaxed);
                let error_rate = if current_message_count > 0 {
                    (current_error_count - last_error_count) as f64 / (current_message_count - last_message_count + current_error_count - last_error_count) as f64 * 100.0
                } else {
                    0.0
                };
                last_error_count = current_error_count;
                
                // 시스템 리소스 수집
                let memory_usage = Self::get_memory_usage();
                let cpu_usage = Self::get_cpu_usage();
                
                // 방별 통계 수집
                let room_stats = Self::collect_room_stats(&room_service).await;
                
                // 메트릭 업데이트
                let mut metrics_guard = metrics.write().await;
                metrics_guard.timestamp = chrono::Utc::now();
                metrics_guard.connections = room_service.get_total_users();
                metrics_guard.messages_per_second = messages_per_second;
                metrics_guard.avg_response_time_ms = avg_response_time;
                metrics_guard.memory_usage_mb = memory_usage;
                metrics_guard.cpu_usage_percent = cpu_usage;
                metrics_guard.error_rate_percent = error_rate;
                metrics_guard.room_stats = room_stats;
                
                tracing::info!(
                    "Metrics: {} conn, {:.1} msg/s, {:.2}ms avg, {:.1}MB mem, {:.1}% cpu, {:.2}% err",
                    metrics_guard.connections,
                    metrics_guard.messages_per_second,
                    metrics_guard.avg_response_time_ms,
                    metrics_guard.memory_usage_mb,
                    metrics_guard.cpu_usage_percent,
                    metrics_guard.error_rate_percent
                );
            }
        });
    }
    
    /// 메시지 처리 기록
    pub fn record_message(&self) {
        self.message_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// 응답 시간 기록
    pub async fn record_response_time(&self, duration: Duration) {
        let mut times = self.response_times.write().await;
        if times.len() < 1000 { // 메모리 제한
            times.push(duration);
        }
    }
    
    /// 에러 기록
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// 현재 메트릭 조회
    pub async fn get_current_metrics(&self) -> ServerMetrics {
        self.metrics.read().await.clone()
    }
    
    // 시스템 리소스 유틸리티
    fn get_memory_usage() -> f64 {
        // 실제 구현에서는 시스템 메모리 사용량 조회
        // Windows: GetProcessMemoryInfo
        // Linux: /proc/self/status
        0.0 // Placeholder
    }
    
    fn get_cpu_usage() -> f64 {
        // 실제 구현에서는 CPU 사용률 조회
        0.0 // Placeholder
    }
    
    async fn collect_room_stats(room_service: &Arc<RoomConnectionService>) -> HashMap<u32, RoomMetrics> {
        let mut room_stats = HashMap::new();
        
        for room_info in room_service.get_all_rooms() {
            let user_count = room_service.get_room_user_count(room_info.room_id);
            
            room_stats.insert(room_info.room_id, RoomMetrics {
                room_id: room_info.room_id,
                user_count,
                messages_per_minute: 0.0, // 구현 필요
                last_activity: room_info.last_activity,
                game_state: None, // 게임 핸들러에서 가져올 수 있음
            });
        }
        
        room_stats
    }
}

/// 성능 미들웨어
pub struct PerformanceMiddleware {
    metrics_collector: Arc<RealTimeMetricsCollector>,
}

impl PerformanceMiddleware {
    pub fn new(metrics_collector: Arc<RealTimeMetricsCollector>) -> Self {
        Self { metrics_collector }
    }
}

#[async_trait::async_trait]
impl MessageMiddleware for PerformanceMiddleware {
    async fn before_process(&self, _user_id: u32, _message: &mut GameMessage) -> Result<bool> {
        // 메시지 처리 시작 시간 기록
        Ok(true)
    }
    
    async fn after_process(
        &self,
        _user_id: u32,
        _message: &GameMessage,
        result: &Result<GameMessage>
    ) -> Result<()> {
        // 메시지 처리 완료 기록
        self.metrics_collector.record_message();
        
        if result.is_err() {
            self.metrics_collector.record_error();
        }
        
        Ok(())
    }
}
```

## 💾 캐시 시스템 확장

### 다단계 캐시 시스템
```rust
// plugins/cache_system.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use lru::LruCache;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub l1_size: usize,        // L1 캐시 크기 (빠른 메모리)
    pub l2_size: usize,        // L2 캐시 크기 (LRU)
    pub ttl: Duration,         // TTL (Time To Live)
    pub enable_compression: bool, // 압축 사용
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 1000,
            l2_size: 10000,
            ttl: Duration::from_secs(300),
            enable_compression: true,
        }
    }
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    data: T,
    created_at: Instant,
    access_count: u64,
    last_access: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T) -> Self {
        let now = Instant::now();
        Self {
            data,
            created_at: now,
            access_count: 1,
            last_access: now,
        }
    }
    
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
    
    fn access(&mut self) -> &T {
        self.access_count += 1;
        self.last_access = Instant::now();
        &self.data
    }
}

pub struct MultiTierCache<T> 
where 
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>
{
    // L1: 가장 빠른 캐시 (HashMap)
    l1_cache: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    // L2: LRU 캐시
    l2_cache: Arc<RwLock<LruCache<String, CacheEntry<T>>>>,
    config: CacheConfig,
    
    // 통계
    l1_hits: Arc<std::sync::atomic::AtomicU64>,
    l2_hits: Arc<std::sync::atomic::AtomicU64>,
    misses: Arc<std::sync::atomic::AtomicU64>,
}

impl<T> MultiTierCache<T> 
where 
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            l2_cache: Arc::new(RwLock::new(LruCache::new(config.l2_size))),
            config,
            l1_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            l2_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
    
    /// 캐시에서 값 조회
    pub async fn get(&self, key: &str) -> Option<T> {
        // L1 캐시 확인
        {
            let mut l1 = self.l1_cache.write().await;
            if let Some(entry) = l1.get_mut(key) {
                if !entry.is_expired(self.config.ttl) {
                    self.l1_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Some(entry.access().clone());
                } else {
                    l1.remove(key); // 만료된 항목 제거
                }
            }
        }
        
        // L2 캐시 확인
        {
            let mut l2 = self.l2_cache.write().await;
            if let Some(entry) = l2.get_mut(key) {
                if !entry.is_expired(self.config.ttl) {
                    self.l2_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let data = entry.access().clone();
                    
                    // L1으로 승격
                    self.promote_to_l1(key.to_string(), data.clone()).await;
                    return Some(data);
                } else {
                    l2.pop(key); // 만료된 항목 제거
                }
            }
        }
        
        // 캐시 미스
        self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        None
    }
    
    /// 캐시에 값 저장
    pub async fn set(&self, key: String, value: T) {
        let entry = CacheEntry::new(value);
        
        // L1이 가득 찬 경우 일부를 L2로 이동
        if self.should_evict_l1().await {
            self.evict_l1_to_l2().await;
        }
        
        // L1에 저장
        let mut l1 = self.l1_cache.write().await;
        l1.insert(key, entry);
    }
    
    /// 캐시에서 값 제거
    pub async fn remove(&self, key: &str) {
        let mut l1 = self.l1_cache.write().await;
        l1.remove(key);
        
        let mut l2 = self.l2_cache.write().await;
        l2.pop(key);
    }
    
    /// L1 캐시 정리 필요 여부 확인
    async fn should_evict_l1(&self) -> bool {
        let l1 = self.l1_cache.read().await;
        l1.len() >= self.config.l1_size
    }
    
    /// L1에서 사용 빈도가 낮은 항목을 L2로 이동
    async fn evict_l1_to_l2(&self) {
        let mut items_to_move = Vec::new();
        
        // L1에서 가장 오래된 항목들 선별
        {
            let l1 = self.l1_cache.read().await;
            let mut entries: Vec<_> = l1.iter().collect();
            entries.sort_by_key(|(_, entry)| entry.last_access);
            
            // 상위 25% 이동
            let move_count = self.config.l1_size / 4;
            for (key, entry) in entries.iter().take(move_count) {
                items_to_move.push(((*key).clone(), (*entry).clone()));
            }
        }
        
        // L1에서 제거하고 L2로 이동
        {
            let mut l1 = self.l1_cache.write().await;
            let mut l2 = self.l2_cache.write().await;
            
            for (key, entry) in items_to_move {
                l1.remove(&key);
                l2.put(key, entry);
            }
        }
    }
    
    /// 값을 L1으로 승격
    async fn promote_to_l1(&self, key: String, value: T) {
        if self.should_evict_l1().await {
            self.evict_l1_to_l2().await;
        }
        
        let mut l1 = self.l1_cache.write().await;
        l1.insert(key, CacheEntry::new(value));
    }
    
    /// 만료된 항목 정리
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        
        // L1 정리
        {
            let mut l1 = self.l1_cache.write().await;
            l1.retain(|_, entry| now.duration_since(entry.created_at) <= self.config.ttl);
        }
        
        // L2 정리
        {
            let mut l2 = self.l2_cache.write().await;
            let keys_to_remove: Vec<String> = l2.iter()
                .filter(|(_, entry)| entry.is_expired(self.config.ttl))
                .map(|(key, _)| key.clone())
                .collect();
                
            for key in keys_to_remove {
                l2.pop(&key);
            }
        }
    }
    
    /// 캐시 통계 조회
    pub async fn get_stats(&self) -> CacheStats {
        let l1_size = self.l1_cache.read().await.len();
        let l2_size = self.l2_cache.read().await.len();
        let l1_hits = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed);
        let l2_hits = self.l2_hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total_requests = l1_hits + l2_hits + misses;
        
        CacheStats {
            l1_size,
            l2_size,
            l1_hits,
            l2_hits,
            misses,
            hit_rate: if total_requests > 0 {
                (l1_hits + l2_hits) as f64 / total_requests as f64 * 100.0
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
    pub l1_size: usize,
    pub l2_size: usize,
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

/// 게임 상태 캐시 전용 구현
pub type GameStateCache = MultiTierCache<GameState>;

impl GameStateCache {
    /// 방 ID로 게임 상태 캐시 키 생성
    pub fn room_key(room_id: u32) -> String {
        format!("game_state:{}", room_id)
    }
    
    /// 사용자별 게임 정보 캐시 키 생성
    pub fn user_key(user_id: u32) -> String {
        format!("user_game:{}", user_id)
    }
}

/// 캐시 정리 스케줄러
pub struct CacheCleanupScheduler<T>
where 
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>
{
    cache: Arc<MultiTierCache<T>>,
    cleanup_interval: Duration,
}

impl<T> CacheCleanupScheduler<T>
where 
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>
{
    pub fn new(cache: Arc<MultiTierCache<T>>, cleanup_interval: Duration) -> Self {
        Self {
            cache,
            cleanup_interval,
        }
    }
    
    pub async fn start_cleanup_task(&self) {
        let cache = self.cache.clone();
        let interval = self.cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                let start = Instant::now();
                cache.cleanup_expired().await;
                let duration = start.elapsed();
                
                let stats = cache.get_stats().await;
                tracing::debug!(
                    "Cache cleanup completed in {}ms: L1={}, L2={}, Hit rate={:.1}%",
                    duration.as_millis(),
                    stats.l1_size,
                    stats.l2_size,
                    stats.hit_rate
                );
            }
        });
    }
}
```

## 🔐 보안 미들웨어

### 종합 보안 시스템
```rust
// plugins/security_middleware.rs
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use std::net::IpAddr;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub rate_limit_per_minute: u32,
    pub max_message_size: usize,
    pub blocked_ips: Vec<IpAddr>,
    pub enable_ddos_protection: bool,
    pub suspicious_activity_threshold: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            rate_limit_per_minute: 60,
            max_message_size: 64 * 1024, // 64KB
            blocked_ips: Vec::new(),
            enable_ddos_protection: true,
            suspicious_activity_threshold: 100,
        }
    }
}

#[derive(Debug)]
struct UserSecurityInfo {
    message_count: u32,
    last_reset: Instant,
    suspicious_count: u32,
    last_message_time: Option<Instant>,
    blocked_until: Option<Instant>,
}

impl UserSecurityInfo {
    fn new() -> Self {
        Self {
            message_count: 0,
            last_reset: Instant::now(),
            suspicious_count: 0,
            last_message_time: None,
            blocked_until: None,
        }
    }
    
    fn reset_if_needed(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_reset) >= Duration::from_secs(60) {
            self.message_count = 0;
            self.last_reset = now;
        }
    }
    
    fn is_blocked(&self) -> bool {
        if let Some(blocked_until) = self.blocked_until {
            Instant::now() < blocked_until
        } else {
            false
        }
    }
    
    fn block_temporarily(&mut self, duration: Duration) {
        self.blocked_until = Some(Instant::now() + duration);
    }
}

pub struct SecurityMiddleware {
    config: SecurityConfig,
    user_info: Arc<RwLock<HashMap<u32, UserSecurityInfo>>>,
    ip_info: Arc<RwLock<HashMap<IpAddr, UserSecurityInfo>>>,
}

impl SecurityMiddleware {
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            user_info: Arc::new(RwLock::new(HashMap::new())),
            ip_info: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 메시지 보안 검증
    pub async fn validate_message(
        &self,
        user_id: u32,
        user_ip: IpAddr,
        message: &GameMessage,
    ) -> Result<bool> {
        // IP 차단 확인
        if self.config.blocked_ips.contains(&user_ip) {
            return Err(anyhow!("Blocked IP address"));
        }
        
        // 사용자별 보안 정보 업데이트
        let mut user_info_map = self.user_info.write().await;
        let user_info = user_info_map.entry(user_id).or_insert_with(UserSecurityInfo::new);
        
        // 임시 차단 확인
        if user_info.is_blocked() {
            return Err(anyhow!("User temporarily blocked"));
        }
        
        // 속도 제한 확인
        user_info.reset_if_needed();
        user_info.message_count += 1;
        
        if user_info.message_count > self.config.rate_limit_per_minute {
            user_info.suspicious_count += 1;
            
            // 지속적인 위반 시 임시 차단
            if user_info.suspicious_count > 3 {
                user_info.block_temporarily(Duration::from_secs(300)); // 5분 차단
                tracing::warn!("User {} temporarily blocked for rate limit violations", user_id);
                return Err(anyhow!("Rate limit exceeded - temporarily blocked"));
            }
            
            return Err(anyhow!("Rate limit exceeded"));
        }
        
        // 메시지 크기 확인
        let message_size = self.estimate_message_size(message);
        if message_size > self.config.max_message_size {
            user_info.suspicious_count += 1;
            return Err(anyhow!("Message too large: {} bytes", message_size));
        }
        
        // DDoS 패턴 감지
        if self.config.enable_ddos_protection {
            if let Some(last_time) = user_info.last_message_time {
                let time_diff = Instant::now().duration_since(last_time);
                
                // 너무 빠른 연속 메시지 (1ms 미만)
                if time_diff < Duration::from_millis(1) {
                    user_info.suspicious_count += 10;
                    
                    if user_info.suspicious_count > self.config.suspicious_activity_threshold {
                        user_info.block_temporarily(Duration::from_secs(600)); // 10분 차단
                        tracing::error!("User {} blocked for DDoS-like activity", user_id);
                        return Err(anyhow!("Suspicious activity detected"));
                    }
                }
            }
        }
        
        user_info.last_message_time = Some(Instant::now());
        
        // 메시지 내용 검증
        self.validate_message_content(message)?;
        
        Ok(true)
    }
    
    /// 메시지 내용 검증 (SQL 인젝션, XSS 등)
    fn validate_message_content(&self, message: &GameMessage) -> Result<()> {
        match message {
            GameMessage::Chat { message, .. } => {
                // SQL 인젝션 패턴 검사
                let dangerous_patterns = [
                    "'; DROP", "'; DELETE", "'; UPDATE", "'; INSERT",
                    "<script", "</script>", "javascript:", "onload=",
                    "onerror=", "onclick=", "eval(", "document.cookie",
                ];
                
                let message_lower = message.to_lowercase();
                for pattern in &dangerous_patterns {
                    if message_lower.contains(&pattern.to_lowercase()) {
                        return Err(anyhow!("Potentially dangerous content detected"));
                    }
                }
                
                // 메시지 길이 제한
                if message.len() > 1000 {
                    return Err(anyhow!("Message too long"));
                }
            }
            
            GameMessage::JoinRoom { room_id, .. } => {
                // 유효하지 않은 방 ID 패턴
                if *room_id == 0 || *room_id > 1000000 {
                    return Err(anyhow!("Invalid room ID"));
                }
            }
            
            _ => {} // 다른 메시지 타입 검증
        }
        
        Ok(())
    }
    
    /// 메시지 크기 추정
    fn estimate_message_size(&self, message: &GameMessage) -> usize {
        // 실제 구현에서는 더 정확한 직렬화 크기 계산
        match message {
            GameMessage::Chat { message, .. } => message.len() + 100, // 헤더 추정
            _ => 200, // 기본 크기 추정
        }
    }
    
    /// 보안 통계 조회
    pub async fn get_security_stats(&self) -> SecurityStats {
        let user_info = self.user_info.read().await;
        let blocked_users = user_info.values().filter(|info| info.is_blocked()).count();
        let suspicious_users = user_info.values().filter(|info| info.suspicious_count > 0).count();
        
        SecurityStats {
            total_users: user_info.len(),
            blocked_users,
            suspicious_users,
            blocked_ips: self.config.blocked_ips.len(),
        }
    }
    
    /// 사용자 차단 해제
    pub async fn unblock_user(&self, user_id: u32) -> Result<()> {
        let mut user_info = self.user_info.write().await;
        if let Some(info) = user_info.get_mut(&user_id) {
            info.blocked_until = None;
            info.suspicious_count = 0;
            tracing::info!("User {} unblocked", user_id);
            Ok(())
        } else {
            Err(anyhow!("User not found"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityStats {
    pub total_users: usize,
    pub blocked_users: usize,
    pub suspicious_users: usize,
    pub blocked_ips: usize,
}

#[async_trait::async_trait]
impl MessageMiddleware for SecurityMiddleware {
    async fn before_process(&self, user_id: u32, message: &mut GameMessage) -> Result<bool> {
        // 보안 검증 (실제 구현에서는 IP 주소 정보도 필요)
        let user_ip = IpAddr::from([127, 0, 0, 1]); // Placeholder
        
        match self.validate_message(user_id, user_ip, message).await {
            Ok(true) => Ok(true),
            Ok(false) => Ok(false),
            Err(e) => {
                tracing::warn!("Security validation failed for user {}: {}", user_id, e);
                Ok(false) // 메시지 처리 중단
            }
        }
    }
    
    async fn after_process(
        &self,
        _user_id: u32,
        _message: &GameMessage,
        _result: &Result<GameMessage>
    ) -> Result<()> {
        // 처리 후 보안 로깅 등
        Ok(())
    }
}
```

## 📈 실시간 분석 시스템

### 이벤트 스트림 프로세서
```rust
// plugins/analytics_system.rs
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyticsEvent {
    UserConnected { user_id: u32, timestamp: chrono::DateTime<chrono::Utc> },
    UserDisconnected { user_id: u32, duration: Duration },
    MessageSent { user_id: u32, message_type: String, room_id: Option<u32> },
    GameStarted { room_id: u32, player_count: u32 },
    GameEnded { room_id: u32, duration: Duration, winner: Option<u32> },
    RoomCreated { room_id: u32, creator_id: u32 },
    RoomJoined { room_id: u32, user_id: u32 },
    RoomLeft { room_id: u32, user_id: u32 },
    Error { user_id: u32, error_type: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnalytics {
    pub user_id: u32,
    pub total_sessions: u32,
    pub total_playtime: Duration,
    pub messages_sent: u32,
    pub games_played: u32,
    pub games_won: u32,
    pub favorite_rooms: HashMap<u32, u32>, // room_id -> visit_count
    pub last_active: chrono::DateTime<chrono::Utc>,
    pub activity_pattern: Vec<(u8, u32)>, // (hour, activity_count)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAnalytics {
    pub room_id: u32,
    pub total_sessions: u32,
    pub peak_users: u32,
    pub total_messages: u32,
    pub games_played: u32,
    pub avg_session_duration: Duration,
    pub popular_times: Vec<(u8, u32)>, // (hour, user_count)
}

pub struct RealTimeAnalytics {
    event_sender: broadcast::Sender<AnalyticsEvent>,
    user_analytics: Arc<RwLock<HashMap<u32, UserAnalytics>>>,
    room_analytics: Arc<RwLock<HashMap<u32, RoomAnalytics>>>,
    
    // 실시간 집계용 버퍼
    recent_events: Arc<RwLock<VecDeque<(Instant, AnalyticsEvent)>>>,
    buffer_size: usize,
    
    // 성능 메트릭
    events_processed: Arc<std::sync::atomic::AtomicU64>,
    processing_time: Arc<RwLock<VecDeque<Duration>>>,
}

impl RealTimeAnalytics {
    pub fn new(buffer_size: usize) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        Self {
            event_sender,
            user_analytics: Arc::new(RwLock::new(HashMap::new())),
            room_analytics: Arc::new(RwLock::new(HashMap::new())),
            recent_events: Arc::new(RwLock::new(VecDeque::with_capacity(buffer_size))),
            buffer_size,
            events_processed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            processing_time: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
    
    /// 분석 시스템 시작
    pub async fn start(&self) {
        self.start_event_processor().await;
        self.start_periodic_aggregation().await;
        tracing::info!("Real-time analytics system started");
    }
    
    /// 이벤트 발생
    pub async fn emit_event(&self, event: AnalyticsEvent) {
        let start_time = Instant::now();
        
        // 이벤트 브로드캐스트
        let _ = self.event_sender.send(event.clone());
        
        // 최근 이벤트 버퍼에 추가
        {
            let mut recent = self.recent_events.write().await;
            if recent.len() >= self.buffer_size {
                recent.pop_front();
            }
            recent.push_back((Instant::now(), event));
        }
        
        // 처리 시간 기록
        let processing_duration = start_time.elapsed();
        {
            let mut times = self.processing_time.write().await;
            if times.len() >= 1000 {
                times.pop_front();
            }
            times.push_back(processing_duration);
        }
        
        self.events_processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// 이벤트 프로세서 시작
    async fn start_event_processor(&self) {
        let user_analytics = self.user_analytics.clone();
        let room_analytics = self.room_analytics.clone();
        let mut receiver = self.event_sender.subscribe();
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                if let Err(e) = Self::process_event(&user_analytics, &room_analytics, event).await {
                    tracing::error!("Failed to process analytics event: {}", e);
                }
            }
        });
    }
    
    /// 단일 이벤트 처리
    async fn process_event(
        user_analytics: &Arc<RwLock<HashMap<u32, UserAnalytics>>>,
        room_analytics: &Arc<RwLock<HashMap<u32, RoomAnalytics>>>,
        event: AnalyticsEvent,
    ) -> Result<()> {
        match event {
            AnalyticsEvent::UserConnected { user_id, timestamp } => {
                let mut users = user_analytics.write().await;
                let analytics = users.entry(user_id).or_insert_with(|| UserAnalytics {
                    user_id,
                    total_sessions: 0,
                    total_playtime: Duration::from_secs(0),
                    messages_sent: 0,
                    games_played: 0,
                    games_won: 0,
                    favorite_rooms: HashMap::new(),
                    last_active: timestamp,
                    activity_pattern: vec![(0, 0); 24], // 24시간 패턴
                });
                
                analytics.total_sessions += 1;
                analytics.last_active = timestamp;
                
                // 시간대별 활동 패턴 업데이트
                let hour = timestamp.hour() as usize;
                if hour < 24 {
                    analytics.activity_pattern[hour].1 += 1;
                }
            }
            
            AnalyticsEvent::MessageSent { user_id, message_type: _, room_id } => {
                let mut users = user_analytics.write().await;
                if let Some(analytics) = users.get_mut(&user_id) {
                    analytics.messages_sent += 1;
                }
                drop(users);
                
                if let Some(room_id) = room_id {
                    let mut rooms = room_analytics.write().await;
                    let room_analytics = rooms.entry(room_id).or_insert_with(|| RoomAnalytics {
                        room_id,
                        total_sessions: 0,
                        peak_users: 0,
                        total_messages: 0,
                        games_played: 0,
                        avg_session_duration: Duration::from_secs(0),
                        popular_times: vec![(0, 0); 24],
                    });
                    
                    room_analytics.total_messages += 1;
                }
            }
            
            AnalyticsEvent::GameStarted { room_id, player_count } => {
                let mut rooms = room_analytics.write().await;
                if let Some(analytics) = rooms.get_mut(&room_id) {
                    analytics.games_played += 1;
                    if player_count > analytics.peak_users {
                        analytics.peak_users = player_count;
                    }
                }
            }
            
            AnalyticsEvent::GameEnded { room_id: _, duration: _, winner } => {
                if let Some(winner_id) = winner {
                    let mut users = user_analytics.write().await;
                    if let Some(analytics) = users.get_mut(&winner_id) {
                        analytics.games_won += 1;
                    }
                }
            }
            
            AnalyticsEvent::RoomJoined { room_id, user_id } => {
                let mut users = user_analytics.write().await;
                if let Some(analytics) = users.get_mut(&user_id) {
                    *analytics.favorite_rooms.entry(room_id).or_insert(0) += 1;
                }
            }
            
            _ => {} // 다른 이벤트 처리
        }
        
        Ok(())
    }
    
    /// 주기적 집계 작업
    async fn start_periodic_aggregation(&self) {
        let recent_events = self.recent_events.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // 최근 1분간 이벤트 통계
                let events = recent_events.read().await;
                let one_minute_ago = Instant::now() - Duration::from_secs(60);
                
                let recent_count = events.iter()
                    .filter(|(timestamp, _)| *timestamp > one_minute_ago)
                    .count();
                
                tracing::debug!("Analytics: {} events in last minute", recent_count);
            }
        });
    }
    
    /// 사용자 분석 데이터 조회
    pub async fn get_user_analytics(&self, user_id: u32) -> Option<UserAnalytics> {
        let users = self.user_analytics.read().await;
        users.get(&user_id).cloned()
    }
    
    /// 방 분석 데이터 조회
    pub async fn get_room_analytics(&self, room_id: u32) -> Option<RoomAnalytics> {
        let rooms = self.room_analytics.read().await;
        rooms.get(&room_id).cloned()
    }
    
    /// 실시간 통계 조회
    pub async fn get_realtime_stats(&self) -> RealtimeStats {
        let recent = self.recent_events.read().await;
        let one_minute_ago = Instant::now() - Duration::from_secs(60);
        
        let events_last_minute = recent.iter()
            .filter(|(timestamp, _)| *timestamp > one_minute_ago)
            .count();
            
        let avg_processing_time = {
            let times = self.processing_time.read().await;
            if !times.is_empty() {
                times.iter().sum::<Duration>() / times.len() as u32
            } else {
                Duration::from_nanos(0)
            }
        };
        
        RealtimeStats {
            events_per_minute: events_last_minute,
            total_events_processed: self.events_processed.load(std::sync::atomic::Ordering::Relaxed),
            avg_processing_time_us: avg_processing_time.as_micros() as u64,
            buffer_utilization: (recent.len() as f64 / self.buffer_size as f64 * 100.0) as u32,
        }
    }
    
    /// 이벤트 구독자 생성
    pub fn subscribe(&self) -> broadcast::Receiver<AnalyticsEvent> {
        self.event_sender.subscribe()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealtimeStats {
    pub events_per_minute: usize,
    pub total_events_processed: u64,
    pub avg_processing_time_us: u64,
    pub buffer_utilization: u32,
}

/// 분석 미들웨어
pub struct AnalyticsMiddleware {
    analytics: Arc<RealTimeAnalytics>,
}

impl AnalyticsMiddleware {
    pub fn new(analytics: Arc<RealTimeAnalytics>) -> Self {
        Self { analytics }
    }
}

#[async_trait::async_trait]
impl MessageMiddleware for AnalyticsMiddleware {
    async fn before_process(&self, user_id: u32, message: &mut GameMessage) -> Result<bool> {
        // 메시지 송신 이벤트 발생
        let message_type = match message {
            GameMessage::Chat { .. } => "chat",
            GameMessage::JoinRoom { .. } => "join_room",
            GameMessage::LeaveRoom { .. } => "leave_room",
            GameMessage::Heartbeat { .. } => "heartbeat",
            _ => "other",
        }.to_string();
        
        let room_id = match message {
            GameMessage::Chat { room_id, .. } => Some(*room_id),
            GameMessage::JoinRoom { room_id, .. } => Some(*room_id),
            GameMessage::LeaveRoom { room_id, .. } => Some(*room_id),
            _ => None,
        };
        
        self.analytics.emit_event(AnalyticsEvent::MessageSent {
            user_id,
            message_type,
            room_id,
        }).await;
        
        Ok(true)
    }
    
    async fn after_process(
        &self,
        user_id: u32,
        _message: &GameMessage,
        result: &Result<GameMessage>
    ) -> Result<()> {
        // 에러 발생시 분석 이벤트
        if let Err(e) = result {
            self.analytics.emit_event(AnalyticsEvent::Error {
                user_id,
                error_type: "message_processing".to_string(),
                message: e.to_string(),
            }).await;
        }
        
        Ok(())
    }
}
```

이러한 플러그인들을 통해 TCP 서버의 기능을 크게 확장할 수 있으며, 각각은 독립적으로 활성화/비활성화할 수 있어 유연한 서버 구성이 가능합니다.