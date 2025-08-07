# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based Police Thief game server with a microservice architecture. The project uses a Cargo workspace with three main components:

- **shared**: Common Redis helper libraries and configuration
- **grpcserver**: gRPC API server for room and user management
- **gamecenter**: Game logic server with Redis integration

## Architecture

The project follows a layered architecture:

### Shared Library (`shared/`)
- Redis connection management and configuration
- Helper modules for different Redis data types (hash, list, set, zset, geo, cash)
- Core Redis operations with retry logic
- Used by both grpcserver and gamecenter

### gRPC Server (`grpcserver/`)
- Protocol Buffer definitions in `proto/` (room.proto, user.proto)
- Controller layer handling gRPC requests
- Service layer containing business logic
- Generated Rust code from protobuf using tonic-build
- Serves room management and user authentication

### Game Center (`gamecenter/`)
- Main game server with Redis management
- Automatic Redis server lifecycle management
- Support for multiple run modes (start, stop, test, server, status)
- Background server operation with graceful shutdown

## Development Commands

### Environment Setup
```bash
# Set required environment variables (or use .env file)
export grpc_host=127.0.0.1
export grpc_port=50051
export redis_host=127.0.0.1
export redis_port=6379
```

### Build and Run
```bash
# Build entire workspace
cargo build

# Build specific component
cargo build -p shared
cargo build -p grpcserver
cargo build -p gamecenter

# Run gRPC server
cargo run --bin grpcserver

# Run game center (various modes)
cargo run -p gamecenter start     # Start game center
cargo run -p gamecenter stop      # Stop game center
cargo run -p gamecenter test      # Run tests
cargo run -p gamecenter server    # Background server mode
cargo run -p gamecenter status    # Check status
```

### Testing
```bash
# Run all tests
cargo test

# Run integration tests for gRPC server
cargo test --test integration_test

# Run Redis tests
cargo test -p shared
cargo test -p gamecenter
```

## Key Configuration

- Environment variables are loaded from `.env` file in project root
- gRPC server port configuration via `grpc_host` and `grpc_port`
- Redis connection via `redis_host` and `redis_port`
- Game center includes automatic Redis server management

## Protocol Buffers

gRPC services are defined in `grpcserver/proto/`:
- `room.proto`: Room creation and listing services
- `user.proto`: User authentication and registration services

Build script (`build.rs`) automatically generates Rust code from `.proto` files using tonic-build.

## Redis Integration

The shared library provides comprehensive Redis helpers:
- Connection management with retry logic
- Type-specific helpers (Hash, List, Set, ZSet, Geo, Cash)
- Automatic connection pooling via redis connection manager
- Used extensively by game center for state management

## Development Notes

- The project uses workspace dependencies defined in root `Cargo.toml`
- All components share common dependencies (tokio, redis, anyhow, etc.)
- Extensive logging with tracing and tracing-subscriber
- Game center handles Redis server lifecycle automatically
- Environment configuration supports both `.env` files and environment variables