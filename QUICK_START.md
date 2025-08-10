# 🚀 Police Thief 게임 서버 빠른 시작 가이드

## 🎯 한 줄 요약
**Windows**: `run-server.bat start` | **Linux/Mac**: `./run-server.sh start`

---

## 📋 사전 준비사항

### 필수 설치 항목
- [Rust](https://rustup.rs/) (cargo 명령어 필요)
- [Docker](https://www.docker.com/get-started) (권장) 또는 Redis Server
- [Git](https://git-scm.com/) 

### 선택 설치 항목  
- [Protocol Buffers](https://github.com/protocolbuffers/protobuf/releases) (gRPC용)
- [MariaDB/MySQL](https://mariadb.org/download/) (데이터베이스용)

---

## ⚡ 초고속 시작 (30초)

### Windows
```batch
# 1. 환경 설정
run-server.bat setup

# 2. 서버 시작 (Docker 통합 모드)
run-server.bat start

# 3. 상태 확인
run-server.bat health
```

### Linux/Mac  
```bash
# 1. 실행 권한 부여
chmod +x run-server.sh

# 2. 환경 설정
./run-server.sh setup

# 3. 서버 시작 (Docker 통합 모드)
./run-server.sh start

# 4. 상태 확인  
./run-server.sh health
```

### 접속 정보
- **gRPC API**: `http://localhost:50051`
- **TCP 게임**: `localhost:4000`
- **RUDP 게임**: `localhost:5000`  
- **Redis DB**: `localhost:6379`
- **모니터링**: `http://localhost:9090`

---

## 🎮 실행 모드별 가이드

### 1. 통합 Docker 모드 (권장) 🐳
**가장 쉬운 방법 - 모든 서버를 하나의 컨테이너에서 실행**

```bash
# 시작
./run-server.sh start

# 또는 명시적으로
./run-server.sh start-docker

# 로그 확인
./run-server.sh logs

# 중지
./run-server.sh stop
```

### 2. 마이크로서비스 모드 🔧
**서버별 독립 컨테이너 - 개별 스케일링 가능**

```bash
# 마이크로서비스 시작
./run-server.sh start-micro

# gRPC 서버만 3개로 확장
cd gamecenter/docker
make scale-grpc REPLICAS=3

# TCP 서버만 2개로 확장  
make scale-tcp REPLICAS=2
```

### 3. 네이티브 모드 ⚡
**최고 성능 - Docker 없이 직접 실행**

```bash  
# 직접 실행 (Redis가 별도로 필요)
./run-server.sh start-native

# 또는 Cargo 직접 사용
cargo run -p gamecenter --release -- start
```

### 4. 개별 서비스 모드 🎛️
**특정 서버만 실행**

```bash
# gRPC 서버만
./run-server.sh grpc

# TCP 서버만  
./run-server.sh tcp

# RUDP 서버만
./run-server.sh rudp
```

---

## 🔧 고급 설정

### 환경변수 편집
```bash
# .env 파일 편집
nano .env

# 주요 설정
redis_host=127.0.0.1
tcp_port=4000
JWT_SECRET_KEY=your_secure_key_here
MAX_CONCURRENT_PLAYERS=500
```

### Docker 환경 설정
```bash  
# Docker 환경변수
nano gamecenter/docker/.env

# Docker Compose 직접 사용
cd gamecenter/docker
docker-compose -f docker-compose.unified.yml up -d
```

### 성능 튜닝
```bash
# Release 모드 빌드
./run-server.sh build

# Docker 이미지 빌드
./run-server.sh build-docker

# 캐시 정리
./run-server.sh clean
```

---

## 📊 모니터링 & 관리

### 상태 확인
```bash
# 전체 상태
./run-server.sh status

# 헬스체크  
./run-server.sh health

# 실시간 로그
./run-server.sh logs
```

### 성능 테스트
```bash
# 기본 연결 테스트
./run-server.sh test

# TCP 서버 성능 테스트
python tcp_load_test.py

# RUDP 서버 성능 테스트  
python rudp_load_test.py
```

### 개발자 도구
```bash
# 컨테이너 접속
./run-server.sh shell

# Redis 접속
docker exec -it police-redis redis-cli

# 모니터링 대시보드  
./run-server.sh monitor
```

---

## 🚨 문제 해결

### 일반적인 문제

#### 1. 포트 충돌
```bash
# 포트 사용 확인
netstat -tulpn | grep :4000

# 다른 포트 사용
export tcp_port=4001
./run-server.sh start
```

#### 2. Docker 문제  
```bash
# Docker 서비스 재시작
sudo systemctl restart docker

# 컨테이너 강제 정리
docker system prune -f
```

#### 3. 빌드 실패
```bash
# 의존성 업데이트
cargo update

# 캐시 정리 후 재빌드
cargo clean
cargo build --release
```

#### 4. Redis 연결 실패
```bash
# Redis 상태 확인
redis-cli ping

# Redis 서버 시작
redis-server --daemonize yes
```

### 로그 분석
```bash
# 실시간 로그 
./run-server.sh logs

# 특정 서비스 로그
docker logs police-grpc
docker logs police-tcp  
docker logs police-rudp
```

---

## 📈 성능 벤치마크

### 현재 성능 수치
- **TCP 서버**: 12,991+ msg/sec (500 동시접속)
- **RUDP 서버**: 20,000+ msg/sec (목표)
- **지연시간**: <1ms p99
- **메모리 사용량**: 22KB per connection

### 성능 테스트 실행
```bash
# TCP 서버 로드 테스트
python tcp_load_test.py

# 결과 예시:
# Messages/sec: 12,991
# Connections: 500
# Success rate: 100%
# Memory usage: 11MB
```

---

## 🎯 프로덕션 배포

### 클라우드 배포 옵션

#### AWS
```bash
# ECS Fargate 배포
aws ecs create-service --service-name police-thief

# EC2 직접 배포  
./run-server.sh start-native
```

#### Google Cloud
```bash
# Cloud Run 배포
gcloud run deploy police-thief --source .

# GKE 배포
kubectl apply -f k8s/
```

#### Docker Swarm
```bash  
# 스웜 모드 초기화
docker swarm init

# 스택 배포
docker stack deploy -c docker-compose.yml police-thief
```

---

## 📞 지원

### 명령어 도움말
```bash
./run-server.sh help           # 전체 도움말
./run-server.sh --help         # 상세 도움말  
./run-server.sh version        # 버전 정보
```

### 프로젝트 문서
- `CLAUDE.md` - 프로젝트 전체 가이드
- `gamecenter/docker/README.md` - Docker 상세 가이드
- `deployment-comparison.md` - 배포 방식 비교

### 성능 최적화  
- TCP 서버: 8개 최적화 서비스 내장
- RUDP 서버: 16개 최적화 서비스 계획  
- 메모리 풀링, SIMD 가속, 비동기 I/O

---

## 🎉 완료!

이제 Police Thief 게임 서버가 실행 중입니다!

**다음 단계:**
1. 게임 클라이언트 연결 테스트
2. 성능 모니터링 설정  
3. 프로덕션 환경 구성
4. 백업 및 보안 설정

**Happy Gaming! 🎮**