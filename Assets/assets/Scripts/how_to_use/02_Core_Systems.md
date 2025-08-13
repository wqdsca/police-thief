# Core Systems 상세 가이드

## 개요

Police-Thief 프로젝트의 Core 계층은 모든 상위 시스템에서 활용하는 핵심 인프라스트럭처를 제공합니다. 각 시스템은 단일 책임을 가지며, 높은 재사용성과 확장성을 제공합니다.

## 목차
1. [Config 시스템](#config-시스템)
2. [DI (ServiceLocator)](#di-servicelocator)
3. [Event 시스템](#event-시스템)
4. [Logging 시스템](#logging-시스템)
5. [Pool 시스템](#pool-시스템)
6. [State 시스템](#state-시스템)

---

## Config 시스템

### 개요
중앙화된 설정 관리 시스템으로, JSON/XML 파일이나 런타임에서 설정을 로드하고 관리합니다.

### 핵심 클래스: ConfigManager

```csharp
public class ConfigManager : MonoBehaviour
{
    public static ConfigManager Instance { get; private set; }
    
    [SerializeField] private string configFilePath = "config.json";
    private Dictionary<Type, object> _configs = new Dictionary<Type, object>();
    
    public T GetConfig<T>() where T : class, new()
    {
        if (_configs.TryGetValue(typeof(T), out var config))
        {
            return config as T;
        }
        
        // 기본 설정 생성
        var newConfig = new T();
        _configs[typeof(T)] = newConfig;
        return newConfig;
    }
}
```

### NetworkConfig 사용 예제

```csharp
[Serializable]
public class NetworkConfig
{
    [Header("QUIC Settings")]
    public string quicHost = "localhost";
    public int quicPort = 5000;
    public int connectTimeoutMs = 5000;
    public bool enable0Rtt = true;
    public bool enableConnectionMigration = true;
    public int keepAliveIntervalMs = 30000;
    
    [Header("gRPC Settings")]
    public string grpcEndpoint = "https://localhost:5001";
    public int grpcTimeoutMs = 10000;
    public bool grpcEnableRetry = true;
    
    [Header("TCP Settings")]
    public string tcpHost = "localhost";
    public int tcpPort = 5002;
    public int tcpBufferSize = 8192;
    
    public string GetQuicEndpoint()
    {
        return $"https://{quicHost}:{quicPort}";
    }
    
    public string GetTcpEndpoint()
    {
        return $"{tcpHost}:{tcpPort}";
    }
}
```

### 실제 사용법

```csharp
public class NetworkInitializer : MonoBehaviour
{
    void Start()
    {
        // 설정 로드
        var config = ConfigManager.Instance.GetConfig<NetworkConfig>();
        
        // 네트워크 매니저에 설정 전달
        var networkManager = ServiceLocator.Instance.Get<INetworkManager>();
        networkManager.Initialize(config);
        
        Debug.Log($"QUIC 서버: {config.GetQuicEndpoint()}");
    }
}
```

### 고급 기능

#### 환경별 설정 관리
```csharp
public enum Environment
{
    Development,
    Staging,
    Production
}

public class EnvironmentConfig
{
    public Environment currentEnvironment = Environment.Development;
    
    public NetworkConfig GetNetworkConfig()
    {
        return currentEnvironment switch
        {
            Environment.Development => new NetworkConfig
            {
                quicHost = "localhost",
                quicPort = 5000
            },
            Environment.Production => new NetworkConfig
            {
                quicHost = "game-server.com",
                quicPort = 443
            },
            _ => new NetworkConfig()
        };
    }
}
```

---

## DI (ServiceLocator)

### 개요
의존성 주입을 위한 서비스 로케이터 패턴 구현체입니다. 타입 안전성을 보장하며, 싱글톤과 인스턴스 등록을 모두 지원합니다.

### 핵심 클래스: ServiceLocator

```csharp
public class ServiceLocator
{
    private static ServiceLocator _instance;
    public static ServiceLocator Instance => _instance ??= new ServiceLocator();
    
    private readonly Dictionary<Type, object> _services = new Dictionary<Type, object>();
    private readonly Dictionary<Type, Func<object>> _factories = new Dictionary<Type, Func<object>>();
    
    // 싱글톤 등록
    public void Register<T>(T service) where T : class
    {
        _services[typeof(T)] = service;
    }
    
    // 팩토리 등록 (매번 새 인스턴스)
    public void RegisterFactory<T>(Func<T> factory) where T : class
    {
        _factories[typeof(T)] = () => factory();
    }
    
    // 서비스 가져오기
    public T Get<T>() where T : class
    {
        var type = typeof(T);
        
        // 싱글톤 체크
        if (_services.TryGetValue(type, out var service))
        {
            return service as T;
        }
        
        // 팩토리 체크
        if (_factories.TryGetValue(type, out var factory))
        {
            return factory() as T;
        }
        
        throw new InvalidOperationException($"Service {type.Name} not registered");
    }
}
```

### Bootstrap에서 서비스 등록

```csharp
public class Bootstrap : MonoBehaviour
{
    void Awake()
    {
        InitializeServices();
    }
    
    private void InitializeServices()
    {
        // 설정 등록
        var networkConfig = new NetworkConfig();
        ServiceLocator.Instance.Register<NetworkConfig>(networkConfig);
        
        // 네트워크 매니저 등록
        var networkManager = new NetworkConnectionManager();
        ServiceLocator.Instance.Register<INetworkManager>(networkManager);
        
        // 연결 풀 등록
        var connectionPool = new ConnectionPool(networkConfig);
        ServiceLocator.Instance.Register<ConnectionPool>(connectionPool);
        
        // 팩토리 등록 (매번 새 인스턴스)
        ServiceLocator.Instance.RegisterFactory<ILogger>(() => new LogOptimized());
        
        Debug.Log("Services initialized successfully");
    }
}
```

### 실제 사용 예제

```csharp
public class LoginManager : MonoBehaviour
{
    private INetworkManager _networkManager;
    private ConnectionPool _connectionPool;
    
    void Start()
    {
        // 의존성 주입
        _networkManager = ServiceLocator.Instance.Get<INetworkManager>();
        _connectionPool = ServiceLocator.Instance.Get<ConnectionPool>();
    }
    
    public async Task<bool> LoginAsync(string username, string password)
    {
        // gRPC 클라이언트 가져오기
        var grpcClient = await _connectionPool.GetGrpcClientAsync();
        
        // 로그인 로직
        var loginRequest = new LoginRequest
        {
            Username = username,
            Password = password
        };
        
        var response = await grpcClient.LoginAsync(loginRequest);
        return response.Success;
    }
}
```

---

## Event 시스템

### 개요
타입 안전한 이벤트 버스 시스템으로, 느슨한 결합을 통해 컴포넌트 간 통신을 담당합니다.

### EventBusOptimized 구현

```csharp
public class EventBusOptimized
{
    public static EventBusOptimized Instance { get; } = new EventBusOptimized();
    
    private readonly Dictionary<Type, List<IEventHandler>> _handlers
        = new Dictionary<Type, List<IEventHandler>>();
    
    private readonly object _lock = new object();
    
    public void Subscribe<T>(Action<T> handler) where T : class
    {
        lock (_lock)
        {
            var eventType = typeof(T);
            
            if (!_handlers.ContainsKey(eventType))
            {
                _handlers[eventType] = new List<IEventHandler>();
            }
            
            _handlers[eventType].Add(new EventHandler<T>(handler));
        }
    }
    
    public void Publish<T>(T eventData) where T : class
    {
        List<IEventHandler> handlers = null;
        
        lock (_lock)
        {
            if (_handlers.TryGetValue(typeof(T), out handlers))
            {
                handlers = new List<IEventHandler>(handlers); // 복사본 생성
            }
        }
        
        handlers?.ForEach(h => h.Handle(eventData));
    }
    
    public void Unsubscribe<T>(Action<T> handler) where T : class
    {
        lock (_lock)
        {
            if (_handlers.TryGetValue(typeof(T), out var handlerList))
            {
                handlerList.RemoveAll(h => h is EventHandler<T> eh && eh.Handler.Equals(handler));
            }
        }
    }
}
```

### 네트워크 이벤트 정의

```csharp
// 네트워크 연결 이벤트
public class NetworkConnectedEvent
{
    public NetworkProtocol Protocol { get; set; }
    public string ConnectionId { get; set; }
    public DateTime ConnectedAt { get; set; }
}

public class NetworkDisconnectedEvent
{
    public NetworkProtocol Protocol { get; set; }
    public string Reason { get; set; }
    public DateTime DisconnectedAt { get; set; }
}

public class NetworkErrorEvent
{
    public NetworkProtocol Protocol { get; set; }
    public string Error { get; set; }
    public Exception Exception { get; set; }
}

// 게임 이벤트
public class PlayerJoinedEvent
{
    public string PlayerId { get; set; }
    public string PlayerName { get; set; }
    public DateTime JoinedAt { get; set; }
}

public class GameStartEvent
{
    public string RoomId { get; set; }
    public List<string> PlayerIds { get; set; }
}
```

### 실제 사용 예제

```csharp
public class NetworkEventHandler : MonoBehaviour
{
    void Start()
    {
        // 이벤트 구독
        EventBusOptimized.Instance.Subscribe<NetworkConnectedEvent>(OnNetworkConnected);
        EventBusOptimized.Instance.Subscribe<NetworkErrorEvent>(OnNetworkError);
        EventBusOptimized.Instance.Subscribe<PlayerJoinedEvent>(OnPlayerJoined);
    }
    
    void OnDestroy()
    {
        // 이벤트 구독 해제
        EventBusOptimized.Instance.Unsubscribe<NetworkConnectedEvent>(OnNetworkConnected);
        EventBusOptimized.Instance.Unsubscribe<NetworkErrorEvent>(OnNetworkError);
        EventBusOptimized.Instance.Unsubscribe<PlayerJoinedEvent>(OnPlayerJoined);
    }
    
    private void OnNetworkConnected(NetworkConnectedEvent eventData)
    {
        Debug.Log($"네트워크 연결됨: {eventData.Protocol} - {eventData.ConnectionId}");
        
        // UI 업데이트
        UpdateConnectionStatus(true, eventData.Protocol);
    }
    
    private void OnNetworkError(NetworkErrorEvent eventData)
    {
        Debug.LogError($"네트워크 오류: {eventData.Protocol} - {eventData.Error}");
        
        // 오류 UI 표시
        ShowErrorDialog(eventData.Error);
    }
    
    private void OnPlayerJoined(PlayerJoinedEvent eventData)
    {
        Debug.Log($"플레이어 참가: {eventData.PlayerName}");
        
        // 플레이어 리스트 업데이트
        UpdatePlayerList(eventData);
    }
}

// 네트워크 매니저에서 이벤트 발행
public class NetworkConnectionManager : INetworkManager
{
    public async Task<bool> ConnectAsync(NetworkProtocol protocol)
    {
        try
        {
            // 연결 로직...
            var connected = await ConnectToProtocol(protocol);
            
            if (connected)
            {
                // 연결 성공 이벤트 발행
                EventBusOptimized.Instance.Publish(new NetworkConnectedEvent
                {
                    Protocol = protocol,
                    ConnectionId = Guid.NewGuid().ToString(),
                    ConnectedAt = DateTime.UtcNow
                });
            }
            
            return connected;
        }
        catch (Exception ex)
        {
            // 오류 이벤트 발행
            EventBusOptimized.Instance.Publish(new NetworkErrorEvent
            {
                Protocol = protocol,
                Error = ex.Message,
                Exception = ex
            });
            
            return false;
        }
    }
}
```

---

## Logging 시스템

### 개요
성능 최적화된 로깅 시스템으로, 카테고리별 필터링과 다양한 로그 레벨을 지원합니다.

### LogOptimized 구현

```csharp
public enum LogLevel
{
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
    Fatal = 5
}

public static class Log
{
    private static ILogger _logger = new LogOptimized();
    
    public static void Trace(string message, string category = "General") 
        => _logger.Log(LogLevel.Trace, message, category);
    
    public static void Debug(string message, string category = "General") 
        => _logger.Log(LogLevel.Debug, message, category);
    
    public static void Info(string message, string category = "General") 
        => _logger.Log(LogLevel.Info, message, category);
    
    public static void Warning(string message, string category = "General") 
        => _logger.Log(LogLevel.Warning, message, category);
    
    public static void Error(string message, string category = "General") 
        => _logger.Log(LogLevel.Error, message, category);
    
    public static void Fatal(string message, string category = "General") 
        => _logger.Log(LogLevel.Fatal, message, category);
}

public class LogOptimized : ILogger
{
    private readonly Dictionary<string, LogLevel> _categoryLevels = new();
    private readonly Queue<LogEntry> _logQueue = new();
    private readonly object _lock = new object();
    
    public LogLevel GlobalLevel { get; set; } = LogLevel.Info;
    
    public void Log(LogLevel level, string message, string category)
    {
        // 레벨 체크
        if (!ShouldLog(level, category))
            return;
        
        var entry = new LogEntry
        {
            Level = level,
            Message = message,
            Category = category,
            Timestamp = DateTime.UtcNow,
            ThreadId = System.Threading.Thread.CurrentThread.ManagedThreadId
        };
        
        lock (_lock)
        {
            _logQueue.Enqueue(entry);
            
            // 큐 크기 제한
            if (_logQueue.Count > 1000)
            {
                _logQueue.Dequeue();
            }
        }
        
        // 즉시 콘솔 출력 (Unity)
        var formattedMessage = FormatMessage(entry);
        
        switch (level)
        {
            case LogLevel.Error:
            case LogLevel.Fatal:
                UnityEngine.Debug.LogError(formattedMessage);
                break;
            case LogLevel.Warning:
                UnityEngine.Debug.LogWarning(formattedMessage);
                break;
            default:
                UnityEngine.Debug.Log(formattedMessage);
                break;
        }
    }
    
    private bool ShouldLog(LogLevel level, string category)
    {
        var requiredLevel = _categoryLevels.ContainsKey(category) 
            ? _categoryLevels[category] 
            : GlobalLevel;
        
        return level >= requiredLevel;
    }
    
    private string FormatMessage(LogEntry entry)
    {
        return $"[{entry.Timestamp:HH:mm:ss.fff}] [{entry.Level}] [{entry.Category}] {entry.Message}";
    }
}
```

### 실제 사용 예제

```csharp
public class QuicClient
{
    public async Task<bool> ConnectAsync(string serverUrl)
    {
        Log.Info($"QUIC 연결 시작: {serverUrl}", "Network");
        
        try
        {
            // 연결 로직
            var connected = await PerformConnection(serverUrl);
            
            if (connected)
            {
                Log.Info("QUIC 연결 성공", "Network");
                return true;
            }
            else
            {
                Log.Warning("QUIC 연결 실패", "Network");
                return false;
            }
        }
        catch (Exception ex)
        {
            Log.Error($"QUIC 연결 오류: {ex.Message}", "Network");
            Log.Debug($"QUIC 연결 스택 트레이스: {ex.StackTrace}", "Network");
            return false;
        }
    }
}

// 카테고리별 로그 레벨 설정
public class LogConfig
{
    public void SetupLogging()
    {
        var logger = ServiceLocator.Instance.Get<ILogger>() as LogOptimized;
        
        // 개발 환경에서는 Debug 레벨
        logger.SetCategoryLevel("Network", LogLevel.Debug);
        logger.SetCategoryLevel("Game", LogLevel.Info);
        
        // 프로덕션에서는 Info 레벨
        #if !DEVELOPMENT
        logger.GlobalLevel = LogLevel.Info;
        #endif
    }
}
```

---

## Pool 시스템

### 개요
메모리 할당을 최소화하기 위한 오브젝트 풀링 시스템입니다. 네트워크 메시지, 게임 오브젝트 등에 활용됩니다.

### GenericPool 구현

```csharp
public interface IPoolable
{
    void OnGetFromPool();
    void OnReturnToPool();
}

public class GenericPool<T> where T : class, IPoolable
{
    private readonly ConcurrentQueue<T> _objects = new ConcurrentQueue<T>();
    private readonly Func<T> _objectGenerator;
    private readonly int _maxSize;
    private int _currentSize;
    
    public GenericPool(int initialSize = 10, int maxSize = 100, Func<T> generator = null)
    {
        _maxSize = maxSize;
        _objectGenerator = generator ?? (() => Activator.CreateInstance<T>());
        
        // 초기 오브젝트 생성
        for (int i = 0; i < initialSize; i++)
        {
            var obj = _objectGenerator();
            _objects.Enqueue(obj);
            _currentSize++;
        }
    }
    
    public T Get()
    {
        if (_objects.TryDequeue(out var obj))
        {
            Interlocked.Decrement(ref _currentSize);
            obj.OnGetFromPool();
            return obj;
        }
        
        // 풀이 비었으면 새로 생성
        obj = _objectGenerator();
        obj.OnGetFromPool();
        return obj;
    }
    
    public void Return(T obj)
    {
        if (obj == null) return;
        
        obj.OnReturnToPool();
        
        if (_currentSize < _maxSize)
        {
            _objects.Enqueue(obj);
            Interlocked.Increment(ref _currentSize);
        }
    }
}
```

### 네트워크 메시지 풀링

```csharp
public class PooledNetworkMessage : IPoolable, IDisposable
{
    private static readonly GenericPool<PooledNetworkMessage> _pool = 
        new GenericPool<PooledNetworkMessage>(initialSize: 20, maxSize: 100);
    
    public uint MessageId { get; private set; }
    public MessageType MessageType { get; set; }
    public uint SequenceNumber { get; set; }
    public ArraySegment<byte> Payload { get; private set; }
    
    private ByteBuffer _buffer;
    
    public static PooledNetworkMessage Get()
    {
        return _pool.Get();
    }
    
    public static PooledNetworkMessage Get(MessageType type, byte[] data, int offset, int count)
    {
        var message = _pool.Get();
        message.SetData(type, data, offset, count);
        return message;
    }
    
    public void SetData(MessageType type, byte[] data, int offset, int count)
    {
        MessageType = type;
        
        if (_buffer == null)
        {
            _buffer = ByteBufferPool.Get();
        }
        
        _buffer.SetData(data, offset, count);
        Payload = _buffer.Segment;
    }
    
    public void OnGetFromPool()
    {
        MessageId = GenerateMessageId();
        MessageType = MessageType.Connect;
        SequenceNumber = 0;
    }
    
    public void OnReturnToPool()
    {
        MessageId = 0;
        MessageType = MessageType.Connect;
        SequenceNumber = 0;
        
        if (_buffer != null)
        {
            ByteBufferPool.Return(_buffer);
            _buffer = null;
        }
    }
    
    public void Dispose()
    {
        _pool.Return(this);
    }
}
```

### 실제 사용 예제

```csharp
public class NetworkMessageSender
{
    public async Task SendMessageAsync(MessageType type, byte[] data)
    {
        // 풀에서 메시지 가져오기
        using (var message = PooledNetworkMessage.Get(type, data, 0, data.Length))
        {
            // 메시지 전송
            await SendToNetwork(message);
            
            // using 블록이 끝나면 자동으로 풀에 반환
        }
    }
    
    // 배치 전송을 위한 메시지 풀링
    public async Task SendBatchAsync(List<(MessageType, byte[])> messages)
    {
        var pooledMessages = new List<PooledNetworkMessage>();
        
        try
        {
            // 메시지들을 풀에서 가져오기
            foreach (var (type, data) in messages)
            {
                var message = PooledNetworkMessage.Get(type, data, 0, data.Length);
                pooledMessages.Add(message);
            }
            
            // 배치 전송
            await SendBatchToNetwork(pooledMessages);
        }
        finally
        {
            // 모든 메시지를 풀에 반환
            foreach (var message in pooledMessages)
            {
                message.Dispose();
            }
        }
    }
}
```

---

## State 시스템

### 개요
게임 상태 관리를 위한 상태 머신 시스템입니다. 네트워크 연결 상태, 게임 진행 상태 등을 관리합니다.

### StateMachine 구현

```csharp
public interface IState
{
    void OnEnter();
    void OnUpdate();
    void OnExit();
}

public class StateMachine<T> where T : System.Enum
{
    private readonly Dictionary<T, IState> _states = new Dictionary<T, IState>();
    private IState _currentState;
    private T _currentStateType;
    
    public T CurrentState => _currentStateType;
    
    public void RegisterState(T stateType, IState state)
    {
        _states[stateType] = state;
    }
    
    public void ChangeState(T newStateType)
    {
        if (_states.TryGetValue(newStateType, out var newState))
        {
            _currentState?.OnExit();
            _currentState = newState;
            _currentStateType = newStateType;
            _currentState.OnEnter();
            
            // 상태 변경 이벤트 발행
            EventBusOptimized.Instance.Publish(new StateChangedEvent<T>
            {
                PreviousState = _currentStateType,
                NewState = newStateType
            });
        }
    }
    
    public void Update()
    {
        _currentState?.OnUpdate();
    }
}
```

### 네트워크 연결 상태 관리

```csharp
public enum NetworkState
{
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error
}

public class DisconnectedState : IState
{
    private readonly INetworkManager _networkManager;
    
    public DisconnectedState(INetworkManager networkManager)
    {
        _networkManager = networkManager;
    }
    
    public void OnEnter()
    {
        Log.Info("네트워크 연결 해제 상태 진입", "Network");
        // UI 업데이트, 재연결 버튼 활성화 등
    }
    
    public void OnUpdate()
    {
        // 자동 재연결 로직 등
    }
    
    public void OnExit()
    {
        Log.Info("네트워크 연결 해제 상태 종료", "Network");
    }
}

public class ConnectedState : IState
{
    public void OnEnter()
    {
        Log.Info("네트워크 연결 상태 진입", "Network");
        
        // 연결 성공 이벤트 발행
        EventBusOptimized.Instance.Publish(new NetworkConnectedEvent());
    }
    
    public void OnUpdate()
    {
        // 하트비트, 연결 상태 모니터링 등
    }
    
    public void OnExit()
    {
        Log.Info("네트워크 연결 상태 종료", "Network");
    }
}
```

### 게임 상태 관리

```csharp
public enum GameState
{
    MainMenu,
    Lobby,
    InGame,
    GameOver,
    Paused
}

public class GameStateMachine : MonoBehaviour
{
    private StateMachine<GameState> _stateMachine;
    
    void Start()
    {
        _stateMachine = new StateMachine<GameState>();
        
        // 상태들 등록
        _stateMachine.RegisterState(GameState.MainMenu, new MainMenuState());
        _stateMachine.RegisterState(GameState.Lobby, new LobbyState());
        _stateMachine.RegisterState(GameState.InGame, new InGameState());
        _stateMachine.RegisterState(GameState.GameOver, new GameOverState());
        _stateMachine.RegisterState(GameState.Paused, new PausedState());
        
        // 초기 상태 설정
        _stateMachine.ChangeState(GameState.MainMenu);
    }
    
    void Update()
    {
        _stateMachine.Update();
    }
    
    public void StartGame()
    {
        _stateMachine.ChangeState(GameState.InGame);
    }
    
    public void PauseGame()
    {
        _stateMachine.ChangeState(GameState.Paused);
    }
}
```

## 통합 사용 예제

모든 Core 시스템을 함께 활용하는 예제입니다.

```csharp
public class PoliceThiefGameManager : MonoBehaviour
{
    private StateMachine<GameState> _gameStateMachine;
    private INetworkManager _networkManager;
    private ILogger _logger;
    
    void Start()
    {
        InitializeCoreSystem();
        SetupGame();
    }
    
    private void InitializeCoreSystem()
    {
        // 설정 로드
        var config = ConfigManager.Instance.GetConfig<NetworkConfig>();
        ServiceLocator.Instance.Register<NetworkConfig>(config);
        
        // 네트워크 매니저 등록
        var networkManager = new NetworkConnectionManager();
        ServiceLocator.Instance.Register<INetworkManager>(networkManager);
        
        // 로거 등록
        var logger = new LogOptimized();
        ServiceLocator.Instance.Register<ILogger>(logger);
        
        // 이벤트 구독
        EventBusOptimized.Instance.Subscribe<NetworkConnectedEvent>(OnNetworkConnected);
        EventBusOptimized.Instance.Subscribe<PlayerJoinedEvent>(OnPlayerJoined);
    }
    
    private void SetupGame()
    {
        _networkManager = ServiceLocator.Instance.Get<INetworkManager>();
        _logger = ServiceLocator.Instance.Get<ILogger>();
        
        // 게임 상태 머신 설정
        _gameStateMachine = new StateMachine<GameState>();
        _gameStateMachine.RegisterState(GameState.MainMenu, new MainMenuState(_networkManager));
        _gameStateMachine.RegisterState(GameState.Lobby, new LobbyState(_networkManager));
        _gameStateMachine.RegisterState(GameState.InGame, new InGameState(_networkManager));
        
        _gameStateMachine.ChangeState(GameState.MainMenu);
        
        _logger.Log(LogLevel.Info, "게임 초기화 완료", "Game");
    }
    
    private void OnNetworkConnected(NetworkConnectedEvent eventData)
    {
        _logger.Log(LogLevel.Info, $"네트워크 연결: {eventData.Protocol}", "Network");
        
        // 로비 상태로 전환
        _gameStateMachine.ChangeState(GameState.Lobby);
    }
    
    private void OnPlayerJoined(PlayerJoinedEvent eventData)
    {
        _logger.Log(LogLevel.Info, $"플레이어 참가: {eventData.PlayerName}", "Game");
        
        // 플레이어 목록 업데이트 등
        UpdatePlayerList(eventData);
    }
}
```

## 다음 단계

Core Systems 가이드를 마스터했다면, 다음 문서들을 참조하세요:

1. [Network Infrastructure](./03_Network_Infrastructure.md) - 네트워크 상세 구현
2. [Game Logic](./04_Game_Logic.md) - 게임 로직 구현  
3. [Performance Optimization](./05_Performance_Optimization.md) - 성능 최적화
4. [Extension Guide](./06_Extension_Guide.md) - 확장 방안