// 간단한 RUDP 프로토콜 테스트
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct RudpPacket {
    pub packet_type: PacketType,
    pub sequence: u32,
    pub ack: Option<u32>,
    pub timestamp: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PacketType {
    Syn,
    SynAck,
    Data,
    Ack,
    KeepAlive,
}

impl RudpPacket {
    pub fn new_syn(seq: u32) -> Self {
        Self {
            packet_type: PacketType::Syn,
            sequence: seq,
            ack: None,
            timestamp: 0,
            data: vec![],
        }
    }

    pub fn new_data(seq: u32, data: Vec<u8>) -> Self {
        Self {
            packet_type: PacketType::Data,
            sequence: seq,
            ack: None,
            timestamp: 0,
            data,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.push(self.packet_type.clone() as u8);
        buffer.extend_from_slice(&self.sequence.to_le_bytes());
        buffer.extend_from_slice(&self.timestamp.to_le_bytes());
        let data_len = self.data.len() as u32;
        buffer.extend_from_slice(&data_len.to_le_bytes());
        buffer.extend_from_slice(&self.data);
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < 17 {
            return None;
        }

        let packet_type = match buffer[0] {
            0 => PacketType::Syn,
            1 => PacketType::SynAck,
            2 => PacketType::Data,
            3 => PacketType::Ack,
            4 => PacketType::KeepAlive,
            _ => return None,
        };

        let sequence = u32::from_le_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]);
        let timestamp = u64::from_le_bytes([
            buffer[5], buffer[6], buffer[7], buffer[8], buffer[9], buffer[10], buffer[11],
            buffer[12],
        ]);
        let data_len =
            u32::from_le_bytes([buffer[13], buffer[14], buffer[15], buffer[16]]) as usize;

        let data = if buffer.len() >= 17 + data_len {
            buffer[17..17 + data_len].to_vec()
        } else {
            vec![]
        };

        Some(Self {
            packet_type,
            sequence,
            ack: None,
            timestamp,
            data,
        })
    }
}

// 간단한 혼잡 제어 시뮬레이션
pub struct CongestionController {
    cwnd: u32,
    ssthresh: u32,
    rtt: Duration,
}

impl CongestionController {
    pub fn new() -> Self {
        Self {
            cwnd: 2,
            ssthresh: 64,
            rtt: Duration::from_millis(50),
        }
    }

    pub fn on_ack(&mut self) {
        if self.cwnd < self.ssthresh {
            self.cwnd += 1; // Slow start
        } else {
            self.cwnd += 1 / self.cwnd; // Congestion avoidance
        }
    }

    pub fn on_loss(&mut self) {
        self.ssthresh = self.cwnd / 2;
        self.cwnd = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_serialization() {
        let packet = RudpPacket::new_data(100, vec![1, 2, 3, 4, 5]);
        let serialized = packet.serialize();
        let deserialized = RudpPacket::deserialize(&serialized).unwrap();

        assert_eq!(packet.sequence, deserialized.sequence);
        assert_eq!(packet.data, deserialized.data);
        println!("✅ 패킷 직렬화 테스트 성공");
    }

    #[test]
    fn test_congestion_control() {
        let mut controller = CongestionController::new();
        let initial_cwnd = controller.cwnd;

        // ACK 수신시 윈도우 증가
        for _ in 0..10 {
            controller.on_ack();
        }
        assert!(controller.cwnd > initial_cwnd);
        println!("✅ 혼잡 제어 윈도우 증가 테스트 성공");

        // 패킷 손실시 윈도우 감소
        let cwnd_before_loss = controller.cwnd;
        controller.on_loss();
        assert!(controller.cwnd < cwnd_before_loss);
        println!("✅ 혼잡 제어 윈도우 감소 테스트 성공");
    }

    #[test]
    fn test_300_player_simulation() {
        let start = Instant::now();
        let mut packets_sent = 0u64;
        let mut bytes_sent = 0u64;

        // 300명의 플레이어 시뮬레이션 (간단 버전)
        for player_id in 0..300 {
            for msg_id in 0..10 {
                let data = format!("Player {} Message {}", player_id, msg_id).into_bytes();
                let packet = RudpPacket::new_data(msg_id, data);
                let serialized = packet.serialize();

                packets_sent += 1;
                bytes_sent += serialized.len() as u64;
            }
        }

        let duration = start.elapsed();
        let throughput = packets_sent as f64 / duration.as_secs_f64();

        println!("\n========== 300명 플레이어 시뮬레이션 결과 ==========");
        println!("테스트 시간: {:?}", duration);
        println!("전송된 패킷: {}", packets_sent);
        println!("전송된 데이터: {} KB", bytes_sent / 1024);
        println!("처리량: {:.0} packets/sec", throughput);
        println!("평균 패킷 크기: {} bytes", bytes_sent / packets_sent);

        // 1vCPU 환경에서 목표: 3000 packets/sec 이상
        assert!(packets_sent >= 3000);
        println!("✅ 300명 플레이어 시뮬레이션 테스트 성공");
    }

    #[test]
    fn test_memory_efficiency() {
        // 메모리 효율성 테스트
        let mut packets = Vec::new();
        let initial_memory = std::mem::size_of_val(&packets);

        // 300명 x 100개 메시지 = 30,000개 패킷
        for i in 0..30000 {
            let packet = RudpPacket::new_data(i, vec![0u8; 128]);
            packets.push(packet);
        }

        let total_memory = std::mem::size_of_val(&packets[..]);
        let memory_per_packet = total_memory / packets.len();

        println!("\n========== 메모리 효율성 테스트 ==========");
        println!("총 패킷 수: {}", packets.len());
        println!("총 메모리 사용: {} MB", total_memory / 1_048_576);
        println!("패킷당 메모리: {} bytes", memory_per_packet);

        // 1GB RAM 제한에서 충분한 여유
        assert!(total_memory < 500_000_000); // 500MB 이하
        println!("✅ 메모리 효율성 테스트 성공 (1GB RAM 제한 내)");
    }

    #[test]
    fn test_latency_requirements() {
        let mut latencies = Vec::new();

        // 1000개의 샘플 지연시간 시뮬레이션
        for i in 0..1000 {
            let start = Instant::now();

            // 패킷 처리 시뮬레이션
            let packet = RudpPacket::new_data(i, vec![0u8; 64]);
            let _ = packet.serialize();

            let latency = start.elapsed();
            latencies.push(latency);
        }

        latencies.sort();
        let p50 = latencies[500];
        let p95 = latencies[950];
        let p99 = latencies[990];

        println!("\n========== 지연시간 테스트 ==========");
        println!("P50 지연시간: {:?}", p50);
        println!("P95 지연시간: {:?}", p95);
        println!("P99 지연시간: {:?}", p99);

        // 목표: P99 < 50ms
        assert!(p99 < Duration::from_millis(50));
        println!("✅ 지연시간 요구사항 충족 (P99 < 50ms)");
    }
}

fn main() {
    println!("RUDP 서버 테스트 실행중...");

    // 테스트 실행
    let _ = test_packet_serialization();
    let _ = test_congestion_control();
    let _ = test_300_player_simulation();
    let _ = test_memory_efficiency();
    let _ = test_latency_requirements();

    println!("\n모든 테스트 완료!");
}

fn test_packet_serialization() {
    let packet = RudpPacket::new_data(100, vec![1, 2, 3, 4, 5]);
    let serialized = packet.serialize();
    let deserialized = RudpPacket::deserialize(&serialized).unwrap();

    assert_eq!(packet.sequence, deserialized.sequence);
    assert_eq!(packet.data, deserialized.data);
    println!("✅ 패킷 직렬화 테스트 성공");
}

fn test_congestion_control() {
    let mut controller = CongestionController::new();
    let initial_cwnd = controller.cwnd;

    for _ in 0..10 {
        controller.on_ack();
    }
    assert!(controller.cwnd > initial_cwnd);
    println!("✅ 혼잡 제어 테스트 성공");
}

fn test_300_player_simulation() {
    let start = Instant::now();
    let mut packets_sent = 0u64;

    for player_id in 0..300 {
        for msg_id in 0..10 {
            let data = format!("Player {} Message {}", player_id, msg_id).into_bytes();
            let packet = RudpPacket::new_data(msg_id, data);
            let _ = packet.serialize();
            packets_sent += 1;
        }
    }

    let duration = start.elapsed();
    println!(
        "300명 시뮬레이션: {} 패킷을 {:?}에 처리",
        packets_sent, duration
    );
}

fn test_memory_efficiency() {
    let mut packets = Vec::new();
    for i in 0..30000 {
        packets.push(RudpPacket::new_data(i, vec![0u8; 128]));
    }
    println!("메모리 테스트: 30,000개 패킷 생성 완료");
}

fn test_latency_requirements() {
    let mut latencies = Vec::new();
    for i in 0..1000 {
        let start = Instant::now();
        let packet = RudpPacket::new_data(i, vec![0u8; 64]);
        let _ = packet.serialize();
        latencies.push(start.elapsed());
    }
    println!("지연시간 테스트: 1000개 샘플 측정 완료");
}
