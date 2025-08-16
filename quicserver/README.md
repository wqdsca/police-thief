# 🚀 QUIC Game Server - 고성능 게임 서버 프레임워크

게임 로직과 통신 최적화를 완전히 분리한 고성능 QUIC 기반 게임 서버 프레임워크입니다.

## 📋 주요 특징

- **🎯 게임 로직 분리**: 통신 최적화는 프레임워크가 담당, 게임 로직에만 집중
- **⚡ 고성능**: 목표 15,000-20,000 msg/sec, <0.5ms p99 레이턴시
- **🔧 통신 최적화**: 압축, 델타 압축, 스트림 멀티플렉싱 내장
- **🛡️ 안정성**: 0-RTT 재개, 연결 마이그레이션, 자동 복구
- **📊 모니터링**: 실시간 성능 메트릭 및 통계
- **🎮 게임 특화**: 이동, 공격 등 핵심 게임 로직 인터페이스

## 🚀 빠른 시작

### 1. 기본 설정

```bash
# 환경 설정
cp .env.example .env
# .env 파일에서 필요한 설정 수정

# 빌드
cargo build --release
```

### 2. 기본 서버 실행 (DefaultGameLogicHandler 사용)

```rust
use quicserver::{QuicServerConfig, create_server_with_default_logic};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 설정 로드
    let config = QuicServerConfig::from_env()?;
    
    // 2. 기본 게임 로직으로 서버 생성
    let server = create_server_with_default_logic(config).await?;
    
    // 3. 서버 실행
    server.run().await?;
    
    Ok(())
}
```

### 3. 🎮 사용자 게임 로직 구현

**핵심: `GameLogicHandler` trait만 구현하면 됩니다!**

```rust
use quicserver::game_logic::{GameLogicHandler, GameResponse, Position};
use async_trait::async_trait;
use anyhow::Result;

pub struct MyGameLogic {
    // 게임 상태 관리
    players: Arc<DashMap<String, PlayerInfo>>,
}

#[async_trait]
impl GameLogicHandler for MyGameLogic {
    /// 🎮 플레이어 이동 처리 - 핵심 로직만 구현하세요
    async fn handle_player_move(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        let new_pos: Position = serde_json::from_value(payload["position"].clone())?;
        
        // ✅ 이곳에 게임 로직 구현
        // - 이동 유효성 검증
        // - 플레이어 위치 업데이트  
        // - 충돌 검사
        // - 상태 브로드캐스트
        
        Ok(GameResponse {
            msg_type: "player_moved".to_string(),
            success: true,
            data: serde_json::json!({"player_id": player_id, "position": new_pos}),
            broadcast_to: Some(self.get_room_players(player_id)?),
        })
    }
    
    /// ⚔️ 플레이어 공격 처리 - 핵심 로직만 구현하세요
    async fn handle_player_attack(&self, player_id: &str, payload: serde_json::Value) -> Result<GameResponse> {
        let target_id: String = serde_json::from_value(payload["target_id"].clone())?;
        
        // ✅ 이곳에 게임 로직 구현
        // - 공격 범위 검증
        // - 데미지 계산
        // - HP 감소 처리
        // - 결과 브로드캐스트
        
        let damage = self.calculate_damage(player_id, &target_id)?;
        
        Ok(GameResponse {
            msg_type: "player_attacked".to_string(),
            success: true,
            data: serde_json::json!({"attacker": player_id, "target": target_id, "damage": damage}),
            broadcast_to: Some(self.get_room_players(player_id)?),
        })
    }
    
    // 나머지 메소드들도 동일하게 게임 로직만 구현
    // 통신, 압축, 네트워킹은 프레임워크가 자동 처리!
}
```

## 🏗️ 아키텍처

```
사용자 게임 로직 (GameLogicHandler)
           ↓
통합 메시지 핸들러 (UnifiedMessageHandler)  
           ↓                    ↓
게임 로직 라우팅        통신 최적화 (MessageProcessor)
           ↓                    ↓
QUIC 스트림 처리 ← ← ← ← 압축/델타압축/체크섬
           ↓
QUIC 프로토콜 (15-20K msg/sec)
```

### 스트림 멀티플렉싱
- **Control Stream**: 게임 로직 메시지 (이동, 공격, 로그인)
- **Game State Stream**: 실시간 상태 동기화
- **Chat Stream**: 채팅 메시지
- **Voice Stream**: 음성 데이터
- **Bulk Stream**: 파일 전송

### 통신 최적화 기능
- **압축**: LZ4/Zstd 적응형 압축
- **델타 압축**: 변경 사항만 전송
- **체크섬**: CRC32 데이터 무결성
- **시퀀스 번호**: 메시지 순서 보장
- **8가지 최적화 서비스**: TCP 서버에서 검증된 고성능 유틸리티

## 🔧 환경 설정

`.env` 파일 예시:
```env
# QUIC 서버 설정
QUIC_HOST=0.0.0.0
QUIC_PORT=5000
QUIC_MAX_CONNECTIONS=1000
QUIC_MAX_STREAMS=100

# TLS 설정
USE_SELF_SIGNED=true
# CERT_PATH=path/to/cert.pem
# KEY_PATH=path/to/key.pem

# 성능 설정
ENABLE_MIGRATION=true
IDLE_TIMEOUT_SECONDS=300
KEEP_ALIVE_INTERVAL_SECONDS=30

# 로그 레벨
RUST_LOG=info
```

## 📊 성능 및 특징

### 성능 목표
- **처리량**: 15,000-20,000 msg/sec
- **레이턴시**: <0.5ms p99
- **동시 연결**: 1,000+ 연결
- **메모리**: <50MB for 1,000 connections
- **0-RTT 성공률**: >90%

### QUIC 프로토콜 특징
- **0-RTT 재개**: 빠른 재연결
- **연결 마이그레이션**: 네트워크 변경 시 끊김 없는 연결
- **다중 스트림**: 하나의 연결로 여러 스트림 처리
- **내장 암호화**: TLS 1.3 기반 보안

## 🛠️ 개발 명령어

```bash
# 빌드
cargo build --release

# 개발 모드 실행
cargo run

# 테스트
cargo test

# 성능 벤치마크
cargo bench

# 코드 포맷팅
cargo fmt

# 린팅
cargo clippy
```

## 🔄 마이그레이션 가이드

### 기존 코드에서 변경사항

#### Before (Old - 더 이상 사용하지 마세요)
```rust
// ❌ 게임 로직이 통신 코드와 섞여있었음
impl MessageHandler {
    fn handle_move() { /* 네트워킹 + 게임로직 */ }
    fn handle_attack() { /* 네트워킹 + 게임로직 */ }
}
```

#### After (New - 권장 방식)
```rust
// ✅ 게임 로직만 깔끔하게 분리
#[async_trait]
impl GameLogicHandler for MyGameLogic {
    async fn handle_player_move() -> GameResponse { 
        // 순수 게임 로직만! 네트워킹은 프레임워크가 처리
    }
    async fn handle_player_attack() -> GameResponse { 
        // 순수 게임 로직만! 압축/전송은 자동
    }
}
```

## ❓ FAQ

### Q: 기존 TCP/gRPC 서버와 어떤 차이가 있나요?
A: QUIC는 더 빠른 연결 설정(0-RTT), 연결 마이그레이션, 다중 스트림을 제공하여 게임에 최적화된 성능을 제공합니다.

### Q: 게임 로직만 구현하면 되나요?
A: 네! `GameLogicHandler` trait만 구현하면 통신 최적화, 압축, 스트림 처리는 모두 프레임워크에서 자동으로 처리됩니다.

### Q: 기존 데이터베이스나 Redis를 사용할 수 있나요?
A: 물론입니다. `GameLogicHandler` 구현체 내에서 자유롭게 데이터베이스, Redis, 외부 API 등을 사용할 수 있습니다.

### Q: 성능 모니터링은 어떻게 하나요?
A: 내장된 `MetricsCollector`가 실시간으로 성능 지표를 수집합니다. 30초마다 통계가 출력됩니다.

## 🏗️ 프로젝트 구조

```
quicserver/
├── src/
│   ├── lib.rs                 # 라이브러리 엔트리포인트
│   ├── main.rs               # 바이너리 엔트리포인트
│   ├── game_logic.rs         # 🎮 게임 로직 인터페이스 (여기서 구현!)
│   ├── communication.rs      # 📡 통신 최적화 유틸리티
│   ├── handler/
│   │   ├── unified_handler.rs # 통합 메시지 핸들러
│   │   └── message.rs        # 레거시 핸들러 (deprecated)
│   ├── network/
│   │   ├── server.rs         # QUIC 서버 구현
│   │   ├── connection.rs     # 연결 관리
│   │   └── stream.rs         # 스트림 멀티플렉싱
│   └── optimization/
│       └── optimizer.rs      # 8가지 성능 최적화
├── Cargo.toml
├── README.md
└── .env.example
```

## 📝 개발 상태

✅ **완료됨**
- QUIC 서버 핵심 구현
- 게임 로직과 통신 최적화 분리
- GameLogicHandler trait 인터페이스
- 스트림 멀티플렉싱
- 8가지 최적화 서비스 통합
- 실시간 성능 모니터링

🔄 **진행 중**
- 빌드 및 테스트 검증
- 성능 벤치마크
- 클라이언트 SDK 개발

## 🎯 마무리

**이제 네트워킹 걱정 없이 게임 로직에만 집중하세요!**

- ✅ 통신 최적화: 프레임워크가 자동 처리
- ✅ 압축/델타압축: 자동 적용
- ✅ 스트림 관리: 자동 멀티플렉싱
- ✅ 성능 모니터링: 실시간 통계
- 🎮 **당신의 역할**: `GameLogicHandler`만 구현하면 끝!

---

Police Thief Game Team에서 개발 🚀