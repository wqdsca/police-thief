use rudpserver::congestion::{CongestionController, CongestionState};
use rudpserver::protocol::{AckRange, PacketType, RudpPacket};
use rudpserver::reliability::{PacketState, ReliabilityManager};
use std::time::{Duration, Instant};

#[cfg(test)]
mod protocol_tests {
    use super::*;

    #[test]
    fn test_packet_serialization() {
        let packet = RudpPacket {
            packet_type: PacketType::Data,
            sequence: 12345,
            ack: Some(12340),
            ack_ranges: vec![
                AckRange {
                    start: 12330,
                    end: 12335,
                },
                AckRange {
                    start: 12337,
                    end: 12339,
                },
            ],
            timestamp: 1000,
            data: vec![1, 2, 3, 4, 5],
        };

        let serialized = packet.serialize();
        let deserialized = RudpPacket::deserialize(&serialized).unwrap();

        assert_eq!(packet.packet_type, deserialized.packet_type);
        assert_eq!(packet.sequence, deserialized.sequence);
        assert_eq!(packet.ack, deserialized.ack);
        assert_eq!(packet.ack_ranges.len(), deserialized.ack_ranges.len());
        assert_eq!(packet.data, deserialized.data);
    }

    #[test]
    fn test_packet_types() {
        let syn = RudpPacket::new_syn(0);
        assert_eq!(syn.packet_type, PacketType::Syn);

        let syn_ack = RudpPacket::new_syn_ack(1, 0);
        assert_eq!(syn_ack.packet_type, PacketType::SynAck);
        assert_eq!(syn_ack.ack, Some(0));

        let data = RudpPacket::new_data(2, vec![1, 2, 3]);
        assert_eq!(data.packet_type, PacketType::Data);
        assert_eq!(data.data, vec![1, 2, 3]);

        let keepalive = RudpPacket::new_keepalive(3);
        assert_eq!(keepalive.packet_type, PacketType::KeepAlive);
    }

    #[test]
    fn test_large_packet_handling() {
        let large_data = vec![0u8; 65536];
        let packet = RudpPacket::new_data(100, large_data.clone());

        let serialized = packet.serialize();
        let deserialized = RudpPacket::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.data.len(), 65536);
        assert_eq!(deserialized.data, large_data);
    }

    #[test]
    fn test_ack_range_optimization() {
        let mut ranges = vec![
            AckRange { start: 1, end: 5 },
            AckRange { start: 6, end: 10 },
            AckRange { start: 12, end: 15 },
            AckRange { start: 16, end: 20 },
        ];

        RudpPacket::optimize_ack_ranges(&mut ranges);

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], AckRange { start: 1, end: 10 });
        assert_eq!(ranges[1], AckRange { start: 12, end: 20 });
    }

    #[test]
    fn test_packet_validation() {
        let mut packet = RudpPacket::new_data(100, vec![1, 2, 3]);
        assert!(packet.is_valid());

        packet.sequence = u32::MAX;
        assert!(packet.is_valid());

        packet.packet_type = PacketType::Invalid;
        assert!(!packet.is_valid());
    }

    #[test]
    fn test_connection_handshake_sequence() {
        let syn = RudpPacket::new_syn(0);
        assert!(syn.is_connection_packet());

        let syn_ack = RudpPacket::new_syn_ack(0, 0);
        assert!(syn_ack.is_connection_packet());

        let ack = RudpPacket::new_ack(1, 0);
        assert!(ack.is_connection_packet());

        let data = RudpPacket::new_data(2, vec![1, 2, 3]);
        assert!(!data.is_connection_packet());
    }
}

#[cfg(test)]
mod congestion_tests {
    use super::*;

    #[test]
    fn test_congestion_state_transitions() {
        let mut controller = CongestionController::new();

        assert_eq!(controller.state(), CongestionState::SlowStart);
        assert_eq!(controller.cwnd(), 2);

        for _ in 0..10 {
            controller.on_ack_received(1);
        }
        assert!(controller.cwnd() > 10);

        controller.on_packet_loss();
        assert_eq!(controller.state(), CongestionState::FastRecovery);
        assert!(controller.cwnd() < 10);

        for _ in 0..5 {
            controller.on_ack_received(1);
        }
        assert_eq!(controller.state(), CongestionState::CongestionAvoidance);
    }

    #[test]
    fn test_rtt_calculation() {
        let mut controller = CongestionController::new();

        controller.update_rtt(Duration::from_millis(50));
        assert!(controller.srtt() > Duration::ZERO);
        assert!(controller.rttvar() > Duration::ZERO);

        for i in 1..100 {
            controller.update_rtt(Duration::from_millis(50 + i % 10));
        }

        let srtt = controller.srtt();
        assert!(srtt > Duration::from_millis(45));
        assert!(srtt < Duration::from_millis(65));
    }

    #[test]
    fn test_bandwidth_estimation() {
        let mut controller = CongestionController::new();

        controller.update_bandwidth(1000, Duration::from_millis(100));
        controller.update_bandwidth(2000, Duration::from_millis(200));
        controller.update_bandwidth(1500, Duration::from_millis(300));

        let bandwidth = controller.estimated_bandwidth();
        assert!(bandwidth > 0.0);
        assert!(bandwidth < 100_000.0);
    }

    #[test]
    fn test_congestion_window_limits() {
        let mut controller = CongestionController::new();

        for _ in 0..1000 {
            controller.on_ack_received(1);
        }

        assert!(controller.cwnd() <= CongestionController::MAX_CWND);

        for _ in 0..100 {
            controller.on_packet_loss();
        }

        assert!(controller.cwnd() >= CongestionController::MIN_CWND);
    }

    #[test]
    fn test_timeout_calculation() {
        let mut controller = CongestionController::new();

        controller.update_rtt(Duration::from_millis(100));
        let rto = controller.calculate_rto();

        assert!(rto >= Duration::from_millis(200));
        assert!(rto <= Duration::from_secs(1));

        for _ in 0..10 {
            controller.on_timeout();
        }

        let rto_after = controller.calculate_rto();
        assert!(rto_after > rto);
    }
}

#[cfg(test)]
mod reliability_tests {
    use super::*;

    #[test]
    fn test_packet_tracking() {
        let mut manager = ReliabilityManager::new();

        let seq = manager.send_packet(vec![1, 2, 3], true);
        assert_eq!(seq, 0);

        let state = manager.get_packet_state(seq);
        assert_eq!(state, Some(PacketState::Sent));

        manager.mark_acked(seq);
        let state = manager.get_packet_state(seq);
        assert_eq!(state, Some(PacketState::Acked));
    }

    #[test]
    fn test_retransmission_logic() {
        let mut manager = ReliabilityManager::new();

        let seq1 = manager.send_packet(vec![1], true);
        let seq2 = manager.send_packet(vec![2], false);
        let seq3 = manager.send_packet(vec![3], true);

        std::thread::sleep(Duration::from_millis(100));

        let retransmit = manager.get_packets_for_retransmission(Duration::from_millis(50));
        assert_eq!(retransmit.len(), 2);
        assert!(retransmit.contains(&seq1));
        assert!(retransmit.contains(&seq3));
        assert!(!retransmit.contains(&seq2));
    }

    #[test]
    fn test_selective_ack_processing() {
        let mut manager = ReliabilityManager::new();

        for i in 0..10 {
            manager.send_packet(vec![i], true);
        }

        manager.process_sack(vec![
            AckRange { start: 0, end: 3 },
            AckRange { start: 5, end: 7 },
            AckRange { start: 9, end: 9 },
        ]);

        assert_eq!(manager.get_packet_state(0), Some(PacketState::Acked));
        assert_eq!(manager.get_packet_state(4), Some(PacketState::Sent));
        assert_eq!(manager.get_packet_state(8), Some(PacketState::Sent));
        assert_eq!(manager.get_packet_state(9), Some(PacketState::Acked));
    }

    #[test]
    fn test_duplicate_detection() {
        let mut manager = ReliabilityManager::new();

        assert!(!manager.is_duplicate(100));
        manager.mark_received(100);
        assert!(manager.is_duplicate(100));

        assert!(!manager.is_duplicate(101));
        assert!(!manager.is_duplicate(99));
    }

    #[test]
    fn test_window_management() {
        let mut manager = ReliabilityManager::new();
        manager.set_window_size(5);

        let mut sent = vec![];
        for i in 0..10 {
            if manager.can_send() {
                let seq = manager.send_packet(vec![i], true);
                sent.push(seq);
            }
        }

        assert_eq!(sent.len(), 5);

        manager.mark_acked(sent[0]);
        assert!(manager.can_send());

        let seq = manager.send_packet(vec![10], true);
        assert_eq!(seq, 5);
    }

    #[test]
    fn test_out_of_order_handling() {
        let mut manager = ReliabilityManager::new();

        manager.receive_packet(5, vec![5]);
        manager.receive_packet(3, vec![3]);
        manager.receive_packet(4, vec![4]);
        manager.receive_packet(2, vec![2]);
        manager.receive_packet(1, vec![1]);

        let ordered = manager.get_ordered_packets();
        assert_eq!(ordered.len(), 5);
        for i in 0..5 {
            assert_eq!(ordered[i], vec![i as u8 + 1]);
        }
    }
}
