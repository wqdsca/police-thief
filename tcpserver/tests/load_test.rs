//! TCP 서버 부하 테스트
//!
//! 500명 동시 사용자, 50개 방, 방당 10명씩 배치
//! 1초마다 메시지 송수신, 1분간 지속
//! 1vCPU 0.5GB RAM 제한 환경 테스트

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use sysinfo::{ProcessExt, System, SystemExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::{interval, timeout, Duration, Instant};
use tracing::{debug, error, info, warn};

/// 부하 테스트용 메시지
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LoadTestMessage {
    Connect {
        room_id: u32,
        user_id: u32,
    },
    ConnectionAck {
        user_id: u32,
    },
    RoomMessage {
        room_id: u32,
        user_id: u32,
        message: String,
        timestamp: i64,
    },
    HeartBeat,
    HeartBeatResponse {
        timestamp: i64,
    },
    Error {
        code: u16,
        message: String,
    },
}

impl LoadTestMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let json = serde_json::to_string(self)?;
        let data = json.as_bytes();
        let length = data.len() as u32;

        let mut result = Vec::with_capacity(4 + data.len());
        result.extend_from_slice(&length.to_be_bytes());
        result.extend_from_slice(data);

        Ok(result)
    }

    pub async fn read_from_stream(stream: &mut TcpStream) -> Result<Self> {
        let mut length_bytes = [0u8; 4];
        timeout(Duration::from_secs(5), stream.read_exact(&mut length_bytes)).await??;
        let length = u32::from_be_bytes(length_bytes) as usize;

        if length > 10240 {
            // 10KB 제한
            return Err(anyhow!("메시지가 너무 큼: {} bytes", length));
        }

        let mut buffer = vec![0u8; length];
        timeout(Duration::from_secs(5), stream.read_exact(&mut buffer)).await??;

        let json_str = std::str::from_utf8(&buffer)?;
        let message: LoadTestMessage = serde_json::from_str(json_str)?;

        Ok(message)
    }

    pub async fn write_to_stream(&self, stream: &mut TcpStream) -> Result<()> {
        let data = self.to_bytes()?;
        timeout(Duration::from_secs(5), stream.write_all(&data)).await??;
        timeout(Duration::from_secs(5), stream.flush()).await??;
        Ok(())
    }
}

/// 부하 테스트 통계
#[derive(Debug, Default)]
pub struct LoadTestStats {
    pub connected_users: AtomicUsize,
    pub total_messages_sent: AtomicU64,
    pub total_messages_received: AtomicU64,
    pub connection_failures: AtomicU64,
    pub message_failures: AtomicU64,
    pub avg_latency_ms: AtomicU64,
    pub max_latency_ms: AtomicU64,
    pub min_latency_ms: AtomicU64,
    pub heartbeat_responses: AtomicU64,
    pub room_message_responses: AtomicU64,
}

impl LoadTestStats {
    pub fn record_message_latency(&self, latency_ms: u64) {
        // 평균 계산을 위한 간단한 이동 평균
        let current_avg = self.avg_latency_ms.load(Ordering::Relaxed);
        let new_avg = (current_avg * 9 + latency_ms) / 10;
        self.avg_latency_ms.store(new_avg, Ordering::Relaxed);

        // 최대/최소 업데이트
        let current_max = self.max_latency_ms.load(Ordering::Relaxed);
        if latency_ms > current_max {
            self.max_latency_ms.store(latency_ms, Ordering::Relaxed);
        }

        let current_min = self.min_latency_ms.load(Ordering::Relaxed);
        if current_min == 0 || latency_ms < current_min {
            self.min_latency_ms.store(latency_ms, Ordering::Relaxed);
        }
    }

    pub fn print_summary(&self) {
        let connected = self.connected_users.load(Ordering::Relaxed);
        let sent = self.total_messages_sent.load(Ordering::Relaxed);
        let received = self.total_messages_received.load(Ordering::Relaxed);
        let conn_failures = self.connection_failures.load(Ordering::Relaxed);
        let msg_failures = self.message_failures.load(Ordering::Relaxed);
        let avg_lat = self.avg_latency_ms.load(Ordering::Relaxed);
        let max_lat = self.max_latency_ms.load(Ordering::Relaxed);
        let min_lat = self.min_latency_ms.load(Ordering::Relaxed);
        let hb_responses = self.heartbeat_responses.load(Ordering::Relaxed);
        let room_responses = self.room_message_responses.load(Ordering::Relaxed);

        println!("📊 부하 테스트 결과 요약");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔗 연결된 사용자: {} / 500", connected);
        println!("📤 총 전송 메시지: {}", sent);
        println!("📥 총 수신 메시지: {}", received);
        println!("❌ 연결 실패: {}", conn_failures);
        println!("⚠️ 메시지 실패: {}", msg_failures);
        println!("⏱️ 평균 지연시간: {} ms", avg_lat);
        println!("⏱️ 최대 지연시간: {} ms", max_lat);
        println!("⏱️ 최소 지연시간: {} ms", min_lat);
        println!("💓 하트비트 응답: {}", hb_responses);
        println!("💬 방 메시지 응답: {}", room_responses);

        // 성공률 계산
        let connection_success_rate = if connected + conn_failures as usize > 0 {
            (connected as f64 / (connected + conn_failures as usize) as f64) * 100.0
        } else {
            0.0
        };

        let message_success_rate = if sent > 0 {
            (received as f64 / sent as f64) * 100.0
        } else {
            0.0
        };

        println!("📈 연결 성공률: {:.1}%", connection_success_rate);
        println!("📈 메시지 성공률: {:.1}%", message_success_rate);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

/// 시스템 리소스 모니터
pub struct ResourceMonitor {
    system: Arc<Mutex<System>>,
    cpu_samples: Arc<Mutex<Vec<f32>>>,
    memory_samples: Arc<Mutex<Vec<u64>>>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(Mutex::new(system)),
            cpu_samples: Arc::new(Mutex::new(Vec::new())),
            memory_samples: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn start_monitoring(&self, duration: Duration) -> tokio::task::JoinHandle<()> {
        let system = self.system.clone();
        let cpu_samples = self.cpu_samples.clone();
        let memory_samples = self.memory_samples.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(2));
            let start = Instant::now();

            while start.elapsed() < duration {
                interval.tick().await;

                let mut sys = system.lock().await;
                sys.refresh_all();

                // CPU 사용률 (전체 시스템)
                let cpu_usage = sys.global_cpu_info().cpu_usage();
                cpu_samples.lock().await.push(cpu_usage);

                // 메모리 사용률
                let memory_usage = sys.used_memory();
                memory_samples.lock().await.push(memory_usage);

                debug!(
                    "리소스 모니터링: CPU {:.1}%, 메모리 {} KB",
                    cpu_usage, memory_usage
                );
            }
        })
    }

    pub async fn get_summary(&self) -> (f32, f32, u64, u64) {
        let cpu_samples = self.cpu_samples.lock().await;
        let memory_samples = self.memory_samples.lock().await;

        let avg_cpu = if !cpu_samples.is_empty() {
            cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
        } else {
            0.0
        };

        let max_cpu = cpu_samples.iter().fold(0.0f32, |a, &b| a.max(b));

        let avg_memory = if !memory_samples.is_empty() {
            memory_samples.iter().sum::<u64>() / memory_samples.len() as u64
        } else {
            0
        };

        let max_memory = memory_samples.iter().fold(0u64, |a, &b| a.max(b));

        (avg_cpu, max_cpu, avg_memory / 1024, max_memory / 1024) // KB 변환
    }
}

/// 가상 사용자 클라이언트
pub struct VirtualUser {
    pub user_id: u32,
    pub room_id: u32,
    pub stream: Option<TcpStream>,
    pub stats: Arc<LoadTestStats>,
    pub connected: Arc<AtomicUsize>,
}

impl VirtualUser {
    pub fn new(
        user_id: u32,
        room_id: u32,
        stats: Arc<LoadTestStats>,
        connected: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            user_id,
            room_id,
            stream: None,
            stats,
            connected,
        }
    }

    /// 서버 연결
    pub async fn connect(&mut self, server_addr: &str) -> Result<()> {
        match timeout(Duration::from_secs(10), TcpStream::connect(server_addr)).await {
            Ok(Ok(stream)) => {
                self.stream = Some(stream);

                // Connect 메시지 전송
                let connect_msg = LoadTestMessage::Connect {
                    room_id: self.room_id,
                    user_id: self.user_id,
                };

                if let Some(ref mut stream) = self.stream {
                    connect_msg.write_to_stream(stream).await?;

                    // ConnectionAck 대기
                    match LoadTestMessage::read_from_stream(stream).await {
                        Ok(LoadTestMessage::ConnectionAck { user_id }) => {
                            if user_id == self.user_id {
                                self.connected.fetch_add(1, Ordering::Relaxed);
                                self.stats.connected_users.fetch_add(1, Ordering::Relaxed);
                                debug!("사용자 {} 연결 성공 (방 {})", self.user_id, self.room_id);
                                return Ok(());
                            }
                        }
                        Ok(LoadTestMessage::Error { code, message }) => {
                            return Err(anyhow!("연결 에러: {} - {}", code, message));
                        }
                        _ => {
                            return Err(anyhow!("예상하지 못한 응답"));
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                self.stats
                    .connection_failures
                    .fetch_add(1, Ordering::Relaxed);
                return Err(anyhow!("TCP 연결 실패: {}", e));
            }
            Err(_) => {
                self.stats
                    .connection_failures
                    .fetch_add(1, Ordering::Relaxed);
                return Err(anyhow!("연결 타임아웃"));
            }
        }

        Err(anyhow!("연결 실패"))
    }

    /// 주기적 메시지 전송 (1초마다)
    pub async fn start_messaging(&mut self, duration: Duration) -> Result<()> {
        if self.stream.is_none() {
            return Err(anyhow!("연결되지 않음"));
        }

        let mut interval = interval(Duration::from_secs(1));
        let start = Instant::now();
        let mut message_counter = 0u32;

        while start.elapsed() < duration {
            interval.tick().await;

            if let Some(ref mut stream) = self.stream {
                let send_start = Instant::now();

                // 방 메시지 전송
                let room_msg = LoadTestMessage::RoomMessage {
                    room_id: self.room_id,
                    user_id: self.user_id,
                    message: format!(
                        "Hello from user {} - message {}",
                        self.user_id, message_counter
                    ),
                    timestamp: chrono::Utc::now().timestamp(),
                };

                match room_msg.write_to_stream(stream).await {
                    Ok(_) => {
                        self.stats
                            .total_messages_sent
                            .fetch_add(1, Ordering::Relaxed);
                        message_counter += 1;

                        // 응답 대기 (비동기적으로)
                        if let Ok(response) = timeout(
                            Duration::from_millis(500),
                            LoadTestMessage::read_from_stream(stream),
                        )
                        .await
                        {
                            match response {
                                Ok(_) => {
                                    let latency = send_start.elapsed().as_millis() as u64;
                                    self.stats.record_message_latency(latency);
                                    self.stats
                                        .total_messages_received
                                        .fetch_add(1, Ordering::Relaxed);
                                    self.stats
                                        .room_message_responses
                                        .fetch_add(1, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    self.stats.message_failures.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        self.stats.message_failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        Ok(())
    }

    /// 하트비트 전송
    pub async fn send_heartbeat(&mut self) -> Result<()> {
        if let Some(ref mut stream) = self.stream {
            let heartbeat = LoadTestMessage::HeartBeat;
            heartbeat.write_to_stream(stream).await?;

            // 하트비트 응답 대기
            match timeout(
                Duration::from_secs(5),
                LoadTestMessage::read_from_stream(stream),
            )
            .await
            {
                Ok(Ok(LoadTestMessage::HeartBeatResponse { .. })) => {
                    self.stats
                        .heartbeat_responses
                        .fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
                _ => Err(anyhow!("하트비트 응답 없음")),
            }
        } else {
            Err(anyhow!("연결되지 않음"))
        }
    }
}

/// 부하 테스트 실행기
pub struct LoadTester {
    pub total_users: usize,
    pub total_rooms: usize,
    pub users_per_room: usize,
    pub test_duration: Duration,
    pub server_address: String,
    pub stats: Arc<LoadTestStats>,
    pub resource_monitor: ResourceMonitor,
}

impl LoadTester {
    pub fn new(total_users: usize, total_rooms: usize, test_duration: Duration) -> Self {
        Self {
            total_users,
            total_rooms,
            users_per_room: total_users / total_rooms,
            test_duration,
            server_address: "127.0.0.1:4000".to_string(),
            stats: Arc::new(LoadTestStats::default()),
            resource_monitor: ResourceMonitor::new(),
        }
    }

    pub async fn run_load_test(&self) -> Result<()> {
        info!("🚀 부하 테스트 시작");
        info!("   총 사용자: {} 명", self.total_users);
        info!("   총 방 수: {} 개", self.total_rooms);
        info!("   방당 사용자: {} 명", self.users_per_room);
        info!("   테스트 지속시간: {} 초", self.test_duration.as_secs());
        info!("   서버 주소: {}", self.server_address);

        // 리소스 모니터링 시작
        let monitor_handle = self
            .resource_monitor
            .start_monitoring(self.test_duration + Duration::from_secs(10))
            .await;

        // 연결 제한 세마포어 (동시 연결 수 제한)
        let semaphore = Arc::new(Semaphore::new(100));
        let connected_counter = Arc::new(AtomicUsize::new(0));

        // 가상 사용자들 생성
        let mut user_handles = Vec::new();

        for room_id in 1..=self.total_rooms as u32 {
            for user_index in 0..self.users_per_room {
                let user_id = (room_id - 1) * self.users_per_room as u32 + user_index as u32 + 1;

                let semaphore = semaphore.clone();
                let stats = self.stats.clone();
                let connected = connected_counter.clone();
                let server_addr = self.server_address.clone();
                let test_duration = self.test_duration;

                let handle = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.expect("Test assertion failed");

                    let mut virtual_user = VirtualUser::new(user_id, room_id, stats, connected);

                    // 연결 시도
                    match virtual_user.connect(&server_addr).await {
                        Ok(_) => {
                            // 연결 성공 후 메시징 시작
                            if let Err(e) = virtual_user.start_messaging(test_duration).await {
                                warn!("사용자 {} 메시징 실패: {}", user_id, e);
                            }
                        }
                        Err(e) => {
                            warn!("사용자 {} 연결 실패: {}", user_id, e);
                        }
                    }
                });

                user_handles.push(handle);

                // 연결 속도 조절 (서버 과부하 방지)
                if user_handles.len() % 10 == 0 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        info!("⏳ {} 초간 테스트 진행 중...", self.test_duration.as_secs());

        // 중간 상태 보고
        let stats_handle = {
            let stats = self.stats.clone();
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(10));
                let start = Instant::now();

                while start.elapsed() < test_duration {
                    interval.tick().await;
                    let connected = stats.connected_users.load(Ordering::Relaxed);
                    let sent = stats.total_messages_sent.load(Ordering::Relaxed);
                    let received = stats.total_messages_received.load(Ordering::Relaxed);
                    let failures = stats.message_failures.load(Ordering::Relaxed);

                    info!(
                        "📊 중간 상태: 연결 {}, 전송 {}, 수신 {}, 실패 {}",
                        connected, sent, received, failures
                    );
                }
            })
        };

        // 모든 사용자 태스크 완료 대기
        for handle in user_handles {
            let _ = handle.await;
        }

        stats_handle.abort();
        monitor_handle.abort();

        // 최종 결과 출력
        self.print_final_results().await;

        Ok(())
    }

    async fn print_final_results(&self) {
        // 통계 요약 출력
        self.stats.print_summary();

        // 리소스 사용량 요약
        let (avg_cpu, max_cpu, avg_memory_kb, max_memory_kb) =
            self.resource_monitor.get_summary().await;

        println!("💻 시스템 리소스 사용량");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔥 평균 CPU: {:.1}%", avg_cpu);
        println!("🔥 최대 CPU: {:.1}%", max_cpu);
        println!("💾 평균 메모리: {} MB", avg_memory_kb / 1024);
        println!("💾 최대 메모리: {} MB", max_memory_kb / 1024);

        // 리소스 제한 환경 평가
        let memory_limit_mb = 512; // 0.5GB
        let memory_usage_percent = (max_memory_kb / 1024) as f32 / memory_limit_mb as f32 * 100.0;

        println!("📊 제한 환경 평가 (1vCPU, 0.5GB RAM)");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔥 CPU 사용률: {:.1}% / 100% (1vCPU)", max_cpu);
        println!(
            "💾 메모리 사용률: {:.1}% / 100% (512MB)",
            memory_usage_percent
        );

        // 성능 등급 평가
        let performance_grade = if max_cpu < 80.0 && memory_usage_percent < 80.0 {
            "A (우수)"
        } else if max_cpu < 90.0 && memory_usage_percent < 90.0 {
            "B (양호)"
        } else {
            "C (주의)"
        };

        println!("🏆 종합 성능 등급: {}", performance_grade);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

#[tokio::test]
#[ignore = "대규모 부하 테스트 - 수동 실행 필요"]
async fn test_500_users_50_rooms_load() -> Result<()> {
    // 로깅 설정
    let _ = tracing_subscriber::fmt::try_init();

    // 부하 테스트 설정
    let total_users = 500;
    let total_rooms = 50;
    let test_duration = Duration::from_secs(60); // 1분

    let load_tester = LoadTester::new(total_users, total_rooms, test_duration);

    info!("🎯 TCP 서버 부하 테스트 시작");
    info!("   시나리오: 500명, 50개 방, 1분간 1초마다 메시지 전송");
    info!("   환경: 1vCPU, 0.5GB RAM 제한");

    // 서버 연결 확인
    match TcpStream::connect(&load_tester.server_address).await {
        Ok(_) => {
            info!("✅ TCP 서버 연결 확인: {}", load_tester.server_address);
        }
        Err(e) => {
            error!("❌ TCP 서버에 연결할 수 없습니다: {}", e);
            println!("💡 tcpserver를 먼저 실행해주세요:");
            println!("   cd tcpserver && cargo run --bin tcpserver");
            return Ok(());
        }
    }

    // 부하 테스트 실행
    load_tester.run_load_test().await?;

    Ok(())
}

#[tokio::test]
async fn test_small_scale_load() -> Result<()> {
    // 작은 규모 테스트 (개발용)
    let _ = tracing_subscriber::fmt::try_init();

    let total_users = 20;
    let total_rooms = 5;
    let test_duration = Duration::from_secs(10);

    let load_tester = LoadTester::new(total_users, total_rooms, test_duration);

    info!("🧪 소규모 부하 테스트 시작 (개발용)");

    // 서버 연결 확인
    if TcpStream::connect(&load_tester.server_address)
        .await
        .is_err()
    {
        println!("TCP 서버가 실행되지 않아 테스트를 건너뜁니다");
        return Ok(());
    }

    load_tester.run_load_test().await?;
    Ok(())
}

#[tokio::test]
async fn test_connection_stress() -> Result<()> {
    // 연결 스트레스 테스트
    let _ = tracing_subscriber::fmt::try_init();

    info!("🔗 연결 스트레스 테스트");

    let server_addr = "127.0.0.1:4000";

    // 서버 연결 확인
    if TcpStream::connect(server_addr).await.is_err() {
        println!("TCP 서버가 실행되지 않아 테스트를 건너뜁니다");
        return Ok(());
    }

    let stats = Arc::new(LoadTestStats::default());
    let connected = Arc::new(AtomicUsize::new(0));
    let semaphore = Arc::new(Semaphore::new(50));

    let mut handles = Vec::new();

    // 100개 동시 연결 시도
    for i in 1..=100 {
        let semaphore = semaphore.clone();
        let stats = stats.clone();
        let connected = connected.clone();
        let server_addr = server_addr.to_string();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.expect("Test assertion failed");

            let mut virtual_user = VirtualUser::new(i, (i % 10) + 1, stats, connected);

            match virtual_user.connect(&server_addr).await {
                Ok(_) => {
                    // 간단한 하트비트 테스트
                    let _ = virtual_user.send_heartbeat().await;
                }
                Err(_) => {}
            }
        });

        handles.push(handle);
    }

    // 모든 연결 완료 대기
    for handle in handles {
        let _ = handle.await;
    }

    let final_connected = stats.connected_users.load(Ordering::Relaxed);
    let connection_failures = stats.connection_failures.load(Ordering::Relaxed);

    info!(
        "연결 테스트 결과: 성공 {}, 실패 {}",
        final_connected, connection_failures
    );

    assert!(final_connected > 50, "50개 이상 연결이 성공해야 함");

    Ok(())
}
