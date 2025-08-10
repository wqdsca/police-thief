# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Police Thief game server - Rust-based real-time multiplayer game backend with proven 12,991+ msg/sec performance supporting 500+ concurrent connections.

**Workspace Components**:
- **shared**: Core libraries (Redis/MariaDB helpers, high-performance tools, error handling)
- **grpcserver**: gRPC API server for room/user management (requires protoc)
- **tcpserver**: Production-ready TCP server with 8 optimization services
- **rudpserver**: Experimental RUDP server with 16 optimization services
- **gamecenter**: Game logic orchestrator with Redis lifecycle management

## Critical Setup Requirements

### External Dependencies
```bash
# MUST HAVE: Protocol Buffer Compiler for gRPC
brew install protobuf              # macOS
apt-get install protobuf-compiler  # Linux
# Or download from: https://github.com/protocolbuffers/protobuf/releases

# MUST HAVE: Redis Server
redis-server                        # Start Redis on port 6379

# MUST HAVE: MariaDB Server
mysql.server start                  # Start MariaDB
mysql -u root -p                   # Create 'police' database
```

### Environment Configuration
```bash
# Copy .env to .env and configure:
cp .env .env

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
cargo build -p rudpserver   # Experimental
cargo build -p gamecenter   # Game logic
```

### Run Commands
```bash
# Start servers
cargo run --bin grpcserver  # API server (port 50051)
cargo run --bin tcpserver   # Game server (port 4000)
cargo run --bin rudpserver  # RUDP server (port 5000)

# Game center modes
cargo run -p gamecenter start   # With Redis lifecycle
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
```

### Testing Commands
```bash
# Unit tests per component
cargo test -p shared --lib
cargo test -p tcpserver --lib
cargo test -p rudpserver --lib

# Integration tests
cargo run --bin test_client  # Full gRPC + Redis test

# Performance testing
cd tcpserver && ./test_runner.sh  # Linux/Mac only
python tcp_load_test.py           # Load testing
python rudp_load_test.py          # RUDP testing

# Single test execution
cargo test -p tcpserver test_protocol::test_decode_message
```

### Debugging Commands
```bash
# Redis monitoring
redis-cli MONITOR           # Watch all commands
redis-cli KEYS "*"         # List all keys
redis-cli HGETALL "user:1" # Inspect user data

# Service health checks
redis-cli ping             # Redis connectivity
cargo check --all          # Compilation status

# Performance monitoring (Windows)
powershell "Get-Process tcpserver* | Select-Object Name,CPU,WorkingSet"

# Performance monitoring (Linux/Mac)
watch -n 1 'ps aux | grep tcpserver'
```

## High-Level Architecture

### Data Flow Pattern
```
Client Request → gRPC Server → Service Layer → Redis Cache → MariaDB
                     ↓              ↓               ↓           ↓
                JWT Auth      Business Logic   Fast Access  Persistence
```

### TCP Server Performance Architecture
The TCP server achieves 12,991+ msg/sec through 8 optimization services:
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
RUDP Protocol: [2-byte seq][1-byte flags][payload]
gRPC: Protocol Buffers v3 (room.proto, user.proto)
```

### JWT Authentication Modes
```rust
// Three authentication patterns in grpcserver:
RequiredAuth    // All requests need valid token
OptionalAuth    // Token validated if present
ConditionalAuth // Per-endpoint requirements
```

## Performance Benchmarks

### TCP Server (Production Ready)
- **Throughput**: 12,991+ msg/sec sustained, 41,064+ peak
- **Connections**: 500+ concurrent with 100% success rate
- **Memory**: 11MB for 500 connections (22KB per connection)
- **Latency**: <1ms p99 response time
- **CPU**: Optimized for single-core, scales with multi-core

### RUDP Server (Experimental)
- **Target**: 20,000+ msg/sec
- **Memory Target**: 8-10MB for 1000 connections
- **Latency Target**: <0.5ms
- **Status**: Active development, not production ready

### Load Test Results
```bash
# UDP Simple Server: 8,883 msg/sec (Grade A - 98/100)
# TCP Production: 12,991+ msg/sec (Production Ready)
# RUDP: Under development
```

## Critical Code Patterns

### Error Handling Pattern
```rust
// ALWAYS use AppError, NEVER unwrap() in production
use shared::error::AppError;
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
- Memory pools with object recycling
- Atomic statistics (lock-free)
- DashMap optimizer (CPU sharding)
- SIMD operations (AVX2/SSE4.2)
- Message compression (LZ4/Zstd)
- Parallel processing (Rayon)

### Proto File Changes
After modifying `.proto` files:
```bash
cargo build -p grpcserver  # Regenerates Rust code
```

### Redis Data Verification
```bash
# During testing, verify data consistency:
redis-cli KEYS "*" | wc -l  # Count total keys
redis-cli DBSIZE            # Database size
redis-cli INFO memory       # Memory usage
```

### Production Deployment
- **Memory**: 1GB RAM supports 1000+ connections
- **CPU**: 1 vCPU sufficient for 5000+ players (UDP)
- **Protocol**: Ensure clients use 4-byte header + JSON
- **Monitoring**: Enable performance_monitor service
- **Scaling**: Deploy multiple instances at 10,000+ scale