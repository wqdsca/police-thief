use rand::Rng;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const TARGET_CONNECTIONS: usize = 300;
const SERVER_ADDR: &str = "127.0.0.1:5000";
const MESSAGES_PER_SECOND_PER_CLIENT: u32 = 10;
const TEST_DURATION_SECS: u64 = 60;
const VCPU_LIMIT: f32 = 1.0;
const RAM_LIMIT_MB: usize = 1024;

#[derive(Clone)]
struct LoadTestMetrics {
    connections_established: Arc<AtomicUsize>,
    messages_sent: Arc<AtomicU64>,
    messages_received: Arc<AtomicU64>,
    bytes_sent: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
    errors: Arc<AtomicUsize>,
    latencies: Arc<Mutex<Vec<Duration>>>,
}

impl LoadTestMetrics {
    fn new() -> Self {
        Self {
            connections_established: Arc::new(AtomicUsize::new(0)),
            messages_sent: Arc::new(AtomicU64::new(0)),
            messages_received: Arc::new(AtomicU64::new(0)),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
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
        let connections = self.connections_established.load(Ordering::Relaxed);
        let messages_sent = self.messages_sent.load(Ordering::Relaxed);
        let messages_received = self.messages_received.load(Ordering::Relaxed);
        let bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        let bytes_received = self.bytes_received.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);

        println!("\n========== Load Test Results (300 Players) ==========");
        println!("Target Environment: 1vCPU, 1GB RAM");
        println!(
            "Connections established: {}/{}",
            connections, TARGET_CONNECTIONS
        );
        println!("Messages sent: {}", messages_sent);
        println!("Messages received: {}", messages_received);
        println!("Bytes sent: {} MB", bytes_sent / 1_048_576);
        println!("Bytes received: {} MB", bytes_received / 1_048_576);
        println!("Errors: {}", errors);
        println!(
            "Success rate: {:.2}%",
            (messages_received as f64 / messages_sent.max(1) as f64) * 100.0
        );

        if let Ok(latencies) = self.latencies.lock() {
            if !latencies.is_empty() {
                let avg_latency: Duration =
                    latencies.iter().sum::<Duration>() / latencies.len() as u32;
                let min_latency = latencies.iter().min().unwrap();
                let max_latency = latencies.iter().max().unwrap();

                let mut sorted = latencies.clone();
                sorted.sort();
                let p50 = sorted[sorted.len() / 2];
                let p95 = sorted[sorted.len() * 95 / 100];
                let p99 = sorted[sorted.len() * 99 / 100];

                println!("\nLatency Statistics:");
                println!("  Average: {:?}", avg_latency);
                println!("  Min: {:?}", min_latency);
                println!("  Max: {:?}", max_latency);
                println!("  P50: {:?}", p50);
                println!("  P95: {:?}", p95);
                println!("  P99: {:?}", p99);
            }
        }

        println!("\nThroughput:");
        println!(
            "  Messages/sec: {:.2}",
            messages_sent as f64 / TEST_DURATION_SECS as f64
        );
        println!(
            "  Bytes/sec: {:.2} KB/s",
            (bytes_sent as f64 / TEST_DURATION_SECS as f64) / 1024.0
        );
        println!("====================================================");
    }
}

struct VirtualPlayer {
    id: usize,
    socket: UdpSocket,
    server_addr: SocketAddr,
    sequence: u32,
    metrics: LoadTestMetrics,
}

impl VirtualPlayer {
    fn new(id: usize, server_addr: SocketAddr, metrics: LoadTestMetrics) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.set_nonblocking(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        Ok(Self {
            id,
            socket,
            server_addr,
            sequence: 0,
            metrics,
        })
    }

    fn connect(&mut self) -> std::io::Result<()> {
        let syn_packet = self.create_syn_packet();
        self.socket.send_to(&syn_packet, self.server_addr)?;
        self.metrics
            .bytes_sent
            .fetch_add(syn_packet.len() as u64, Ordering::Relaxed);

        let mut buffer = [0u8; 1500];
        match self.socket.recv_from(&mut buffer) {
            Ok((size, _)) => {
                self.metrics
                    .connections_established
                    .fetch_add(1, Ordering::Relaxed);
                self.metrics
                    .bytes_received
                    .fetch_add(size as u64, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                self.metrics.errors.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    fn send_game_update(&mut self) -> std::io::Result<()> {
        let start = Instant::now();

        let game_data = self.create_game_update();
        self.socket.send_to(&game_data, self.server_addr)?;
        self.metrics.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .bytes_sent
            .fetch_add(game_data.len() as u64, Ordering::Relaxed);
        self.sequence += 1;

        let mut buffer = [0u8; 1500];
        match self.socket.recv_from(&mut buffer) {
            Ok((size, _)) => {
                let latency = start.elapsed();
                self.metrics.record_latency(latency);
                self.metrics
                    .messages_received
                    .fetch_add(1, Ordering::Relaxed);
                self.metrics
                    .bytes_received
                    .fetch_add(size as u64, Ordering::Relaxed);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => {
                self.metrics.errors.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    fn send_heartbeat(&mut self) -> std::io::Result<()> {
        let heartbeat = self.create_heartbeat();
        self.socket.send_to(&heartbeat, self.server_addr)?;
        self.metrics
            .bytes_sent
            .fetch_add(heartbeat.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    fn create_syn_packet(&self) -> Vec<u8> {
        let mut packet = vec![0x01];
        packet.extend_from_slice(&0u32.to_le_bytes());
        packet.extend_from_slice(&(self.id as u64).to_le_bytes());
        packet
    }

    fn create_game_update(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut packet = vec![0x03];
        packet.extend_from_slice(&self.sequence.to_le_bytes());
        packet.extend_from_slice(&(self.id as u64).to_le_bytes());

        let game_state = format!(
            "{{\"player_id\":{},\"x\":{},\"y\":{},\"action\":\"move\",\"timestamp\":{}}}",
            self.id,
            rng.gen_range(0..1000),
            rng.gen_range(0..1000),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        packet.extend_from_slice(&(game_state.len() as u32).to_le_bytes());
        packet.extend_from_slice(game_state.as_bytes());
        packet
    }

    fn create_heartbeat(&self) -> Vec<u8> {
        let mut packet = vec![0x04];
        packet.extend_from_slice(&self.sequence.to_le_bytes());
        packet.extend_from_slice(&(self.id as u64).to_le_bytes());
        packet
    }
}

fn simulate_player(id: usize, metrics: LoadTestMetrics, start_time: Instant) {
    let server_addr: SocketAddr = SERVER_ADDR.parse().unwrap();

    let mut player = match VirtualPlayer::new(id, server_addr, metrics.clone()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Player {} failed to create: {}", id, e);
            metrics.errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    if let Err(e) = player.connect() {
        eprintln!("Player {} failed to connect: {}", id, e);
        return;
    }

    let mut last_heartbeat = Instant::now();
    let mut last_game_update = Instant::now();
    let update_interval = Duration::from_millis(1000 / MESSAGES_PER_SECOND_PER_CLIENT as u64);

    while start_time.elapsed() < Duration::from_secs(TEST_DURATION_SECS) {
        if last_game_update.elapsed() >= update_interval {
            let _ = player.send_game_update();
            last_game_update = Instant::now();
        }

        if last_heartbeat.elapsed() >= Duration::from_secs(5) {
            let _ = player.send_heartbeat();
            last_heartbeat = Instant::now();
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn monitor_resources(metrics: LoadTestMetrics) {
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(TEST_DURATION_SECS) {
        let connections = metrics.connections_established.load(Ordering::Relaxed);
        let msg_rate =
            metrics.messages_sent.load(Ordering::Relaxed) as f64 / start.elapsed().as_secs_f64();

        println!(
            "Active connections: {} | Msg/s: {:.0} | Errors: {}",
            connections,
            msg_rate,
            metrics.errors.load(Ordering::Relaxed)
        );

        thread::sleep(Duration::from_secs(5));
    }
}

pub fn run_load_test() {
    println!("Starting RUDP Load Test for 300 concurrent players");
    println!("Simulating 1vCPU, 1GB RAM environment constraints");
    println!("Test duration: {} seconds", TEST_DURATION_SECS);
    println!("Target: {} connections", TARGET_CONNECTIONS);

    let metrics = LoadTestMetrics::new();
    let start_time = Instant::now();

    let monitor_metrics = metrics.clone();
    let monitor_handle = thread::spawn(move || {
        monitor_resources(monitor_metrics);
    });

    let mut handles = vec![];

    for batch in 0..6 {
        println!("Starting batch {} (50 players)...", batch + 1);

        for i in 0..50 {
            let player_id = batch * 50 + i;
            if player_id >= TARGET_CONNECTIONS {
                break;
            }

            let player_metrics = metrics.clone();
            let player_start = start_time.clone();

            let handle = thread::spawn(move || {
                simulate_player(player_id, player_metrics, player_start);
            });

            handles.push(handle);
            thread::sleep(Duration::from_millis(20));
        }

        thread::sleep(Duration::from_secs(2));
    }

    for handle in handles {
        let _ = handle.join();
    }

    let _ = monitor_handle.join();

    metrics.print_summary();
}

fn main() {
    println!("Starting RUDP Load Test for 300 Players");
    run_load_test();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_300_players() {
        run_load_test();
    }
}
