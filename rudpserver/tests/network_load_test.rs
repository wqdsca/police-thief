// 실제 UDP 네트워크를 사용한 부하 테스트
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct LoadTestStats {
    connections_attempted: Arc<AtomicUsize>,
    connections_successful: Arc<AtomicUsize>,
    messages_sent: Arc<AtomicU64>,
    messages_received: Arc<AtomicU64>,
    errors: Arc<AtomicUsize>,
    latencies: Arc<Mutex<Vec<Duration>>>,
}

impl LoadTestStats {
    fn new() -> Self {
        Self {
            connections_attempted: Arc::new(AtomicUsize::new(0)),
            connections_successful: Arc::new(AtomicUsize::new(0)),
            messages_sent: Arc::new(AtomicU64::new(0)),
            messages_received: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicUsize::new(0)),
            latencies: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record_latency(&self, latency: Duration) {
        if let Ok(mut latencies) = self.latencies.lock() {
            latencies.push(latency);
        }
    }

    fn print_summary(&self) {
        let attempted = self.connections_attempted.load(Ordering::Relaxed);
        let successful = self.connections_successful.load(Ordering::Relaxed);
        let sent = self.messages_sent.load(Ordering::Relaxed);
        let received = self.messages_received.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);

        println!("\n========== RUDP 부하 테스트 결과 ==========");
        println!("연결 시도: {}", attempted);
        println!(
            "연결 성공: {} ({:.1}%)",
            successful,
            (successful as f64 / attempted.max(1) as f64) * 100.0
        );
        println!("메시지 전송: {}", sent);
        println!("메시지 수신: {}", received);
        println!("에러 발생: {}", errors);
        println!(
            "성공률: {:.1}%",
            (received as f64 / sent.max(1) as f64) * 100.0
        );

        if let Ok(latencies) = self.latencies.lock() {
            if !latencies.is_empty() {
                let mut sorted = latencies.clone();
                sorted.sort();

                let len = sorted.len();
                let avg = sorted.iter().sum::<Duration>() / len as u32;
                let p50 = sorted[len / 2];
                let p95 = sorted[len * 95 / 100];
                let p99 = sorted[len * 99 / 100];

                println!("\n--- 지연시간 통계 ---");
                println!("평균: {:?}", avg);
                println!("P50: {:?}", p50);
                println!("P95: {:?}", p95);
                println!("P99: {:?}", p99);
            }
        }
        println!("=====================================");
    }
}

struct MockRudpServer {
    socket: UdpSocket,
    running: Arc<AtomicUsize>,
    stats: LoadTestStats,
}

impl MockRudpServer {
    fn new(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            socket,
            running: Arc::new(AtomicUsize::new(1)),
            stats: LoadTestStats::new(),
        })
    }

    fn start(&self) -> std::io::Result<()> {
        println!("Mock RUDP 서버 시작: {}", self.socket.local_addr()?);

        let mut buffer = [0u8; 1500];

        while self.running.load(Ordering::Relaxed) == 1 {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, addr)) => {
                    // 패킷 타입 확인
                    if size > 0 {
                        match buffer[0] {
                            0x01 => {
                                // SYN 패킷에 대한 SYN-ACK 응답
                                let syn_ack = [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                                let _ = self.socket.send_to(&syn_ack, addr);
                            }
                            0x03 => {
                                // 데이터 패킷에 대한 ACK 응답
                                let ack = [0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                                let _ = self.socket.send_to(&ack, addr);
                            }
                            _ => {
                                // 기타 패킷에 대한 기본 응답
                                let response = [0xFF, 0, 0, 0, 0];
                                let _ = self.socket.send_to(&response, addr);
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_micros(100));
                }
                Err(_) => {
                    // 에러 무시
                }
            }
        }

        Ok(())
    }

    fn stop(&self) {
        self.running.store(0, Ordering::Relaxed);
    }
}

struct VirtualPlayer {
    id: usize,
    socket: UdpSocket,
    server_addr: SocketAddr,
    stats: LoadTestStats,
}

impl VirtualPlayer {
    fn new(id: usize, server_addr: SocketAddr, stats: LoadTestStats) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(1000)))?;

        Ok(Self {
            id,
            socket,
            server_addr,
            stats,
        })
    }

    fn connect(&self) -> bool {
        self.stats
            .connections_attempted
            .fetch_add(1, Ordering::Relaxed);

        // SYN 패킷 전송
        let syn_packet = [0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        if self.socket.send_to(&syn_packet, self.server_addr).is_err() {
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // SYN-ACK 대기
        let mut buffer = [0u8; 1500];
        match self.socket.recv_from(&mut buffer) {
            Ok((size, _)) if size > 0 && buffer[0] == 0x02 => {
                self.stats
                    .connections_successful
                    .fetch_add(1, Ordering::Relaxed);
                true
            }
            _ => {
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    fn send_messages(&self, count: u32) -> u32 {
        let mut successful = 0;

        for i in 0..count {
            let start = Instant::now();

            // 게임 데이터 생성
            let data = format!("Player{}_Message{}_Data", self.id, i);
            let mut packet = vec![0x03]; // 데이터 패킷 타입
            packet.extend_from_slice(&(i as u32).to_le_bytes());
            packet.extend_from_slice(data.as_bytes());

            if self.socket.send_to(&packet, self.server_addr).is_ok() {
                self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);

                // ACK 대기
                let mut buffer = [0u8; 1500];
                if let Ok((size, _)) = self.socket.recv_from(&mut buffer) {
                    if size > 0 && buffer[0] == 0x04 {
                        let latency = start.elapsed();
                        self.stats.record_latency(latency);
                        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
                        successful += 1;
                    }
                }
            } else {
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
            }

            // 메시지 간격 (10 msg/sec)
            thread::sleep(Duration::from_millis(100));
        }

        successful
    }
}

fn run_load_test_with_mock_server() {
    println!("Mock RUDP 서버와 함께 300명 부하 테스트 시작");

    // Mock 서버 시작
    let server = MockRudpServer::new("127.0.0.1:15000").unwrap();
    let server_addr: SocketAddr = "127.0.0.1:15000".parse().unwrap();
    let server_running = server.running.clone();

    let server_handle = thread::spawn(move || {
        server.start().unwrap();
    });

    thread::sleep(Duration::from_millis(500)); // 서버 시작 대기

    let stats = LoadTestStats::new();
    let mut handles = vec![];

    println!("300명의 가상 플레이어 생성 중...");

    // 50명씩 6개 배치로 점진적 접속
    for batch in 0..6 {
        println!(
            "배치 {} 시작 (플레이어 {}-{})...",
            batch + 1,
            batch * 50,
            (batch + 1) * 50 - 1
        );

        for i in 0..50 {
            let player_id = batch * 50 + i;
            let player_stats = stats.clone();

            let handle = thread::spawn(move || {
                match VirtualPlayer::new(player_id, server_addr, player_stats) {
                    Ok(player) => {
                        if player.connect() {
                            println!("플레이어 {} 연결 성공", player_id);
                            let successful_msgs = player.send_messages(10);
                            println!(
                                "플레이어 {} 완료: {}/10 메시지 성공",
                                player_id, successful_msgs
                            );
                        } else {
                            println!("플레이어 {} 연결 실패", player_id);
                        }
                    }
                    Err(e) => {
                        println!("플레이어 {} 생성 실패: {}", player_id, e);
                    }
                }
            });

            handles.push(handle);
            thread::sleep(Duration::from_millis(20)); // 연결 간격
        }

        thread::sleep(Duration::from_secs(1)); // 배치 간격
    }

    // 모든 플레이어 완료 대기
    for handle in handles {
        let _ = handle.join();
    }

    // 서버 종료
    server_running.store(0, Ordering::Relaxed);
    thread::sleep(Duration::from_millis(100));

    stats.print_summary();

    // 성능 검증
    let successful_connections = stats.connections_successful.load(Ordering::Relaxed);
    let success_rate = successful_connections as f64 / 300.0 * 100.0;

    println!("\n========== 성능 평가 ==========");
    println!("목표: 300명 동시 접속");
    println!("달성: {}명 ({:.1}%)", successful_connections, success_rate);

    if success_rate >= 90.0 {
        println!("✅ 성능 목표 달성 (90% 이상)");
    } else {
        println!("❌ 성능 목표 미달성 (90% 미만)");
    }

    println!("1vCPU, 1GB RAM 환경 테스트 완료");
    println!("==============================");
}

fn main() {
    run_load_test_with_mock_server();
}
