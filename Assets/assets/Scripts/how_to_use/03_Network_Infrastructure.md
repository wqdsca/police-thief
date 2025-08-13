# Network Infrastructure 상세 가이드

## 개요

Police-Thief 프로젝트의 네트워크 인프라스트럭처는 QUIC, gRPC, TCP 등 다양한 프로토콜을 지원하는 모듈러 아키텍처를 기반으로 구축되었습니다. Unity 환경에서 최적화된 성능과 안정성을 제공하며, 실시간 게임 통신에 특화되어 있습니다.

## 목차
1. [네트워크 아키텍처 개요](#네트워크-아키텍처-개요)
2. [QUIC 프로토콜](#quic-프로토콜)
3. [gRPC 구현](#grpc-구현)
4. [TCP 클라이언트](#tcp-클라이언트)
5. [연결 풀 관리](#연결-풀-관리)
6. [메시지 시스템](#메시지-시스템)
7. [성능 최적화](#성능-최적화)
8. [오류 처리 및 복구](#오류-처리-및-복구)

---

## 네트워크 아키텍처 개요

### 계층 구조

```
INetworkManager (인터페이스)
    ├── NetworkConnectionManager (구현체)
    └── ConnectionPool (연결 풀)
        ├── QuicClientNonMono
        ├── GrpcClientOptimized  
        └── TcpClientOptimized

Protocol Managers:
    ├── QuicProtocolManager
    ├── GrpcProtocolManager
    └── TcpProtocolManager (확장 가능)
```

### 핵심 클래스: INetworkManager

```csharp
public interface INetworkManager
{
    // 연결 관리
    Task<bool> ConnectAsync(NetworkProtocol protocol);
    Task DisconnectAsync(NetworkProtocol protocol);
    Task DisconnectAllAsync();
    
    // 연결 상태
    bool IsConnected(NetworkProtocol protocol);
    ConnectionState GetConnectionState(NetworkProtocol protocol);
    
    // 프로토콜 매니저 관리
    void RegisterProtocolManager(NetworkProtocol protocol, IProtocolManager manager);
    T GetProtocolManager<T>() where T : class;
    
    // 이벤트
    event Action<NetworkProtocol> OnProtocolConnected;
    event Action<NetworkProtocol> OnProtocolDisconnected;
    event Action<NetworkProtocol, string> OnProtocolError;
}
```

### NetworkConnectionManager 구현

```csharp
public class NetworkConnectionManager : INetworkManager
{
    private readonly Dictionary<NetworkProtocol, IProtocolManager> _protocolManagers;
    private readonly object _lock = new object();
    
    public NetworkConnectionManager()
    {
        _protocolManagers = new Dictionary<NetworkProtocol, IProtocolManager>();
        InitializeProtocolManagers();
    }
    
    private void InitializeProtocolManagers()
    {
        try
        {
            // QUIC 매니저 등록
            var networkConfig = ServiceLocator.Instance.Get<NetworkConfig>();
            var quicClient = new QuicClientNonMono(networkConfig);
            var quicManager = new QuicProtocolManager(quicClient, networkConfig);
            RegisterProtocolManager(NetworkProtocol.QUIC, quicManager);
            
            // gRPC 매니저 등록
            var grpcClient = ServiceLocator.Instance.Get<IGrpcClient>();
            var grpcManager = new GrpcProtocolManager(grpcClient);
            RegisterProtocolManager(NetworkProtocol.GRPC, grpcManager);
            
            Log.Info("NetworkConnectionManager initialized successfully", "Network");
        }
        catch (Exception ex)
        {
            Log.Error($"Failed to initialize NetworkConnectionManager: {ex.Message}", "Network");
        }
    }
}
```

---

## QUIC 프로토콜

### 개요
QUIC(Quick UDP Internet Connections)는 UDP 기반의 차세대 전송 프로토콜로, HTTP/3의 기반이 됩니다. 낮은 지연시간과 높은 성능을 제공하며, 실시간 게임 통신에 최적화되어 있습니다.

### 주요 특징
- ✅ **0-RTT 연결**: 세션 재개 시 지연 없는 연결
- ✅ **내장 TLS 1.3**: 자동 암호화 및 보안
- ✅ **연결 마이그레이션**: 네트워크 변경 시 자동 연결 유지
- ✅ **멀티플렉싱**: 여러 스트림을 하나의 연결로 처리
- ✅ **혼잡 제어**: 고급 혼잡 제어 알고리즘 내장

### QuicClient 구현 (Unity 호환)

```csharp
public class QuicClient : MonoBehaviour, IDisposable
{
    #region Events
    public event Action OnConnected;
    public event Action OnDisconnected;
    public event Action<string> OnError;
    public event Action<NetworkMessage> OnMessageReceived;
    #endregion
    
    #region Fields
    private NetworkConfig _config;
    private string _serverUrl;
    private ClientConnectionState _connectionState = ClientConnectionState.Disconnected;
    private CancellationTokenSource _cancellationTokenSource;
    
    // Statistics
    public NetworkStatsOptimized Statistics { get; private set; } = new NetworkStatsOptimized();
    
    // Connection info
    private string _connectionId;
    private string _sessionTicket;
    #endregion
    
    public async Task<bool> ConnectAsync(string serverUrl)
    {
        if (_connectionState != ClientConnectionState.Disconnected)
        {
            Debug.LogWarning("[QuicClient] Already connected or connecting");
            return false;
        }
        
        try
        {
            _connectionState = ClientConnectionState.Connecting;
            _serverUrl = serverUrl;
            _cancellationTokenSource = new CancellationTokenSource();
            
            // Try 0-RTT connection if we have a session ticket
            bool connected = false;
            if (!string.IsNullOrEmpty(_sessionTicket))
            {
                connected = await TryZeroRttConnection();
            }
            
            // Fall back to 1-RTT connection
            if (!connected)
            {
                connected = await PerformHandshake();
            }
            
            if (connected)
            {
                _connectionState = ClientConnectionState.Connected;
                StartBackgroundTasks();
                OnConnected?.Invoke();
                Debug.Log($"[QuicClient] Connected to {serverUrl}");
                return true;
            }
            
            _connectionState = ClientConnectionState.Error;
            return false;
        }
        catch (Exception ex)
        {
            _connectionState = ClientConnectionState.Error;
            OnError?.Invoke($"Connection failed: {ex.Message}");
            Debug.LogError($"[QuicClient] Connection failed: {ex}");
            return false;
        }
    }
}
```

### QuicClientNonMono 래퍼

Unity의 제약사항을 우회하기 위한 Non-MonoBehaviour 래퍼 클래스입니다.

```csharp
public class QuicClientNonMono : IDisposable
{
    private QuicClient _internalClient;
    private GameObject _clientObject;
    
    public event Action OnConnected;
    public event Action OnDisconnected;
    public event Action<string> OnError;
    public event Action<NetworkMessage> OnMessageReceived;
    
    public NetworkStatsOptimized Statistics => _internalClient?.Statistics ?? new NetworkStatsOptimized();
    
    public QuicClientNonMono(NetworkConfig config)
    {
        // Create GameObject to host MonoBehaviour
        _clientObject = new GameObject("QuicClient");
        UnityEngine.Object.DontDestroyOnLoad(_clientObject);
        
        _internalClient = _clientObject.AddComponent<QuicClient>();
        _internalClient.Initialize(config);
        
        // Forward events
        _internalClient.OnConnected += () => OnConnected?.Invoke();
        _internalClient.OnDisconnected += () => OnDisconnected?.Invoke();
        _internalClient.OnError += (error) => OnError?.Invoke(error);
        _internalClient.OnMessageReceived += (msg) => OnMessageReceived?.Invoke(msg);
    }
    
    public Task<bool> ConnectAsync(string serverUrl) => _internalClient.ConnectAsync(serverUrl);
    public Task DisconnectAsync() => _internalClient.DisconnectAsync();
    public Task<bool> SendAsync(NetworkMessage message) => _internalClient.SendAsync(message);
}
```

### 실제 사용 예제

```csharp
public class QuicGameClient : MonoBehaviour
{
    private QuicClientNonMono _quicClient;
    private NetworkConfig _config;
    
    async void Start()
    {
        // 설정 로드
        _config = ServiceLocator.Instance.Get<NetworkConfig>();
        
        // QUIC 클라이언트 생성
        _quicClient = new QuicClientNonMono(_config);
        
        // 이벤트 구독
        _quicClient.OnConnected += OnQuicConnected;
        _quicClient.OnDisconnected += OnQuicDisconnected;
        _quicClient.OnError += OnQuicError;
        _quicClient.OnMessageReceived += OnQuicMessageReceived;
        
        // 서버 연결
        await ConnectToServer();
    }
    
    private async Task ConnectToServer()
    {
        string serverUrl = _config.GetQuicEndpoint();
        bool connected = await _quicClient.ConnectAsync(serverUrl);
        
        if (connected)
        {
            Debug.Log("QUIC 서버 연결 성공!");
            await SendHeartbeat();
        }
        else
        {
            Debug.LogError("QUIC 서버 연결 실패");
        }
    }
    
    private async Task SendHeartbeat()
    {
        var message = new NetworkMessage
        {
            messageType = MessageType.Heartbeat,
            payload = BitConverter.GetBytes(DateTime.UtcNow.Ticks)
        };
        
        await _quicClient.SendAsync(message);
    }
    
    private void OnQuicMessageReceived(NetworkMessage message)
    {
        switch (message.messageType)
        {
            case MessageType.GameData:
                HandleGameData(message);
                break;
            case MessageType.PlayerAction:
                HandlePlayerAction(message);
                break;
            case MessageType.Heartbeat:
                HandleHeartbeat(message);
                break;
        }
    }
}
```

---

## gRPC 구현

### 개요
gRPC는 구글이 개발한 고성능 RPC 프레임워크로, Protocol Buffers를 사용하여 효율적인 통신을 제공합니다. 주로 서버-클라이언트 간 안정적인 통신이 필요한 상황에서 사용됩니다.

### Proto 파일 정의

```protobuf
// user.proto
syntax = "proto3";

package policeThief;

service UserService {
  rpc Login(LoginRequest) returns (LoginResponse);
  rpc Register(RegisterRequest) returns (RegisterResponse);
  rpc GetProfile(GetProfileRequest) returns (GetProfileResponse);
  rpc UpdateProfile(UpdateProfileRequest) returns (UpdateProfileResponse);
}

message LoginRequest {
  string username = 1;
  string password = 2;
  string deviceId = 3;
}

message LoginResponse {
  bool success = 1;
  string message = 2;
  string token = 3;
  UserProfile profile = 4;
}

message UserProfile {
  string userId = 1;
  string username = 2;
  string email = 3;
  int32 level = 4;
  int64 experience = 5;
  repeated string achievements = 6;
}

// room.proto
syntax = "proto3";

package policeThief;

service RoomService {
  rpc CreateRoom(CreateRoomRequest) returns (CreateRoomResponse);
  rpc JoinRoom(JoinRoomRequest) returns (JoinRoomResponse);
  rpc LeaveRoom(LeaveRoomRequest) returns (LeaveRoomResponse);
  rpc GetRoomList(GetRoomListRequest) returns (GetRoomListResponse);
}

message CreateRoomRequest {
  string roomName = 1;
  int32 maxPlayers = 2;
  string gameMode = 3;
  string mapId = 4;
}

message CreateRoomResponse {
  bool success = 1;
  string message = 2;
  RoomInfo room = 3;
}

message RoomInfo {
  string roomId = 1;
  string roomName = 2;
  string hostId = 3;
  int32 currentPlayers = 4;
  int32 maxPlayers = 5;
  string gameMode = 6;
  string mapId = 7;
  repeated PlayerInfo players = 8;
}

message PlayerInfo {
  string playerId = 1;
  string playerName = 2;
  bool isReady = 3;
  string role = 4; // "police" or "thief"
}
```

### IGrpcClient 인터페이스

```csharp
public interface IGrpcClient
{
    bool IsConnected { get; }
    bool IsConnecting { get; }
    
    Task<bool> ConnectAsync();
    Task DisconnectAsync();
    Task<bool> CheckHealthAsync();
    
    // User Services
    Task<LoginResponse> LoginAsync(LoginRequest request);
    Task<RegisterResponse> RegisterAsync(RegisterRequest request);
    Task<GetProfileResponse> GetProfileAsync(GetProfileRequest request);
    Task<UpdateProfileResponse> UpdateProfileAsync(UpdateProfileRequest request);
    
    // Room Services
    Task<CreateRoomResponse> CreateRoomAsync(CreateRoomRequest request);
    Task<JoinRoomResponse> JoinRoomAsync(JoinRoomRequest request);
    Task<LeaveRoomResponse> LeaveRoomAsync(LeaveRoomRequest request);
    Task<GetRoomListResponse> GetRoomListAsync(GetRoomListRequest request);
    
    // Events
    event Action OnConnected;
    event Action OnDisconnected;
    event Action<string> OnError;
    
    // Generic client creation
    T CreateClient<T>() where T : class;
}
```

### GrpcClientOptimized 구현

```csharp
public class GrpcClientOptimized : IGrpcClient, IDisposable
{
    private GrpcChannel _channel;
    private UserService.UserServiceClient _userClient;
    private RoomService.RoomServiceClient _roomClient;
    private readonly NetworkConfig _config;
    private ClientConnectionState _state = ClientConnectionState.Disconnected;
    
    public bool IsConnected => _state == ClientConnectionState.Connected;
    public bool IsConnecting => _state == ClientConnectionState.Connecting;
    
    public event Action OnConnected;
    public event Action OnDisconnected;
    public event Action<string> OnError;
    
    public GrpcClientOptimized(NetworkConfig config)
    {
        _config = config ?? throw new ArgumentNullException(nameof(config));
    }
    
    public async Task<bool> ConnectAsync()
    {
        try
        {
            _state = ClientConnectionState.Connecting;
            
            // gRPC 채널 설정
            var channelOptions = new GrpcChannelOptions
            {
                MaxReceiveMessageSize = 4 * 1024 * 1024, // 4MB
                MaxSendMessageSize = 4 * 1024 * 1024,    // 4MB
                KeepAliveInterval = TimeSpan.FromSeconds(30),
                KeepAliveTimeout = TimeSpan.FromSeconds(5),
            };
            
            _channel = GrpcChannel.ForAddress(_config.grpcEndpoint, channelOptions);
            
            // 서비스 클라이언트 생성
            _userClient = new UserService.UserServiceClient(_channel);
            _roomClient = new RoomService.RoomServiceClient(_channel);
            
            // 연결 테스트
            var healthCheck = await CheckHealthAsync();
            
            if (healthCheck)
            {
                _state = ClientConnectionState.Connected;
                OnConnected?.Invoke();
                Log.Info("gRPC 연결 성공", "Network");
                return true;
            }
            else
            {
                _state = ClientConnectionState.Error;
                OnError?.Invoke("gRPC 연결 실패");
                return false;
            }
        }
        catch (Exception ex)
        {
            _state = ClientConnectionState.Error;
            OnError?.Invoke($"gRPC 연결 오류: {ex.Message}");
            Log.Error($"gRPC 연결 오류: {ex.Message}", "Network");
            return false;
        }
    }
    
    public async Task<LoginResponse> LoginAsync(LoginRequest request)
    {
        try
        {
            if (!IsConnected)
                throw new InvalidOperationException("gRPC client not connected");
            
            var response = await _userClient.LoginAsync(request, deadline: DateTime.UtcNow.AddSeconds(_config.grpcTimeoutMs / 1000));
            Log.Info($"로그인 응답: {response.Success}", "Network");
            return response;
        }
        catch (RpcException ex)
        {
            Log.Error($"gRPC 로그인 오류: {ex.Status}", "Network");
            OnError?.Invoke($"로그인 실패: {ex.Status.Detail}");
            return new LoginResponse { Success = false, Message = ex.Status.Detail };
        }
    }
    
    public async Task<CreateRoomResponse> CreateRoomAsync(CreateRoomRequest request)
    {
        try
        {
            if (!IsConnected)
                throw new InvalidOperationException("gRPC client not connected");
            
            var response = await _roomClient.CreateRoomAsync(request);
            Log.Info($"방 생성 응답: {response.Success}", "Network");
            return response;
        }
        catch (RpcException ex)
        {
            Log.Error($"gRPC 방 생성 오류: {ex.Status}", "Network");
            return new CreateRoomResponse { Success = false, Message = ex.Status.Detail };
        }
    }
    
    public async Task<bool> CheckHealthAsync()
    {
        try
        {
            // 간단한 서버 연결 테스트
            var testRequest = new GetProfileRequest { UserId = "health_check" };
            await _userClient.GetProfileAsync(testRequest, deadline: DateTime.UtcNow.AddSeconds(5));
            return true;
        }
        catch (RpcException ex) when (ex.Status.StatusCode == StatusCode.NotFound)
        {
            // 404는 서버가 응답하는 것이므로 연결은 정상
            return true;
        }
        catch
        {
            return false;
        }
    }
    
    public void Dispose()
    {
        _channel?.Dispose();
        _state = ClientConnectionState.Disconnected;
        OnDisconnected?.Invoke();
    }
}
```

### 실제 사용 예제

```csharp
public class LoginManager : MonoBehaviour
{
    private IGrpcClient _grpcClient;
    
    void Start()
    {
        var config = ServiceLocator.Instance.Get<NetworkConfig>();
        _grpcClient = new GrpcClientOptimized(config);
        
        // 이벤트 구독
        _grpcClient.OnConnected += OnGrpcConnected;
        _grpcClient.OnError += OnGrpcError;
    }
    
    public async Task<bool> LoginAsync(string username, string password)
    {
        try
        {
            // gRPC 연결
            if (!_grpcClient.IsConnected)
            {
                var connected = await _grpcClient.ConnectAsync();
                if (!connected)
                {
                    Debug.LogError("gRPC 서버 연결 실패");
                    return false;
                }
            }
            
            // 로그인 요청
            var request = new LoginRequest
            {
                Username = username,
                Password = password,
                DeviceId = SystemInfo.deviceUniqueIdentifier
            };
            
            var response = await _grpcClient.LoginAsync(request);
            
            if (response.Success)
            {
                Debug.Log($"로그인 성공: {response.Profile.Username}");
                
                // 프로필 정보 저장
                SaveUserProfile(response.Profile);
                
                // 로그인 성공 이벤트 발행
                EventBusOptimized.Instance.Publish(new UserLoggedInEvent
                {
                    UserId = response.Profile.UserId,
                    Username = response.Profile.Username,
                    Token = response.Token
                });
                
                return true;
            }
            else
            {
                Debug.LogWarning($"로그인 실패: {response.Message}");
                return false;
            }
        }
        catch (Exception ex)
        {
            Debug.LogError($"로그인 오류: {ex.Message}");
            return false;
        }
    }
    
    public async Task<string> CreateRoomAsync(string roomName, int maxPlayers)
    {
        try
        {
            var request = new CreateRoomRequest
            {
                RoomName = roomName,
                MaxPlayers = maxPlayers,
                GameMode = "classic",
                MapId = "map_01"
            };
            
            var response = await _grpcClient.CreateRoomAsync(request);
            
            if (response.Success)
            {
                Debug.Log($"방 생성 성공: {response.Room.RoomId}");
                return response.Room.RoomId;
            }
            else
            {
                Debug.LogWarning($"방 생성 실패: {response.Message}");
                return null;
            }
        }
        catch (Exception ex)
        {
            Debug.LogError($"방 생성 오류: {ex.Message}");
            return null;
        }
    }
}
```

---

## TCP 클라이언트

### 개요
TCP는 신뢰성 있는 연결 지향 프로토콜로, 데이터의 정확한 전송이 보장되어야 하는 상황에서 사용됩니다. 친구 시스템, 채팅, 파일 전송 등에 활용됩니다.

### TcpClient 구현

```csharp
public class TcpClientOptimized : IDisposable
{
    private TcpClient _tcpClient;
    private NetworkStream _stream;
    private readonly NetworkConfig _config;
    private ClientConnectionState _state = ClientConnectionState.Disconnected;
    private CancellationTokenSource _cancellationTokenSource;
    
    // 버퍼링
    private readonly byte[] _receiveBuffer;
    private readonly MemoryStream _messageBuffer;
    
    public ClientConnectionState State => _state;
    public bool IsConnected => _state == ClientConnectionState.Connected;
    
    public event Action OnConnected;
    public event Action OnDisconnected;
    public event Action<string> OnError;
    public event Action<byte[]> OnDataReceived;
    
    public TcpClientOptimized(NetworkConfig config)
    {
        _config = config ?? throw new ArgumentNullException(nameof(config));
        _receiveBuffer = new byte[config.tcpBufferSize];
        _messageBuffer = new MemoryStream();
    }
    
    public async Task<bool> ConnectAsync()
    {
        try
        {
            _state = ClientConnectionState.Connecting;
            _cancellationTokenSource = new CancellationTokenSource();
            
            _tcpClient = new TcpClient();
            
            // 타임아웃 설정
            var connectTask = _tcpClient.ConnectAsync(_config.tcpHost, _config.tcpPort);
            var timeoutTask = Task.Delay(_config.connectTimeoutMs, _cancellationTokenSource.Token);
            
            var completedTask = await Task.WhenAny(connectTask, timeoutTask);
            
            if (completedTask == timeoutTask)
            {
                _state = ClientConnectionState.Error;
                OnError?.Invoke("연결 타임아웃");
                return false;
            }
            
            if (_tcpClient.Connected)
            {
                _stream = _tcpClient.GetStream();
                _state = ClientConnectionState.Connected;
                
                // 수신 루프 시작
                _ = Task.Run(ReceiveLoop, _cancellationTokenSource.Token);
                
                OnConnected?.Invoke();
                Log.Info($"TCP 연결 성공: {_config.tcpHost}:{_config.tcpPort}", "Network");
                return true;
            }
            
            _state = ClientConnectionState.Error;
            return false;
        }
        catch (Exception ex)
        {
            _state = ClientConnectionState.Error;
            OnError?.Invoke($"연결 실패: {ex.Message}");
            Log.Error($"TCP 연결 실패: {ex.Message}", "Network");
            return false;
        }
    }
    
    public async Task<bool> SendAsync(byte[] data)
    {
        if (!IsConnected || data == null || data.Length == 0)
            return false;
        
        try
        {
            // 메시지 길이를 헤더로 전송 (4바이트)
            var lengthBytes = BitConverter.GetBytes(data.Length);
            await _stream.WriteAsync(lengthBytes, 0, 4);
            
            // 실제 데이터 전송
            await _stream.WriteAsync(data, 0, data.Length);
            await _stream.FlushAsync();
            
            return true;
        }
        catch (Exception ex)
        {
            OnError?.Invoke($"전송 실패: {ex.Message}");
            Log.Error($"TCP 전송 실패: {ex.Message}", "Network");
            return false;
        }
    }
    
    private async Task ReceiveLoop()
    {
        try
        {
            while (_state == ClientConnectionState.Connected && !_cancellationTokenSource.Token.IsCancellationRequested)
            {
                // 메시지 길이 읽기 (4바이트)
                var lengthBytes = new byte[4];
                var bytesRead = await ReadExactAsync(lengthBytes, 4);
                
                if (bytesRead != 4)
                    break;
                
                var messageLength = BitConverter.ToInt32(lengthBytes, 0);
                
                if (messageLength <= 0 || messageLength > 1024 * 1024) // 1MB 제한
                {
                    OnError?.Invoke("잘못된 메시지 크기");
                    break;
                }
                
                // 메시지 데이터 읽기
                var messageData = new byte[messageLength];
                bytesRead = await ReadExactAsync(messageData, messageLength);
                
                if (bytesRead == messageLength)
                {
                    OnDataReceived?.Invoke(messageData);
                }
            }
        }
        catch (Exception ex) when (!_cancellationTokenSource.Token.IsCancellationRequested)
        {
            OnError?.Invoke($"수신 오류: {ex.Message}");
            Log.Error($"TCP 수신 오류: {ex.Message}", "Network");
        }
        finally
        {
            await DisconnectAsync();
        }
    }
    
    private async Task<int> ReadExactAsync(byte[] buffer, int count)
    {
        int totalRead = 0;
        
        while (totalRead < count && _state == ClientConnectionState.Connected)
        {
            var read = await _stream.ReadAsync(buffer, totalRead, count - totalRead, _cancellationTokenSource.Token);
            
            if (read == 0)
                break; // 연결 종료
            
            totalRead += read;
        }
        
        return totalRead;
    }
    
    public async Task DisconnectAsync()
    {
        if (_state == ClientConnectionState.Disconnected)
            return;
        
        _state = ClientConnectionState.Disconnecting;
        
        try
        {
            _cancellationTokenSource?.Cancel();
            _stream?.Close();
            _tcpClient?.Close();
        }
        catch (Exception ex)
        {
            Log.Warning($"TCP 연결 해제 중 오류: {ex.Message}", "Network");
        }
        finally
        {
            _state = ClientConnectionState.Disconnected;
            OnDisconnected?.Invoke();
        }
    }
    
    public void Dispose()
    {
        _ = DisconnectAsync();
        _messageBuffer?.Dispose();
        _cancellationTokenSource?.Dispose();
    }
}
```

---

## 연결 풀 관리

### 개요
ConnectionPool은 네트워크 연결을 효율적으로 관리하기 위한 시스템입니다. 연결 생성 비용을 줄이고, 리소스를 재사용하여 성능을 향상시킵니다.

### ConnectionPool 구현

```csharp
public class ConnectionPool : IDisposable
{
    private readonly NetworkConfig _config;
    private readonly ConcurrentQueue<TcpClientOptimized> _tcpPool = new();
    private readonly ConcurrentQueue<QuicClientNonMono> _quicPool = new();
    private readonly object _poolLock = new object();
    
    private volatile int _tcpPoolSize = 0;
    private volatile int _quicPoolSize = 0;
    
    private const int MAX_POOL_SIZE = 10;
    private const int INITIAL_POOL_SIZE = 3;
    
    public ConnectionPool(NetworkConfig config)
    {
        _config = config ?? throw new ArgumentNullException(nameof(config));
        InitializePools();
    }
    
    private void InitializePools()
    {
        // TCP 풀 초기화
        for (int i = 0; i < INITIAL_POOL_SIZE; i++)
        {
            var tcpClient = new TcpClientOptimized(_config);
            _tcpPool.Enqueue(tcpClient);
            _tcpPoolSize++;
        }
        
        Log.Info($"ConnectionPool initialized with {INITIAL_POOL_SIZE} TCP connections", "Network");
    }
    
    public async Task<TcpClientOptimized> GetTcpClientAsync()
    {
        TcpClientOptimized client = null;
        
        // 풀에서 사용 가능한 연결 가져오기
        if (_tcpPool.TryDequeue(out client))
        {
            lock (_poolLock) _tcpPoolSize--;
            
            // 연결 상태 확인
            if (client.State == ClientConnectionState.Connected)
            {
                return client;
            }
            else
            {
                client?.Dispose();
                lock (_poolLock) _tcpPoolSize--;
            }
        }
        
        // 새로운 클라이언트 생성
        client = new TcpClientOptimized(_config);
        var connected = await client.ConnectAsync();
        
        if (connected)
        {
            Log.Info("새로운 TCP 클라이언트가 생성되고 연결되었습니다", "ConnectionPool");
            return client;
        }
        
        client.Dispose();
        return null;
    }
    
    public async Task<QuicClientNonMono> GetQuicClientAsync()
    {
        QuicClientNonMono client = null;
        
        if (_quicPool.TryDequeue(out client))
        {
            lock (_poolLock) _quicPoolSize--;
            
            // QUIC doesn't have State property in same way, check if it's not null
            // and rely on connection test
            if (client != null)
            {
                return client;
            }
            else
            {
                client?.Dispose();
                lock (_poolLock) _quicPoolSize--;
            }
        }
        
        // 새로운 클라이언트 생성
        client = new QuicClientNonMono(_config);
        var connected = await client.ConnectAsync(_config.GetQuicEndpoint());
        
        if (connected)
        {
            Log.Info("새로운 QUIC 클라이언트가 생성되고 연결되었습니다", "ConnectionPool");
            return client;
        }
        
        client.Dispose();
        return null;
    }
    
    public void ReturnTcpClient(TcpClientOptimized client)
    {
        if (client?.State == ClientConnectionState.Connected)
        {
            lock (_poolLock)
            {
                if (_tcpPoolSize < MAX_POOL_SIZE)
                {
                    _tcpPool.Enqueue(client);
                    _tcpPoolSize++;
                    return;
                }
            }
        }
        
        // 풀이 가득 찼거나 연결이 끊어진 경우 해제
        client?.Dispose();
    }
    
    public void ReturnQuicClient(QuicClientNonMono client)
    {
        if (client != null)
        {
            lock (_poolLock)
            {
                if (_quicPoolSize < MAX_POOL_SIZE)
                {
                    _quicPool.Enqueue(client);
                    _quicPoolSize++;
                    return;
                }
            }
        }
        
        client?.Dispose();
    }
    
    public void Dispose()
    {
        // TCP 풀 정리
        while (_tcpPool.TryDequeue(out var tcpClient))
        {
            tcpClient.Dispose();
        }
        
        // QUIC 풀 정리
        while (_quicPool.TryDequeue(out var quicClient))
        {
            quicClient.Dispose();
        }
        
        Log.Info("ConnectionPool disposed", "Network");
    }
}
```

---

## 메시지 시스템

### 네트워크 메시지 구조

```csharp
public enum MessageType
{
    Connect = 0,
    Disconnect = 1,
    Heartbeat = 2,
    GameData = 3,
    PlayerAction = 4,
    RoomUpdate = 5,
    Chat = 6,
    Error = 99
}

[Serializable]
public struct NetworkMessage
{
    public uint messageId;
    public MessageType messageType;
    public uint sequenceNumber;
    public DateTime timestamp;
    public byte[] payload;
}
```

### 최적화된 메시지 시스템

```csharp
// Zero-allocation을 위한 최적화된 메시지 구조체
[StructLayout(LayoutKind.Sequential)]
public struct NetworkMessageOptimized : IPoolable
{
    public uint messageId;
    public MessageType messageType;
    public uint sequenceNumber;
    public long timestampTicks; // Using ticks instead of DateTime to avoid allocation
    public ArraySegment<byte> payload; // Zero-copy payload reference
    
    public DateTime Timestamp => new DateTime(timestampTicks, DateTimeKind.Utc);
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public void Initialize(MessageType type, ArraySegment<byte> data, uint sequence = 0)
    {
        messageId = GenerateMessageId();
        messageType = type;
        sequenceNumber = sequence;
        timestampTicks = DateTime.UtcNow.Ticks;
        payload = data;
    }
    
    public void Reset()
    {
        messageId = 0;
        messageType = MessageType.Connect;
        sequenceNumber = 0;
        timestampTicks = 0;
        payload = default;
    }
}
```

### 메시지 배칭 시스템

```csharp
public sealed class NetworkMessageBatch : IDisposable
{
    private readonly PooledNetworkMessage[] _messages;
    private int _count;
    private readonly int _capacity;
    
    public int Count => _count;
    public bool IsFull => _count >= _capacity;
    
    public NetworkMessageBatch(int capacity = 10)
    {
        _capacity = capacity;
        _messages = new PooledNetworkMessage[capacity];
    }
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public bool TryAdd(MessageType type, byte[] data, int offset, int count)
    {
        if (IsFull) return false;
        
        var message = PooledNetworkMessage.Get(type, data, offset, count);
        _messages[_count++] = message;
        return true;
    }
    
    public void Clear()
    {
        for (int i = 0; i < _count; i++)
        {
            _messages[i]?.Dispose();
            _messages[i] = null;
        }
        _count = 0;
    }
    
    public void Dispose()
    {
        Clear();
    }
}
```

---

## 성능 최적화

### 1. 네트워크 통계 모니터링

```csharp
[StructLayout(LayoutKind.Sequential)]
public struct NetworkStatsOptimized
{
    public int totalMessagesSent;
    public int totalMessagesReceived;
    public float averageLatency;
    public float packetLossRate;
    public long totalBytesSent;
    public long totalBytesReceived;
    
    public void IncrementSent(int bytes = 0)
    {
        totalMessagesSent++;
        totalBytesSent += bytes;
        UpdateActivity();
    }
    
    public void IncrementReceived(int bytes = 0)
    {
        totalMessagesReceived++;
        totalBytesReceived += bytes;
        UpdateActivity();
    }
    
    public void UpdateLatency(float newLatency)
    {
        averageLatency = (averageLatency * 0.9f) + (newLatency * 0.1f);
    }
}
```

### 2. 메시지 압축

```csharp
public class MessageCompressor
{
    public static byte[] Compress(byte[] data)
    {
        if (data.Length < 100) // 작은 메시지는 압축하지 않음
            return data;
        
        using (var output = new MemoryStream())
        {
            using (var gzip = new GZipStream(output, CompressionMode.Compress, true))
            {
                gzip.Write(data, 0, data.Length);
            }
            return output.ToArray();
        }
    }
    
    public static byte[] Decompress(byte[] data)
    {
        using (var input = new MemoryStream(data))
        using (var gzip = new GZipStream(input, CompressionMode.Decompress))
        using (var output = new MemoryStream())
        {
            gzip.CopyTo(output);
            return output.ToArray();
        }
    }
}
```

### 3. 연결 최적화

```csharp
public class NetworkOptimizer
{
    public static void OptimizeForMobile(NetworkConfig config)
    {
        // 모바일 환경에 최적화된 설정
        config.connectTimeoutMs = 8000;        // 8초
        config.keepAliveIntervalMs = 60000;    // 60초
        config.tcpBufferSize = 4096;           // 4KB
    }
    
    public static void OptimizeForDesktop(NetworkConfig config)
    {
        // 데스크톱 환경에 최적화된 설정
        config.connectTimeoutMs = 5000;        // 5초
        config.keepAliveIntervalMs = 30000;    // 30초
        config.tcpBufferSize = 8192;           // 8KB
    }
}
```

---

## 오류 처리 및 복구

### 자동 재연결 시스템

```csharp
public class AutoReconnectManager
{
    private readonly INetworkManager _networkManager;
    private readonly Dictionary<NetworkProtocol, ReconnectInfo> _reconnectInfo;
    
    public AutoReconnectManager(INetworkManager networkManager)
    {
        _networkManager = networkManager;
        _reconnectInfo = new Dictionary<NetworkProtocol, ReconnectInfo>();
        
        // 네트워크 이벤트 구독
        _networkManager.OnProtocolDisconnected += OnProtocolDisconnected;
        _networkManager.OnProtocolError += OnProtocolError;
    }
    
    private async void OnProtocolDisconnected(NetworkProtocol protocol)
    {
        Log.Warning($"Protocol {protocol} disconnected, attempting reconnection", "Network");
        await AttemptReconnect(protocol);
    }
    
    private async void OnProtocolError(NetworkProtocol protocol, string error)
    {
        Log.Error($"Protocol {protocol} error: {error}", "Network");
        await AttemptReconnect(protocol);
    }
    
    private async Task AttemptReconnect(NetworkProtocol protocol)
    {
        if (!_reconnectInfo.ContainsKey(protocol))
        {
            _reconnectInfo[protocol] = new ReconnectInfo();
        }
        
        var info = _reconnectInfo[protocol];
        
        if (info.AttemptCount >= MAX_RECONNECT_ATTEMPTS)
        {
            Log.Error($"Max reconnection attempts reached for {protocol}", "Network");
            return;
        }
        
        info.AttemptCount++;
        var delay = TimeSpan.FromSeconds(Math.Min(Math.Pow(2, info.AttemptCount), MAX_RECONNECT_DELAY));
        
        Log.Info($"Reconnection attempt {info.AttemptCount} for {protocol} in {delay.TotalSeconds}s", "Network");
        
        await Task.Delay(delay);
        
        var success = await _networkManager.ConnectAsync(protocol);
        
        if (success)
        {
            Log.Info($"Reconnection successful for {protocol}", "Network");
            info.Reset();
        }
        else
        {
            await AttemptReconnect(protocol);
        }
    }
    
    private class ReconnectInfo
    {
        public int AttemptCount { get; set; } = 0;
        public DateTime LastAttempt { get; set; } = DateTime.MinValue;
        
        public void Reset()
        {
            AttemptCount = 0;
            LastAttempt = DateTime.MinValue;
        }
    }
}
```

## 통합 사용 예제

모든 네트워크 컴포넌트를 함께 활용하는 예제입니다.

```csharp
public class PoliceThiefNetworkManager : MonoBehaviour
{
    private INetworkManager _networkManager;
    private ConnectionPool _connectionPool;
    private AutoReconnectManager _reconnectManager;
    
    async void Start()
    {
        await InitializeNetworkSystems();
        await ConnectToGameServer();
    }
    
    private async Task InitializeNetworkSystems()
    {
        // 네트워크 설정 로드
        var config = ServiceLocator.Instance.Get<NetworkConfig>();
        
        // 연결 풀 생성
        _connectionPool = new ConnectionPool(config);
        ServiceLocator.Instance.Register<ConnectionPool>(_connectionPool);
        
        // 네트워크 매니저 생성
        _networkManager = new NetworkConnectionManager();
        ServiceLocator.Instance.Register<INetworkManager>(_networkManager);
        
        // 자동 재연결 매니저 설정
        _reconnectManager = new AutoReconnectManager(_networkManager);
        
        // 이벤트 구독
        _networkManager.OnProtocolConnected += OnProtocolConnected;
        _networkManager.OnProtocolError += OnProtocolError;
        
        Log.Info("Network systems initialized", "Network");
    }
    
    private async Task ConnectToGameServer()
    {
        try
        {
            // QUIC 연결 (실시간 게임 데이터)
            var quicConnected = await _networkManager.ConnectAsync(NetworkProtocol.QUIC);
            if (quicConnected)
            {
                Log.Info("QUIC 연결 성공", "Network");
            }
            
            // gRPC 연결 (서버 통신)
            var grpcConnected = await _networkManager.ConnectAsync(NetworkProtocol.GRPC);
            if (grpcConnected)
            {
                Log.Info("gRPC 연결 성공", "Network");
            }
            
            if (quicConnected && grpcConnected)
            {
                // 게임 준비 완료
                EventBusOptimized.Instance.Publish(new GameReadyEvent());
            }
        }
        catch (Exception ex)
        {
            Log.Error($"서버 연결 실패: {ex.Message}", "Network");
        }
    }
    
    private void OnProtocolConnected(NetworkProtocol protocol)
    {
        Log.Info($"Protocol {protocol} 연결됨", "Network");
        
        switch (protocol)
        {
            case NetworkProtocol.QUIC:
                StartGameDataStreaming();
                break;
            case NetworkProtocol.GRPC:
                StartServerCommunication();
                break;
        }
    }
    
    private async void StartGameDataStreaming()
    {
        // QUIC를 통한 실시간 게임 데이터 스트리밍
        var quicManager = _networkManager.GetProtocolManager<QuicProtocolManager>();
        var quicClient = quicManager.GetClient();
        
        // 하트비트 전송
        _ = Task.Run(async () =>
        {
            while (_networkManager.IsConnected(NetworkProtocol.QUIC))
            {
                var heartbeat = new NetworkMessage
                {
                    messageType = MessageType.Heartbeat,
                    payload = BitConverter.GetBytes(DateTime.UtcNow.Ticks)
                };
                
                await quicClient.SendAsync(heartbeat);
                await Task.Delay(30000); // 30초마다
            }
        });
    }
    
    private void StartServerCommunication()
    {
        // gRPC를 통한 서버 통신 준비 완료
        var grpcManager = _networkManager.GetProtocolManager<GrpcProtocolManager>();
        
        // 서버와 통신 가능한 상태
        Log.Info("서버 통신 준비 완료", "Network");
    }
}
```

## 다음 단계

Network Infrastructure를 마스터했다면, 다음 문서들을 참조하세요:

1. [Game Logic](./04_Game_Logic.md) - 게임 로직 구현
2. [Performance Optimization](./05_Performance_Optimization.md) - 성능 최적화  
3. [Extension Guide](./06_Extension_Guide.md) - 확장 방안