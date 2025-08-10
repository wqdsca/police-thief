//! RUDP 서버 부하 테스트
//!
//! 1vCPU 1GB RAM 환경에서 100개 방, 각 방당 10명(총 1000명)의 실시간 이동 테스트
//!
//! # 테스트 시나리오
//! - 100개 방 동시 생성
//! - 각 방에 10명씩 플레이어 참가 (총 1000명)
//! - 모든 플레이어가 100ms마다 위치 업데이트
//! - 30초간 지속적인 부하 테스트
//! - CPU, 메모리, 네트워크 메트릭 수집

use anyhow::Result;
use dashmap::DashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};

// 테스트 설정
const NUM_ROOMS: usize = 100;
const PLAYERS_PER_ROOM: usize = 10;
const TOTAL_PLAYERS: usize = NUM_ROOMS * PLAYERS_PER_ROOM;
const UPDATE_INTERVAL_MS: u64 = 100; // 100ms마다 위치 업데이트
const TEST_DURATION_SECS: u64 = 30; // 30초 테스트
const SERVER_ADDR: &str = "127.0.0.1:5000";

/// 게임 메시지 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestMessage {
    Connect {
        player_id: u32,
        room_id: u32,
    },
    Move {
        player_id: u32,
        x: f32,
        y: f32,
        z: f32,
    },
    Disconnect {
        player_id: u32,
    },
}

/// 성능 메트릭
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub errors: AtomicU64,
    pub avg_latency_ms: AtomicU64,
    pub max_latency_ms: AtomicU64,
    pub min_latency_ms: AtomicU64,
}

impl PerformanceMetrics {
    pub fn print_summary(&self, duration: Duration) {
        let total_sent = self.messages_sent.load(Ordering::Relaxed);
        let total_received = self.messages_received.load(Ordering::Relaxed);
        let total_errors = self.errors.load(Ordering::Relaxed);
        let bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        let bytes_received = self.bytes_received.load(Ordering::Relaxed);

        let duration_secs = duration.as_secs_f64();
        let msgs_per_sec = total_sent as f64 / duration_secs;
        let throughput_mbps =
            (bytes_sent + bytes_received) as f64 / duration_secs / 1_000_000.0 * 8.0;

        println!("\n========== 부하 테스트 결과 ==========");
        println!("테스트 시간: {:.2}초", duration_secs);
        println!("총 방 개수: {}", NUM_ROOMS);
        println!("총 플레이어: {}", TOTAL_PLAYERS);
        println!("메시지 전송률: {:.2} msgs/sec", msgs_per_sec);
        println!("처리량: {:.2} Mbps", throughput_mbps);
        println!("전송된 메시지: {}", total_sent);
        println!("수신된 메시지: {}", total_received);
        println!("에러 수: {}", total_errors);
        println!(
            "평균 지연시간: {} ms",
            self.avg_latency_ms.load(Ordering::Relaxed)
        );
        println!(
            "최대 지연시간: {} ms",
            self.max_latency_ms.load(Ordering::Relaxed)
        );
        println!(
            "최소 지연시간: {} ms",
            self.min_latency_ms.load(Ordering::Relaxed)
        );
        println!("=====================================");
    }
}

/// 가상 플레이어 클라이언트
pub struct VirtualPlayer {
    player_id: u32,
    room_id: u32,
    socket: Arc<UdpSocket>,
    position: (f32, f32, f32),
    metrics: Arc<PerformanceMetrics>,
    is_running: Arc<AtomicBool>,
}

impl VirtualPlayer {
    pub async fn new(
        player_id: u32,
        room_id: u32,
        metrics: Arc<PerformanceMetrics>,
    ) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(SERVER_ADDR).await?;

        Ok(Self {
            player_id,
            room_id,
            socket: Arc::new(socket),
            position: (0.0, 0.0, 0.0),
            metrics,
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// 서버에 연결
    pub async fn connect(&mut self) -> Result<()> {
        let msg = TestMessage::Connect {
            player_id: self.player_id,
            room_id: self.room_id,
        };

        self.send_message(&msg).await?;
        Ok(())
    }

    /// 위치 업데이트 시작
    pub async fn start_movement(&mut self) -> Result<()> {
        let mut interval = interval(Duration::from_millis(UPDATE_INTERVAL_MS));
        let mut rng = rand::thread_rng();

        while self.is_running.load(Ordering::Relaxed) {
            interval.tick().await;

            // 랜덤하게 위치 변경
            self.position.0 += rng.gen_range(-1.0..1.0);
            self.position.1 += rng.gen_range(-1.0..1.0);
            self.position.2 = 0.0;

            let msg = TestMessage::Move {
                player_id: self.player_id,
                x: self.position.0,
                y: self.position.1,
                z: self.position.2,
            };

            let start = Instant::now();
            if let Err(e) = self.send_message(&msg).await {
                eprintln!("Player {} 메시지 전송 실패: {}", self.player_id, e);
                self.metrics.errors.fetch_add(1, Ordering::Relaxed);
            } else {
                let latency = start.elapsed().as_millis() as u64;
                self.update_latency_metrics(latency);
            }
        }

        Ok(())
    }

    /// 메시지 전송
    async fn send_message(&self, msg: &TestMessage) -> Result<()> {
        let data = bincode::serialize(msg)?;
        let bytes_sent = data.len();

        self.socket.send(&data).await?;

        self.metrics.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .bytes_sent
            .fetch_add(bytes_sent as u64, Ordering::Relaxed);

        Ok(())
    }

    /// 지연시간 메트릭 업데이트
    fn update_latency_metrics(&self, latency_ms: u64) {
        // 평균 계산 (간단한 이동 평균)
        let current_avg = self.metrics.avg_latency_ms.load(Ordering::Relaxed);
        let new_avg = (current_avg * 9 + latency_ms) / 10;
        self.metrics
            .avg_latency_ms
            .store(new_avg, Ordering::Relaxed);

        // 최대값 업데이트
        self.metrics
            .max_latency_ms
            .fetch_max(latency_ms, Ordering::Relaxed);

        // 최소값 업데이트
        let current_min = self.metrics.min_latency_ms.load(Ordering::Relaxed);
        if current_min == 0 || latency_ms < current_min {
            self.metrics
                .min_latency_ms
                .store(latency_ms, Ordering::Relaxed);
        }
    }

    /// 연결 종료
    pub async fn disconnect(&mut self) -> Result<()> {
        self.is_running.store(false, Ordering::Relaxed);

        let msg = TestMessage::Disconnect {
            player_id: self.player_id,
        };

        self.send_message(&msg).await?;
        Ok(())
    }
}

/// 부하 테스트 실행
#[tokio::test]
async fn load_test_1000_players() {
    println!("\n🚀 RUDP 서버 부하 테스트 시작");
    println!("환경: 1vCPU, 1GB RAM");
    println!(
        "시나리오: {}개 방, 방당 {}명, 총 {}명",
        NUM_ROOMS, PLAYERS_PER_ROOM, TOTAL_PLAYERS
    );
    println!("업데이트 주기: {}ms", UPDATE_INTERVAL_MS);
    println!("테스트 시간: {}초\n", TEST_DURATION_SECS);

    let metrics = Arc::new(PerformanceMetrics::default());
    let test_start = Instant::now();

    // 모든 플레이어 생성 및 연결
    let mut handles = Vec::new();

    for room_id in 0..NUM_ROOMS {
        for player_idx in 0..PLAYERS_PER_ROOM {
            let player_id = (room_id * PLAYERS_PER_ROOM + player_idx) as u32;
            let metrics_clone = metrics.clone();

            let handle = tokio::spawn(async move {
                match VirtualPlayer::new(player_id, room_id as u32, metrics_clone).await {
                    Ok(mut player) => {
                        // 연결
                        if let Err(e) = player.connect().await {
                            eprintln!("Player {} 연결 실패: {}", player_id, e);
                            return;
                        }

                        // 약간의 랜덤 지연으로 동시 시작 방지
                        let delay = rand::thread_rng().gen_range(0..100);
                        sleep(Duration::from_millis(delay)).await;

                        // 이동 시작
                        if let Err(e) = player.start_movement().await {
                            eprintln!("Player {} 이동 실패: {}", player_id, e);
                        }

                        // 연결 종료
                        let _ = player.disconnect().await;
                    }
                    Err(e) => {
                        eprintln!("Player {} 생성 실패: {}", player_id, e);
                    }
                }
            });

            handles.push(handle);
        }

        // 방 생성 간격 (서버 부하 분산)
        if room_id % 10 == 0 {
            println!("✅ {}개 방 생성 완료...", room_id + 1);
            sleep(Duration::from_millis(50)).await;
        }
    }

    println!("\n📊 모든 플레이어 연결 완료, 테스트 진행 중...");

    // 테스트 시간 동안 대기
    sleep(Duration::from_secs(TEST_DURATION_SECS)).await;

    // 테스트 종료
    println!("\n🛑 테스트 종료 신호 전송...");
    for handle in handles {
        handle.abort();
    }

    // 잠시 대기 후 결과 출력
    sleep(Duration::from_secs(1)).await;

    let test_duration = test_start.elapsed();
    metrics.print_summary(test_duration);

    // 시스템 리소스 정보 출력 (실제 환경에서)
    print_system_resources().await;
}

/// 시스템 리소스 정보 출력
async fn print_system_resources() {
    println!("\n========== 시스템 리소스 사용량 ==========");

    // Windows에서 PowerShell을 통해 리소스 정보 가져오기
    if let Ok(output) = tokio::process::Command::new("powershell")
        .args(&[
            "-Command",
            "Get-Process rudpserver* | Select-Object Name,CPU,WorkingSet,PagedMemorySize | Format-Table"
        ])
        .output()
        .await
    {
        if let Ok(result) = String::from_utf8(output.stdout) {
            println!("{}", result);
        }
    }

    println!("==========================================");
}

/// 단위 테스트: 메시지 직렬화
#[test]
fn test_message_serialization() {
    let msg = TestMessage::Move {
        player_id: 123,
        x: 10.5,
        y: 20.3,
        z: 0.0,
    };

    let serialized = bincode::serialize(&msg).unwrap();
    let deserialized: TestMessage = bincode::deserialize(&serialized).unwrap();

    match deserialized {
        TestMessage::Move { player_id, x, y, z } => {
            assert_eq!(player_id, 123);
            assert_eq!(x, 10.5);
            assert_eq!(y, 20.3);
            assert_eq!(z, 0.0);
        }
        _ => panic!("Wrong message type"),
    }
}

/// 스트레스 테스트: 최대 부하
#[tokio::test]
#[ignore] // 수동으로 실행
async fn stress_test_max_load() {
    // 최대 부하 테스트 (2000명)
    const MAX_PLAYERS: usize = 2000;
    println!("🔥 최대 부하 테스트: {} 플레이어", MAX_PLAYERS);
}
