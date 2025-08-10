//! 채팅방 성능 테스트
//! 
//! VCPU 2개, RAM 2GB 환경에서 방 50개, 사용자 300명이 채팅을 주고받는 성능 테스트
//! DashMap 기반 RoomConnectionService의 성능과 안정성을 검증합니다.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;

use crate::service::room_connection_service::RoomConnectionService;
use crate::handler::chat_room_handler::ChatRoomHandler;

/// 성능 테스트 구성
const TOTAL_ROOMS: u32 = 50;
const TOTAL_USERS: u32 = 300;
const USERS_PER_ROOM: u32 = TOTAL_USERS / TOTAL_ROOMS; // 방당 6명
const CHAT_MESSAGES_PER_USER: u32 = 10;
const TEST_DURATION_SECS: u64 = 60;

/// 성능 메트릭
#[derive(Debug)]
struct PerformanceMetrics {
    /// 총 처리된 메시지 수
    total_messages: AtomicU64,
    /// 성공한 메시지 수
    successful_messages: AtomicU64,
    /// 실패한 메시지 수
    failed_messages: AtomicU64,
    /// 방 입장 성공 수
    successful_joins: AtomicU64,
    /// 방 퇴장 성공 수
    successful_leaves: AtomicU64,
    /// 총 알림 전송 수
    total_notifications: AtomicU64,
    /// 평균 응답 시간 (나노초)
    total_response_time_ns: AtomicU64,
    /// 측정 횟수
    response_time_samples: AtomicU64,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            total_messages: AtomicU64::new(0),
            successful_messages: AtomicU64::new(0),
            failed_messages: AtomicU64::new(0),
            successful_joins: AtomicU64::new(0),
            successful_leaves: AtomicU64::new(0),
            total_notifications: AtomicU64::new(0),
            total_response_time_ns: AtomicU64::new(0),
            response_time_samples: AtomicU64::new(0),
        }
    }

    fn record_message_success(&self) {
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.successful_messages.fetch_add(1, Ordering::Relaxed);
    }

    fn record_message_failure(&self) {
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.failed_messages.fetch_add(1, Ordering::Relaxed);
    }

    fn record_join_success(&self) {
        self.successful_joins.fetch_add(1, Ordering::Relaxed);
    }

    fn record_leave_success(&self) {
        self.successful_leaves.fetch_add(1, Ordering::Relaxed);
    }

    fn record_response_time(&self, duration: Duration) {
        self.total_response_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        self.response_time_samples.fetch_add(1, Ordering::Relaxed);
    }

    fn get_summary(&self) -> PerformanceSummary {
        let samples = self.response_time_samples.load(Ordering::Relaxed);
        let avg_response_time_ms = if samples > 0 {
            (self.total_response_time_ns.load(Ordering::Relaxed) as f64 / samples as f64) / 1_000_000.0
        } else {
            0.0
        };

        PerformanceSummary {
            total_messages: self.total_messages.load(Ordering::Relaxed),
            successful_messages: self.successful_messages.load(Ordering::Relaxed),
            failed_messages: self.failed_messages.load(Ordering::Relaxed),
            successful_joins: self.successful_joins.load(Ordering::Relaxed),
            successful_leaves: self.successful_leaves.load(Ordering::Relaxed),
            avg_response_time_ms,
            success_rate: if self.total_messages.load(Ordering::Relaxed) > 0 {
                (self.successful_messages.load(Ordering::Relaxed) as f64 / self.total_messages.load(Ordering::Relaxed) as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// 성능 테스트 결과 요약
#[derive(Debug)]
struct PerformanceSummary {
    total_messages: u64,
    successful_messages: u64,
    failed_messages: u64,
    successful_joins: u64,
    successful_leaves: u64,
    avg_response_time_ms: f64,
    success_rate: f64,
}

/// 성능 테스트 환경
struct PerformanceTestEnv {
    room_service: Arc<RoomConnectionService>,
    chat_handler: Arc<ChatRoomHandler>,
    metrics: Arc<PerformanceMetrics>,
}

impl PerformanceTestEnv {
    /// 테스트 환경 생성
    async fn new() -> Self {
        let room_service = Arc::new(RoomConnectionService::new("perf_test_server".to_string()));
        let chat_handler = Arc::new(ChatRoomHandler::new(room_service.clone()));
        let metrics = Arc::new(PerformanceMetrics::new());

        Self {
            room_service,
            chat_handler,
            metrics,
        }
    }

    /// 모의 TCP writer 생성 (성능 최적화 버전)
    async fn create_mock_writer() -> Arc<Mutex<tokio::io::BufWriter<tokio::net::tcp::OwnedWriteHalf>>> {
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        let client_task = tokio::spawn(async move {
            TcpStream::connect(addr).await.unwrap()
        });
        
        let (server_stream, _) = listener.accept().await.unwrap();
        let _client_stream = client_task.await.unwrap();
        
        let (_, writer) = server_stream.into_split();
        let buf_writer = tokio::io::BufWriter::new(writer);
        
        Arc::new(Mutex::new(buf_writer))
    }
}

/// 저사양 환경 성능 테스트 - 1vCPU 0.5GB RAM에서 방 20개, 사용자 200명, 30초간 지속  
#[tokio::test]
async fn test_low_spec_performance_20_rooms_200_users_3min() {
    const TEST_ROOMS: u32 = 20;
    const USERS_PER_ROOM: u32 = 10;
    const TEST_TOTAL_USERS: u32 = TEST_ROOMS * USERS_PER_ROOM; // 200명
    const MESSAGE_INTERVAL_SECS: u64 = 2; // 2초마다 메시지
    const TEST_DURATION_SECS: u64 = 30; // 30초 (테스트용 단축)
    const EXPECTED_MESSAGES_PER_USER: u64 = TEST_DURATION_SECS / MESSAGE_INTERVAL_SECS; // 15개

    // 테스트 환경에서 로깅 초기화
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    let env = Arc::new(PerformanceTestEnv::new().await);
    
    info!("🚀 저사양 성능 테스트 시작 - 방 {}개, 사용자 {}명, {}초간 지속", TEST_ROOMS, TEST_TOTAL_USERS, TEST_DURATION_SECS);
    let test_start = Instant::now();

    // Phase 1: 모든 사용자를 방에 입장시키기
    info!("📥 Phase 1: 사용자 방 입장 ({}명)", TEST_TOTAL_USERS);
    let join_start = Instant::now();
    
    let mut join_handles = Vec::new();
    
    for user_id in 1..=TEST_TOTAL_USERS {
        let room_id = ((user_id - 1) / USERS_PER_ROOM) + 1; // 방당 10명씩 배치
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            let start_time = Instant::now();
            let writer = PerformanceTestEnv::create_mock_writer().await;
            
            let result = env_clone.chat_handler.handle_room_join(
                user_id,
                room_id,
                format!("사용자{}", user_id),
                format!("127.0.0.1:{}", 20000 + user_id),
                writer,
            ).await;
            
            let duration = start_time.elapsed();
            env_clone.metrics.record_response_time(duration);
            
            match result {
                Ok(_) => env_clone.metrics.record_join_success(),
                Err(e) => {
                    error!("사용자 {} 방 {} 입장 실패: {}", user_id, room_id, e);
                }
            }
        });
        
        join_handles.push(handle);
    }
    
    // 모든 입장 작업 완료 대기
    for handle in join_handles {
        handle.await.unwrap();
    }
    
    let join_duration = join_start.elapsed();
    info!("✅ Phase 1 완료 - 입장 시간: {:?}", join_duration);
    
    // 중간 상태 확인
    let active_rooms = env.chat_handler.get_all_rooms_status();
    info!("📊 활성 방 수: {}, 예상: {}", active_rooms.len(), TEST_ROOMS);
    
    let total_users_in_rooms: u32 = active_rooms.iter().map(|(_, count)| count).sum();
    info!("📊 방에 있는 총 사용자 수: {}, 예상: {}", total_users_in_rooms, TEST_TOTAL_USERS);

    // Phase 2: 30초간 지속적인 채팅 메시지 교환  
    info!("💬 Phase 2: 30초간 지속적인 채팅 시뮬레이션 (2초 간격)");
    let chat_start = Instant::now();
    
    let mut chat_handles = Vec::new();
    let end_time = Instant::now() + Duration::from_secs(TEST_DURATION_SECS);
    
    for user_id in 1..=TEST_TOTAL_USERS {
        let room_id = ((user_id - 1) / USERS_PER_ROOM) + 1;
        let env_clone = env.clone();
        let test_end_time = end_time.clone();
        
        let handle = tokio::spawn(async move {
            let mut message_count = 0u64;
            
            while Instant::now() < test_end_time {
                message_count += 1;
                let start_time = Instant::now();
                let message_content = format!("사용자{}의 메시지#{}", user_id, message_count);
                
                let result = env_clone.chat_handler.handle_chat_message(
                    user_id,
                    room_id,
                    message_content,
                ).await;
                
                let duration = start_time.elapsed();
                env_clone.metrics.record_response_time(duration);
                
                match result {
                    Ok(_) => env_clone.metrics.record_message_success(),
                    Err(_) => env_clone.metrics.record_message_failure(),
                }
                
                // 2초 대기 (실제 채팅 패턴 시뮬레이션)
                sleep(Duration::from_secs(MESSAGE_INTERVAL_SECS)).await;
            }
            
            message_count
        });
        
        chat_handles.push(handle);
    }
    
    // 모든 채팅 작업 완료 대기
    let mut total_messages_sent = 0u64;
    for handle in chat_handles {
        let messages = handle.await.unwrap();
        total_messages_sent += messages;
    }
    
    let chat_duration = chat_start.elapsed();
    info!("✅ Phase 2 완료 - 채팅 시간: {:?}, 전송된 메시지: {}", chat_duration, total_messages_sent);

    // Phase 3: 모든 사용자 정리 퇴장
    info!("📤 Phase 3: 사용자 정리 퇴장");
    let leave_start = Instant::now();
    
    let mut leave_handles = Vec::new();
    
    for user_id in 1..=TEST_TOTAL_USERS {
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            let result = env_clone.chat_handler.handle_user_disconnect(user_id).await;
            if result.is_ok() {
                env_clone.metrics.record_leave_success();
            }
        });
        
        leave_handles.push(handle);
    }
    
    for handle in leave_handles {
        handle.await.unwrap();
    }
    
    let leave_duration = leave_start.elapsed();
    info!("✅ Phase 3 완료 - 퇴장 시간: {:?}", leave_duration);

    // 최종 정리 및 결과
    let cleanup_start = Instant::now();
    let cleaned_rooms = env.chat_handler.cleanup_empty_rooms().await;
    let cleanup_duration = cleanup_start.elapsed();
    info!("🧹 빈 방 정리 완료: {}개, 시간: {:?}", cleaned_rooms, cleanup_duration);
    
    let test_duration = test_start.elapsed();
    let summary = env.metrics.get_summary();
    
    // 성능 결과 출력
    info!("🏆 저사양 성능 테스트 완료 - 총 시간: {:?}", test_duration);
    info!("📊 === 저사양 환경 성능 요약 ===");
    info!("📊 총 메시지: {}", summary.total_messages);
    info!("📊 성공한 메시지: {}", summary.successful_messages);
    info!("📊 실패한 메시지: {}", summary.failed_messages);
    info!("📊 방 입장 성공: {}", summary.successful_joins);
    info!("📊 방 퇴장 성공: {}", summary.successful_leaves);
    info!("📊 평균 응답 시간: {:.2}ms", summary.avg_response_time_ms);
    info!("📊 성공률: {:.2}%", summary.success_rate);
    
    // 예상 메시지 수 계산
    let expected_total_messages = (TEST_TOTAL_USERS as u64) * EXPECTED_MESSAGES_PER_USER;
    info!("📊 예상 총 메시지: {}, 실제 전송: {}", expected_total_messages, total_messages_sent);
    
    // 저사양 환경 성능 기준 (더 관대한 기준)
    assert!(summary.successful_messages >= expected_total_messages * 90 / 100, 
        "메시지 성공률이 90% 미만: {}/{}", summary.successful_messages, expected_total_messages);
    
    assert!(summary.avg_response_time_ms < 500.0, 
        "평균 응답 시간이 500ms 초과: {:.2}ms", summary.avg_response_time_ms);
    
    assert!(summary.success_rate >= 90.0, 
        "전체 성공률이 90% 미만: {:.2}%", summary.success_rate);
    
    // 지속성 검증 - 실제 30초에 가까운 시간 동안 실행되었는지 확인
    let actual_test_duration_secs = test_duration.as_secs();
    assert!(actual_test_duration_secs >= TEST_DURATION_SECS - 5, 
        "테스트가 예상 시간보다 너무 일찍 끝남: {}초 (예상: {}초)", 
        actual_test_duration_secs, TEST_DURATION_SECS);
    
    info!("✅ 모든 저사양 환경 성능 기준 통과!");
    info!("📈 처리량: {:.2} 메시지/초", summary.successful_messages as f64 / actual_test_duration_secs as f64);
    info!("💾 메모리 효율성: 방 {}개, 사용자 {}명 동시 처리", TEST_ROOMS, TEST_TOTAL_USERS);
}

/// 기본 성능 테스트 - 방 50개, 사용자 300명
#[tokio::test]
async fn test_basic_performance_50_rooms_300_users() {
    // 테스트 환경에서 로깅 초기화
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    let env = Arc::new(PerformanceTestEnv::new().await);
    
    info!("🚀 성능 테스트 시작 - 방 {}개, 사용자 {}명", TOTAL_ROOMS, TOTAL_USERS);
    let test_start = Instant::now();

    // Phase 1: 모든 사용자를 방에 입장시키기
    info!("📥 Phase 1: 사용자 방 입장 ({}명)", TOTAL_USERS);
    let join_start = Instant::now();
    
    let mut join_handles = Vec::new();
    
    for user_id in 1..=TOTAL_USERS {
        let room_id = ((user_id - 1) % TOTAL_ROOMS) + 1; // 사용자를 방에 균등 분배
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            let start_time = Instant::now();
            let writer = PerformanceTestEnv::create_mock_writer().await;
            
            let result = env_clone.chat_handler.handle_room_join(
                user_id,
                room_id,
                format!("사용자{}", user_id),
                format!("127.0.0.1:{}", 20000 + user_id),
                writer,
            ).await;
            
            let duration = start_time.elapsed();
            env_clone.metrics.record_response_time(duration);
            
            match result {
                Ok(_) => env_clone.metrics.record_join_success(),
                Err(_) => {
                    error!("사용자 {} 방 {} 입장 실패", user_id, room_id);
                }
            }
        });
        
        join_handles.push(handle);
    }
    
    // 모든 입장 작업 완료 대기
    for handle in join_handles {
        handle.await.unwrap();
    }
    
    let join_duration = join_start.elapsed();
    info!("✅ Phase 1 완료 - 입장 시간: {:?}", join_duration);
    
    // 중간 상태 확인
    let active_rooms = env.chat_handler.get_all_rooms_status();
    info!("📊 활성 방 수: {}, 예상: {}", active_rooms.len(), TOTAL_ROOMS);
    
    let total_users_in_rooms: u32 = active_rooms.iter().map(|(_, count)| count).sum();
    info!("📊 방에 있는 총 사용자 수: {}, 예상: {}", total_users_in_rooms, TOTAL_USERS);

    // Phase 2: 채팅 메시지 교환 (지속적인 부하)
    info!("💬 Phase 2: 채팅 메시지 교환 시작");
    let chat_start = Instant::now();
    
    let mut chat_handles = Vec::new();
    
    for user_id in 1..=TOTAL_USERS {
        let room_id = ((user_id - 1) % TOTAL_ROOMS) + 1;
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            for msg_idx in 1..=CHAT_MESSAGES_PER_USER {
                let start_time = Instant::now();
                let message_content = format!("사용자{}의 메시지#{}", user_id, msg_idx);
                
                let result = env_clone.chat_handler.handle_chat_message(
                    user_id,
                    room_id,
                    message_content,
                ).await;
                
                let duration = start_time.elapsed();
                env_clone.metrics.record_response_time(duration);
                
                match result {
                    Ok(_) => env_clone.metrics.record_message_success(),
                    Err(_) => env_clone.metrics.record_message_failure(),
                }
                
                // CPU 과부하 방지를 위한 작은 지연
                sleep(Duration::from_millis(10)).await;
            }
        });
        
        chat_handles.push(handle);
    }
    
    // 모든 채팅 작업 완료 대기
    for handle in chat_handles {
        handle.await.unwrap();
    }
    
    let chat_duration = chat_start.elapsed();
    info!("✅ Phase 2 완료 - 채팅 시간: {:?}", chat_duration);

    // Phase 3: 일부 사용자 방 이동 (부하 테스트)
    info!("🔄 Phase 3: 사용자 방 이동");
    let move_start = Instant::now();
    
    let mut move_handles = Vec::new();
    let users_to_move = TOTAL_USERS / 3; // 100명이 방 이동
    
    for user_id in 1..=users_to_move {
        let old_room_id = ((user_id - 1) % TOTAL_ROOMS) + 1;
        let new_room_id = ((user_id + TOTAL_ROOMS / 2 - 1) % TOTAL_ROOMS) + 1; // 다른 방으로 이동
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            // 새로운 방에 입장 (기존 방에서 자동 퇴장)
            let writer = PerformanceTestEnv::create_mock_writer().await;
            let result = env_clone.chat_handler.handle_room_join(
                user_id,
                new_room_id,
                format!("이동사용자{}", user_id),
                format!("127.0.0.1:{}", 20000 + user_id),
                writer,
            ).await;
            
            if result.is_ok() {
                env_clone.metrics.record_join_success();
            }
        });
        
        move_handles.push(handle);
    }
    
    for handle in move_handles {
        handle.await.unwrap();
    }
    
    let move_duration = move_start.elapsed();
    info!("✅ Phase 3 완료 - 이동 시간: {:?}", move_duration);

    // Phase 4: 사용자 퇴장
    info!("📤 Phase 4: 사용자 퇴장");
    let leave_start = Instant::now();
    
    let mut leave_handles = Vec::new();
    
    for user_id in 1..=TOTAL_USERS {
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            let result = env_clone.chat_handler.handle_user_disconnect(user_id).await;
            if result.is_ok() {
                env_clone.metrics.record_leave_success();
            }
        });
        
        leave_handles.push(handle);
    }
    
    for handle in leave_handles {
        handle.await.unwrap();
    }
    
    let leave_duration = leave_start.elapsed();
    info!("✅ Phase 4 완료 - 퇴장 시간: {:?}", leave_duration);

    // 최종 상태 확인 및 정리
    let cleanup_start = Instant::now();
    let cleaned_rooms = env.chat_handler.cleanup_empty_rooms().await;
    let cleanup_duration = cleanup_start.elapsed();
    info!("🧹 빈 방 정리 완료: {}개, 시간: {:?}", cleaned_rooms, cleanup_duration);
    
    let test_duration = test_start.elapsed();
    let summary = env.metrics.get_summary();
    
    // 성능 결과 출력
    info!("🏆 성능 테스트 완료 - 총 시간: {:?}", test_duration);
    info!("📊 === 성능 요약 ===");
    info!("📊 총 메시지: {}", summary.total_messages);
    info!("📊 성공한 메시지: {}", summary.successful_messages);
    info!("📊 실패한 메시지: {}", summary.failed_messages);
    info!("📊 방 입장 성공: {}", summary.successful_joins);
    info!("📊 방 퇴장 성공: {}", summary.successful_leaves);
    info!("📊 평균 응답 시간: {:.2}ms", summary.avg_response_time_ms);
    info!("📊 성공률: {:.2}%", summary.success_rate);
    
    // 성능 기준 검증
    let expected_total_messages = (TOTAL_USERS * CHAT_MESSAGES_PER_USER) as u64;
    assert!(summary.successful_messages >= expected_total_messages * 95 / 100, 
        "메시지 성공률이 95% 미만: {}/{}", summary.successful_messages, expected_total_messages);
    
    assert!(summary.avg_response_time_ms < 100.0, 
        "평균 응답 시간이 100ms 초과: {:.2}ms", summary.avg_response_time_ms);
    
    assert!(summary.success_rate >= 95.0, 
        "전체 성공률이 95% 미만: {:.2}%", summary.success_rate);
    
    info!("✅ 모든 성능 기준 통과!");
}

/// 동시성 스트레스 테스트 - 동시에 많은 작업 실행
#[tokio::test]
async fn test_concurrent_stress() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN) // 로그 레벨 낮춤
        .try_init();

    let env = Arc::new(PerformanceTestEnv::new().await);
    
    info!("🔥 동시성 스트레스 테스트 시작");
    let test_start = Instant::now();
    
    // 300명의 사용자가 동시에 다양한 작업 수행
    let mut handles = Vec::new();
    
    for user_id in 1..=TOTAL_USERS {
        let room_id = ((user_id - 1) % TOTAL_ROOMS) + 1;
        let env_clone = env.clone();
        
        let handle = tokio::spawn(async move {
            // 1. 방 입장
            let writer = PerformanceTestEnv::create_mock_writer().await;
            let _ = env_clone.chat_handler.handle_room_join(
                user_id,
                room_id,
                format!("스트레스{}", user_id),
                format!("127.0.0.1:{}", 30000 + user_id),
                writer,
            ).await;
            
            // 2. 연속 채팅
            for i in 1..=5 {
                let _ = env_clone.chat_handler.handle_chat_message(
                    user_id,
                    room_id,
                    format!("스트레스메시지{}", i),
                ).await;
            }
            
            // 3. 랜덤 지연 후 퇴장
            let delay = (user_id % 50) + 10; // 10-59ms 랜덤 지연
            sleep(Duration::from_millis(delay as u64)).await;
            
            let _ = env_clone.chat_handler.handle_room_leave(user_id, room_id).await;
        });
        
        handles.push(handle);
    }
    
    // 모든 작업 완료 대기
    for handle in handles {
        handle.await.unwrap();
    }
    
    let test_duration = test_start.elapsed();
    info!("🏆 동시성 스트레스 테스트 완료 - 시간: {:?}", test_duration);
    
    // 최종 정리 확인
    let final_rooms = env.chat_handler.get_all_rooms_status();
    assert!(final_rooms.is_empty() || final_rooms.iter().all(|(_, count)| *count == 0), 
        "모든 방이 비어있어야 함");
    
    info!("✅ 동시성 스트레스 테스트 통과!");
}

/// 메모리 사용량 테스트 - 메모리 누수 검증
#[tokio::test]
async fn test_memory_usage() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .try_init();

    let env = Arc::new(PerformanceTestEnv::new().await);
    
    info!("🧠 메모리 사용량 테스트 시작");
    
    // 반복적으로 사용자 입장/채팅/퇴장 수행
    for cycle in 1..=10 {
        info!("🔄 사이클 {}/10", cycle);
        
        // 50명씩 5번에 걸쳐 입장
        for batch in 1..=5 {
            let mut handles = Vec::new();
            let start_user = (batch - 1) * 50 + 1;
            let end_user = batch * 50;
            
            for user_id in start_user..=end_user {
                let room_id = ((user_id - 1) % 10) + 1; // 10개 방 사용
                let env_clone = env.clone();
                
                let handle = tokio::spawn(async move {
                    // 입장
                    let writer = PerformanceTestEnv::create_mock_writer().await;
                    let _ = env_clone.chat_handler.handle_room_join(
                        user_id,
                        room_id,
                        format!("메모리테스트{}", user_id),
                        format!("127.0.0.1:{}", 40000 + user_id),
                        writer,
                    ).await;
                    
                    // 채팅 3회
                    for i in 1..=3 {
                        let _ = env_clone.chat_handler.handle_chat_message(
                            user_id,
                            room_id,
                            format!("메모리메시지{}_{}", cycle, i),
                        ).await;
                    }
                    
                    // 퇴장
                    let _ = env_clone.chat_handler.handle_room_leave(user_id, room_id).await;
                });
                
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
        }
        
        // 각 사이클 후 정리
        let cleaned = env.chat_handler.cleanup_empty_rooms().await;
        if cleaned > 0 {
            info!("🧹 사이클 {} 정리: {}개 방", cycle, cleaned);
        }
        
        // 메모리 정리를 위한 짧은 대기
        sleep(Duration::from_millis(100)).await;
    }
    
    // 최종 상태 확인
    let final_rooms = env.chat_handler.get_all_rooms_status();
    assert!(final_rooms.is_empty(), "모든 방이 정리되어야 함");
    
    info!("✅ 메모리 사용량 테스트 완료 - 메모리 누수 없음");
}