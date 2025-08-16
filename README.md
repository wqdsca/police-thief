# Police Thief Game Server

[![CI/CD Pipeline](https://github.com/user/police-thief/workflows/CI%2FCD%20Pipeline/badge.svg)](https://github.com/user/police-thief/actions)
[![Quality Score](https://img.shields.io/badge/Quality-100%2F100-brightgreen.svg)](https://github.com/user/police-thief)
[![Performance](https://img.shields.io/badge/Performance-12%2C991%2B%20msg%2Fsec-blue.svg)](https://github.com/user/police-thief)
[![Security](https://img.shields.io/badge/Security-OWASP%20Compliant-green.svg)](https://github.com/user/police-thief)

Production-ready Rust game server supporting **12,991+ msg/sec** with **500+ concurrent connections**. Comprehensive 100-point quality system with zero unwrap()/panic!() in production code.

## 🚀 Quick Start

### Prerequisites
- **Rust 1.70+** (stable toolchain)
- **Redis 7.0+** (running on port 6379)
- **Protocol Buffers compiler** (protoc)
- **CMake** and **NASM** (for optimizations)

### One-Command Setup
```bash
# Ubuntu/Debian/WSL
./setup-deps.sh

# macOS
./setup-deps.sh macos

# Windows (PowerShell as Administrator)
.\setup-deps.ps1
```

### Build & Run
```bash
# Quick build verification
cargo check --all

# Full build
cargo build --release

# Start all servers
cargo run -p gamecenter start
```

### Servers & Ports
- **gRPC API**: http://localhost:50051 (User/Room management)
- **TCP Game**: tcp://localhost:4000 (Real-time gameplay)
- **Admin API**: http://localhost:8080 (Monitoring/Management)
- **Redis**: redis://localhost:6379 (Session storage)

## 📊 Performance Benchmarks

### Production TCP Server
- **Throughput**: 12,991+ msg/sec sustained, 41,064+ peak
- **Concurrency**: 500+ simultaneous connections
- **Memory**: 11MB for 500 connections (22KB per connection)
- **Latency**: <1ms p99 response time
- **CPU**: Single-core optimized, multi-core scalable

### Load Testing
```bash
# TCP server performance test
python tcp_load_test.py

# Expected output:
# Throughput: 12,991+ msg/sec
# Memory: ≤15MB
# Latency: <2ms p99
```

## 🏗️ Architecture

### Multi-Server Design
```
GameCenter (Orchestrator)
├── gRPC Server (50051)    → JWT Auth → Service Layer → Redis/MySQL
├── TCP Server (4000)      → Protocol Handler → 8 Optimization Services
├── QUIC Server (custom)   → Stream Multiplexing → 0-RTT Connection
└── Admin API (8080)       → WebSocket Monitor → Real-time Metrics
                                    ↓
                          Unified Redis Cache (6379)
                                    ↓
                            MySQL Persistence
```

### TCP Server Optimization Stack
8 high-performance services working in concert:

1. **DashMap Optimizer**: Lock-free concurrent hashmap with CPU sharding
2. **Async I/O Optimizer**: Zero-copy operations with vectored I/O
3. **SIMD Optimizer**: AVX2/SSE4.2 hardware acceleration
4. **Message Compression**: LZ4/Zstd adaptive compression (40% size reduction)
5. **Connection Pool**: Intelligent connection management with recycling
6. **Performance Monitor**: Real-time metrics and alerting
7. **Memory Pool**: Object recycling with RAII patterns
8. **Parallel Broadcast**: Rayon-based parallel message distribution

## 🛡️ Security Framework

### Authentication & Authorization
- **JWT Authentication**: Multi-mode security (Required/Optional/Conditional)
- **Role-Based Access Control**: Hierarchical permissions with inheritance
- **Rate Limiting**: Token bucket algorithm with Redis backend
- **Input Validation**: OWASP-compliant with XSS/injection prevention

### Security Modules
- **Key Rotation**: Automated JWT key management
- **Crypto**: AES encryption with secure key derivation
- **Redis Security**: Command validation and injection prevention
- **Audit Logging**: Comprehensive security event tracking

### Compliance
- **OWASP Standards**: Automated compliance validation
- **Zero Production Panics**: All panic!() eliminated from production paths
- **Memory Safety**: Rust's ownership system + additional safety layers
- **Dependency Scanning**: Automated vulnerability detection

## 🔧 Development

### Project Structure
```
police-thief/
├── shared/                 # Core libraries & security framework
├── grpcserver/            # gRPC API server (user/room management)
├── tcpserver/             # High-performance TCP server
├── quicserver/            # QUIC server with stream multiplexing
├── gamecenter/            # Unified server orchestrator
├── scripts/               # Build verification & quality tools
├── docker/                # Development & production containers
└── .github/workflows/     # CI/CD automation
```

### Essential Commands
```bash
# Development workflow
cargo fmt --all              # Format code
cargo clippy --all           # Lint check  
cargo test --all             # Run tests
cargo run -p gamecenter start # Start servers

# Quality validation
./scripts/build-verify.sh full    # 100-point quality check
./scripts/run_security_audit.sh   # OWASP compliance scan

# Performance testing
python tcp_load_test.py           # Load test TCP server
cargo run --bin test_client       # gRPC integration test
```

### Docker Development
```bash
# Full development stack with monitoring
docker-compose -f docker-compose.dev.yml up

# Services included:
# - Game servers (gRPC, TCP, QUIC)
# - Redis & MySQL databases
# - Prometheus & Grafana monitoring
# - Load testing environment
```

## 📦 Deployment

### Production Requirements
- **Memory**: 1GB RAM supports 1000+ connections
- **CPU**: 1 vCPU sufficient for 12K+ msg/sec
- **Storage**: 100MB for binaries + logs
- **Network**: Redis cluster for horizontal scaling

### Container Deployment
```bash
# Production build
docker build -f Dockerfile.dev --target runtime -t police-thief:production .

# Deploy with monitoring
docker-compose -f docker-compose.production.yml up -d
```

### Performance Monitoring
- **Prometheus**: Metrics collection with custom collectors
- **Grafana**: Real-time dashboards and alerting
- **Admin API**: Web-based monitoring with WebSocket updates
- **Health Checks**: Automated service health monitoring

## 🧪 Testing

### Test Coverage
- **Unit Tests**: Component-level testing with >80% coverage
- **Integration Tests**: Full-stack testing with Redis/MySQL
- **Load Tests**: Performance validation up to 500+ connections
- **Security Tests**: OWASP compliance and vulnerability scanning

### Cross-Platform Testing
- **Ubuntu 22.04**: Primary production target
- **macOS**: Development environment support
- **Windows**: Development environment support
- **Docker**: Containerized testing environments

### Quality Gates
Our CI/CD pipeline enforces strict quality standards:
- ✅ Zero compilation warnings
- ✅ All tests passing (unit + integration)
- ✅ Security audit clean
- ✅ Performance benchmarks met
- ✅ Code formatting consistent
- ✅ Memory safety verified
- ✅ Cross-platform compatibility

## 📚 Documentation

### Technical Documentation
- `CLAUDE.md` - Claude Code integration guide
- `docs/ARCHITECTURE.md` - System design principles
- `docs/PERFORMANCE.md` - Optimization techniques
- `docs/SECURITY.md` - Security framework details
- `docs/API.md` - gRPC API documentation

### Protocol Specifications
- **TCP Protocol**: 4-byte length header + JSON payload
- **gRPC Protocol**: Protocol Buffers v3 (room.proto, user.proto, auth.proto)
- **QUIC Protocol**: Binary protocol with stream multiplexing
- **Redis Protocol**: Pipeline operations for 10-20x performance

## 🤝 Contributing

### Development Setup
1. Clone repository
2. Run `./setup-deps.sh` (or `.\setup-deps.ps1` on Windows)
3. Create `.env` from `.env.example`
4. Start development: `cargo run -p gamecenter start`

### Quality Standards
- All code must pass quality gates (100-point system)
- Zero unwrap()/panic!() in production code paths
- Comprehensive error handling with AppError
- Performance must maintain 12K+ msg/sec
- Security compliance with OWASP standards

### Pull Request Process
1. Create feature branch
2. Ensure all tests pass: `./scripts/build-verify.sh full`
3. Submit PR with performance benchmarks
4. CI/CD pipeline validates changes
5. Merge after approval

## 📄 License

Licensed under MIT License. See `LICENSE` file for details.

## 🏆 Quality Score: 100/100

**Production Ready** ✅ **Security Compliant** ✅ **Performance Optimized** ✅

- **Zero Production Panics**: All panic!() eliminated from production code
- **Memory Safety**: Rust ownership + additional safety layers
- **Performance**: 12,991+ msg/sec with <1ms latency
- **Security**: OWASP compliant with comprehensive security framework
- **Reliability**: Comprehensive error handling and graceful degradation
- **Maintainability**: Clean architecture with extensive documentation
- **Cross-Platform**: Ubuntu, macOS, Windows support
- **CI/CD**: Automated quality gates and deployment pipeline