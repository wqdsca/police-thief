# PoliceThief NodeServer

PoliceThief 게임을 위한 Node.js 서버입니다. Redis를 사용한 실시간 사용자 관리와 UDP 통신을 지원합니다.

## 🚀 빠른 시작

### 1. 필수 요구사항

- **Node.js** (v16.0.0 이상)
- **Redis** (v6.0.0 이상)
- **Windows** (start.bat 사용 시)

### 2. 설치

```bash
# 저장소 클론
git clone <repository-url>
cd NodeServer

# 의존성 설치
npm install
```

### 3. 환경 설정

```bash
# 환경변수 파일 생성
copy env.example .env

# .env 파일 편집 (필요한 값 설정)
notepad .env
```

### 4. 서버 실행

#### Windows (권장)
```bash
# Redis + UDP 서버 자동 시작
start.bat
```

#### 수동 실행
```bash
# Redis 서버 시작 (별도 터미널)
redis-server

# Node.js 서버 시작
npm start
```

## 📁 프로젝트 구조

```
NodeServer/
├── Server/                 # 서버 파일들
│   ├── Udp-server.js      # UDP 서버
│   ├── Tcp-server.js      # TCP 서버 (예정)
│   └── Redis-server.js    # Redis 서버 (예정)
├── Shared/                # 공통 모듈
│   ├── config/           # 설정 파일
│   │   └── redis.js      # Redis 설정
│   ├── service/          # 비즈니스 로직
│   │   ├── redisBase.js  # Redis 헬퍼
│   │   └── redisUser.js  # 사용자 서비스
│   └── utils/            # 유틸리티
│       └── Logger.js     # 로깅 시스템
├── Utils/                # 유틸리티
│   └── redisUtils.js     # Redis 유틸리티
├── Controller/           # 컨트롤러
├── Handlers/            # 핸들러
├── Model/               # 데이터 모델
├── start.bat           # Windows 시작 스크립트
├── package.json        # 프로젝트 설정
└── env.example         # 환경변수 예시
```

## ⚙️ 환경변수

| 변수명 | 기본값 | 설명 |
|--------|--------|------|
| `REDIS_HOST` | localhost | Redis 서버 호스트 |
| `REDIS_PORT` | 6379 | Redis 서버 포트 |
| `REDIS_PASSWORD` | | Redis 비밀번호 (선택) |
| `REDIS_DB` | 0 | Redis 데이터베이스 번호 |
| `UDP_IP` | 0.0.0.0 | UDP 서버 IP |
| `UDP_PORT` | 8080 | UDP 서버 포트 |

## 🔧 주요 기능

### Redis 헬퍼
- **Cache Helper**: JSON + List 동시 관리
- **Set Helper**: 다대다 관계 관리
- **ZSet Helper**: 순위/리더보드 관리
- **Hash Helper**: 객체 속성 저장
- **Geo Helper**: 위치 기반 서비스

### UDP 서버
- 실시간 게임 통신
- 사용자 세션 관리
- 방 관리 시스템

## 📊 모니터링

서버는 다음 메트릭을 제공합니다:
- Redis 연결 상태
- UDP 서버 상태
- 사용자 수
- 방 수

## 🛠️ 개발

### 개발 모드 실행
```bash
npm run dev
```

### 로그 확인
```bash
# 실시간 로그 확인
tail -f logs/server.log
```

## 🐛 문제 해결

### Redis 연결 실패
1. Redis 서버가 실행 중인지 확인
2. 환경변수 설정 확인
3. 방화벽 설정 확인

### UDP 서버 시작 실패
1. 포트가 사용 중인지 확인
2. 권한 문제 확인
3. 환경변수 설정 확인

## 📝 라이선스

MIT License

## 🤝 기여

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request 