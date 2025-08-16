//! 네트워크 유틸리티
//!
//! IP 주소 파싱, 포트 검증, 네트워크 상태 확인 등의 기능을 제공합니다.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, warn};

/// 네트워크 유틸리티
pub struct NetworkUtils;

impl NetworkUtils {
    /// IP 주소 문자열 파싱
    ///
    /// 문자열을 IpAddr로 파싱합니다. IPv4와 IPv6를 모두 지원합니다.
    ///
    /// # Arguments
    ///
    /// * `ip_str` - 파싱할 IP 주소 문자열
    ///
    /// # Returns
    ///
    /// * `Result<IpAddr>` - 성공 시 IpAddr, 실패 시 에러
    ///
    /// # Examples
    ///
    /// ```rust
    /// let ip = NetworkUtils::parse_ip("192.168.1.1").expect("Test assertion failed");
    /// let ipv6 = NetworkUtils::parse_ip("::1").expect("Test assertion failed");
    /// ```
    pub fn parse_ip(ip_str: &str) -> Result<IpAddr> {
        ip_str
            .parse::<IpAddr>()
            .map_err(|e| anyhow!("IP 주소 파싱 실패: {} ({})", ip_str, e))
    }

    /// 소켓 주소 파싱
    pub fn parse_socket_addr(addr_str: &str) -> Result<SocketAddr> {
        addr_str
            .parse::<SocketAddr>()
            .map_err(|e| anyhow!("소켓 주소 파싱 실패: {} ({})", addr_str, e))
    }

    /// 포트 번호 검증
    pub fn validate_port(port: u16) -> Result<u16> {
        match port {
            0 => Err(anyhow!("포트 0은 사용할 수 없습니다")),
            1..=1023 => {
                warn!("시스템 포트 사용: {} (권한 필요 가능)", port);
                Ok(port)
            }
            1024..=49151 => Ok(port),  // 등록된 포트
            49152..=65535 => Ok(port), // 동적 포트
        }
    }

    /// 연결 테스트 (타임아웃 포함)
    pub async fn test_connection(addr: &str, timeout_secs: u64) -> Result<bool> {
        let socket_addr = Self::parse_socket_addr(addr)?;

        match timeout(
            Duration::from_secs(timeout_secs),
            TcpStream::connect(socket_addr),
        )
        .await
        {
            Ok(Ok(_)) => {
                debug!("연결 테스트 성공: {}", addr);
                Ok(true)
            }
            Ok(Err(e)) => {
                debug!("연결 실패: {} ({})", addr, e);
                Ok(false)
            }
            Err(_) => {
                debug!("연결 타임아웃: {} ({}초)", addr, timeout_secs);
                Ok(false)
            }
        }
    }

    /// 로컬호스트 여부 확인
    pub fn is_localhost(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => ipv4.is_loopback(),
            IpAddr::V6(ipv6) => ipv6.is_loopback(),
        }
    }

    /// 사설 IP 여부 확인
    pub fn is_private_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => ipv4.is_private(),
            IpAddr::V6(ipv6) => {
                // IPv6 사설 주소 (fc00::/7, fe80::/10)
                let octets = ipv6.octets();
                (octets[0] & 0xfe == 0xfc) || (octets[0] == 0xfe && octets[1] & 0xc0 == 0x80)
            }
        }
    }

    /// 바인드 주소 검증 및 정규화
    pub fn normalize_bind_address(addr: &str) -> Result<SocketAddr> {
        let socket_addr = if addr.contains(':') {
            // 포트 포함된 주소
            Self::parse_socket_addr(addr)?
        } else {
            // 포트만 지정된 경우
            let port: u16 = addr
                .parse()
                .map_err(|e| anyhow!("포트 파싱 실패: {} ({})", addr, e))?;
            Self::validate_port(port)?;
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port)
        };

        Self::validate_port(socket_addr.port())?;
        Ok(socket_addr)
    }

    /// 클라이언트 주소에서 IP만 추출
    pub fn extract_client_ip(addr: &SocketAddr) -> String {
        addr.ip().to_string()
    }

    /// 연결 품질 평가 (응답 시간 기반)
    pub async fn evaluate_connection_quality(addr: &str) -> Result<ConnectionQuality> {
        let start = std::time::Instant::now();
        let connected = Self::test_connection(addr, 3).await?;
        let duration = start.elapsed();

        if !connected {
            return Ok(ConnectionQuality::Disconnected);
        }

        match duration.as_millis() {
            0..=50 => Ok(ConnectionQuality::Excellent),
            51..=150 => Ok(ConnectionQuality::Good),
            151..=300 => Ok(ConnectionQuality::Fair),
            301..=1000 => Ok(ConnectionQuality::Poor),
            _ => Ok(ConnectionQuality::VeryPoor),
        }
    }

    /// 네트워크 지연시간 측정
    pub async fn measure_latency(addr: &str, samples: usize) -> Result<LatencyStats> {
        let mut measurements = Vec::with_capacity(samples);

        for _ in 0..samples {
            let start = std::time::Instant::now();
            let connected = Self::test_connection(addr, 1).await?;
            let duration = start.elapsed();

            if connected {
                measurements.push(duration.as_millis() as f64);
            }
        }

        if measurements.is_empty() {
            return Err(anyhow!("모든 연결 시도 실패"));
        }

        let avg = measurements.iter().sum::<f64>() / measurements.len() as f64;
        let min = measurements.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = measurements.iter().fold(0.0f64, |a, &b| a.max(b));

        Ok(LatencyStats {
            average_ms: avg,
            min_ms: min,
            max_ms: max,
            samples: measurements.len(),
        })
    }
}

/// 연결 품질 등급
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionQuality {
    Excellent,    // 0-50ms
    Good,         // 51-150ms
    Fair,         // 151-300ms
    Poor,         // 301-1000ms
    VeryPoor,     // >1000ms
    Disconnected, // 연결 불가
}

/// 지연시간 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub average_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub samples: usize,
}

/// IP 주소 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpInfo {
    pub address: String,
    pub is_localhost: bool,
    pub is_private: bool,
    pub ip_version: String,
}

impl IpInfo {
    /// IP 주소 정보 생성
    pub fn from_ip(ip: &IpAddr) -> Self {
        Self {
            address: ip.to_string(),
            is_localhost: NetworkUtils::is_localhost(ip),
            is_private: NetworkUtils::is_private_ip(ip),
            ip_version: match ip {
                IpAddr::V4(_) => "IPv4".to_string(),
                IpAddr::V6(_) => "IPv6".to_string(),
            },
        }
    }

    /// 소켓 주소에서 IP 정보 추출
    pub fn from_socket_addr(addr: &SocketAddr) -> Self {
        Self::from_ip(&addr.ip())
    }
}

mod tests {

    #[test]
    fn test_ip_parsing() {
        let ipv4 = NetworkUtils::parse_ip("192.168.1.1").expect("Test assertion failed");
        let ipv6 = NetworkUtils::parse_ip("::1").expect("Test assertion failed");

        assert!(matches!(ipv4, IpAddr::V4(_)));
        assert!(matches!(ipv6, IpAddr::V6(_)));
    }

    #[test]
    fn test_port_validation() {
        assert!(NetworkUtils::validate_port(8080).is_ok());
        assert!(NetworkUtils::validate_port(0).is_err());
        assert!(NetworkUtils::validate_port(65535).is_ok());
    }

    #[test]
    fn test_localhost_detection() {
        let localhost_v4 = "127.0.0.1"
            .parse::<IpAddr>()
            .expect("Test assertion failed");
        let localhost_v6 = "::1".parse::<IpAddr>().expect("Test assertion failed");
        let external = "8.8.8.8".parse::<IpAddr>().expect("Test assertion failed");

        assert!(NetworkUtils::is_localhost(&localhost_v4));
        assert!(NetworkUtils::is_localhost(&localhost_v6));
        assert!(!NetworkUtils::is_localhost(&external));
    }

    #[test]
    fn test_private_ip_detection() {
        let private = "192.168.1.1"
            .parse::<IpAddr>()
            .expect("Test assertion failed");
        let public = "8.8.8.8".parse::<IpAddr>().expect("Test assertion failed");

        assert!(NetworkUtils::is_private_ip(&private));
        assert!(!NetworkUtils::is_private_ip(&public));
    }

    #[test]
    fn test_bind_address_normalization() {
        let port_only =
            NetworkUtils::normalize_bind_address("8080").expect("Test assertion failed");
        let full_addr =
            NetworkUtils::normalize_bind_address("127.0.0.1:8080").expect("Test assertion failed");

        assert_eq!(port_only.port(), 8080);
        assert_eq!(full_addr.port(), 8080);
        assert_eq!(full_addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn test_connection_quality() {
        // 로컬호스트 테스트 (실제 서버가 없으면 실패)
        if let Ok(quality) = NetworkUtils::evaluate_connection_quality("127.0.0.1:22").await {
            println!("SSH 연결 품질: {:?}", quality);
        }
    }
}
