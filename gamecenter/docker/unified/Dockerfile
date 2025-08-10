# 통합 gamecenter 서버 Dockerfile
FROM rust:1.75-slim as builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# 의존성 파일들 복사 (캐시 최적화)
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml ./shared/
COPY grpcserver/Cargo.toml ./grpcserver/
COPY tcpserver/Cargo.toml ./tcpserver/
COPY rudpserver/Cargo.toml ./rudpserver/
COPY gamecenter/Cargo.toml ./gamecenter/

# 소스 코드 복사
COPY . .

# 통합 gamecenter 빌드
RUN cargo build --release -p gamecenter

# Production stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false gameserver

# 바이너리 복사
COPY --from=builder /usr/src/app/target/release/gamecenter /app/gamecenter

# 통합 서버 헬스체크 스크립트
RUN echo '#!/bin/bash
# gRPC 서버 체크
curl -f http://localhost:50051/health > /dev/null 2>&1
grpc_status=$?

# TCP 서버 체크
echo "PING" | nc -w 1 localhost 4000 > /dev/null 2>&1
tcp_status=$?

# RUDP 서버 체크 (포트 리스닝 확인)
ss -uln | grep :5000 > /dev/null 2>&1
rudp_status=$?

# 하나라도 실패하면 실패
if [ $grpc_status -ne 0 ] || [ $tcp_status -ne 0 ] || [ $rudp_status -ne 0 ]; then
    exit 1
fi

exit 0' > /app/health-check.sh \
    && chmod +x /app/health-check.sh

RUN chown -R gameserver:gameserver /app
USER gameserver

WORKDIR /app
EXPOSE 50051 4000 5000/udp

HEALTHCHECK --interval=30s --timeout=15s --start-period=40s --retries=3 \
    CMD ./health-check.sh

ENTRYPOINT ["./gamecenter"]
CMD ["start"]