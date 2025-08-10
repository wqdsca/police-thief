use rand::Rng;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct StressTestConfig {
    server_addr: String,
    vcpu_limit: f32,
    ram_limit_mb: usize,
    test_duration: Duration,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000".to_string(),
            vcpu_limit: 1.0,
            ram_limit_mb: 1024,
            test_duration: Duration::from_secs(120),
        }
    }
}

#[derive(Clone)]
struct StressMetrics {
    packets_sent: Arc<AtomicU64>,
    packets_dropped: Arc<AtomicU64>,
    connections_failed: Arc<AtomicUsize>,
    memory_pressure_events: Arc<AtomicUsize>,
    cpu_throttle_events: Arc<AtomicUsize>,
    max_latency_ms: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
}

impl StressMetrics {
    fn new() -> Self {
        Self {
            packets_sent: Arc::new(AtomicU64::new(0)),
            packets_dropped: Arc::new(AtomicU64::new(0)),
            connections_failed: Arc::new(AtomicUsize::new(0)),
            memory_pressure_events: Arc::new(AtomicUsize::new(0)),
            cpu_throttle_events: Arc::new(AtomicUsize::new(0)),
            max_latency_ms: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    fn print_report(&self) {
        println!("\n========== Stress Test Report ==========");
        println!(
            "Packets sent: {}",
            self.packets_sent.load(Ordering::Relaxed)
        );
        println!(
            "Packets dropped: {}",
            self.packets_dropped.load(Ordering::Relaxed)
        );
        println!(
            "Connection failures: {}",
            self.connections_failed.load(Ordering::Relaxed)
        );
        println!(
            "Memory pressure events: {}",
            self.memory_pressure_events.load(Ordering::Relaxed)
        );
        println!(
            "CPU throttle events: {}",
            self.cpu_throttle_events.load(Ordering::Relaxed)
        );
        println!(
            "Max latency: {} ms",
            self.max_latency_ms.load(Ordering::Relaxed)
        );
        println!(
            "Total data: {} MB",
            self.total_bytes.load(Ordering::Relaxed) / 1_048_576
        );

        let drop_rate = self.packets_dropped.load(Ordering::Relaxed) as f64
            / self.packets_sent.load(Ordering::Relaxed).max(1) as f64
            * 100.0;
        println!("Packet drop rate: {:.2}%", drop_rate);
        println!("=========================================");
    }
}

pub fn stress_test_packet_flood(config: StressTestConfig) {
    println!("Starting Packet Flood Stress Test");
    println!("Target: Overwhelming server with maximum packet rate");

    let metrics = StressMetrics::new();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let server_addr: SocketAddr = config.server_addr.parse().unwrap();

    let mut handles = vec![];

    for thread_id in 0..10 {
        let metrics_clone = metrics.clone();
        let stop_clone = stop_flag.clone();

        let handle = thread::spawn(move || {
            let socket = match UdpSocket::bind("127.0.0.1:0") {
                Ok(s) => s,
                Err(_) => return,
            };
            socket.set_nonblocking(true).ok();

            let mut rng = rand::thread_rng();
            let mut packet_buffer = vec![0u8; 1400];

            while !stop_clone.load(Ordering::Relaxed) {
                rng.fill(&mut packet_buffer[..]);
                packet_buffer[0] = 0x03;

                match socket.send_to(&packet_buffer, server_addr) {
                    Ok(size) => {
                        metrics_clone.packets_sent.fetch_add(1, Ordering::Relaxed);
                        metrics_clone
                            .total_bytes
                            .fetch_add(size as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        metrics_clone
                            .packets_dropped
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        handles.push(handle);
    }

    thread::sleep(config.test_duration);
    stop_flag.store(true, Ordering::Relaxed);

    for handle in handles {
        let _ = handle.join();
    }

    metrics.print_report();
}

pub fn stress_test_connection_churn(config: StressTestConfig) {
    println!("Starting Connection Churn Stress Test");
    println!("Target: Rapid connection/disconnection cycles");

    let metrics = StressMetrics::new();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let server_addr: SocketAddr = config.server_addr.parse().unwrap();

    let mut handles = vec![];

    for _ in 0..50 {
        let metrics_clone = metrics.clone();
        let stop_clone = stop_flag.clone();

        let handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                let socket = match UdpSocket::bind("127.0.0.1:0") {
                    Ok(s) => s,
                    Err(_) => {
                        metrics_clone
                            .connections_failed
                            .fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                socket
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .ok();

                let syn = vec![0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                if socket.send_to(&syn, server_addr).is_err() {
                    metrics_clone
                        .connections_failed
                        .fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                let mut buffer = [0u8; 1500];
                match socket.recv_from(&mut buffer) {
                    Ok(_) => {
                        let fin = vec![0x05, 0, 0, 0, 0];
                        let _ = socket.send_to(&fin, server_addr);
                    }
                    Err(_) => {
                        metrics_clone
                            .connections_failed
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }

                thread::sleep(Duration::from_millis(10));
            }
        });

        handles.push(handle);
    }

    thread::sleep(config.test_duration);
    stop_flag.store(true, Ordering::Relaxed);

    for handle in handles {
        let _ = handle.join();
    }

    metrics.print_report();
}

pub fn stress_test_memory_exhaustion(config: StressTestConfig) {
    println!("Starting Memory Exhaustion Stress Test");
    println!("Target: Testing server behavior under memory pressure");

    let metrics = StressMetrics::new();
    let server_addr: SocketAddr = config.server_addr.parse().unwrap();

    let mut connections = vec![];
    let mut allocations = vec![];

    allocations.push(vec![0u8; config.ram_limit_mb * 512 * 1024]);

    for i in 0..500 {
        let socket = match UdpSocket::bind("127.0.0.1:0") {
            Ok(s) => s,
            Err(_) => {
                metrics
                    .memory_pressure_events
                    .fetch_add(1, Ordering::Relaxed);
                break;
            }
        };

        socket.set_nonblocking(true).ok();

        let syn = vec![0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        if socket.send_to(&syn, server_addr).is_err() {
            metrics.connections_failed.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        connections.push(socket);

        if i % 50 == 0 {
            match vec![0u8; 10_485_760].try_reserve(0) {
                Ok(_) => allocations.push(vec![0u8; 10_485_760]),
                Err(_) => metrics
                    .memory_pressure_events
                    .fetch_add(1, Ordering::Relaxed),
            }
        }
    }

    println!(
        "Established {} connections under memory pressure",
        connections.len()
    );

    let mut rng = rand::thread_rng();
    let start = Instant::now();

    while start.elapsed() < config.test_duration {
        for socket in &connections {
            let data = vec![rng.gen::<u8>(); rng.gen_range(100..1400)];
            match socket.send_to(&data, server_addr) {
                Ok(size) => {
                    metrics.packets_sent.fetch_add(1, Ordering::Relaxed);
                    metrics
                        .total_bytes
                        .fetch_add(size as u64, Ordering::Relaxed);
                }
                Err(_) => {
                    metrics.packets_dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    metrics.print_report();
}

pub fn stress_test_latency_spike(config: StressTestConfig) {
    println!("Starting Latency Spike Stress Test");
    println!("Target: Testing server response under variable load");

    let metrics = StressMetrics::new();
    let server_addr: SocketAddr = config.server_addr.parse().unwrap();

    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(1000)))
        .unwrap();

    let syn = vec![0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    socket.send_to(&syn, server_addr).unwrap();

    let mut buffer = [0u8; 1500];
    socket.recv_from(&mut buffer).unwrap();

    let mut rng = rand::thread_rng();
    let start = Instant::now();

    while start.elapsed() < config.test_duration {
        let burst_size = if rng.gen_bool(0.1) { 1000 } else { 10 };

        for _ in 0..burst_size {
            let send_time = Instant::now();
            let data = vec![0x03, 0, 0, 0, 0, 1, 2, 3, 4, 5];

            if socket.send_to(&data, server_addr).is_ok() {
                metrics.packets_sent.fetch_add(1, Ordering::Relaxed);

                if let Ok(_) = socket.recv_from(&mut buffer) {
                    let latency = send_time.elapsed().as_millis() as u64;

                    loop {
                        let current_max = metrics.max_latency_ms.load(Ordering::Relaxed);
                        if latency <= current_max {
                            break;
                        }
                        if metrics
                            .max_latency_ms
                            .compare_exchange(
                                current_max,
                                latency,
                                Ordering::Relaxed,
                                Ordering::Relaxed,
                            )
                            .is_ok()
                        {
                            break;
                        }
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    metrics.print_report();
}

pub fn stress_test_cpu_saturation(config: StressTestConfig) {
    println!("Starting CPU Saturation Stress Test");
    println!(
        "Target: Testing server under CPU constraints ({}vCPU)",
        config.vcpu_limit
    );

    let metrics = StressMetrics::new();
    let server_addr: SocketAddr = config.server_addr.parse().unwrap();

    let cpu_burner = thread::spawn(move || {
        let mut counter = 0u64;
        loop {
            for _ in 0..1000000 {
                counter = counter.wrapping_add(1).wrapping_mul(31);
            }
            if counter == 0 {
                break;
            }
        }
    });

    let mut handles = vec![];
    let stop_flag = Arc::new(AtomicBool::new(false));

    for _ in 0..20 {
        let metrics_clone = metrics.clone();
        let stop_clone = stop_flag.clone();

        let handle = thread::spawn(move || {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            socket.set_nonblocking(true).ok();

            let syn = vec![0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let _ = socket.send_to(&syn, server_addr);

            let mut rng = rand::thread_rng();

            while !stop_clone.load(Ordering::Relaxed) {
                let complex_data: Vec<u8> = (0..500)
                    .map(|i| (i as u8).wrapping_mul(rng.gen::<u8>()))
                    .collect();

                match socket.send_to(&complex_data, server_addr) {
                    Ok(size) => {
                        metrics_clone.packets_sent.fetch_add(1, Ordering::Relaxed);
                        metrics_clone
                            .total_bytes
                            .fetch_add(size as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        metrics_clone
                            .cpu_throttle_events
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }

                thread::sleep(Duration::from_millis(1));
            }
        });

        handles.push(handle);
    }

    thread::sleep(config.test_duration);
    stop_flag.store(true, Ordering::Relaxed);

    for handle in handles {
        let _ = handle.join();
    }

    drop(cpu_burner);

    metrics.print_report();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_all_stress_tests() {
        let config = StressTestConfig {
            test_duration: Duration::from_secs(30),
            ..Default::default()
        };

        println!("\n=== Running Stress Test Suite (1vCPU, 1GB RAM) ===\n");

        stress_test_packet_flood(config.clone());
        thread::sleep(Duration::from_secs(5));

        stress_test_connection_churn(config.clone());
        thread::sleep(Duration::from_secs(5));

        stress_test_memory_exhaustion(config.clone());
        thread::sleep(Duration::from_secs(5));

        stress_test_latency_spike(config.clone());
        thread::sleep(Duration::from_secs(5));

        stress_test_cpu_saturation(config.clone());

        println!("\n=== Stress Test Suite Complete ===");
    }
}
