//! RUDP Server Performance Benchmark
//!
//! RUDP 서버 성능을 측정하고 tcpserver와 비교하는 벤치마크 도구입니다.

use anyhow::Result;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::interval;
use tracing::info;

/// 벤치마크 설정
struct BenchmarkConfig {
    /// 서버 주소
    server_addr: String,
    /// 동시 클라이언트 수
    concurrent_clients: usize,
    /// 테스트 지속 시간 (초)
    duration_secs: u64,
    /// 초당 메시지 전송률 (클라이언트당)
    messages_per_second: u32,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000".to_string(),
            concurrent_clients: 100,
            duration_secs: 30,
            messages_per_second: 10,
        }
    }
}

/// 벤치마크 결과
#[derive(Debug)]
struct BenchmarkResult {
    /// 총 전송 메시지 수
    total_sent: u64,
    /// 총 수신 메시지 수
    total_received: u64,
    /// 평균 응답 시간 (마이크로초)
    avg_latency_us: u64,
    /// 최대 응답 시간 (마이크로초)
    max_latency_us: u64,
    /// 초당 메시지 처리량
    messages_per_second: f64,
    /// 패킷 손실률 (%)
    packet_loss_rate: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt::init();

    info!("🚀 RUDP Server Performance Benchmark");
    info!("🎯 목표 성능:");
    info!("   - 처리량: 20,000+ msg/sec (tcpserver: 12,991+)");
    info!("   - 지연시간: <500μs (tcpserver: ~1000μs)");
    info!("   - 메모리: 8-10MB (tcpserver: 11MB)");
    info!("   - 동시 연결: 1,000+ (tcpserver: 500+)");

    let config = BenchmarkConfig::default();
    let result = run_benchmark(&config).await?;

    print_results(&result, &config);

    Ok(())
}

/// 벤치마크를 실행합니다.
async fn run_benchmark(config: &BenchmarkConfig) -> Result<BenchmarkResult> {
    info!("📊 벤치마크 시작:");
    info!("   - 서버: {}", config.server_addr);
    info!("   - 클라이언트: {}개", config.concurrent_clients);
    info!("   - 지속시간: {}초", config.duration_secs);
    info!(
        "   - 메시지 전송률: {}msg/s (클라이언트당)",
        config.messages_per_second
    );

    let mut handles = Vec::new();
    let start_time = Instant::now();

    // 동시 클라이언트 실행
    for client_id in 0..config.concurrent_clients {
        let server_addr = config.server_addr.clone();
        let duration = Duration::from_secs(config.duration_secs);
        let msg_rate = config.messages_per_second;

        let handle =
            tokio::spawn(
                async move { run_client(client_id, &server_addr, duration, msg_rate).await },
            );

        handles.push(handle);
    }

    // 모든 클라이언트 완료 대기
    let mut total_sent = 0u64;
    let mut total_received = 0u64;
    let mut total_latency_us = 0u64;
    let mut max_latency_us = 0u64;

    for handle in handles {
        let (sent, received, avg_lat, max_lat) = handle.await??;
        total_sent += sent;
        total_received += received;
        total_latency_us += avg_lat * received;
        max_latency_us = max_latency_us.max(max_lat);
    }

    let elapsed = start_time.elapsed();
    let avg_latency_us = if total_received > 0 {
        total_latency_us / total_received
    } else {
        0
    };

    let messages_per_second = total_received as f64 / elapsed.as_secs_f64();
    let packet_loss_rate = if total_sent > 0 {
        ((total_sent - total_received) as f64 / total_sent as f64) * 100.0
    } else {
        0.0
    };

    Ok(BenchmarkResult {
        total_sent,
        total_received,
        avg_latency_us,
        max_latency_us,
        messages_per_second,
        packet_loss_rate,
    })
}

/// 개별 클라이언트를 실행합니다.
async fn run_client(
    client_id: usize,
    server_addr: &str,
    duration: Duration,
    messages_per_second: u32,
) -> Result<(u64, u64, u64, u64)> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(server_addr).await?;

    let mut sent = 0u64;
    let mut received = 0u64;
    let mut total_latency_us = 0u64;
    let mut max_latency_us = 0u64;

    let start_time = Instant::now();
    let mut interval = interval(Duration::from_millis(1000 / messages_per_second as u64));
    let message = format!("benchmark_client_{}_message", client_id);
    let mut buffer = vec![0u8; 1024];

    while start_time.elapsed() < duration {
        interval.tick().await;

        let send_time = Instant::now();

        // 메시지 전송
        if socket.send(message.as_bytes()).await.is_ok() {
            sent += 1;

            // 응답 대기 (타임아웃 설정)
            tokio::select! {
                result = socket.recv(&mut buffer) => {
                    if result.is_ok() {
                        let latency_us = send_time.elapsed().as_micros() as u64;
                        received += 1;
                        total_latency_us += latency_us;
                        max_latency_us = max_latency_us.max(latency_us);
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // 타임아웃 - 패킷 손실로 간주
                }
            }
        }
    }

    let avg_latency_us = if received > 0 {
        total_latency_us / received
    } else {
        0
    };

    Ok((sent, received, avg_latency_us, max_latency_us))
}

/// 벤치마크 결과를 출력합니다.
fn print_results(result: &BenchmarkResult, config: &BenchmarkConfig) {
    info!("📈 벤치마크 결과:");
    info!("════════════════════════════════════════════════");
    info!("📊 처리량 성능:");
    info!("   • 총 전송 메시지: {}", result.total_sent);
    info!("   • 총 수신 메시지: {}", result.total_received);
    info!(
        "   • 초당 처리량: {:.0} msg/sec",
        result.messages_per_second
    );
    info!(
        "   • 목표 달성률: {:.1}% (목표: 20,000 msg/sec)",
        (result.messages_per_second / 20000.0) * 100.0
    );

    info!("⚡ 지연시간 성능:");
    info!(
        "   • 평균 응답시간: {}μs ({:.3}ms)",
        result.avg_latency_us,
        result.avg_latency_us as f64 / 1000.0
    );
    info!(
        "   • 최대 응답시간: {}μs ({:.3}ms)",
        result.max_latency_us,
        result.max_latency_us as f64 / 1000.0
    );
    info!(
        "   • 목표 달성: {} (목표: <500μs)",
        if result.avg_latency_us < 500 {
            "✅ 성공"
        } else {
            "❌ 미달성"
        }
    );

    info!("🔗 연결 성능:");
    info!("   • 동시 클라이언트: {}", config.concurrent_clients);
    info!("   • 패킷 손실률: {:.2}%", result.packet_loss_rate);
    info!("   • 성공률: {:.2}%", 100.0 - result.packet_loss_rate);

    info!("🏆 tcpserver 대비 성능:");
    info!(
        "   • 처리량 비교: {:.1}% (tcpserver: 12,991 msg/sec)",
        (result.messages_per_second / 12991.0) * 100.0
    );
    info!(
        "   • 지연시간 비교: {:.1}% (tcpserver: ~1000μs)",
        (result.avg_latency_us as f64 / 1000.0) * 100.0
    );

    // 최종 평가
    let performance_score = calculate_performance_score(result);
    info!("📊 종합 성능 점수: {:.1}/100", performance_score);

    if performance_score >= 80.0 {
        info!("🎉 우수한 성능! 목표 달성");
    } else if performance_score >= 60.0 {
        info!("⚠️  양호한 성능, 일부 최적화 필요");
    } else {
        info!("❌ 성능 개선 필요");
    }
}

/// 성능 점수를 계산합니다 (0-100점).
fn calculate_performance_score(result: &BenchmarkResult) -> f64 {
    let throughput_score = (result.messages_per_second / 20000.0 * 40.0).min(40.0);
    let latency_score = if result.avg_latency_us < 500 {
        30.0
    } else {
        (1000.0 - result.avg_latency_us as f64).max(0.0) / 1000.0 * 30.0
    };
    let reliability_score = (100.0 - result.packet_loss_rate) / 100.0 * 30.0;

    throughput_score + latency_score + reliability_score
}
