//! RUDP 게임 서버
//!
//! 실시간 멀티플레이어 게임을 위한 고성능 RUDP 서버입니다.
//! 연결(Connect), 이동(Move), 공격(Attack), 사망(Die) 등 핵심 게임 메커니즘을 구현합니다.
//!
//! # 주요 기능
//! - **RUDP 프로토콜**: 신뢰성과 성능을 모두 확보한 UDP 기반 통신
//! - **실시간 게임 로직**: 60 TPS 게임 루프와 지연 보상 시스템
//! - **확장 가능한 아키텍처**: 2000명 동시접속 지원
//! - **종합 보안**: JWT 인증, 패킷 검증, Rate Limiting
//! - **성능 최적화**: 공간 분할, 메모리 풀링, 비동기 I/O
//!
//! # 성능 목표 (2000명 동시접속 기준)
//! - **처리량**: 100,000+ packets/sec
//! - **지연시간**: <50ms RTT
//! - **메모리**: <4GB 사용량
//! - **CPU**: <70% 사용률
//! - **패킷 손실률**: <0.1%

use anyhow::Result;
use dotenv::{dotenv, from_path};
use std::time::Duration;
use std::{env, path::PathBuf, sync::Arc};
use tokio::{signal, time::interval};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

// 내부 모듈들
mod config;
mod game;
mod network;
mod protocol;
mod types;
mod utils;

// 모듈 사용
use config::RudpServerConfig;
use game::{messages::GameMessage, player::PlayerManager, state_manager::GameStateManager};
use network::session::SessionManager;
use protocol::rudp::RudpServer;
use utils::performance::PerformanceMonitor;

// Shared library imports
use shared::security::SecurityMiddleware;
use shared::tool::high_performance::redis_optimizer::RedisOptimizer;

/// RUDP 게임 서버 메인 구조체
///
/// 모든 게임 시스템을 통합 관리하는 최상위 서버 구조체입니다.
/// RUDP 프로토콜, 게임 상태 관리, 세션 관리, 성능 모니터링을 조합합니다.
pub struct RudpGameServer {
    /// 서버 설정
    config: RudpServerConfig,
    /// RUDP 프로토콜 서버
    rudp_server: Arc<RudpServer>,
    /// 게임 상태 관리자
    game_state_manager: Arc<GameStateManager>,
    /// 세션 관리자
    session_manager: Arc<SessionManager>,
    /// 플레이어 관리자
    player_manager: Arc<PlayerManager>,
    /// 성능 모니터
    performance_monitor: Arc<PerformanceMonitor>,
    /// 보안 미들웨어
    security_middleware: Arc<SecurityMiddleware>,
    /// Redis 최적화기
    redis_optimizer: Arc<RedisOptimizer>,
}

impl RudpGameServer {
    /// 새로운 RUDP 게임 서버 생성
    ///
    /// 모든 필요한 컴포넌트를 초기화하고 서로 연결합니다.
    ///
    /// # Arguments
    /// * `config` - 서버 설정
    ///
    /// # Returns
    /// 초기화된 게임 서버
    ///
    /// # Examples
    /// ```rust
    /// let config = RudpServerConfig::from_env_and_args().await?;
    /// let server = RudpGameServer::new(config).await?;
    /// ```
    pub async fn new(config: RudpServerConfig) -> Result<Self> {
        info!("🚀 RUDP 게임 서버 초기화 시작...");

        // Redis 연결 초기화
        let redis_url = format!("redis://{}:{}", config.redis.host, config.redis.port);
        let redis_optimizer_config =
            shared::tool::high_performance::redis_optimizer::RedisOptimizerConfig {
                pipeline_batch_size: 100,
                connection_pool_size: config.redis.pool_size as usize,
                max_retries: 3,
                retry_delay_ms: 100,
                connection_timeout_secs: config.redis.connection_timeout_secs,
                enable_key_compression: false,
                enable_value_compression: true,
                default_ttl_secs: config.redis.session_ttl_secs as usize,
            };
        let redis_optimizer =
            Arc::new(RedisOptimizer::new(&redis_url, redis_optimizer_config).await?);
        info!("✅ Redis 연결 설정 완료");

        // 보안 미들웨어 초기화
        let security_middleware = Arc::new(SecurityMiddleware::from_env().await?);
        info!("🛡️ 보안 시스템 초기화 완료");

        // RUDP 서버 초기화
        let rudp_config = protocol::rudp::RudpConfig {
            max_connections: config.game.max_concurrent_players as usize,
            max_packet_size: 1024,
            ack_timeout_ms: 100,
            max_retransmissions: 3,
            keepalive_interval_secs: 30,
            connection_timeout_secs: 60,
            receive_buffer_size: 8192,
            send_buffer_size: 8192,
            enable_congestion_control: true,
            enable_compression: true,
        };
        let bind_addr = format!("{}:{}", config.network.host, config.network.port);
        let rudp_server = Arc::new(
            RudpServer::new(
                &bind_addr,
                rudp_config,
                security_middleware.clone(),
                redis_optimizer.clone(),
            )
            .await?,
        );
        info!("📡 RUDP 프로토콜 서버 초기화 완료");

        // 플레이어 관리자 초기화
        let player_manager = Arc::new(PlayerManager::new());
        info!("👥 플레이어 관리 시스템 초기화 완료");

        // 월드 관리는 클라이언트(Unity)에서 처리
        info!("🗺️ 월드 관리는 클라이언트측에서 처리 - 서버는 Redis 기반 상태 관리");

        // 세션 관리자 초기화
        let session_manager_config = network::session::SessionManagerConfig::default();
        let session_manager = Arc::new(
            SessionManager::new(
                session_manager_config,
                security_middleware.clone(),
                redis_optimizer.clone(),
                player_manager.clone(),
            )
            .await?,
        );
        info!("🔗 세션 관리 시스템 초기화 완료");

        // 게임 상태 관리자 초기화
        let game_state_manager = Arc::new(
            GameStateManager::new(
                config.game.clone(),
                player_manager.clone(),
                security_middleware.clone(),
                redis_optimizer.clone(),
            )
            .await?,
        );
        info!("🎮 게임 상태 관리자 초기화 완료");

        // 성능 모니터 초기화
        let monitoring_config = utils::performance::MonitoringConfig {
            enable_system_monitoring: true,
            enable_game_monitoring: true,
            metrics_retention_seconds: config.monitoring.metrics_retention_hours as u64 * 3600,
            alert_thresholds: utils::performance::AlertThresholds {
                high_cpu_percent: 80.0,
                high_memory_percent: 85.0,
                high_latency_ms: 100.0,
                low_tps_threshold: 20.0,
            },
        };
        let performance_monitor = Arc::new(PerformanceMonitor::new(monitoring_config).await?);
        info!("📊 성능 모니터링 시스템 초기화 완료");

        info!("🎯 서버 성능 목표:");
        info!(
            "   - 동시 접속자: {} 명",
            config.game.max_concurrent_players
        );
        info!("   - 틱 레이트: {} TPS", config.game.tick_rate);
        info!(
            "   - 지연시간: <{}ms",
            config.monitoring.network_latency_warning_threshold_ms
        );
        info!("   - 패킷 처리량: 100,000+ packets/sec");

        Ok(Self {
            config,
            rudp_server,
            game_state_manager,
            session_manager,
            player_manager,
            performance_monitor,
            security_middleware,
            redis_optimizer,
        })
    }

    /// 서버 시작 및 실행
    ///
    /// 게임 서버의 모든 컴포넌트를 시작하고 메인 루프를 실행합니다.
    ///
    /// # Returns
    /// 서버 실행 결과
    ///
    /// # Main Loops
    /// 1. **게임 틱 루프**: 60 TPS로 게임 상태 업데이트
    /// 2. **네트워크 루프**: RUDP 패킷 수신 및 처리
    /// 3. **성능 모니터링 루프**: 10초마다 성능 메트릭 수집
    /// 4. **세션 정리 루프**: 30초마다 비활성 세션 정리
    pub async fn start(&self) -> Result<()> {
        info!("🎮 RUDP 게임 서버 시작!");
        info!(
            "📍 서버 주소: {}:{}",
            self.config.network.host, self.config.network.port
        );

        // 게임 이벤트 구독
        let mut event_receiver = self.game_state_manager.subscribe_events();

        // 1. 게임 틱 루프 시작 (60 TPS)
        let game_tick_handle = {
            let game_state = self.game_state_manager.clone();
            let tick_rate = self.config.game.tick_rate;

            tokio::spawn(async move {
                let mut tick_interval = interval(Duration::from_millis(1000 / tick_rate as u64));
                let mut tick_number = 0u64;
                let mut last_tick_time = tokio::time::Instant::now();

                info!(
                    "⚡ 게임 틱 루프 시작 ({}Hz) - Redis 기반 상태 관리",
                    tick_rate
                );

                loop {
                    tick_interval.tick().await;
                    tick_number += 1;

                    let now = tokio::time::Instant::now();
                    let delta_time = now.duration_since(last_tick_time).as_secs_f32();
                    last_tick_time = now;

                    // 게임 상태 업데이트 (Redis 기반)
                    if let Err(e) = game_state.update_game_tick(tick_number, delta_time).await {
                        error!(tick = %tick_number, error = %e, "게임 틱 처리 실패");
                    }

                    // 매초마다 통계 로그
                    if tick_number % tick_rate as u64 == 0 {
                        let stats = game_state.get_game_statistics().await;
                        info!(
                            tick = %tick_number,
                            active_players = %stats.active_players,
                            total_attacks = %stats.total_attacks,
                            "게임 틱 상태"
                        );
                    }
                }
            })
        };

        // 2. 네트워크 메시지 처리 루프
        let network_handle = {
            let rudp_server = self.rudp_server.clone();
            let game_state = self.game_state_manager.clone();
            let session_manager = self.session_manager.clone();
            let security_middleware = self.security_middleware.clone();

            tokio::spawn(async move {
                info!("📡 네트워크 메시지 처리 루프 시작");

                loop {
                    // RUDP 패킷 수신
                    match rudp_server.receive_message().await {
                        Ok((client_addr, packet_data)) => {
                            // 패킷 보안 검증
                            if !security_middleware
                                .validate_packet(&packet_data)
                                .await
                                .unwrap_or(false)
                            {
                                warn!(client = %client_addr, "유효하지 않은 패킷 수신");
                                continue;
                            }

                            // 세션 ID 생성 또는 조회
                            let session_id = crate::utils::socket_addr_to_u64(client_addr);

                            // 메시지 역직렬화
                            let game_message: GameMessage = match bincode::deserialize(&packet_data)
                            {
                                Ok(msg) => msg,
                                Err(e) => {
                                    warn!(
                                        client = %client_addr,
                                        error = %e,
                                        "메시지 역직렬화 실패"
                                    );
                                    continue;
                                }
                            };

                            // 메시지 처리
                            let response = Self::handle_game_message(
                                &game_state,
                                &session_manager,
                                session_id,
                                game_message,
                            )
                            .await;

                            // 응답 전송 (있는 경우)
                            if let Ok(Some(response_msg)) = response {
                                let response_data = match bincode::serialize(&response_msg) {
                                    Ok(data) => data,
                                    Err(e) => {
                                        error!(error = %e, "응답 메시지 직렬화 실패");
                                        continue;
                                    }
                                };

                                if let Err(e) =
                                    rudp_server.send_message(client_addr, response_data).await
                                {
                                    error!(
                                        client = %client_addr,
                                        error = %e,
                                        "응답 메시지 전송 실패"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "네트워크 메시지 수신 실패");
                            tokio::time::sleep(Duration::from_millis(1)).await;
                        }
                    }
                }
            })
        };

        // 3. 게임 이벤트 브로드캐스트 루프
        let broadcast_handle = {
            let rudp_server = self.rudp_server.clone();
            let session_manager = self.session_manager.clone();

            tokio::spawn(async move {
                info!("📢 게임 이벤트 브로드캐스트 루프 시작");

                while let Ok(event) = event_receiver.recv().await {
                    // 이벤트를 관련 클라이언트들에게 브로드캐스트
                    if let Err(e) =
                        Self::broadcast_game_event(&rudp_server, &session_manager, &event).await
                    {
                        error!(event = ?event, error = %e, "이벤트 브로드캐스트 실패");
                    }
                }
            })
        };

        // 4. 성능 모니터링 루프
        let monitoring_handle = {
            let performance_monitor = self.performance_monitor.clone();
            let game_state = self.game_state_manager.clone();

            tokio::spawn(async move {
                let mut monitor_interval = interval(Duration::from_secs(10));

                info!("📊 성능 모니터링 루프 시작");

                loop {
                    monitor_interval.tick().await;

                    // 시스템 메트릭 수집
                    if let Ok(system_metrics) = performance_monitor.collect_system_metrics().await {
                        let stats = game_state.get_game_statistics().await;

                        // Redis에 메트릭 저장
                        if let Err(e) = performance_monitor
                            .store_metrics(
                                &system_metrics,
                                stats.active_players,
                                stats.active_players,
                            )
                            .await
                        {
                            error!(error = %e, "메트릭 저장 실패");
                        }
                    }
                }
            })
        };

        // 5. 세션 정리 루프
        let cleanup_handle = {
            let session_manager = self.session_manager.clone();

            tokio::spawn(async move {
                let mut cleanup_interval = interval(Duration::from_secs(30));

                info!("🧹 세션 정리 루프 시작");

                loop {
                    cleanup_interval.tick().await;

                    let cleaned_count = session_manager.cleanup_inactive_sessions().await;
                    if cleaned_count > 0 {
                        info!(cleaned = %cleaned_count, "비활성 세션 정리 완료");
                    }
                }
            })
        };

        info!("✅ 모든 시스템 루프가 시작되었습니다!");
        info!("🎮 게임 서버가 연결을 수락할 준비가 완료되었습니다.");

        // 종료 신호 대기
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("📴 종료 신호 수신 (Ctrl+C)");
            }
            _ = Self::wait_for_shutdown_signal() => {
                info!("📴 시스템 종료 신호 수신");
            }
        }

        // 서버 종료 시작
        info!("🔄 서버 종료 프로세스 시작...");

        // 모든 태스크 정리 (타임아웃 30초)
        let shutdown_timeout = Duration::from_secs(30);
        tokio::time::timeout(shutdown_timeout, async {
            let _ = tokio::try_join!(
                game_tick_handle,
                network_handle,
                broadcast_handle,
                monitoring_handle,
                cleanup_handle,
            );
        })
        .await
        .unwrap_or_else(|_| {
            warn!("⚠️ 서버 종료 타임아웃 - 강제 종료");
        });

        info!("✅ RUDP 게임 서버 종료 완료");
        Ok(())
    }

    /// 게임 메시지 처리
    ///
    /// 클라이언트로부터 수신된 게임 메시지를 타입별로 처리합니다.
    ///
    /// # Arguments
    /// * `game_state` - 게임 상태 관리자
    /// * `session_manager` - 세션 관리자
    /// * `session_id` - 클라이언트 세션 ID
    /// * `message` - 수신된 게임 메시지
    ///
    /// # Returns
    /// 처리 결과 (응답 메시지 또는 None)
    async fn handle_game_message(
        game_state: &Arc<GameStateManager>,
        session_manager: &Arc<SessionManager>,
        session_id: u64,
        message: GameMessage,
    ) -> Result<Option<GameMessage>> {
        match message {
            // 연결 요청 처리
            GameMessage::Connect {
                player_name,
                auth_token,
                client_version,
            } => {
                let response = game_state
                    .handle_player_connect(session_id, player_name, auth_token, client_version)
                    .await?;
                Ok(Some(response))
            }

            // 이동 요청 처리
            GameMessage::Move {
                target_position,
                direction,
                speed_multiplier,
                client_timestamp,
            } => {
                let result = game_state
                    .handle_player_move(
                        session_id,
                        target_position,
                        direction,
                        speed_multiplier,
                        client_timestamp,
                    )
                    .await?;
                Ok(result)
            }

            // 공격 요청 처리
            GameMessage::Attack {
                target,
                attack_type,
                weapon_id,
                attack_direction,
                predicted_damage,
            } => {
                let response = game_state
                    .handle_player_attack(
                        session_id,
                        target,
                        attack_type,
                        weapon_id,
                        attack_direction,
                        predicted_damage,
                    )
                    .await?;
                Ok(Some(response))
            }

            // 리스폰 요청 처리
            GameMessage::Respawn => {
                let response = game_state
                    .handle_player_respawn(session_id)
                    .await?;
                Ok(Some(response))
            }

            // 연결 해제 처리
            GameMessage::Disconnect { reason } => {
                game_state
                    .handle_player_disconnect(session_id, reason)
                    .await?;
                Ok(None)
            }

            // 기타 메시지 타입
            _ => {
                warn!(session_id = %session_id, message = ?message, "지원되지 않는 메시지 타입");
                Ok(Some(GameMessage::Error {
                    error_code: "UNSUPPORTED_MESSAGE".to_string(),
                    error_message: "Unsupported message type".to_string(),
                    category: game::messages::ErrorCategory::GameLogic,
                    recoverable: false,
                }))
            }
        }
    }

    /// 게임 이벤트 브로드캐스트
    ///
    /// 게임 이벤트를 관련된 모든 클라이언트에게 전송합니다.
    ///
    /// # Arguments
    /// * `rudp_server` - RUDP 서버
    /// * `session_manager` - 세션 관리자
    /// * `event` - 브로드캐스트할 이벤트
    async fn broadcast_game_event(
        rudp_server: &Arc<RudpServer>,
        session_manager: &Arc<SessionManager>,
        event: &game::state_manager::GameEvent,
    ) -> Result<()> {
        use game::state_manager::GameEvent;

        match event {
            GameEvent::PlayerMoved {
                player_id,
                new_position,
                velocity,
                ..
            } => {
                let message = GameMessage::MoveUpdate {
                    player_id: *player_id,
                    current_position: *new_position,
                    velocity: *velocity,
                    server_timestamp: crate::utils::current_timestamp_ms(),
                };

                // 관심 영역 내 플레이어들에게만 전송 (간소화)
                Self::broadcast_to_nearby_players(
                    rudp_server,
                    session_manager,
                    *player_id,
                    message,
                )
                .await?;
            }

            GameEvent::AttackExecuted {
                attacker_id,
                target,
                result,
                ..
            } => {
                let message = GameMessage::AttackResult {
                    attacker_id: *attacker_id,
                    target: target.clone(),
                    hit: result.hit,
                    damage_dealt: result.damage_dealt,
                    critical_hit: result.critical_hit,
                    target_health: result.target_health_after,
                    server_timestamp: crate::utils::current_timestamp_ms(),
                };

                Self::broadcast_to_nearby_players(
                    rudp_server,
                    session_manager,
                    *attacker_id,
                    message,
                )
                .await?;
            }

            GameEvent::PlayerDied {
                player_id,
                killer_id,
                death_cause,
                death_position,
            } => {
                let message = GameMessage::Die {
                    player_id: *player_id,
                    death_cause: death_cause.clone(),
                    killer_id: *killer_id,
                    death_position: *death_position,
                    dropped_items: vec![], // TODO: 실제 드롭 아이템
                    respawn_cooldown: 30,
                    death_penalty: game::messages::DeathPenalty {
                        gold_lost: 0,
                        durability_loss: 0.1,
                    },
                };

                Self::broadcast_to_all_players(rudp_server, session_manager, message).await?;
            }

            _ => {
                // 기타 이벤트는 현재 처리하지 않음
            }
        }

        Ok(())
    }

    /// 근처 플레이어들에게 브로드캐스트
    async fn broadcast_to_nearby_players(
        _rudp_server: &Arc<RudpServer>,
        _session_manager: &Arc<SessionManager>,
        _center_player_id: u32,
        _message: GameMessage,
    ) -> Result<()> {
        // TODO: 관심 영역 기반 브로드캐스트 구현
        Ok(())
    }

    /// 모든 플레이어에게 브로드캐스트
    async fn broadcast_to_all_players(
        _rudp_server: &Arc<RudpServer>,
        _session_manager: &Arc<SessionManager>,
        _message: GameMessage,
    ) -> Result<()> {
        // TODO: 전체 플레이어 브로드캐스트 구현
        Ok(())
    }

    /// 종료 신호 대기
    async fn wait_for_shutdown_signal() {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            let mut sigint = signal(SignalKind::interrupt()).unwrap();

            tokio::select! {
                _ = sigterm.recv() => {},
                _ = sigint.recv() => {},
            }
        }

        #[cfg(not(unix))]
        {
            // Windows에서는 Ctrl+C만 처리
            let _ = signal::ctrl_c().await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // .env 파일 로드
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let env_path = workspace_root.join(".env");

    if env_path.exists() {
        from_path(&env_path).map_err(|e| anyhow::anyhow!("Failed to load .env: {}", e))?;
    } else {
        dotenv().ok();
    }

    // 로깅 시스템 초기화
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("info".parse()?)
                .add_directive("rudpserver=debug".parse()?),
        )
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("🎮 RUDP 게임 서버 v1.0.0 시작!");

    // 서버 설정 로드
    let config = RudpServerConfig::from_env_and_args()
        .await
        .map_err(|e| anyhow::anyhow!("설정 로드 실패: {}", e))?;

    // 서버 생성 및 실행
    let server = RudpGameServer::new(config)
        .await
        .map_err(|e| anyhow::anyhow!("서버 생성 실패: {}", e))?;

    server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("서버 실행 실패: {}", e))?;

    Ok(())
}
