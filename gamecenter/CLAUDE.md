# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**GameCenter** - A unified game server orchestrator for the Police Thief multiplayer game. This is the main entry point that manages and coordinates two server implementations (gRPC and TCP) with Redis lifecycle management and an admin API.

## Essential Commands

### Build Commands
```bash
# Full build (requires protoc installed)
cargo build --release -p gamecenter

# Check compilation without building
cargo check -p gamecenter

# Format code
cargo fmt --all

# Run linter
cargo clippy -p gamecenter -- -W clippy::all
```

### Run Commands
```bash
# Start unified server with Redis lifecycle management
cargo run -p gamecenter start    # Default: starts all servers + Redis

# Individual server modes
cargo run -p gamecenter grpc     # gRPC server only (port 50051)
cargo run -p gamecenter tcp      # TCP server only (port 4000)

# Other modes
cargo run -p gamecenter server   # Background server mode
cargo run -p gamecenter test     # Run integration tests
cargo run -p gamecenter status   # Check server status
cargo run -p gamecenter stop     # Stop running servers
```

### Test Commands
```bash
# Run unit tests
cargo test -p gamecenter --lib

# Run specific test
cargo test -p gamecenter test_config_validation

# Run integration tests with output
cargo test -p gamecenter -- --nocapture
```

### Database Setup
```bash
# Setup database (requires MySQL/MariaDB running)
./setup_database.sh

# Manual database creation
mysql -u root -p
CREATE DATABASE police_thief_simple;
```

## High-Level Architecture

### Server Coordination Flow
```
GameCenter Main Entry
    ├── Redis Lifecycle Manager (starts/stops Redis)
    ├── UnifiedGameServer
    │   ├── gRPC Server (port 50051) - User/Room management APIs
    │   ├── TCP Server (port 4000) - Game protocol, production-ready
    │   └── Admin API (port 8080) - Web dashboard + WebSocket monitoring
    └── Performance Monitor (30s interval metrics)
```

### Key Components

**`src/main.rs`**: Main entry point with command-line interface
- `run_gamecenter()`: Starts all servers with Redis management
- `run_individual_server()`: Runs specific server type
- `GameCenterServer`: Manages Redis process lifecycle

**`src/unified_server.rs`**: Core orchestration logic
- `UnifiedGameServer`: Coordinates multiple server instances
- `UnifiedServerConfig`: Configuration with environment variable support
- Manages server lifecycle (start/stop/status)

**`src/admin_api.rs`**: Admin dashboard API
- RESTful endpoints for server management
- Real-time WebSocket updates
- System metrics monitoring

**`src/auth_middleware.rs`**: JWT authentication
- Token validation middleware
- Protected endpoint management

**`src/social_auth_handler.rs`**: OAuth 2.0 integration
- Google, Apple, Kakao, Naver login support
- State management for OAuth flows

### Protocol Buffer Compilation
The project uses gRPC with Protocol Buffers. The `build.rs` automatically compiles:
- `proto/auth.proto` → Authentication service definitions

### Environment Configuration
Required environment variables (create `.env` from `.env.example`):
```
# Server addresses
grpc_host=127.0.0.1
grpc_port=50051
tcp_host=127.0.0.1
tcp_port=4000
udp_host=127.0.0.1
udp_port=5000

# Redis (managed by GameCenter)
redis_host=127.0.0.1
redis_port=6379

# Database
DATABASE_URL=mysql://user:pass@localhost/police_thief_simple

# Feature flags
ENABLE_GRPC=true
ENABLE_TCP=true
ENABLE_MONITORING=true
```

### Server Integration Pattern
GameCenter integrates with two server crates from the parent workspace:
- **grpcserver**: Imported and started via `grpcserver::server::start_server()`
- **tcpserver**: Uses `ConnectionService`, `HeartbeatService`, `MessageService`

Each server runs in its own async task, managed by `UnifiedGameServer::server_handles`.

### Admin API Architecture
The Admin API (`port 8080`) provides:
- `/api/admin/*` - Protected endpoints (JWT required)
- `/ws` - WebSocket for real-time updates
- `/api/auth/social/*` - OAuth 2.0 endpoints
- Broadcasts server metrics every 5 seconds via WebSocket

### Testing Strategy
- Unit tests in `src/unified_server.rs` (bottom of file)
- Integration tests in `src/tests.rs`
- Social auth tests in `tests/` directory
- Mock implementations for testing without external dependencies