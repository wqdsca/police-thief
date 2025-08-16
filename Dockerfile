# Police Thief 통합 게임 서버 Dockerfile
# Rust 멀티스테이지 빌드로 최적화

# Build Stage
FROM rust:1.75-slim as builder

# 빌드 도구 설치
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# 작업 디렉토리 설정
WORKDIR /usr/src/app

# 의존성 파일들 먼저 복사 (캐시 최적화)
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml ./shared/
COPY grpcserver/Cargo.toml ./grpcserver/
COPY tcpserver/Cargo.toml ./tcpserver/
COPY rudpserver/Cargo.toml ./rudpserver/
COPY gamecenter/Cargo.toml ./gamecenter/

# 소스 코드 복사
COPY . .

# Release 빌드 (최적화 활성화)
RUN cargo build --release -p gamecenter

# Production Stage
FROM debian:bookworm-slim

# 런타임 의존성 설치
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 비root 사용자 생성
RUN useradd -r -s /bin/false gameserver

# 실행 파일 복사
COPY --from=builder /usr/src/app/target/release/gamecenter /app/gamecenter

# 헬스체크 스크립트 생성
RUN echo '#!/bin/bash\ncurl -f http://localhost:50051/health || exit 1' > /app/health-check.sh \
    && chmod +x /app/health-check.sh

# 사용자 권한 설정
RUN chown -R gameserver:gameserver /app

# 사용자 전환
USER gameserver

# 작업 디렉토리
WORKDIR /app

# 포트 노출
EXPOSE 50051 4000 5000/udp

# 헬스체크
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD ./health-check.sh

# 실행
ENTRYPOINT ["./gamecenter"]
CMD ["start"]