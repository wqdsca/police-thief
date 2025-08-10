use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use rudpserver::congestion::CongestionController;
use rudpserver::optimization::*;
use rudpserver::protocol::{AckRange, PacketType, RudpPacket};
use rudpserver::reliability::ReliabilityManager;
use std::time::Duration;

fn benchmark_packet_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_serialization");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];
        let packet = RudpPacket::new_data(12345, data);

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let serialized = packet.serialize();
                black_box(serialized);
            });
        });
    }

    group.finish();
}

fn benchmark_packet_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_deserialization");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];
        let packet = RudpPacket::new_data(12345, data);
        let serialized = packet.serialize();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &serialized,
            |b, serialized| {
                b.iter(|| {
                    let deserialized = RudpPacket::deserialize(serialized).unwrap();
                    black_box(deserialized);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_congestion_control(c: &mut Criterion) {
    let mut group = c.benchmark_group("congestion_control");

    group.bench_function("rtt_update", |b| {
        let mut controller = CongestionController::new();
        b.iter(|| {
            controller.update_rtt(Duration::from_millis(50));
        });
    });

    group.bench_function("ack_processing", |b| {
        let mut controller = CongestionController::new();
        b.iter(|| {
            controller.on_ack_received(1);
        });
    });

    group.bench_function("loss_detection", |b| {
        let mut controller = CongestionController::new();
        b.iter(|| {
            controller.on_packet_loss();
        });
    });

    group.bench_function("bandwidth_estimation", |b| {
        let mut controller = CongestionController::new();
        b.iter(|| {
            controller.update_bandwidth(1500, Duration::from_millis(10));
        });
    });

    group.finish();
}

fn benchmark_reliability_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("reliability");

    group.bench_function("packet_tracking", |b| {
        let mut manager = ReliabilityManager::new();
        b.iter(|| {
            let seq = manager.send_packet(vec![1, 2, 3], true);
            black_box(seq);
        });
    });

    group.bench_function("ack_processing", |b| {
        let mut manager = ReliabilityManager::new();
        for i in 0..100 {
            manager.send_packet(vec![i as u8], true);
        }

        b.iter(|| {
            manager.mark_acked(black_box(50));
        });
    });

    group.bench_function("sack_processing", |b| {
        let mut manager = ReliabilityManager::new();
        for i in 0..1000 {
            manager.send_packet(vec![i as u8], true);
        }

        let ranges = vec![
            AckRange {
                start: 100,
                end: 200,
            },
            AckRange {
                start: 300,
                end: 400,
            },
            AckRange {
                start: 500,
                end: 600,
            },
        ];

        b.iter(|| {
            manager.process_sack(ranges.clone());
        });
    });

    group.bench_function("retransmission_check", |b| {
        let mut manager = ReliabilityManager::new();
        for i in 0..100 {
            manager.send_packet(vec![i as u8], true);
        }

        b.iter(|| {
            let packets = manager.get_packets_for_retransmission(Duration::from_millis(100));
            black_box(packets);
        });
    });

    group.finish();
}

fn benchmark_memory_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool");

    group.bench_function("buffer_allocation", |b| {
        let pool = MemoryPool::new(1024, 100);
        b.iter(|| {
            let buffer = pool.allocate();
            black_box(buffer);
        });
    });

    group.bench_function("buffer_reuse", |b| {
        let pool = MemoryPool::new(1024, 10);
        let buffers: Vec<_> = (0..10).map(|_| pool.allocate()).collect();

        b.iter(|| {
            for buffer in &buffers {
                pool.recycle(buffer.clone());
            }
            for _ in 0..10 {
                let buffer = pool.allocate();
                black_box(buffer);
            }
        });
    });

    group.finish();
}

fn benchmark_simd_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd");

    for size in [128, 512, 2048, 8192].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("checksum", size), &data, |b, data| {
            b.iter(|| {
                let checksum = simd_checksum(data);
                black_box(checksum);
            });
        });

        group.bench_with_input(BenchmarkId::new("xor", size), &data, |b, data| {
            b.iter(|| {
                let result = simd_xor(data, 0xFF);
                black_box(result);
            });
        });
    }

    group.finish();
}

fn benchmark_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    for size in [256, 1024, 4096, 16384].iter() {
        let mut rng = rand::thread_rng();
        let data: Vec<u8> = (0..*size).map(|_| rng.gen()).collect();

        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("lz4_compress", size), &data, |b, data| {
            b.iter(|| {
                let compressed = lz4_compress(data);
                black_box(compressed);
            });
        });

        let compressed = lz4_compress(&data);
        group.bench_with_input(
            BenchmarkId::new("lz4_decompress", size),
            &compressed,
            |b, compressed| {
                b.iter(|| {
                    let decompressed = lz4_decompress(compressed);
                    black_box(decompressed);
                });
            },
        );
    }

    group.finish();
}

fn benchmark_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent");

    group.bench_function("dashmap_insert", |b| {
        let map = dashmap::DashMap::new();
        let mut counter = 0;
        b.iter(|| {
            map.insert(counter, vec![0u8; 128]);
            counter += 1;
        });
    });

    group.bench_function("dashmap_get", |b| {
        let map = dashmap::DashMap::new();
        for i in 0..1000 {
            map.insert(i, vec![0u8; 128]);
        }

        b.iter(|| {
            let value = map.get(&500);
            black_box(value);
        });
    });

    group.bench_function("atomic_stats_update", |b| {
        use std::sync::atomic::{AtomicU64, Ordering};
        let stats = AtomicU64::new(0);

        b.iter(|| {
            stats.fetch_add(1, Ordering::Relaxed);
        });
    });

    group.finish();
}

fn benchmark_game_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("game_scenarios");

    group.bench_function("player_update_300", |b| {
        let mut manager = ReliabilityManager::new();
        let mut controller = CongestionController::new();

        let player_data = vec![0u8; 128];

        b.iter(|| {
            for _ in 0..300 {
                let seq = manager.send_packet(player_data.clone(), false);
                controller.on_ack_received(1);
                black_box(seq);
            }
        });
    });

    group.bench_function("broadcast_300_players", |b| {
        let connections: Vec<_> = (0..300).map(|i| (i, vec![0u8; 64])).collect();

        b.iter(|| {
            for (id, data) in &connections {
                black_box(id);
                black_box(data);
            }
        });
    });

    group.bench_function("collision_detection_300", |b| {
        let positions: Vec<(f32, f32)> = (0..300)
            .map(|i| (i as f32 * 10.0, i as f32 * 10.0))
            .collect();

        b.iter(|| {
            for i in 0..positions.len() {
                for j in i + 1..positions.len() {
                    let dx = positions[i].0 - positions[j].0;
                    let dy = positions[i].1 - positions[j].1;
                    let distance_sq = dx * dx + dy * dy;
                    black_box(distance_sq < 100.0);
                }
            }
        });
    });

    group.finish();
}

fn benchmark_resource_constrained(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_constrained");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    group.bench_function("1vcpu_simulation", |b| {
        use std::thread;
        use std::time::Duration;

        b.iter(|| {
            thread::sleep(Duration::from_micros(3));
        });
    });

    group.bench_function("memory_pressure_1gb", |b| {
        let allocations: Vec<Vec<u8>> = (0..100).map(|_| vec![0u8; 1024]).collect();

        b.iter(|| {
            for alloc in &allocations {
                black_box(alloc.len());
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_packet_serialization,
    benchmark_packet_deserialization,
    benchmark_congestion_control,
    benchmark_reliability_manager,
    benchmark_memory_pool,
    benchmark_simd_operations,
    benchmark_compression,
    benchmark_concurrent_operations,
    benchmark_game_scenarios,
    benchmark_resource_constrained
);

criterion_main!(benches);
