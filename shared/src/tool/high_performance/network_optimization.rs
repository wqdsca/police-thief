use bytes::{BufMut, Bytes, BytesMut};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tokio::net::{TcpListener, TcpSocket};
use tracing::info;

/// 네트워크 최적화 설정
#[derive(Debug, Clone)]
pub struct NetworkOptimizationConfig {
    /// TCP_NODELAY (Nagle 알고리즘 비활성화)
    pub tcp_nodelay: bool,
    /// SO_KEEPALIVE
    pub keepalive: bool,
    /// SO_REUSEADDR
    pub reuse_addr: bool,
    /// SO_REUSEPORT (Linux/Unix)
    pub reuse_port: bool,
    /// 송신 버퍼 크기
    pub send_buffer_size: Option<usize>,
    /// 수신 버퍼 크기
    pub recv_buffer_size: Option<usize>,
    /// SO_LINGER
    pub linger: Option<Duration>,
    /// TCP Keepalive 간격
    pub keepalive_interval: Option<Duration>,
    /// TCP Keepalive 재시도
    pub keepalive_retries: Option<u32>,
}

impl Default for NetworkOptimizationConfig {
    fn default() -> Self {
        Self {
            tcp_nodelay: true, // 게임 서버는 낮은 지연시간이 중요
            keepalive: true,
            reuse_addr: true,
            reuse_port: cfg!(unix),         // Unix 계열만 지원
            send_buffer_size: Some(262144), // 256KB
            recv_buffer_size: Some(262144), // 256KB
            linger: None,
            keepalive_interval: Some(Duration::from_secs(30)),
            keepalive_retries: Some(3),
        }
    }
}

impl NetworkOptimizationConfig {
    /// 고성능 게임 서버용 설정
    pub fn for_game_server() -> Self {
        Self {
            tcp_nodelay: true,
            keepalive: true,
            reuse_addr: true,
            reuse_port: cfg!(unix),
            send_buffer_size: Some(524288), // 512KB
            recv_buffer_size: Some(524288), // 512KB
            linger: None,
            keepalive_interval: Some(Duration::from_secs(20)),
            keepalive_retries: Some(5),
        }
    }

    /// 대용량 전송용 설정
    pub fn for_bulk_transfer() -> Self {
        Self {
            tcp_nodelay: false, // Nagle 알고리즘 활성화
            keepalive: true,
            reuse_addr: true,
            reuse_port: false,
            send_buffer_size: Some(1048576), // 1MB
            recv_buffer_size: Some(1048576), // 1MB
            linger: Some(Duration::from_secs(5)),
            keepalive_interval: Some(Duration::from_secs(60)),
            keepalive_retries: Some(3),
        }
    }
}

/// 네트워크 소켓 최적화
pub struct NetworkOptimizer;

impl NetworkOptimizer {
    /// TcpListener 최적화
    pub async fn optimize_listener(
        addr: SocketAddr,
        config: &NetworkOptimizationConfig,
    ) -> io::Result<TcpListener> {
        let socket = if addr.is_ipv4() {
            TcpSocket::new_v4()?
        } else {
            TcpSocket::new_v6()?
        };

        // 소켓 옵션 적용
        Self::apply_socket_options(&socket, config)?;

        socket.bind(addr)?;
        let listener = socket.listen(1024)?; // backlog 크기

        info!("🚀 Optimized TCP listener created on {}", addr);
        info!("  └─ TCP_NODELAY: {}", config.tcp_nodelay);
        info!(
            "  └─ Buffer sizes: send={:?}, recv={:?}",
            config.send_buffer_size, config.recv_buffer_size
        );

        Ok(listener)
    }

    /// TcpSocket에 최적화 옵션 적용
    fn apply_socket_options(
        socket: &TcpSocket,
        config: &NetworkOptimizationConfig,
    ) -> io::Result<()> {
        // SO_REUSEADDR
        if config.reuse_addr {
            socket.set_reuseaddr(true)?;
        }

        // SO_REUSEPORT (Unix/Linux only)
        #[cfg(unix)]
        if config.reuse_port {
            socket.set_reuseport(true)?;
        }

        // 송신 버퍼 크기
        if let Some(size) = config.send_buffer_size {
            socket.set_send_buffer_size(size as u32)?;
        }

        // 수신 버퍼 크기
        if let Some(size) = config.recv_buffer_size {
            socket.set_recv_buffer_size(size as u32)?;
        }

        Ok(())
    }

    /// TcpStream 최적화
    pub fn optimize_stream(
        stream: &TcpStream,
        config: &NetworkOptimizationConfig,
    ) -> io::Result<()> {
        use socket2::Socket;
        #[cfg(unix)]
        use std::os::unix::io::{AsRawFd, FromRawFd};
        #[cfg(windows)]
        use std::os::windows::io::{AsRawSocket, FromRawSocket};

        let socket = unsafe {
            #[cfg(unix)]
            {
                Socket::from_raw_fd(stream.as_raw_fd())
            }
            #[cfg(windows)]
            {
                Socket::from_raw_socket(stream.as_raw_socket())
            }
        };

        // TCP_NODELAY
        socket.set_nodelay(config.tcp_nodelay)?;

        // SO_KEEPALIVE
        if config.keepalive {
            socket.set_tcp_keepalive(
                &socket2::TcpKeepalive::new()
                    .with_time(config.keepalive_interval.unwrap_or(Duration::from_secs(30))),
            )?;
        }

        // SO_LINGER
        if let Some(linger) = config.linger {
            socket.set_linger(Some(linger))?;
        }

        // 버퍼 크기
        if let Some(size) = config.send_buffer_size {
            socket.set_send_buffer_size(size)?;
        }

        if let Some(size) = config.recv_buffer_size {
            socket.set_recv_buffer_size(size)?;
        }

        // Socket을 다시 leak하여 ownership 유지
        std::mem::forget(socket);

        Ok(())
    }
}

/// Zero-copy 버퍼 매니저
pub struct ZeroCopyBuffer {
    buffer: BytesMut,
}

impl ZeroCopyBuffer {
    /// 새 버퍼 생성
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
        }
    }

    /// 데이터 쓰기 (zero-copy)
    #[inline]
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        if self.buffer.remaining_mut() < data.len() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "Buffer full"));
        }

        self.buffer.put_slice(data);
        Ok(())
    }

    /// 데이터 읽기 (zero-copy)
    #[inline]
    pub fn read(&mut self, len: usize) -> Option<Bytes> {
        if self.buffer.len() >= len {
            Some(self.buffer.split_to(len).freeze())
        } else {
            None
        }
    }

    /// 전체 데이터를 Bytes로 변환 (zero-copy)
    #[inline]
    pub fn freeze(self) -> Bytes {
        self.buffer.freeze()
    }

    /// 버퍼 클리어
    #[inline]
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// 남은 용량
    #[inline]
    pub fn remaining(&self) -> usize {
        self.buffer.remaining_mut()
    }

    /// 현재 데이터 크기
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// 버퍼가 비어있는지 확인
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// 용량 예약
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
    }
}

/// Vectored I/O를 위한 헬퍼
pub struct VectoredIO;

impl VectoredIO {
    /// Vectored write (scatter-gather I/O)
    pub fn write_vectored(stream: &mut TcpStream, buffers: &[&[u8]]) -> io::Result<usize> {
        use std::io::IoSlice;

        let io_slices: Vec<IoSlice> = buffers.iter().map(|buf| IoSlice::new(buf)).collect();

        stream.write_vectored(&io_slices)
    }

    /// Vectored read
    pub fn read_vectored(stream: &mut TcpStream, buffers: &mut [&mut [u8]]) -> io::Result<usize> {
        use std::io::IoSliceMut;

        let mut io_slices: Vec<IoSliceMut> =
            buffers.iter_mut().map(|buf| IoSliceMut::new(buf)).collect();

        stream.read_vectored(&mut io_slices)
    }
}

/// 네트워크 통계
#[derive(Debug, Default)]
pub struct NetworkStats {
    pub bytes_sent: std::sync::atomic::AtomicU64,
    pub bytes_received: std::sync::atomic::AtomicU64,
    pub packets_sent: std::sync::atomic::AtomicU64,
    pub packets_received: std::sync::atomic::AtomicU64,
    pub errors: std::sync::atomic::AtomicU64,
}

impl NetworkStats {
    pub fn record_send(&self, bytes: usize) {
        self.bytes_sent
            .fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        self.packets_sent
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_receive(&self, bytes: usize) {
        self.bytes_received
            .fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        self.packets_received
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> NetworkStatsSnapshot {
        NetworkStatsSnapshot {
            bytes_sent: self.bytes_sent.load(std::sync::atomic::Ordering::Relaxed),
            bytes_received: self
                .bytes_received
                .load(std::sync::atomic::Ordering::Relaxed),
            packets_sent: self.packets_sent.load(std::sync::atomic::Ordering::Relaxed),
            packets_received: self
                .packets_received
                .load(std::sync::atomic::Ordering::Relaxed),
            errors: self.errors.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkStatsSnapshot {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub errors: u64,
}

impl NetworkStatsSnapshot {
    pub fn throughput_mbps(&self, duration_secs: f64) -> (f64, f64) {
        let send_mbps = (self.bytes_sent as f64 * 8.0) / (duration_secs * 1_000_000.0);
        let recv_mbps = (self.bytes_received as f64 * 8.0) / (duration_secs * 1_000_000.0);
        (send_mbps, recv_mbps)
    }
}

mod tests {

    #[test]
    fn test_zero_copy_buffer() {
        let mut buffer = ZeroCopyBuffer::new(1024);

        // 쓰기
        assert!(buffer.write(b"Hello").is_ok());
        assert_eq!(buffer.len(), 5);

        // 읽기
        let data = buffer.read(5).expect("Test assertion failed");
        assert_eq!(&data[..], b"Hello");
        assert_eq!(buffer.len(), 0);

        // 다시 쓰기
        buffer.write(b"World").expect("Test assertion failed");
        let frozen = buffer.freeze();
        assert_eq!(&frozen[..], b"World");
    }

    #[test]
    fn test_network_stats() {
        let stats = NetworkStats::default();

        stats.record_send(1024);
        stats.record_send(2048);
        stats.record_receive(512);

        let snapshot = stats.get_stats();
        assert_eq!(snapshot.bytes_sent, 3072);
        assert_eq!(snapshot.bytes_received, 512);
        assert_eq!(snapshot.packets_sent, 2);
        assert_eq!(snapshot.packets_received, 1);
    }

    #[tokio::test]
    async fn test_network_optimizer() {
        let config = NetworkOptimizationConfig::for_game_server();
        let addr = "127.0.0.1:0".parse().expect("Test assertion failed");

        let result = NetworkOptimizer::optimize_listener(addr, &config).await;
        assert!(result.is_ok());
    }
}
