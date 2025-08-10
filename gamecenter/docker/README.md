# Police Thief 게임 서버 Docker 배포 가이드

## 🏗️ 아키텍처 옵션

### 1. 통합 서버 (Unified) - 권장
```bash
cd gamecenter/docker
docker-compose -f docker-compose.unified.yml up -d
```

### 2. 마이크로서비스 분리
```bash
cd gamecenter/docker
docker-compose -f docker-compose.microservices.yml up -d
```

## 📁 디렉토리 구조

```
gamecenter/docker/
├── grpc/
│   └── Dockerfile              # gRPC 서버 전용
├── tcp/
│   └── Dockerfile              # TCP 서버 전용  
├── rudp/
│   └── Dockerfile              # RUDP 서버 전용
├── unified/
│   └── Dockerfile              # 통합 서버
├── docker-compose.unified.yml   # 통합 서버 구성
├── docker-compose.microservices.yml # 마이크로서비스 구성
├── nginx.conf                  # 로드밸런서 설정
├── prometheus.yml              # 모니터링 설정
└── README.md                   # 이 파일
```

## 🚀 빠른 시작

### 통합 서버 실행
```bash
# 1. 프로젝트 루트에서 실행
cd C:\Users\Administrator\Desktop\PoliceTheif\Backend

# 2. 환경변수 설정
cp .env .env
# .env 파일 편집 필요

# 3. 통합 서버 시작
docker-compose -f gamecenter/docker/docker-compose.unified.yml up -d

# 4. 로그 확인
docker-compose -f gamecenter/docker/docker-compose.unified.yml logs -f
```

### 마이크로서비스 실행
```bash
# 1. 마이크로서비스 시작
docker-compose -f gamecenter/docker/docker-compose.microservices.yml up -d

# 2. 개별 서비스 로그 확인
docker logs police-grpc
docker logs police-tcp
docker logs police-rudp
```

## 🔧 설정 옵션

### 환경변수 설정
```bash
# Redis 설정
redis_host=redis
redis_port=6379

# 서버 포트 설정
grpc_port=50051
tcp_port=4000
udp_port=5000

# JWT 보안키 (프로덕션에서 반드시 변경)
JWT_SECRET_KEY=your_production_secret_key_minimum_256_bits_required

# 로깅 레벨
RUST_LOG=info
```

### 포트 매핑
| 서비스 | 내부 포트 | 외부 포트 | 프로토콜 |
|--------|-----------|-----------|----------|
| gRPC   | 50051     | 50051     | HTTP/2   |
| TCP    | 4000      | 4000      | TCP      |
| RUDP   | 5000      | 5000      | UDP      |
| Redis  | 6379      | 6379      | TCP      |
| 모니터링| 9090      | 9090      | HTTP     |

## 📊 모니터링 & 헬스체크

### 헬스체크 엔드포인트
```bash
# 통합 서버 상태
docker exec police-gamecenter ./health-check.sh

# 개별 서비스 상태
curl http://localhost:50051/health  # gRPC
nc -z localhost 4000                # TCP
ss -uln | grep :5000              # RUDP
```

### Prometheus 모니터링
```bash
# 모니터링 대시보드 접속
http://localhost:9090

# 주요 메트릭
- up{job="gamecenter-unified"}
- redis_connected_clients
- process_cpu_seconds_total
- process_resident_memory_bytes
```

## 🔄 운영 명령어

### 시작/중지
```bash
# 시작
docker-compose -f gamecenter/docker/docker-compose.unified.yml up -d

# 중지
docker-compose -f gamecenter/docker/docker-compose.unified.yml down

# 재시작
docker-compose -f gamecenter/docker/docker-compose.unified.yml restart
```

### 스케일링
```bash
# TCP 서버를 3개로 확장
docker-compose -f gamecenter/docker/docker-compose.microservices.yml up -d --scale tcp-service=3

# gRPC 서버를 2개로 확장
docker-compose -f gamecenter/docker/docker-compose.microservices.yml up -d --scale grpc-service=2
```

### 로그 관리
```bash
# 실시간 로그
docker-compose -f gamecenter/docker/docker-compose.unified.yml logs -f

# 특정 서비스 로그
docker logs police-gamecenter

# 로그 파일 크기 제한 (docker-compose.yml에 추가)
logging:
  driver: "json-file"
  options:
    max-size: "10m"
    max-file: "3"
```

## 🛠️ 개발 & 디버깅

### 개발 모드 실행
```bash
# 로컬 소스 마운트
docker-compose -f gamecenter/docker/docker-compose.unified.yml \
  -f docker-compose.dev.yml up -d
```

### 컨테이너 접속
```bash
# 통합 서버 컨테이너 접속
docker exec -it police-gamecenter bash

# Redis 컨테이너 접속
docker exec -it police-redis redis-cli
```

## 🔐 보안 설정

### JWT 키 관리
```bash
# 안전한 키 생성
openssl rand -base64 32

# 환경변수 설정
export JWT_SECRET_KEY="generated_secure_key_here"
```

### 방화벽 설정
```bash
# 필요한 포트만 열기
ufw allow 50051/tcp  # gRPC
ufw allow 4000/tcp   # TCP  
ufw allow 5000/udp   # RUDP
```

## 🚨 문제 해결

### 일반적인 문제들

1. **포트 충돌**
   ```bash
   # 포트 사용 확인
   netstat -tulpn | grep :4000
   
   # 다른 포트로 변경
   docker-compose -f docker-compose.unified.yml up -d -p 4001:4000
   ```

2. **Redis 연결 실패**
   ```bash
   # Redis 컨테이너 상태 확인
   docker logs police-redis
   
   # Redis 연결 테스트
   docker exec police-redis redis-cli ping
   ```

3. **메모리 부족**
   ```bash
   # 리소스 사용량 확인
   docker stats
   
   # 메모리 제한 조정 (docker-compose.yml)
   deploy:
     resources:
       limits:
         memory: 4G
   ```

## 📈 성능 튜닝

### 최적화 설정
```yaml
# docker-compose.unified.yml에 추가
environment:
  - RUST_MIN_STACK=8388608      # 8MB 스택
  - RUST_BACKTRACE=0            # 프로덕션에서 비활성화
  
deploy:
  resources:
    limits:
      memory: 2G
      cpus: '2.0'
    reservations:
      memory: 1G
      cpus: '1.0'
```

### Redis 최적화
```yaml
redis:
  command: redis-server --appendonly yes --maxmemory 512mb --maxmemory-policy allkeys-lru
```

이제 유연한 Docker 기반 배포 시스템이 완성되었습니다! 🎯