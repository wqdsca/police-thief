# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based Police Thief game server with a microservice architecture. The project uses a Cargo workspace with four main components:

- **shared**: Common Redis/MariaDB helper libraries and configuration
- **grpcserver**: gRPC API server for room and user management  
- **gamecenter**: Game logic server with Redis integration
- **tcpserver**: Real-time TCP server for game communication

## Architecture

The project follows a layered architecture with significant data persistence capabilities:

### Shared Library (`shared/`)
- **Database Integration**: Both Redis (caching/sessions) and MariaDB (persistent storage)
- **Redis Operations**: Comprehensive helpers for Hash, List, Set, ZSet, Geo operations with retry logic
- **Database Config**: SQLx-based MariaDB connection pooling with error handling
- **Error Management**: Structured AppError types with severity levels and gRPC Status conversion
- **Tool Utilities**: ID generation with Redis-based recycling, hex utilities, time management
- **Model Definitions**: RoomInfo and UserInfo data structures

### gRPC Server (`grpcserver/`)
- **Protocol Buffers**: room.proto and user.proto definitions (requires protoc installation)
- **Controllers**: Request handling with JWT authentication (3 auth modes)
- **Services**: Business logic with Redis/DB integration
- **Test Client**: Comprehensive integration tests with Redis validation
- **Authentication**: Bearer token JWT with configurable modes (required/optional/conditional)

### Game Center (`gamecenter/`)
- Main game server with automatic Redis lifecycle management
- Multiple run modes with graceful shutdown
- Background server operations

### TCP Server (`tcpserver/`)
- Real-time binary protocol (4-byte length header + JSON)
- Heartbeat system (10s intervals, 30s timeout)
- **Status**: Recently fixed compilation issues, now functional

## Critical Dependencies

### External Tools Required
- **protoc** (Protocol Buffer Compiler): Essential for gRPC server compilation
  - Install from: https://github.com/protocolbuffers/protobuf/releases
  - Or via package manager: `brew install protobuf` / `apt-get install protobuf-compiler`
- **Redis Server**: Must be running for cache operations
- **MariaDB Server**: Required for persistent data storage

### Environment Configuration
All services use `.env` file in project root:
```bash
# Redis Configuration
redis_host=127.0.0.1
redis_port=6379

# gRPC Server
grpc_host=127.0.0.1
grpc_port=50051

# TCP Server  
tcp_host=127.0.0.1
tcp_port=4000

# MariaDB Database
db_host=localhost
db_port=3306
db_id=root
db_password=your_password
db_name=police

# JWT Authentication
JWT_SECRET_KEY=your_secret_key
JWT_ALGORITHM=HS256
```

## Development Commands

### Build and Run
```bash
# Build entire workspace (check protoc installation first)
cargo build

# Build specific components
cargo build -p shared               # Always builds successfully
cargo build -p grpcserver          # Requires protoc
cargo build -p gamecenter
cargo build -p tcpserver

# Run services
cargo run --bin grpcserver          # gRPC API server
cargo run --bin tcpserver           # Real-time TCP server

# Game center operations
cargo run -p gamecenter start       # Start with Redis lifecycle
cargo run -p gamecenter server      # Background mode
cargo run -p gamecenter stop        # Graceful shutdown
cargo run -p gamecenter status      # Health check
```

### Code Quality Commands  
```bash
# Format code
cargo fmt                           # Format entire workspace
cargo fmt -p shared                 # Format specific package

# Lint code with Clippy
cargo clippy                        # Lint entire workspace
cargo clippy -p grpcserver          # Lint specific package
cargo clippy -- -W clippy::all      # Run with all warnings

# Check compilation without building
cargo check                         # Fast compilation check
cargo check -p tcpserver            # Check specific package
```

### Testing Strategy
```bash
# Unit tests by component
cargo test -p shared                # Redis/DB helpers, utilities
cargo test -p tcpserver --lib       # TCP protocol tests
cargo test -p gamecenter           # Game logic tests

# Run specific test
cargo test -p tcpserver test_protocol::test_decode_message  # Single test function
cargo test -p shared redis_test     # Test module

# Integration testing
cargo run --bin test_client         # Comprehensive gRPC + Redis validation
                                   # Tests user/room operations, pagination, Redis data

# TCP Server test runner (Linux/Mac)
cd tcpserver && ./test_runner.sh   # Interactive test menu for TCP components

# Manual Redis verification during testing
redis-cli KEYS "*"                  # View all keys
redis-cli HGETALL "user:1"         # Check user data
redis-cli ZREVRANGE "room:list:time:index" 0 -1 WITHSCORES  # Room time ordering
```

### Database Operations
```bash
# Verify connections
cargo run --bin grpcserver          # Auto-connects to Redis + MariaDB
redis-cli ping                      # Test Redis connectivity
mysql -u root -p police            # Test MariaDB connectivity
```

## Data Architecture

### Redis Key Patterns
- **Users**: `user:{user_id}` (Hash) - Session data, TTL 3600s
- **Rooms**: `room:info:{room_id}` (Hash) - Room metadata, TTL 3600s  
- **Room Ordering**: `room:list:time:index` (ZSet) - Time-based room listing
- **ID Management**: `room_counter:id` (String), `recycle_room_id:index` (List)

### MariaDB Schema
- Persistent user profiles, room history, game statistics
- Connection pooling via SQLx with transaction support
- Error mapping from SQLx to structured AppError types

### Data Flow Pattern
1. **Create Operations**: Write to MariaDB → Cache in Redis → Update indices
2. **Read Operations**: Check Redis cache → Fallback to MariaDB if needed
3. **List Operations**: Use Redis ZSet for performance → Batch fetch details

## Performance Characteristics

### Redis Optimizations
- **Pipeline Operations**: Batch commands for room creation/listing (10-20x faster)
- **Connection Pooling**: Shared connection manager across services
- **Retry Logic**: Automatic retry with exponential backoff
- **Time-based Pagination**: ZSet scores for efficient last_id pagination

### Room Listing Performance
- **20-item limit** with time-based sorting (newest first)
- **Single Redis call** for list + batch detail fetch via pipeline
- **Pagination**: Uses last_id timestamp for cursor-based paging

## Authentication & Security

### JWT Implementation
- **Required Auth**: All endpoints require valid Bearer token
- **Optional Auth**: Token validated if present, anonymous allowed
- **Conditional Auth**: Per-endpoint authentication requirements
- **Token Format**: `Authorization: Bearer <jwt_token>`

### Error Management
- **Structured Types**: AppError enum with severity classification
- **gRPC Integration**: Automatic conversion to appropriate Status codes
- **Logging**: Tracing with context-aware error reporting

## Known Issues & Troubleshooting

### Common Problems
1. **protoc not found**: Install Protocol Buffer Compiler before building grpcserver
2. **Redis connection failed**: Ensure Redis server is running on configured port
3. **Database connection timeout**: Check MariaDB credentials in .env file
4. **JWT validation failed**: Verify JWT_SECRET_KEY matches between services

### Debugging Commands
```bash
# Check service health
redis-cli ping                      # Redis connectivity
cargo run --bin grpcserver --help   # gRPC server status
cargo check --bin test_client       # Test compilation

# Monitor Redis during testing  
redis-cli MONITOR                   # Watch all Redis commands
redis-cli --latency                # Connection latency
```

## Development Workflow

### New Feature Development
1. **Database Changes**: Update shared/src/model/ and migrations if needed
2. **Service Layer**: Implement business logic in shared/src/service/
3. **API Layer**: Add gRPC endpoints in grpcserver/src/controller/
4. **Testing**: Add comprehensive tests in test_client.rs
5. **Validation**: Run integration tests to verify Redis/DB consistency

### Performance Testing
- Use test_client.rs for load testing room creation/listing
- Monitor Redis memory usage during high-volume operations
- Verify connection pooling efficiency under concurrent load

## Important Development Notes

- **Workspace Dependencies**: All versions centrally managed in root Cargo.toml
- **Error Handling**: Always use structured AppError types, never unwrap() in production code  
- **Database Transactions**: Use helpers::with_transaction() for multi-operation consistency
- **Redis Operations**: Prefer pipeline operations for multiple commands
- **Testing**: Always verify both Redis cache and MariaDB persistence in tests
- **Code Quality**: Run `cargo fmt` and `cargo clippy` before committing code
- **Proto Changes**: After modifying `.proto` files, rebuild with `cargo build -p grpcserver` to regenerate code