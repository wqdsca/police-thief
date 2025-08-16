# gRPC 서버 전용 Dockerfile
FROM rust:1.75-slim as builder

# 빌드 도구 설치
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# 의존성 파일들 복사
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml ./shared/
COPY grpcserver/Cargo.toml ./grpcserver/
COPY gamecenter/Cargo.toml ./gamecenter/

# 소스 코드 복사
COPY shared/ ./shared/
COPY grpcserver/ ./grpcserver/
COPY gamecenter/src/unified_server.rs ./gamecenter/src/

# gRPC 서버만 빌드
RUN cargo build --release -p grpcserver

# Production stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 사용자 생성
RUN useradd -r -s /bin/false grpcserver

# 바이너리 복사
COPY --from=builder /usr/src/app/target/release/grpcserver /app/grpcserver

# 헬스체크 스크립트
RUN echo '#!/bin/bash\ncurl -f http://localhost:50051/health || exit 1' > /app/health-check.sh \
    && chmod +x /app/health-check.sh

RUN chown -R grpcserver:grpcserver /app
USER grpcserver

WORKDIR /app
EXPOSE 50051

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD ./health-check.sh

ENTRYPOINT ["./grpcserver"]