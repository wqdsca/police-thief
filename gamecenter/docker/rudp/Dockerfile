# RUDP 서버 전용 Dockerfile
FROM rust:1.75-slim as builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# 의존성 파일들 복사
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml ./shared/
COPY rudpserver/Cargo.toml ./rudpserver/
COPY gamecenter/Cargo.toml ./gamecenter/

# 소스 코드 복사
COPY shared/ ./shared/
COPY rudpserver/ ./rudpserver/
COPY gamecenter/src/unified_server.rs ./gamecenter/src/

# RUDP 서버만 빌드
RUN cargo build --release -p rudpserver

# Production stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false rudpserver

# 바이너리 복사
COPY --from=builder /usr/src/app/target/release/rudpserver /app/rudpserver

# UDP 헬스체크 스크립트 (간단한 포트 체크)
RUN echo '#!/bin/bash\nss -uln | grep :5000 > /dev/null || exit 1' > /app/health-check.sh \
    && chmod +x /app/health-check.sh

RUN chown -R rudpserver:rudpserver /app
USER rudpserver

WORKDIR /app
EXPOSE 5000/udp

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD ./health-check.sh

ENTRYPOINT ["./rudpserver"]