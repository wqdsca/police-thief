# GameCenter 서버 관리자 페이지

React 기반의 GameCenter 서버 관리자 대시보드입니다.

## 주요 기능

### 1. 서버 모니터링 대시보드
- **실시간 CPU 사용량 모니터링**: 전체 CPU 및 코어별 사용량 추적
- **메모리 사용량 추적**: 서버별 메모리 사용 현황
- **연결된 클라이언트 수**: 실시간 연결 상태 확인
- **서버 가동 시간**: 각 서버의 업타임 표시
- **실시간 차트**: CPU 및 메모리 사용량 추이 그래프

### 2. 유저 차단 관리
- **차단 유저 목록**: 현재 차단된 모든 유저 조회
- **새 유저 차단**: 사유와 기간 설정 가능
- **차단 해제**: 즉시 차단 해제 기능
- **차단 기간 설정**: 1시간부터 영구까지 다양한 옵션

### 3. 이벤트 보상 관리
- **이벤트 생성**: 새로운 이벤트 및 보상 설정
- **진행 상황 모니터링**: 참여자 수 및 진행률 확인
- **이벤트 종료**: 진행 중인 이벤트 즉시 종료
- **보상 타입**: 코인, 보석, 아이템, 경험치 지원

## 기술 스택

- **Frontend**: React 19, TypeScript
- **UI Framework**: Material-UI v7
- **Charts**: Recharts
- **HTTP Client**: Axios
- **Testing**: Jest, React Testing Library

## API 엔드포인트

### 서버 상태
- `GET /api/admin/status` - 메인 서버 상태
- `GET /api/admin/servers` - 모든 서버 상태

### 유저 관리
- `GET /api/admin/users/banned` - 차단 유저 목록
- `POST /api/admin/users/ban` - 유저 차단
- `DELETE /api/admin/users/unban/{user_id}` - 차단 해제

### 이벤트 관리
- `GET /api/admin/events` - 이벤트 목록
- `POST /api/admin/events` - 이벤트 생성
- `PUT /api/admin/events/{event_id}/end` - 이벤트 종료

## 시작하기

### 요구사항
- Node.js 16+
- npm 또는 yarn

### 설치 및 실행

```bash
# 의존성 설치
npm install

# 개발 서버 시작 (포트 3000)
npm start

# 프로덕션 빌드
npm run build

# 테스트 실행
npm test
```

### 환경 변수

`.env` 파일 생성:
```env
REACT_APP_API_URL=http://localhost:8080/api/admin
```

## 프로젝트 구조

```
src/
├── components/          # React 컴포넌트
│   ├── ServerMonitor.tsx      # 서버 모니터링 대시보드
│   ├── UserBanManager.tsx     # 유저 차단 관리
│   └── EventRewardManager.tsx # 이벤트 보상 관리
├── services/           # API 서비스
│   └── api.ts         # 백엔드 API 통신
├── App.tsx            # 메인 애플리케이션
└── index.tsx          # 진입점
```

## 개발 가이드

### TDD (Test-Driven Development)
모든 컴포넌트는 TDD 방식으로 개발되었습니다:
1. 테스트 작성
2. 컴포넌트 구현
3. 리팩토링

### 테스트 실행
```bash
# 모든 테스트 실행
npm test

# 커버리지 확인
npm test -- --coverage
```

## 보안 고려사항

- CORS 설정으로 허가된 도메인만 API 접근 가능
- 관리자 인증 토큰 필요 (추후 구현 예정)
- 민감한 작업에 대한 확인 다이얼로그

## 성능 최적화

- 5초마다 서버 상태 자동 새로고침
- 10초마다 이벤트 목록 자동 업데이트
- React.memo를 통한 불필요한 리렌더링 방지
- 차트 데이터 최대 20개 포인트로 제한

## 향후 개선 사항

- [ ] JWT 기반 관리자 인증
- [ ] 실시간 알림 (WebSocket)
- [ ] 서버 로그 뷰어
- [ ] 데이터베이스 상태 모니터링
- [ ] 백업 및 복구 기능
- [ ] 다국어 지원 (i18n)
