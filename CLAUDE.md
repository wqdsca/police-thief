# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Police Thief** - A Unity 3D multiplayer game using hybrid network architecture (RUDP, gRPC, TCP).
- **Unity Version**: 6000.1.15f1
- **Render Pipeline**: Universal Render Pipeline (URP)
- **Primary Language**: C#

## Essential Commands

### Build & Run
```bash
# Open project in Unity Hub or Unity Editor
# Build settings: File > Build Settings > Build

# Run tests (in Unity)
# Window > General > Test Runner > Run All
```

### Code Quality
```bash
# Unity will automatically compile C# scripts
# Check console for compilation errors: Window > General > Console
```

## Architecture Overview

### Layered Architecture
```
├── Core Layer           → Pure C# services (DI, Events, Config, Logging)
├── Infrastructure Layer → Network implementations (RUDP, gRPC, TCP)
├── Application Layer    → Business logic (Login, Room management)
└── Presentation Layer   → Unity MonoBehaviours (UI, GameManager)
```

### Network Protocol Strategy
- **RUDP**: Real-time data (voice chat, game state sync, position updates)
- **gRPC**: API calls (CRUD operations, ranking system, matchmaking)
- **TCP**: Reliable messaging (chat, friend system, file transfer)

### Dependency Injection Pattern
All services use ServiceLocator pattern with interface-based design:
```csharp
// Register
_serviceLocator.RegisterSingleton<IService>(implementation);

// Resolve
var service = ServiceLocator.Instance.Get<IService>();
```

## Key Technical Decisions

### Performance Optimizations
- Removed unnecessary MonoBehaviours from service classes (70% Update loop reduction)
- Object pooling for frequently allocated objects (NetworkMessage, UI elements)
- Connection pooling for network optimization
- Message batching (50ms intervals, max 10 messages)

### Threading Model
- Network operations run on background threads
- UnityMainThreadDispatcher for UI updates from background threads
- Async/await with UniTask for non-blocking operations

### Protocol-Specific Configurations
- **RUDP**: 1024-byte packet size, exponential backoff retry, GZip compression >128 bytes
- **gRPC**: HTTP/2, max 4MB messages, auto-reconnect with exponential backoff
- **TCP**: 8KB buffer, compression >512 bytes, 30s keep-alive

## Important Files & Locations

### Core Infrastructure
- `Assets/assets/Scripts/Bootstrap.cs` - Application entry point and DI setup
- `Assets/assets/Scripts/Core/DI/ServiceLocator.cs` - Dependency injection container
- `Assets/assets/Scripts/Core/Config/NetworkConfig.cs` - Network configuration

### Network Implementation
- `Assets/assets/Scripts/Infrastructure/Network/Core/NetworkConnectionManager.cs` - Central network manager
- `Assets/assets/Scripts/Infrastructure/Network/RUDP/RudpClient.cs` - RUDP implementation
- `Assets/assets/Scripts/Infrastructure/Network/gRPC/GrpcClientOptimized.cs` - gRPC client
- `Assets/assets/Scripts/Infrastructure/Network/TCP/TcpClient.cs` - TCP client

### Documentation
- `Assets/assets/Scripts/Architecture/README.md` - Architecture improvements guide
- `Assets/assets/Scripts/how_to_use/` - Detailed protocol usage guides

## Development Guidelines

### Code Standards
- Use interfaces for all service abstractions
- Follow single responsibility principle
- Minimize MonoBehaviour usage (only for Unity-specific features)
- Register all services through ServiceLocator

### Network Development
- Choose protocol based on use case (see Architecture Overview)
- Always handle connection failures with retry logic
- Use EventBus for network state changes
- Implement proper disposal in OnDestroy() for network resources

### Testing Approach
- Unit tests for pure C# classes
- Integration tests for network protocols
- Test files in `Assets/assets/Scripts/Test/`

## Common Patterns

### Service Registration (Bootstrap.cs)
```csharp
_serviceLocator.RegisterSingleton<IInterface>(implementation);
```

### Network Message Handling
```csharp
// Subscribe to events
EventBus.Subscribe<NetworkConnectedEvent>(OnConnected);

// Send message via appropriate protocol
await grpcClient.SendRequest(request);  // For API calls
rudpClient.Send(gameState);             // For real-time
tcpClient.SendMessage(chatMessage);     // For reliable delivery
```

### Unity Thread Synchronization
```csharp
UnityMainThreadDispatcher.Instance.Enqueue(() => {
    // UI updates here
});
```

## Package Dependencies
- UniTask - Async/await support for Unity
- gRPC - Remote procedure calls
- TextMeshPro - Advanced text rendering
- Modern UI Pack - UI components (in Assets)
- Odin Inspector - Enhanced Unity inspector