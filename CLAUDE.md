# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Police Thief game server - Rust-based real-time multiplayer game backend with proven 12,991+ msg/sec performance supporting 500+ concurrent connections.

**Workspace Components**:
- **shared**: Core libraries (Redis/MariaDB helpers, security framework, high-performance tools)
- **grpcserver**: gRPC API server for room/user management (requires protoc)
- **tcpserver**: Production-ready TCP server with 8 optimization services
- **quicserver**: QUIC server with stream multiplexing support
- **gamecenter**: Unified server orchestrator with Admin API and OAuth integration

## Critical Setup Requirements

### External Dependencies
```bash
# MUST HAVE: Protocol Buffer Compiler for gRPC
brew install protobuf              # macOS
apt-get install protobuf-compiler  # Linux
# Or download from: https://github.com/protocolbuffers/protobuf/releases

# MUST HAVE: Redis Server
redis-server                        # Start Redis on port 6379

# OPTIONAL: MariaDB Server (for persistence)
mysql.server start                  # Start MariaDB
mysql -u root -p                   # Create 'police' database
```

### Environment Configuration
```bash
# Copy .env.example to .env and configure:
cp .env.example .env

# Minimum required settings:
redis_host=127.0.0.1
redis_port=6379
tcp_host=127.0.0.1
tcp_port=4000
db_host=localhost
db_id=root
db_password=your_password
db_name=police
JWT_SECRET_KEY=minimum_256_bits_key
```

## Essential Commands

### Build Commands
```bash
# Full workspace build (verify protoc first!)
cargo build --release

# Component-specific builds
cargo build -p shared       # Always succeeds
cargo build -p grpcserver   # Needs protoc
cargo build -p tcpserver    # Production server
cargo build -p quicserver   # QUIC server
cargo build -p gamecenter   # Game logic
```

### Run Commands
```bash
# Quick start with wrapper scripts
./run-server.sh start        # Linux/Mac - All services with Docker
run-server.bat start         # Windows - All services with Docker

# Individual server startup
cargo run --bin grpcserver   # API server (port 50051)
cargo run --bin tcpserver    # Game server (port 4000)
cargo run --bin quicserver   # QUIC server (configurable)

# Game center modes (unified orchestrator)
cargo run -p gamecenter start   # All servers + Redis lifecycle
cargo run -p gamecenter grpc    # gRPC server only
cargo run -p gamecenter tcp     # TCP server only
cargo run -p gamecenter server  # Background mode
cargo run -p gamecenter stop    # Graceful shutdown
```

### Quality Commands
```bash
# Code formatting
cargo fmt --all

# Linting with detailed warnings
cargo clippy --all -- -W clippy::all

# Fast compilation check
cargo check --all

# Fast compilation check (pre-build validation)
./check_compilation.sh

# Full quality validation (100-point grading system)
./scripts/run_full_validation.sh

# Security audit
./scripts/run_security_audit.sh
```

### Testing Commands
```bash
# Unit tests per component
cargo test -p shared --lib
cargo test -p tcpserver --lib
cargo test -p grpcserver --lib
cargo test -p gamecenter --lib
cargo test -p quicserver --lib

# Integration tests
cargo run --bin test_client  # Full gRPC + Redis test
cargo test -p gamecenter     # GameCenter integration tests

# Performance testing
cd tcpserver && ./test_runner.sh  # Linux/Mac only
python tcp_load_test.py           # Load testing

# Single test execution
cargo test -p tcpserver test_protocol::test_decode_message
cargo test -p shared test_redis_connection
cargo test -p gamecenter test_config_validation
cargo test -p grpcserver test_grpc_connection --lib -- --nocapture
```

### Debugging Commands
```bash
# Redis monitoring
redis-cli MONITOR           # Watch all commands
redis-cli KEYS "*"         # List all keys
redis-cli HGETALL "user:1" # Inspect user data

# Service health checks
redis-cli ping             # Redis connectivity
./run-server.sh health     # All services health
./run-server.sh status     # Service status

# Performance monitoring
./run-server.sh logs       # Real-time logs
watch -n 1 'curl -s localhost:4000/stats'  # TCP server metrics
```

## High-Level Architecture

### Multi-Server Architecture Pattern
```
GameCenter (Orchestrator)
├── gRPC Server (50051)    → JWT Auth → Service Layer → Redis/MariaDB
├── TCP Server (4000)      → Protocol Handler → Connection Pool → Game Logic
├── QUIC Server (custom)   → Stream Multiplexing → Connection Management
└── Admin API (8080)       → WebSocket Monitor → System Metrics
                                    ↓
                          Unified Redis Cache (6379)
                                    ↓
                            MariaDB Persistence
```

### TCP Server Performance Architecture (12,991+ msg/sec)
8 optimization services working in concert:
1. **DashMap Optimizer**: Lock-free concurrent hashmap with CPU-specific sharding
2. **Async I/O Optimizer**: Zero-copy operations with vectored I/O
3. **SIMD Optimizer**: AVX2/SSE4.2 hardware acceleration
4. **Message Compression**: LZ4/Zstd adaptive compression
5. **Connection Pool**: Intelligent connection management
6. **Performance Monitor**: Real-time metrics and alerting
7. **Memory Pool**: Object recycling with RAII
8. **Parallel Broadcast**: Rayon-based parallel processing

### Redis Architecture
```
Key Patterns:
- user:{id}              → User session (Hash, TTL 3600s)
- room:info:{id}         → Room metadata (Hash, TTL 3600s)
- room:list:time:index   → Time-sorted rooms (ZSet)
- room_counter:id        → ID generation (String)
- recycle_room_id:index  → ID recycling (List)

Performance Features:
- Pipeline operations for 10-20x speedup
- Connection pooling with retry logic
- Time-based pagination using ZSet scores
```

### Protocol Specifications
```
TCP Protocol: [4-byte length header][JSON payload]
gRPC: Protocol Buffers v3 (room.proto, user.proto, auth.proto)
QUIC: Binary protocol with stream multiplexing
```

### Security Framework Architecture
**JWT Authentication Modes** (grpcserver):
```rust
RequiredAuth    // All requests need valid token
OptionalAuth    // Token validated if present
ConditionalAuth // Per-endpoint requirements
```

**Security Modules** (shared/src/security/):
- **Access Control**: Role-based permissions with inheritance
- **Rate Limiter**: Token bucket with Redis backend
- **Input Validator**: OWASP-compliant validation
- **Redis Command Validator**: Injection attack prevention
- **Security Auditor**: Automated OWASP compliance checking
- **Crypto**: AES encryption with secure key management
- **Key Rotation**: Automated key management system

## Performance Benchmarks

### TCP Server (Production Ready)
- **Throughput**: 12,991+ msg/sec sustained, 41,064+ peak
- **Connections**: 500+ concurrent with 100% success rate
- **Memory**: 11MB for 500 connections (22KB per connection)
- **Latency**: <1ms p99 response time
- **CPU**: Optimized for single-core, scales with multi-core

### QUIC Server (Under Development)
- **Protocol**: Quinn-based QUIC implementation
- **Features**: Stream multiplexing, 0-RTT connection resumption
- **Compression**: LZ4/Zstd binary protocol support
- **Security**: TLS 1.3 with certificate generation

### Load Test Results
```bash
# TCP Production: 12,991+ msg/sec (Production Ready)
# QUIC: Under development
```

## Critical Code Patterns

### Error Handling Pattern
```rust
// ALWAYS use AppError, NEVER unwrap() in production
use shared::tool::error::AppError;
fn operation() -> Result<T, AppError> {
    // Never: value.unwrap()
    // Always: value.map_err(AppError::from)?
}
```

### Redis Pipeline Pattern
```rust
// Batch operations for 10-20x performance
let operations = vec![
    RedisOp::Set("key1", "value1"),
    RedisOp::Set("key2", "value2"),
];
execute_pipeline(operations).await?;
```

### Transaction Pattern
```rust
// Multi-operation consistency
helpers::with_transaction(|tx| async {
    // Multiple DB operations
    // Auto rollback on error
}).await?;
```

## TCP Server Maintenance Guidelines

### Safe Extension Pattern
```rust
// Add features via plugins, NOT core modifications
pub trait MessageProcessor {
    async fn process(&self, msg: Message) -> Result<Response>;
}
// Register: handler.register_plugin(Box::new(NewFeature));
```

### Performance Thresholds
- **Alert if**: Throughput < 10,000 msg/sec
- **Alert if**: Memory > 15MB for 500 connections
- **Alert if**: p99 latency > 2ms
- **Alert if**: Error rate > 1%

### DO NOT Modify Directly
- Core optimization services (8 services in tcpserver)
- DashMap configuration
- SIMD implementations
- Memory pool structures

## Common Issues & Solutions

### Build Issues
```bash
# "protoc not found"
→ Install protobuf compiler first

# "Redis connection failed"
→ Start redis-server on port 6379

# "Too many open files"
→ ulimit -n 10000  # Increase file descriptor limit
```

### Performance Issues
```bash
# Low throughput
→ Check CPU usage, enable SIMD optimizations

# High memory usage
→ Check for connection leaks, adjust pool sizes

# High latency
→ Review message size, enable compression
```

## Development Workflow

### Adding New Features
1. Check impact on 8 optimization services
2. Use plugin pattern for extensions
3. Run performance benchmarks before/after
4. Verify with `cargo clippy --all`
5. Test with 500+ connections load test

### Pre-Commit Checklist
```bash
cargo fmt --all              # Format code
cargo clippy --all           # Lint check
cargo test --all             # Run tests
cargo build --release        # Verify build
```

### Performance Testing Workflow
```bash
# Baseline measurement
python tcp_load_test.py

# Make changes

# Verify no regression
python tcp_load_test.py
# Throughput should be ≥ 12,000 msg/sec
# Memory should be ≤ 15MB
```

## Project-Specific Notes

### Shared High-Performance Tools
Located in `shared/src/tool/high_performance/`:
- **Memory Pools**: Object recycling with RAII and safe primitives
- **Atomic Statistics**: Lock-free counters and metrics collection
- **DashMap Optimizer**: CPU-aware sharding for concurrent hashmaps
- **SIMD Operations**: AVX2/SSE4.2 vectorized computation
- **Message Compression**: Adaptive LZ4/Zstd with size-based selection
- **Parallel Processing**: Rayon-based work stealing
- **Safe Primitives**: Memory-safe alternatives to unsafe operations

### Monitoring & Observability
- **Prometheus Integration**: Metrics collection with custom collectors
- **Performance Monitoring**: Real-time system metrics (30s intervals)
- **Docker Compose Monitoring**: Pre-configured Prometheus + Grafana stack
- **Admin Dashboard**: Web-based monitoring with WebSocket updates
- **Security Auditing**: Automated OWASP compliance validation

### Proto File Changes
After modifying `.proto` files:
```bash
cargo build -p grpcserver  # Regenerates Rust code
cargo build -p gamecenter  # If auth.proto changed
```

### Redis Data Verification
```bash
# During testing, verify data consistency:
redis-cli KEYS "*" | wc -l  # Count total keys
redis-cli DBSIZE            # Database size
redis-cli INFO memory       # Memory usage
```

### Production Deployment
- **Memory**: 1GB RAM supports 1000+ connections (TCP)
- **CPU**: 1 vCPU sufficient for 12K+ msg/sec (TCP)
- **Protocols**: TCP (4-byte header + JSON), gRPC (protobuf), QUIC (binary)
- **Security**: JWT auth, role-based access control, key rotation
- **Monitoring**: Prometheus metrics, Admin dashboard, real-time WebSocket updates
- **Scaling**: Multi-instance deployment with Redis cluster, load balancer support
- **Docker**: Full containerization with monitoring stack (docker-compose.monitoring.yml)