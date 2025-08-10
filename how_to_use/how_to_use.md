# RUDP/TCP 서버 기능 및 헤더 읽기 로직 가이드

## 📋 목차

1. [프로젝트 개요](#프로젝트-개요)
2. [TCP 서버 프로토콜](#tcp-서버-프로토콜)
3. [RUDP 서버 프로토콜](#rudp-서버-프로토콜)
4. [헤더 읽기 로직](#헤더-읽기-로직)
5. [성능 최적화 기능](#성능-최적화-기능)
6. [연결 관리 시스템](#연결-관리-시스템)
7. [사용 예시](#사용-예시)
8. [문제 해결](#문제-해결)

---

## 프로젝트 개요

Police Thief 게임 서버는 두 가지 주요 통신 프로토콜을 지원합니다:

### 🚀 TCP 서버 (Production Ready)
- **위치**: `tcpserver/`
- **상태**: 운영 준비 완료
- **성능**: 500+ 동시 연결, 12,991+ 메시지/초
- **메모리**: 11MB RAM (500 연결시)

### ⚡ RUDP 서버 (Experimental)
- **위치**: `rudpserver/`
- **상태**: 실험적 개발 단계
- **목표**: 20,000+ 메시지/초, 1,000+ 연결, <0.5ms 지연시간

---

## TCP 서버 프로토콜

### 📦 프로토콜 구조

TCP 서버는 두 가지 프로토콜 형식을 지원합니다:

#### 1. JSON 프로토콜 (기본)
```
[4바이트 길이 헤더 (Big-Endian)][JSON 메시지 데이터]
```

#### 2. 바이너리 프로토콜 (최적화)
```
[4바이트 길이 헤더 (Little-Endian)][바이너리 메시지 데이터]
```

### 🔍 헤더 읽기 로직

#### JSON 프로토콜 헤더 처리 (`tcpserver/src/protocol.rs`)

```rust
pub async fn read_from_stream(stream: &mut BufReader<OwnedReadHalf>) -> Result<Self> {
    // 1. 4바이트 길이 헤더 읽기 (Big-Endian)
    let mut length_bytes = [0u8; 4];
    stream.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    
    // 2. 메시지 데이터 읽기
    let mut buffer = vec![0u8; length];
    stream.read_exact(&mut buffer).await?;
    
    // 3. JSON 역직렬화
    let json_str = std::str::from_utf8(&buffer)?;
    let message: GameMessage = serde_json::from_str(json_str)?;
    
    Ok(message)
}
```

#### 바이너리 프로토콜 헤더 처리 (`tcpserver/src/protocol/optimized.rs`)

```rust
pub async fn read_from_async_stream<R>(reader: &mut R) -> Result<Self>
where R: AsyncRead + Unpin,
{
    // 1. 4바이트 길이 헤더 읽기 (Little-Endian)
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    
    // 2. 길이 검증
    if len == 0 {
        return Err(anyhow!("메시지 길이가 0"));
    }
    if len > 1024 * 1024 { // 1MB 제한
        return Err(anyhow!("메시지가 너무 큼: {}바이트", len));
    }
    
    // 3. 데이터 읽기 및 바이너리 역직렬화
    let mut data = vec![0u8; len as usize];
    reader.read_exact(&mut data).await?;
    
    let message = Self::from_bytes(&data)?;
    Ok(message)
}
```

### 📨 메시지 타입

TCP 서버는 다음 메시지 타입들을 지원합니다:

#### 기본 연결 관리
- `HeartBeat`: 연결 상태 확인 (클라이언트 → 서버)
- `HeartBeatResponse`: 하트비트 응답 (서버 → 클라이언트)
- `Connect`: 연결 요청 (클라이언트 → 서버)
- `ConnectionAck`: 연결 확인 (서버 → 클라이언트)

#### 방 관리
- `RoomJoin`: 방 입장 요청
- `RoomLeave`: 방 퇴장 요청
- `RoomJoinSuccess`/`RoomLeaveSuccess`: 방 입장/퇴장 성공
- `UserJoinedRoom`/`UserLeftRoom`: 사용자 입장/퇴장 알림

#### 채팅 및 기타
- `ChatMessage`: 채팅 메시지
- `ChatResponse`: 채팅 응답
- `UserInfo`: 사용자 정보
- `SystemMessage`: 시스템 메시지
- `Error`: 에러 메시지

### ⚡ 바이너리 프로토콜 최적화

바이너리 프로토콜은 메시지 타입별로 고유한 바이트 코드를 사용합니다:

```rust
#[repr(u8)]
pub enum OptimizedGameMessage {
    HeartBeat = 0x01,
    UserInfo { user_id: u32, nickname: String } = 0x02,
    ChatMessage { user_id: u32, room_id: u32, message: String } = 0x10,
    RoomJoin { user_id: u32, room_id: u32, nickname: String } = 0x20,
    Error { code: u16, message: String } = 0x41,
    // ... 기타
}
```

**성능 개선:**
- 70% 성능 향상
- 50% 크기 감소
- CPU 사용량 감소

---

## RUDP 서버 프로토콜

### 🔄 RUDP (Reliable UDP) 개요

RUDP는 UDP의 속도와 TCP의 신뢰성을 결합한 프로토콜입니다.

### 📋 RUDP 패킷 구조

```rust
pub struct RudpPacket {
    pub header: RudpHeader,
    pub payload: Vec<u8>,
}

pub struct RudpHeader {
    pub packet_type: PacketType,
    pub sequence_number: u32,
    pub ack_number: u32,
    pub window_size: u16,
    pub checksum: u16,
    pub flags: u8,
}
```

### 🔍 RUDP 헤더 읽기 로직

#### 패킷 헤더 처리 (`rudpserver/src/protocol/rudp.rs`)

```rust
impl RudpPacket {
    pub fn from_bytes(data: &[u8]) -> Result<Self, RudpError> {
        if data.len() < RUDP_HEADER_SIZE {
            return Err(RudpError::PacketTooSmall);
        }
        
        let mut cursor = Cursor::new(data);
        
        // 헤더 필드 읽기
        let packet_type = PacketType::from_u8(cursor.read_u8()?)?;
        let sequence_number = cursor.read_u32::<BigEndian>()?;
        let ack_number = cursor.read_u32::<BigEndian>()?;
        let window_size = cursor.read_u16::<BigEndian>()?;
        let checksum = cursor.read_u16::<BigEndian>()?;
        let flags = cursor.read_u8()?;
        
        let header = RudpHeader {
            packet_type,
            sequence_number,
            ack_number,
            window_size,
            checksum,
            flags,
        };
        
        // 페이로드 읽기
        let payload_size = data.len() - RUDP_HEADER_SIZE;
        let mut payload = vec![0; payload_size];
        cursor.read_exact(&mut payload)?;
        
        Ok(RudpPacket { header, payload })
    }
}
```

### 🛡️ RUDP 신뢰성 기능

#### 1. 순서 보장 (Sequence Control)
```rust
impl ConnectionState {
    fn handle_incoming_packet(&mut self, packet: &RudpPacket) -> Result<Option<Vec<u8>>, RudpError> {
        let seq_num = packet.header.sequence_number;
        
        if seq_num == self.expected_sequence {
            // 순서대로 도착한 패킷
            self.expected_sequence = self.expected_sequence.wrapping_add(1);
            self.send_ack(seq_num).await?;
            Ok(Some(packet.payload.clone()))
        } else if seq_num > self.expected_sequence {
            // 순서에서 벗어난 패킷 - 버퍼에 저장
            self.out_of_order_buffer.insert(seq_num, packet.payload.clone());
            Ok(None)
        } else {
            // 중복 패킷 - ACK만 전송
            self.send_ack(seq_num).await?;
            Ok(None)
        }
    }
}
```

#### 2. 재전송 메커니즘
```rust
impl ConnectionState {
    async fn handle_timeout(&mut self) -> Result<(), RudpError> {
        for (seq_num, packet) in &self.unacked_packets {
            if packet.timestamp.elapsed() > self.rto {
                // 패킷 재전송
                self.socket.send_to(&packet.data, &packet.addr).await?;
                
                // RTO 증가 (지수 백오프)
                self.rto = (self.rto * 2).min(Duration::from_secs(60));
                
                // 재전송 카운트 증가
                self.retransmission_count += 1;
            }
        }
        Ok(())
    }
}
```

#### 3. 혼잡 제어 (Congestion Control)
```rust
impl ConnectionState {
    fn on_ack_received(&mut self, ack_num: u32) {
        // RTT 계산
        if let Some(send_time) = self.send_times.get(&ack_num) {
            let rtt = send_time.elapsed();
            self.update_rtt(rtt);
        }
        
        // 혼잡 윈도우 조정
        if self.congestion_window < self.slow_start_threshold {
            // Slow Start
            self.congestion_window += 1;
        } else {
            // Congestion Avoidance
            self.congestion_window += 1.0 / self.congestion_window as f32;
        }
        
        // 전송 윈도우 업데이트
        self.window_size = self.congestion_window.min(self.advertised_window) as u16;
    }
    
    fn on_packet_lost(&mut self) {
        // 패킷 손실 감지시 혼잡 윈도우 반으로 감소
        self.slow_start_threshold = (self.congestion_window / 2.0).max(2.0);
        self.congestion_window = 1.0;
    }
}
```

---

## 헤더 읽기 로직

### 🔄 연결 핸들러 (`tcpserver/src/handler/connection_handler.rs`)

#### 새 연결 처리 프로세스

```rust
pub async fn handle_new_connection(&self, stream: TcpStream, addr: String) -> Result<u32> {
    // 1. IP 검증
    let socket_addr = NetworkUtils::parse_socket_addr(&addr)?;
    let ip_info = IpInfo::from_socket_addr(&socket_addr);
    self.validate_user_connection(&ip_info).await?;
    
    // 2. 스트림 분리
    let (reader, writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    
    // 3. Connect 메시지 대기 (헤더 읽기 포함)
    let connect_msg = GameMessage::read_from_stream(&mut buf_reader).await?;
    
    // 4. Connect 메시지 검증
    let (room_id, user_id) = match connect_msg {
        GameMessage::Connect { room_id, user_id } => (room_id, user_id),
        _ => return Err(anyhow!("첫 메시지는 Connect 메시지여야 합니다")),
    };
    
    // 5. Redis에 TCP 호스트 정보 저장
    if let Some(redis_config) = &self.redis_config {
        self.store_tcp_host_to_redis(user_id, &addr, redis_config.as_ref()).await?;
    }
    
    // 6. 연결 서비스에 등록
    let reader = buf_reader.into_inner();
    let reunited_stream = reader.reunite(writer)?;
    let registered_user_id = self.connection_service
        .handle_new_connection_with_id(reunited_stream, addr.clone(), user_id).await?;
    
    // 7. 환영 메시지 전송
    self.send_welcome_message(registered_user_id).await?;
    
    Ok(registered_user_id)
}
```

### ⏱️ 하트비트 시스템

TCP 서버의 하트비트 시스템:
- **간격**: 10초마다 클라이언트가 서버로 전송
- **타임아웃**: 30초 응답 없으면 연결 해제
- **자동 정리**: 타임아웃된 연결 자동 제거

```rust
// 하트비트 메시지 구조
GameMessage::HeartBeat  // 클라이언트 → 서버
GameMessage::HeartBeatResponse { timestamp } // 서버 → 클라이언트
```

---

## 성능 최적화 기능

### 🚀 TCP 서버 최적화 (8가지 서비스)

#### 1. DashMap 최적화기
```rust
// CPU별 샤딩으로 락 경합 최소화
pub struct DashMapOptimizer<K, V> {
    maps: Vec<DashMap<K, V>>,
    shard_count: usize,
}
```

#### 2. 비동기 I/O 최적화
```rust
// 벡터화된 I/O 및 버퍼 풀링
pub struct AsyncIOOptimizer {
    buffer_pool: Arc<Mutex<Vec<Vec<u8>>>>,
    vectored_operations: bool,
}
```

#### 3. SIMD 최적화
```rust
// AVX2/SSE4.2 하드웨어 가속
pub struct SIMDOptimizer;
impl SIMDOptimizer {
    pub fn batch_process_u32(data: &[u32]) -> Vec<u32> {
        // SIMD 명령어를 사용한 병렬 처리
    }
}
```

#### 4. 메시지 압축
```rust
// LZ4/Zstd 적응적 압축
pub struct MessageCompression {
    algorithm: CompressionAlgorithm,
    cache: LruCache<Vec<u8>, Vec<u8>>,
}
```

#### 5. 연결 풀
```rust
// 자동 스케일링 연결 관리
pub struct ConnectionPool {
    connections: DashMap<u32, ConnectionInfo>,
    health_monitor: HealthMonitor,
}
```

#### 6. 성능 모니터링
```rust
// 실시간 메트릭 및 알림
pub struct PerformanceMonitor {
    metrics: AtomicStats,
    alert_thresholds: AlertThresholds,
}
```

#### 7. 메모리 풀
```rust
// 객체 재활용 시스템
pub struct MemoryPool<T> {
    pool: Arc<Mutex<Vec<T>>>,
    factory: Box<dyn Fn() -> T>,
}
```

#### 8. 병렬 브로드캐스트
```rust
// Rayon 기반 병렬 메시지 전송 (300-500% 성능 향상)
pub struct ParallelBroadcast {
    thread_pool: ThreadPool,
    batch_size: usize,
}
```

### ⚡ RUDP 서버 고성능 도구 (16가지)

RUDP 서버는 TCP 서버의 8가지 최적화와 추가로 8가지 RUDP 전용 최적화를 제공:

#### 추가 RUDP 최적화
- **패킷 버퍼링**: 대용량 패킷 버퍼 풀
- **순서 제어**: 락프리 순서 보장 알고리즘
- **RTT 계산**: 정밀한 왕복 시간 측정
- **혼잡 제어**: 적응적 혼잡 윈도우 조정
- **재전송 최적화**: 스마트 재전송 전략
- **플로우 제어**: 동적 윈도우 크기 조정
- **체크섬 검증**: 하드웨어 가속 체크섬
- **연결 상태**: 효율적인 상태 머신

---

## 연결 관리 시스템

### 📊 연결 품질 평가

```rust
pub enum ConnectionQuality {
    Excellent,  // 1시간+ 연결, 10분 내 하트비트
    Good,       // 30분+ 연결, 15분 내 하트비트
    Fair,       // 10분+ 연결, 20분 내 하트비트
    Poor,       // 25분 내 하트비트
    VeryPoor,   // 문제 있는 연결
}
```

### 🔧 연결 상태 모니터링

```rust
pub struct ConnectionsSummary {
    pub total_connections: usize,
    pub quality_distribution: HashMap<String, u32>,
    pub peak_connections: u32,
    pub total_lifetime_connections: u64,
    pub timeout_disconnections: u64,
}

pub struct ProblematicConnection {
    pub user_id: u32,
    pub addr: String,
    pub issue: String,
    pub severity: String,
    pub last_heartbeat_secs: u64,
}
```

### 📈 성능 메트릭

#### TCP 서버 검증된 성능
- **동시 연결**: 500+ (100% 성공률)
- **메시지 처리량**: 12,991+ 메시지/초 (지속), 41,064+ (최대)
- **메모리 효율성**: 11MB RAM (500 연결시), 연결당 22KB
- **연결 성능**: 7,106 연결/초 (저부하), 264 연결/초 (고부하)

#### RUDP 서버 목표 성능
- **메시지 처리량**: 20,000+ 메시지/초
- **메모리 사용량**: 8-10MB RAM
- **동시 연결**: 1,000+ 연결
- **지연시간**: <0.5ms

---

## 사용 예시

### 🔧 TCP 서버 실행

```bash
# 1. 환경 변수 설정 (.env 파일)
tcp_host=127.0.0.1
tcp_port=4000
redis_host=127.0.0.1
redis_port=6379

# 2. 서버 실행
cargo run --bin tcpserver

# 3. 클라이언트 테스트
python tcpserver/high_load_test.py          # 500 연결 스트레스 테스트
python tcpserver/simple_test_client.py      # 기본 연결 테스트
```

### ⚡ RUDP 서버 실행

```bash
# 1. 환경 변수 설정
udp_host=127.0.0.1
udp_port=5000

# 2. 서버 실행
cargo run --bin rudpserver

# 3. 클라이언트 테스트 (개발 중)
# RUDP 클라이언트는 현재 개발 진행 중
```

### 📝 클라이언트 코드 예시

#### TCP 클라이언트 (Python)
```python
import socket
import json
import struct

def send_message(sock, message):
    # JSON 직렬화
    json_data = json.dumps(message).encode('utf-8')
    
    # 4바이트 길이 헤더 (Big-Endian)
    length = struct.pack('>I', len(json_data))
    
    # 전송
    sock.send(length + json_data)

def receive_message(sock):
    # 4바이트 길이 헤더 읽기
    length_data = sock.recv(4)
    length = struct.unpack('>I', length_data)[0]
    
    # 메시지 데이터 읽기
    message_data = sock.recv(length)
    
    # JSON 역직렬화
    return json.loads(message_data.decode('utf-8'))

# 연결 및 사용 예시
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.connect(('127.0.0.1', 4000))

# Connect 메시지 전송
connect_msg = {"Connect": {"room_id": 1, "user_id": 123}}
send_message(sock, connect_msg)

# 응답 받기
response = receive_message(sock)
print(f"서버 응답: {response}")

# 하트비트 전송
heartbeat = {"HeartBeat": {}}
send_message(sock, heartbeat)
```

---

## 문제 해결

### ❗ 일반적인 문제들

#### 1. 연결 실패
```
Error: Redis 연결 실패
```
**해결책**: Redis 서버가 실행 중인지 확인
```bash
redis-cli ping
```

#### 2. 프로토콜 에러
```
Error: 첫 메시지는 Connect 메시지여야 합니다
```
**해결책**: 클라이언트가 첫 메시지로 Connect를 전송하는지 확인

#### 3. 헤더 읽기 실패
```
Error: 메시지가 너무 짧습니다
```
**해결책**: 4바이트 길이 헤더가 올바르게 전송되는지 확인

### 🔍 디버깅 도구

#### 성능 모니터링
```bash
# Windows
powershell "Get-Process tcpserver* | Select-Object Name,Id,CPU,WorkingSet"

# Linux/Mac
watch -n 1 'ps aux | grep tcpserver'
```

#### Redis 모니터링
```bash
redis-cli MONITOR          # 모든 Redis 명령 감시
redis-cli KEYS "*"         # 모든 키 조회
redis-cli HGETALL "user:1" # 사용자 정보 조회
```

#### 네트워크 디버깅
```bash
# 포트 사용 확인
netstat -an | grep 4000    # TCP 서버
netstat -an | grep 5000    # RUDP 서버

# 연결 상태 확인
ss -tuln | grep -E '4000|5000'
```

---

## 📚 추가 정보

### 관련 파일
- **TCP 프로토콜**: `tcpserver/src/protocol.rs`
- **바이너리 프로토콜**: `tcpserver/src/protocol/optimized.rs`
- **RUDP 프로토콜**: `rudpserver/src/protocol/rudp.rs`
- **연결 핸들러**: `tcpserver/src/handler/connection_handler.rs`
- **성능 도구**: `shared/src/tool/high_performance/`

### 설정 파일
- **환경 변수**: `.env`
- **빌드 설정**: `Cargo.toml`
- **프로젝트 문서**: `CLAUDE.md`

### 테스트 파일
- **TCP 테스트**: `tcpserver/high_load_test.py`
- **성능 테스트**: `tcpserver/performance_test_client.py`
- **단위 테스트**: `cargo test -p tcpserver`

---

*이 문서는 Police Thief 게임 서버의 RUDP/TCP 프로토콜 구현을 설명합니다. 추가 질문이나 문제가 있으면 개발팀에 문의하세요.*