# 🎮 Police Thief 서버 명령어 완벽 가이드

## 🚀 원클릭 실행

| 플랫폼 | 통합 실행 | 개별 실행 | Docker 실행 |
|--------|-----------|-----------|-------------|
| **Windows** | `run-server.bat start` | `run-server.bat tcp` | `run-server.bat start-docker` |
| **Linux/Mac** | `./run-server.sh start` | `./run-server.sh tcp` | `./run-server.sh start-docker` |

---

## 📋 명령어 분류별 정리

### 🎯 **핵심 실행 명령어**

#### 통합 서버 실행 (권장)
```bash
# Docker 통합 (가장 쉬운 방법)
./run-server.sh start                    # 기본값: Docker 통합
./run-server.sh start-docker             # 명시적 Docker

# 네이티브 직접 실행 (최고 성능)  
./run-server.sh start-native             # Docker 없이 실행

# 마이크로서비스 분리
./run-server.sh start-micro              # 서비스별 개별 컨테이너
```

#### 개별 서버 실행
```bash
./run-server.sh grpc                     # gRPC 서버만 (포트 50051)
./run-server.sh tcp                      # TCP 서버만 (포트 4000)  
./run-server.sh rudp                     # RUDP 서버만 (포트 5000)
```

#### 서버 관리
```bash
./run-server.sh stop                     # 모든 서버 중지
./run-server.sh restart                  # 서버 재시작
./run-server.sh status                   # 전체 상태 확인
```

---

### 📊 **모니터링 & 디버깅**

#### 실시간 모니터링
```bash
./run-server.sh logs                     # 실시간 로그 스트리밍
./run-server.sh health                   # 헬스체크 수행
./run-server.sh monitor                  # 모니터링 대시보드 열기
./run-server.sh test                     # 연결성 테스트
```

#### 상세 상태 확인
```bash
./run-server.sh status                   # 전체 서비스 상태
docker ps --filter "name=police"        # Docker 컨테이너 상태
cargo run -p gamecenter -- status       # 네이티브 서버 상태
```

---

### 🛠️ **개발 & 빌드**

#### 프로젝트 빌드  
```bash
./run-server.sh build                    # 전체 Rust 프로젝트 빌드
./run-server.sh build-docker             # Docker 이미지 빌드
./run-server.sh clean                    # 빌드 캐시 정리
```

#### 개발 환경
```bash
./run-server.sh dev                      # 개발 모드 시작
./run-server.sh setup                    # 초기 환경 설정
./run-server.sh shell                    # 컨테이너 쉘 접속
```

---

### ⚙️ **Docker 전용 명령어**

#### Docker Compose 방식
```bash
# 통합 서버
cd gamecenter/docker
docker-compose -f docker-compose.unified.yml up -d
docker-compose -f docker-compose.unified.yml logs -f
docker-compose -f docker-compose.unified.yml down

# 마이크로서비스
docker-compose -f docker-compose.microservices.yml up -d
docker-compose -f docker-compose.microservices.yml logs -f
docker-compose -f docker-compose.microservices.yml down
```

#### Makefile 방식 (고급)
```bash
cd gamecenter/docker

# 서버 관리
make unified                             # 통합 서버 시작
make micro                               # 마이크로서비스 시작  
make status                              # 상태 확인
make clean                               # 전체 정리

# 스케일링
make scale-grpc REPLICAS=3               # gRPC 서버 3개로 확장
make scale-tcp REPLICAS=2                # TCP 서버 2개로 확장

# 모니터링  
make logs                                # 실시간 로그
make health                              # 헬스체크
make monitor                             # 모니터링 대시보드

# 백업 & 복구
make backup-redis                        # Redis 데이터 백업
```

---

### 🔧 **네이티브 실행 (Cargo 직접)**

#### Gamecenter 통합 실행
```bash
# 모든 서버 통합 실행
cargo run -p gamecenter --release -- start

# 개별 서버 실행  
cargo run -p gamecenter --release -- grpc
cargo run -p gamecenter --release -- tcp
cargo run -p gamecenter --release -- rudp

# 백그라운드 실행
cargo run -p gamecenter --release -- server

# 서버 중지
cargo run -p gamecenter --release -- stop
```

#### 개별 컴포넌트 직접 실행
```bash
# 각 서버별 독립 실행
cargo run -p grpcserver --release        # gRPC 서버만
cargo run -p tcpserver --release         # TCP 서버만  
cargo run -p rudpserver --release        # RUDP 서버만
```

---

## 🎛️ **실행 모드별 상세 가이드**

### Mode 1: 통합 Docker (권장) 🐳
```bash
# 특징: 가장 쉬운 설정, 운영 편의성 최대
./run-server.sh start

# 접속 정보
# - gRPC: http://localhost:50051
# - TCP: localhost:4000  
# - RUDP: localhost:5000
# - Redis: localhost:6379
```

### Mode 2: 마이크로서비스 분리 🔧  
```bash
# 특징: 개별 스케일링 가능, 서비스별 독립성
./run-server.sh start-micro

# 스케일링 예시
cd gamecenter/docker
make scale-grpc REPLICAS=3    # gRPC만 3개로 확장
make scale-tcp REPLICAS=2     # TCP만 2개로 확장
```

### Mode 3: 네이티브 고성능 ⚡
```bash
# 특징: 최고 성능 (12,991+ msg/sec), Docker 오버헤드 없음
./run-server.sh start-native

# 또는 직접 실행
cargo run -p gamecenter --release -- start
```

### Mode 4: 개발 모드 🔬
```bash
# 특징: 핫 리로드, 디버깅 편의성
./run-server.sh dev

# 개별 서비스 개발 시
cargo run -p gamecenter -- grpc    # Debug 모드로 gRPC만
cargo run -p gamecenter -- tcp     # Debug 모드로 TCP만
```

---

## 🚨 **문제 해결 명령어**

### 일반적인 문제 해결
```bash
# 전체 시스템 상태 진단
./run-server.sh health                   # 헬스체크
./run-server.sh status                   # 서비스 상태
./run-server.sh test                     # 연결성 테스트

# 포트 충돌 확인  
netstat -tulpn | grep :4000             # Linux/Mac
netstat -an | find ":4000"              # Windows

# 프로세스 강제 종료
./run-server.sh stop                    # 정상 종료
pkill -f gamecenter                     # Linux/Mac 강제 종료
taskkill /f /im gamecenter.exe          # Windows 강제 종료
```

### Docker 문제 해결
```bash
# Docker 상태 확인
docker ps -a                           # 모든 컨테이너
docker logs police-gamecenter          # 특정 컨테이너 로그

# Docker 정리 
docker system prune -f                 # 미사용 리소스 정리
./run-server.sh clean                  # 프로젝트별 정리

# 이미지 재빌드
./run-server.sh build-docker           # 전체 이미지 빌드
docker-compose build --no-cache        # 캐시 없이 빌드
```

### 빌드 문제 해결
```bash
# 의존성 문제
cargo clean                            # 캐시 정리
cargo update                           # 의존성 업데이트
cargo build --release                  # 릴리즈 빌드

# 권한 문제  
chmod +x run-server.sh                 # 실행 권한 부여
sudo chown -R $USER:$USER .            # 소유권 변경
```

---

## 📈 **성능 & 로드 테스트**

### 성능 테스트 실행
```bash
# TCP 서버 로드 테스트
python tcp_load_test.py

# RUDP 서버 로드 테스트  
python rudp_load_test.py

# 간단한 연결 테스트
./run-server.sh test
```

### 성능 모니터링
```bash
# 실시간 성능 모니터링
./run-server.sh monitor                # Prometheus 대시보드

# 시스템 리소스 모니터링
docker stats                           # Docker 컨테이너 리소스
htop                                   # 시스템 전체 리소스

# 로그 기반 성능 분석
./run-server.sh logs | grep "msg/sec"  # 처리량 확인
```

---

## 🎯 **프로덕션 운영**

### 프로덕션 시작 절차
```bash
# 1. 환경 설정 확인
./run-server.sh setup
nano .env                              # 환경변수 편집

# 2. 보안 설정
export JWT_SECRET_KEY="secure_production_key"

# 3. 프로덕션 모드 시작  
./run-server.sh start                  # 통합 모드로 시작
./run-server.sh health                 # 상태 확인

# 4. 모니터링 설정
./run-server.sh monitor                # 대시보드 접속
```

### 백업 & 복구
```bash
# Redis 데이터 백업
./run-server.sh backup
# 또는
cd gamecenter/docker && make backup-redis

# 로그 백업
docker logs police-gamecenter > server.log

# 설정 파일 백업
cp .env .env.backup
cp -r gamecenter/docker/.env docker.env.backup
```

---

## 🔍 **디버깅 & 트러블슈팅**

### 로그 분석
```bash
# 실시간 로그 스트리밍
./run-server.sh logs                   # 전체 로그

# 개별 서비스 로그
docker logs police-grpc                # gRPC 서비스
docker logs police-tcp                 # TCP 서비스  
docker logs police-rudp                # RUDP 서비스
docker logs police-redis               # Redis 서비스

# 로그 필터링
./run-server.sh logs | grep ERROR      # 에러만 필터
./run-server.sh logs | grep "msg/sec"  # 성능 로그만
```

### 네트워크 디버깅
```bash
# 포트 상태 확인
ss -tulpn | grep -E ":(4000|5000|50051|6379)"

# 연결성 테스트
curl http://localhost:50051/health     # gRPC 헬스체크
echo "PING" | nc localhost 4000        # TCP 연결 테스트
redis-cli -h localhost -p 6379 ping    # Redis 연결 테스트

# 방화벽 상태 (Linux)
sudo ufw status                        # UFW 방화벽 상태
sudo iptables -L                       # iptables 규칙
```

---

## 🎊 **명령어 치트 시트**

### 일상 사용 명령어 (Top 10)
```bash
./run-server.sh start        # 1. 서버 시작
./run-server.sh health       # 2. 상태 확인  
./run-server.sh logs         # 3. 로그 보기
./run-server.sh stop         # 4. 서버 중지
./run-server.sh restart      # 5. 서버 재시작
./run-server.sh status       # 6. 전체 상태
./run-server.sh test         # 7. 연결 테스트
./run-server.sh build        # 8. 프로젝트 빌드
./run-server.sh clean        # 9. 캐시 정리
./run-server.sh help         # 10. 도움말
```

### 고급 운영 명령어
```bash
# Docker 고급 관리
cd gamecenter/docker
make scale-grpc REPLICAS=3   # 스케일링
make backup-redis            # 백업
make clean                   # 정리

# 성능 최적화
cargo build --release        # 최적화 빌드
./run-server.sh start-native # 네이티브 실행

# 개발 & 디버깅
./run-server.sh dev          # 개발 모드
./run-server.sh shell        # 컨테이너 접속
```

**이제 모든 명령어를 마스터했습니다! 🎯**