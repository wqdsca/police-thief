//! Rate Limiting 모듈
//!
//! DDoS 공격 방지 및 API 속도 제한을 위한 고성능 Rate Limiter

use crate::security::{SecurityConfig, SecurityError};
use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;

/// Rate Limiter 엔트리
#[derive(Debug, Clone)]
pub struct RateLimitEntry {
    /// 요청 횟수
    pub count: u64,
    /// 윈도우 시작 시간
    pub window_start: Instant,
    /// 마지막 요청 시간
    pub last_request: Instant,
    /// 차단 상태
    pub is_blocked: bool,
    /// 차단 해제 시간
    pub unblock_time: Option<Instant>,
}

impl RateLimitEntry {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            count: 0,
            window_start: now,
            last_request: now,
            is_blocked: false,
            unblock_time: None,
        }
    }

    /// 윈도우 리셋이 필요한지 확인
    pub fn should_reset_window(&self, window_duration: Duration) -> bool {
        self.window_start.elapsed() >= window_duration
    }

    /// 차단이 해제되었는지 확인
    pub fn is_unblocked(&self) -> bool {
        if let Some(unblock_time) = self.unblock_time {
            Instant::now() >= unblock_time
        } else {
            !self.is_blocked
        }
    }
}

impl Default for RateLimitEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate Limiter 설정
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 윈도우 지속시간 (분)
    pub window_duration_minutes: u64,
    /// 윈도우당 최대 요청 수
    pub max_requests: u64,
    /// 차단 지속시간 (분)
    pub block_duration_minutes: u64,
    /// 점진적 페널티 활성화
    pub enable_progressive_penalty: bool,
    /// 화이트리스트 IP들
    pub whitelist_ips: Vec<IpAddr>,
    /// 엄격 모드 (의심스러운 활동 시 즉시 차단)
    pub strict_mode: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_duration_minutes: 1,
            max_requests: 100,
            block_duration_minutes: 15,
            enable_progressive_penalty: true,
            whitelist_ips: vec![],
            strict_mode: false,
        }
    }
}

/// 고성능 Rate Limiter
pub struct RateLimiter {
    config: RateLimitConfig,
    /// IP별 요청 추적 (DashMap으로 동시성 최적화)
    entries: DashMap<IpAddr, RateLimitEntry>,
    /// 글로벌 통계
    stats: Arc<RwLock<RateLimitStats>>,
}

/// Rate Limiting 통계
#[derive(Debug, Default)]
pub struct RateLimitStats {
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub unique_ips: u64,
    pub current_blocked_ips: u64,
}

impl RateLimiter {
    /// 새 Rate Limiter 생성
    pub fn new(config: RateLimitConfig) -> Self {
        let limiter = Self {
            config,
            entries: DashMap::new(),
            stats: Arc::new(RwLock::new(RateLimitStats::default())),
        };

        // 정리 작업 시작
        limiter.start_cleanup_task();

        limiter
    }

    /// SecurityConfig에서 Rate Limiter 생성
    pub fn from_security_config(security_config: &SecurityConfig) -> Self {
        let config = RateLimitConfig {
            max_requests: security_config.rate_limit_rpm,
            ..Default::default()
        };
        Self::new(config)
    }

    /// 요청 허용 여부 확인
    pub async fn is_allowed(&self, ip: IpAddr) -> Result<bool, SecurityError> {
        // 화이트리스트 확인
        if self.config.whitelist_ips.contains(&ip) {
            return Ok(true);
        }

        let now = Instant::now();
        let window_duration = Duration::from_secs(self.config.window_duration_minutes * 60);

        // 통계 업데이트
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        // 엔트리 가져오기 또는 생성
        let mut entry = self.entries.entry(ip).or_insert_with(|| {
            // 새 IP 통계 업데이트
            tokio::spawn({
                let stats = self.stats.clone();
                async move {
                    let mut stats = stats.write().await;
                    stats.unique_ips += 1;
                }
            });
            RateLimitEntry::new()
        });

        // 차단 상태 확인
        if entry.is_blocked && !entry.is_unblocked() {
            let mut stats = self.stats.write().await;
            stats.blocked_requests += 1;

            tracing::warn!(
                target: "security",
                ip = %ip,
                unblock_time = ?entry.unblock_time,
                "Rate limited request blocked"
            );

            return Ok(false);
        }

        // 차단이 해제된 경우 상태 리셋
        if entry.is_blocked && entry.is_unblocked() {
            entry.is_blocked = false;
            entry.unblock_time = None;
            entry.count = 0;
            entry.window_start = now;

            let mut stats = self.stats.write().await;
            stats.current_blocked_ips = stats.current_blocked_ips.saturating_sub(1);
        }

        // 윈도우 리셋 확인
        if entry.should_reset_window(window_duration) {
            entry.count = 0;
            entry.window_start = now;
        }

        // 요청 카운트 증가
        entry.count += 1;
        entry.last_request = now;

        // 제한 확인
        if entry.count > self.config.max_requests {
            // 차단 적용
            entry.is_blocked = true;

            // 점진적 페널티 계산
            let block_duration = if self.config.enable_progressive_penalty {
                // 이전 위반 횟수에 따라 차단 시간 증가
                let multiplier = (entry.count - self.config.max_requests).min(5); // 최대 5배
                Duration::from_secs(self.config.block_duration_minutes * 60 * multiplier)
            } else {
                Duration::from_secs(self.config.block_duration_minutes * 60)
            };

            entry.unblock_time = Some(now + block_duration);

            // 통계 업데이트
            {
                let mut stats = self.stats.write().await;
                stats.blocked_requests += 1;
                stats.current_blocked_ips += 1;
            }

            tracing::warn!(
                target: "security",
                ip = %ip,
                requests_in_window = entry.count,
                max_requests = self.config.max_requests,
                block_duration_secs = block_duration.as_secs(),
                "IP address blocked due to rate limiting"
            );

            return Ok(false);
        }

        Ok(true)
    }

    /// 특정 IP 수동 차단
    pub async fn block_ip(
        &self,
        ip: IpAddr,
        duration_minutes: Option<u64>,
    ) -> Result<(), SecurityError> {
        let now = Instant::now();
        let duration = Duration::from_secs(
            duration_minutes.unwrap_or(self.config.block_duration_minutes) * 60,
        );

        let mut entry = self.entries.entry(ip).or_default();
        entry.is_blocked = true;
        entry.unblock_time = Some(now + duration);

        let mut stats = self.stats.write().await;
        stats.current_blocked_ips += 1;

        tracing::warn!(
            target: "security",
            ip = %ip,
            duration_minutes = duration_minutes.unwrap_or(self.config.block_duration_minutes),
            "IP manually blocked"
        );

        Ok(())
    }

    /// 특정 IP 차단 해제
    pub async fn unblock_ip(&self, ip: IpAddr) -> Result<(), SecurityError> {
        if let Some(mut entry) = self.entries.get_mut(&ip) {
            if entry.is_blocked {
                entry.is_blocked = false;
                entry.unblock_time = None;
                entry.count = 0;

                let mut stats = self.stats.write().await;
                stats.current_blocked_ips = stats.current_blocked_ips.saturating_sub(1);

                tracing::info!(
                    target: "security",
                    ip = %ip,
                    "IP manually unblocked"
                );
            }
        }

        Ok(())
    }

    /// 통계 정보 가져오기
    pub async fn get_stats(&self) -> RateLimitStats {
        let stats = self.stats.read().await;
        RateLimitStats {
            total_requests: stats.total_requests,
            blocked_requests: stats.blocked_requests,
            unique_ips: stats.unique_ips,
            current_blocked_ips: stats.current_blocked_ips,
        }
    }

    /// 차단된 IP 목록 가져오기
    pub fn get_blocked_ips(&self) -> Vec<IpAddr> {
        self.entries
            .iter()
            .filter(|entry| entry.is_blocked && !entry.is_unblocked())
            .map(|entry| *entry.key())
            .collect()
    }

    /// IP별 요청 정보 가져오기
    pub fn get_ip_info(&self, ip: IpAddr) -> Option<RateLimitEntry> {
        self.entries.get(&ip).map(|entry| entry.clone())
    }

    /// 정리 작업 시작 (만료된 엔트리 삭제)
    fn start_cleanup_task(&self) {
        let entries = self.entries.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(300)); // 5분마다

            loop {
                cleanup_interval.tick().await;

                let now = Instant::now();
                let mut removed_count = 0;
                let mut unblocked_count = 0;

                // 만료된 엔트리들 제거
                entries.retain(|_ip, entry| {
                    // 1시간 이상 비활성 상태인 엔트리 제거
                    let should_remove =
                        now.duration_since(entry.last_request) > Duration::from_secs(3600);

                    if should_remove {
                        removed_count += 1;
                        if entry.is_blocked {
                            unblocked_count += 1;
                        }
                    }

                    !should_remove
                });

                // 통계 업데이트
                if removed_count > 0 {
                    let mut stats = stats.write().await;
                    stats.unique_ips = stats.unique_ips.saturating_sub(removed_count);
                    stats.current_blocked_ips =
                        stats.current_blocked_ips.saturating_sub(unblocked_count);

                    tracing::debug!(
                        "Rate limiter cleanup: removed {} entries, unblocked {} IPs",
                        removed_count,
                        unblocked_count
                    );
                }
            }
        });
    }

    /// 의심스러운 패턴 감지
    pub async fn detect_suspicious_activity(&self, ip: IpAddr) -> bool {
        if let Some(entry) = self.entries.get(&ip) {
            let now = Instant::now();

            // 연속 요청 패턴 감지 (1초 내 10개 이상)
            if entry.count >= 10 && now.duration_since(entry.window_start) < Duration::from_secs(1)
            {
                return true;
            }

            // 버스트 패턴 감지
            if entry.count > self.config.max_requests / 2
                && now.duration_since(entry.window_start) < Duration::from_secs(10)
            {
                return true;
            }
        }

        false
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

mod tests {
    
    

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = RateLimitConfig {
            max_requests: 5,
            window_duration_minutes: 1,
            ..Default::default()
        };

        let limiter = RateLimiter::new(config);
        let test_ip = IpAddr::from_str("127.0.0.1").expect("Failed to parse IP address");

        // 처음 5개 요청은 허용
        for _ in 0..5 {
            assert!(limiter.is_allowed(test_ip).await.expect("Rate limiter check failed"));
        }

        // 6번째 요청은 차단
        assert!(!limiter.is_allowed(test_ip).await.expect("Rate limiter check failed"));
    }

    #[tokio::test]
    async fn test_whitelist() {
        let test_ip = IpAddr::from_str("192.168.1.1").expect("Failed to parse IP address");
        let config = RateLimitConfig {
            max_requests: 1,
            whitelist_ips: vec![test_ip],
            ..Default::default()
        };

        let limiter = RateLimiter::new(config);

        // 화이트리스트 IP는 제한 없음
        for _ in 0..10 {
            assert!(limiter.is_allowed(test_ip).await.expect("Whitelist check failed"));
        }
    }

    #[tokio::test]
    async fn test_manual_blocking() {
        let limiter = RateLimiter::default();
        let test_ip = IpAddr::from_str("10.0.0.1").expect("Failed to parse IP address");

        // 처음에는 허용
        assert!(limiter.is_allowed(test_ip).await.expect("Initial check failed"));

        // 수동 차단
        limiter.block_ip(test_ip, Some(1)).await.expect("Failed to block IP");

        // 차단 후에는 거부
        assert!(!limiter.is_allowed(test_ip).await.expect("Block check failed"));

        // 수동 해제
        limiter.unblock_ip(test_ip).await.expect("Failed to unblock IP");

        // 해제 후에는 허용
        assert!(limiter.is_allowed(test_ip).await.expect("Unblock check failed"));
    }
}
