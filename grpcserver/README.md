# gRPC Server

Police Thief 게임을 위한 gRPC 서버입니다.

## 기능

- **Room Service**: 방 생성 및 조회
- **User Service**: 사용자 로그인 및 회원가입
- **Integration Tests**: gRPC 클라이언트 테스트

## 실행 방법

```bash
# 환경 변수 설정
export grpc_host=127.0.0.1
export grpc_port=50051

# 서버 실행
cargo run --bin grpcserver
```

## 테스트

```bash
# 통합 테스트 실행
cargo test --test integration_test
```

## 프로젝트 구조

```
grpcserver/
├── src/
│   ├── controller/     # gRPC 컨트롤러
│   ├── service/        # 비즈니스 로직
│   ├── test/          # 테스트 코드
│   └── server.rs      # 서버 설정
├── proto/             # Protocol Buffer 정의
└── tests/             # 통합 테스트
``` 