# Extension Guide - 확장 가이드

## 개요

Police-Thief 프로젝트의 모듈러 아키텍처를 활용하여 새로운 기능, 프로토콜, 게임 모드를 확장하는 방법을 안내합니다. 각 확장 방법은 기존 시스템과의 호환성을 유지하면서 새로운 기능을 추가할 수 있도록 설계되었습니다.

## 목차

1. [네트워크 프로토콜 확장](#네트워크-프로토콜-확장)
2. [게임 모드 확장](#게임-모드-확장)
3. [UI/UX 시스템 확장](#uiux-시스템-확장)
4. [플러그인 시스템 구축](#플러그인-시스템-구축)
5. [플랫폼별 확장](#플랫폼별-확장)
6. [AI 시스템 통합](#ai-시스템-통합)
7. [데이터베이스 통합](#데이터베이스-통합)

---

## 네트워크 프로토콜 확장

### 새로운 프로토콜 추가하기

#### 1. WebSocket 프로토콜 추가 예제

**인터페이스 구현**:
```csharp
public enum NetworkProtocol
{
    QUIC,
    GRPC,
    TCP,
    WebSocket  // 새 프로토콜 추가
}

public class WebSocketProtocolManager : IProtocolManager
{
    private WebSocket _webSocket;
    private readonly NetworkConfig _config;
    private bool _isConnected;
    
    public WebSocketProtocolManager(NetworkConfig config)
    {
        _config = config;
    }
    
    public async Task<bool> ConnectAsync()
    {
        try
        {
            var uri = new Uri($"ws://{_config.websocketHost}:{_config.websocketPort}");
            _webSocket = new WebSocket(uri);
            
            _webSocket.OnOpen += OnWebSocketConnected;
            _webSocket.OnMessage += OnWebSocketMessage;
            _webSocket.OnClose += OnWebSocketDisconnected;
            _webSocket.OnError += OnWebSocketError;
            
            await _webSocket.ConnectAsync();
            return _isConnected;
        }
        catch (Exception ex)
        {
            Log.Error($"WebSocket 연결 실패: {ex.Message}", "Network");
            return false;
        }
    }
    
    public async Task<bool> SendAsync(NetworkMessage message)
    {
        if (!_isConnected) return false;
        
        try
        {
            var jsonData = JsonUtility.ToJson(message);
            await _webSocket.SendAsync(jsonData);
            return true;
        }
        catch (Exception ex)
        {
            Log.Error($"WebSocket 메시지 전송 실패: {ex.Message}", "Network");
            return false;
        }
    }
    
    public async Task DisconnectAsync()
    {
        if (_webSocket != null && _isConnected)
        {
            await _webSocket.CloseAsync();
            _isConnected = false;
        }
    }
    
    private void OnWebSocketConnected(object sender, EventArgs e)
    {
        _isConnected = true;
        EventBusOptimized.Instance.Publish(new NetworkConnectedEvent
        {
            Protocol = NetworkProtocol.WebSocket,
            ConnectionId = Guid.NewGuid().ToString(),
            ConnectedAt = DateTime.UtcNow
        });
    }
    
    private void OnWebSocketMessage(object sender, MessageEventArgs e)
    {
        try
        {
            var message = JsonUtility.FromJson<NetworkMessage>(e.Data);
            // 메시지 처리 로직
            ProcessReceivedMessage(message);
        }
        catch (Exception ex)
        {
            Log.Error($"WebSocket 메시지 처리 오류: {ex.Message}", "Network");
        }
    }
}
```

**설정 확장**:
```csharp
[Serializable]
public class NetworkConfig
{
    // 기존 설정들...
    
    [Header("WebSocket Settings")]
    public string websocketHost = "localhost";
    public int websocketPort = 8080;
    public int websocketTimeoutMs = 5000;
    public bool websocketEnableCompression = true;
    public string websocketSubProtocol = "game-protocol";
    
    public string GetWebSocketEndpoint()
    {
        return $"ws://{websocketHost}:{websocketPort}";
    }
}
```

**ConnectionPool 확장**:
```csharp
public class ConnectionPool
{
    private readonly ConcurrentQueue<WebSocketProtocolManager> _webSocketPool = new();
    
    public async Task<WebSocketProtocolManager> GetWebSocketClientAsync()
    {
        if (_webSocketPool.TryDequeue(out var client))
        {
            if (client.IsConnected)
            {
                return client;
            }
        }
        
        // 새 WebSocket 클라이언트 생성
        client = new WebSocketProtocolManager(_config);
        var connected = await client.ConnectAsync();
        
        if (!connected)
        {
            throw new NetworkException("WebSocket 연결 실패");
        }
        
        return client;
    }
    
    public void ReturnWebSocketClient(WebSocketProtocolManager client)
    {
        if (client?.IsConnected == true)
        {
            _webSocketPool.Enqueue(client);
        }
    }
}
```

#### 2. 사용자 정의 프로토콜 구현

**바이너리 프로토콜 예제**:
```csharp
public class CustomBinaryProtocolManager : IProtocolManager
{
    private NetworkStream _stream;
    private readonly BinaryFormatter _formatter;
    private readonly NetworkConfig _config;
    
    public CustomBinaryProtocolManager(NetworkConfig config)
    {
        _config = config;
        _formatter = new BinaryFormatter();
    }
    
    public async Task<bool> ConnectAsync()
    {
        try
        {
            var client = new TcpClient();
            await client.ConnectAsync(_config.customHost, _config.customPort);
            _stream = client.GetStream();
            
            // 커스텀 핸드셰이크
            await PerformCustomHandshake();
            
            return true;
        }
        catch (Exception ex)
        {
            Log.Error($"Custom protocol 연결 실패: {ex.Message}", "Network");
            return false;
        }
    }
    
    private async Task PerformCustomHandshake()
    {
        // 커스텀 프로토콜 핸드셰이크 로직
        var handshakeData = new HandshakeMessage
        {
            Version = "1.0",
            ClientId = SystemInfo.deviceUniqueIdentifier,
            Timestamp = DateTimeOffset.UtcNow.ToUnixTimeSeconds()
        };
        
        await SendHandshakeAsync(handshakeData);
        var response = await ReceiveHandshakeAsync();
        
        if (!response.Success)
        {
            throw new NetworkException("핸드셰이크 실패");
        }
    }
}
```

---

## 게임 모드 확장

### 새로운 게임 모드 추가하기

#### 1. Battle Royale 모드 구현

**게임 모드 기본 구조**:
```csharp
public abstract class GameMode : GameEntity
{
    protected string _modeId;
    protected Dictionary<string, Player> _players;
    protected GameState _currentState;
    protected GameRules _rules;
    
    public abstract void Initialize(GameRules rules);
    public abstract void StartGame();
    public abstract void EndGame(GameResult result);
    public abstract void HandlePlayerJoin(Player player);
    public abstract void HandlePlayerLeave(Player player);
    public abstract void UpdateGameLogic();
}

public class BattleRoyaleMode : GameMode
{
    private SafeZone _safeZone;
    private List<LootSpawnPoint> _lootSpawns;
    private Dictionary<string, PlayerStatus> _playerStatuses;
    private float _zoneTimer;
    private int _currentZonePhase;
    
    public override void Initialize(GameRules rules)
    {
        _modeId = "battle_royale";
        _rules = rules as BattleRoyaleRules;
        _players = new Dictionary<string, Player>();
        _playerStatuses = new Dictionary<string, PlayerStatus>();
        
        InitializeSafeZone();
        SetupLootSpawns();
        
        Log.Info("Battle Royale 모드 초기화 완료", "Game");
    }
    
    public override void StartGame()
    {
        _currentState = GameState.InProgress;
        _zoneTimer = _rules.initialZoneDelay;
        
        // 모든 플레이어를 랜덤 위치에 스폰
        SpawnAllPlayers();
        
        // 첫 번째 안전지대 축소 시작
        StartZonePhase(1);
        
        EventBusOptimized.Instance.Publish(new GameModeStartedEvent
        {
            ModeId = _modeId,
            PlayerCount = _players.Count,
            StartTime = DateTime.UtcNow
        });
        
        Log.Info($"Battle Royale 시작: {_players.Count}명 참가", "Game");
    }
    
    public override void UpdateGameLogic()
    {
        if (_currentState != GameState.InProgress) return;
        
        UpdateZoneTimer();
        CheckPlayerPositions();
        UpdatePlayerStatuses();
        CheckWinCondition();
        
        // 아이템 리스폰
        UpdateLootSpawns();
    }
    
    private void UpdateZoneTimer()
    {
        _zoneTimer -= Time.deltaTime;
        
        if (_zoneTimer <= 0 && _currentZonePhase < _rules.maxZonePhases)
        {
            _currentZonePhase++;
            StartZonePhase(_currentZonePhase);
            _zoneTimer = _rules.zonePhaseDelays[_currentZonePhase - 1];
        }
    }
    
    private void StartZonePhase(int phase)
    {
        var newRadius = _rules.zoneSizes[phase - 1];
        var shrinkDuration = _rules.zoneShrinkDurations[phase - 1];
        
        _safeZone.ShrinkTo(newRadius, shrinkDuration);
        
        EventBusOptimized.Instance.Publish(new ZonePhaseStartedEvent
        {
            Phase = phase,
            NewRadius = newRadius,
            ShrinkDuration = shrinkDuration,
            DamagePerSecond = _rules.zoneDamages[phase - 1]
        });
        
        Log.Info($"안전지대 {phase}단계 시작: 반지름 {newRadius}m", "Game");
    }
    
    private void CheckPlayerPositions()
    {
        foreach (var kvp in _players)
        {
            var player = kvp.Value;
            var playerId = kvp.Key;
            
            if (!_safeZone.IsInSafeZone(player.Position))
            {
                // 플레이어가 안전지대 밖에 있음
                ApplyZoneDamage(playerId, _rules.zoneDamages[_currentZonePhase - 1]);
            }
        }
    }
    
    private void CheckWinCondition()
    {
        var alivePlayers = _playerStatuses.Values.Count(status => status.IsAlive);
        
        if (alivePlayers <= 1)
        {
            var winner = _playerStatuses.FirstOrDefault(kvp => kvp.Value.IsAlive);
            EndGame(new GameResult
            {
                GameMode = _modeId,
                WinnerId = winner.Key,
                Duration = DateTime.UtcNow - _gameStartTime,
                FinalPlayerCount = alivePlayers
            });
        }
    }
    
    public override void HandlePlayerJoin(Player player)
    {
        if (_currentState == GameState.Waiting && _players.Count < _rules.maxPlayers)
        {
            _players[player.Id] = player;
            _playerStatuses[player.Id] = new PlayerStatus
            {
                PlayerId = player.Id,
                Health = _rules.maxHealth,
                IsAlive = true,
                JoinTime = DateTime.UtcNow
            };
            
            EventBusOptimized.Instance.Publish(new PlayerJoinedGameModeEvent
            {
                ModeId = _modeId,
                PlayerId = player.Id,
                PlayerCount = _players.Count
            });
            
            // 충분한 플레이어가 모이면 게임 시작
            if (_players.Count >= _rules.minPlayers)
            {
                StartCountdown();
            }
        }
    }
}
```

**게임 모드별 설정**:
```csharp
[Serializable]
public class BattleRoyaleRules : GameRules
{
    [Header("Player Settings")]
    public int minPlayers = 2;
    public int maxPlayers = 100;
    public float maxHealth = 100f;
    public float respawnDelay = 0f; // Battle Royale에서는 리스폰 없음
    
    [Header("Zone Settings")]
    public float initialZoneDelay = 120f; // 2분 후 첫 번째 축소
    public int maxZonePhases = 6;
    public float[] zoneSizes = { 2000f, 1500f, 1000f, 600f, 300f, 100f };
    public float[] zoneShrinkDurations = { 30f, 25f, 20f, 15f, 10f, 5f };
    public float[] zonePhaseDelays = { 60f, 45f, 30f, 20f, 15f, 10f };
    public float[] zoneDamages = { 1f, 2f, 5f, 8f, 12f, 20f };
    
    [Header("Loot Settings")]
    public int maxLootSpawns = 200;
    public float lootRespawnRate = 0.1f;
    public LootTable[] lootTables;
}
```

#### 2. 팀 데스매치 모드 구현

```csharp
public class TeamDeathMatchMode : GameMode
{
    private Dictionary<TeamType, Team> _teams;
    private Dictionary<TeamType, int> _teamScores;
    private float _gameTimer;
    
    public override void Initialize(GameRules rules)
    {
        _modeId = "team_deathmatch";
        _rules = rules as TeamDeathMatchRules;
        
        InitializeTeams();
        _teamScores = new Dictionary<TeamType, int>
        {
            { TeamType.Police, 0 },
            { TeamType.Thief, 0 }
        };
        
        _gameTimer = _rules.matchDuration;
        
        Log.Info("Team Deathmatch 모드 초기화 완료", "Game");
    }
    
    private void InitializeTeams()
    {
        _teams = new Dictionary<TeamType, Team>
        {
            { TeamType.Police, new Team(TeamType.Police, "Police", Color.blue) },
            { TeamType.Thief, new Team(TeamType.Thief, "Thieves", Color.red) }
        };
    }
    
    public override void UpdateGameLogic()
    {
        if (_currentState != GameState.InProgress) return;
        
        _gameTimer -= Time.deltaTime;
        
        // 제한시간 종료 확인
        if (_gameTimer <= 0)
        {
            EndGameByTime();
        }
        
        // 목표 점수 달성 확인
        CheckScoreWinCondition();
        
        // 팀 밸런싱
        if (Time.time % 30f < Time.deltaTime) // 30초마다
        {
            CheckTeamBalance();
        }
    }
    
    public void OnPlayerKill(string killerId, string victimId)
    {
        var killer = _players[killerId];
        var victim = _players[victimId];
        
        if (killer.TeamType != victim.TeamType)
        {
            // 적팀 킬 시 점수 증가
            _teamScores[killer.TeamType]++;
            
            EventBusOptimized.Instance.Publish(new PlayerKilledEvent
            {
                KillerId = killerId,
                VictimId = victimId,
                KillerTeam = killer.TeamType,
                VictimTeam = victim.TeamType,
                NewScore = _teamScores[killer.TeamType]
            });
            
            // 승리 조건 확인
            if (_teamScores[killer.TeamType] >= _rules.targetScore)
            {
                EndGameByScore(killer.TeamType);
            }
        }
        
        // 플레이어 리스폰 처리
        ScheduleRespawn(victimId);
    }
    
    private async void ScheduleRespawn(string playerId)
    {
        await Task.Delay((int)(_rules.respawnDelay * 1000));
        
        if (_players.ContainsKey(playerId))
        {
            var player = _players[playerId];
            var spawnPoint = GetTeamSpawnPoint(player.TeamType);
            
            player.Respawn(spawnPoint);
            
            EventBusOptimized.Instance.Publish(new PlayerRespawnedEvent
            {
                PlayerId = playerId,
                TeamType = player.TeamType,
                SpawnPosition = spawnPoint
            });
        }
    }
}
```

---

## UI/UX 시스템 확장

### 새로운 UI 컴포넌트 추가

#### 1. 게임 모드 선택 UI

```csharp
public class GameModeSelector : MonoBehaviour
{
    [Header("UI References")]
    [SerializeField] private Transform modeButtonContainer;
    [SerializeField] private GameObject modeButtonPrefab;
    [SerializeField] private TextMeshProUGUI selectedModeText;
    [SerializeField] private TextMeshProUGUI modeDescriptionText;
    [SerializeField] private Button startGameButton;
    
    [Header("Game Modes")]
    [SerializeField] private GameModeInfo[] availableGameModes;
    
    private GameModeInfo _selectedMode;
    private List<GameModeButton> _modeButtons;
    
    void Start()
    {
        InitializeModeButtons();
        startGameButton.onClick.AddListener(OnStartGameClicked);
    }
    
    private void InitializeModeButtons()
    {
        _modeButtons = new List<GameModeButton>();
        
        foreach (var modeInfo in availableGameModes)
        {
            var buttonObj = Instantiate(modeButtonPrefab, modeButtonContainer);
            var modeButton = buttonObj.GetComponent<GameModeButton>();
            
            modeButton.Initialize(modeInfo, OnModeSelected);
            _modeButtons.Add(modeButton);
        }
        
        // 첫 번째 모드를 기본 선택
        if (_modeButtons.Count > 0)
        {
            OnModeSelected(_modeButtons[0].ModeInfo);
        }
    }
    
    private void OnModeSelected(GameModeInfo modeInfo)
    {
        _selectedMode = modeInfo;
        selectedModeText.text = modeInfo.displayName;
        modeDescriptionText.text = modeInfo.description;
        
        // 모든 버튼의 선택 상태 업데이트
        foreach (var button in _modeButtons)
        {
            button.SetSelected(button.ModeInfo == modeInfo);
        }
        
        EventBusOptimized.Instance.Publish(new GameModeSelectedEvent
        {
            ModeId = modeInfo.modeId,
            ModeName = modeInfo.displayName
        });
    }
    
    private async void OnStartGameClicked()
    {
        if (_selectedMode == null)
        {
            ShowErrorMessage("게임 모드를 선택해주세요.");
            return;
        }
        
        startGameButton.interactable = false;
        
        try
        {
            // 게임 모드 시작 요청
            var gameManager = ServiceLocator.Instance.Get<IGameManager>();
            var success = await gameManager.StartGameModeAsync(_selectedMode.modeId);
            
            if (success)
            {
                // 게임 씬으로 전환
                UnityEngine.SceneManagement.SceneManager.LoadScene(_selectedMode.gameSceneName);
            }
            else
            {
                ShowErrorMessage("게임 시작에 실패했습니다.");
                startGameButton.interactable = true;
            }
        }
        catch (Exception ex)
        {
            Log.Error($"게임 시작 오류: {ex.Message}", "UI");
            ShowErrorMessage("게임 시작 중 오류가 발생했습니다.");
            startGameButton.interactable = true;
        }
    }
}

[Serializable]
public class GameModeInfo
{
    public string modeId;
    public string displayName;
    public string description;
    public Sprite iconSprite;
    public string gameSceneName;
    public int minPlayers;
    public int maxPlayers;
    public float estimatedDuration;
}
```

#### 2. 실시간 통계 UI

```csharp
public class RealTimeStatsUI : MonoBehaviour
{
    [Header("Network Stats")]
    [SerializeField] private TextMeshProUGUI pingText;
    [SerializeField] private TextMeshProUGUI packetLossText;
    [SerializeField] private TextMeshProUGUI bandwidthText;
    [SerializeField] private Image connectionQualityImage;
    
    [Header("Game Stats")]
    [SerializeField] private TextMeshProUGUI fpsText;
    [SerializeField] private TextMeshProUGUI memoryText;
    [SerializeField] private TextMeshProUGUI playerCountText;
    
    [Header("Settings")]
    [SerializeField] private float updateInterval = 1f;
    [SerializeField] private Color[] qualityColors = { Color.red, Color.yellow, Color.green };
    
    private INetworkManager _networkManager;
    private float _lastUpdateTime;
    private float _frameCount;
    private float _deltaTimeSum;
    
    void Start()
    {
        _networkManager = ServiceLocator.Instance.Get<INetworkManager>();
        
        // 네트워크 이벤트 구독
        EventBusOptimized.Instance.Subscribe<NetworkStatsUpdatedEvent>(OnNetworkStatsUpdated);
    }
    
    void Update()
    {
        UpdateFPS();
        
        if (Time.time - _lastUpdateTime >= updateInterval)
        {
            UpdateNetworkStats();
            UpdateSystemStats();
            _lastUpdateTime = Time.time;
        }
    }
    
    private void UpdateFPS()
    {
        _frameCount++;
        _deltaTimeSum += Time.unscaledDeltaTime;
    }
    
    private void UpdateNetworkStats()
    {
        var stats = _networkManager.GetNetworkStats();
        
        // Ping 업데이트
        var ping = stats.GetAverageRTT();
        pingText.text = $"Ping: {ping:F0}ms";
        pingText.color = GetPingColor(ping);
        
        // 패킷 로스 업데이트
        var packetLoss = stats.GetPacketLossRate() * 100;
        packetLossText.text = $"Loss: {packetLoss:F1}%";
        
        // 대역폭 업데이트
        var bandwidth = (stats.GetTotalBytesSent() + stats.GetTotalBytesReceived()) / (1024 * 1024); // MB
        bandwidthText.text = $"Data: {bandwidth:F1}MB";
        
        // 연결 품질 업데이트
        UpdateConnectionQuality(ping, packetLoss);
    }
    
    private void UpdateSystemStats()
    {
        // FPS 업데이트
        if (_frameCount > 0)
        {
            var avgFPS = _frameCount / _deltaTimeSum;
            fpsText.text = $"FPS: {avgFPS:F0}";
            fpsText.color = GetFPSColor(avgFPS);
            
            _frameCount = 0;
            _deltaTimeSum = 0;
        }
        
        // 메모리 사용량 업데이트
        var memoryUsage = Profiler.GetTotalAllocatedMemory(false) / (1024 * 1024); // MB
        memoryText.text = $"Memory: {memoryUsage}MB";
    }
    
    private Color GetPingColor(float ping)
    {
        if (ping < 50) return qualityColors[2]; // Green
        if (ping < 100) return qualityColors[1]; // Yellow
        return qualityColors[0]; // Red
    }
    
    private Color GetFPSColor(float fps)
    {
        if (fps >= 60) return qualityColors[2]; // Green
        if (fps >= 30) return qualityColors[1]; // Yellow
        return qualityColors[0]; // Red
    }
    
    private void UpdateConnectionQuality(float ping, float packetLoss)
    {
        float quality = 1f;
        
        // Ping 기반 품질 계산
        if (ping > 100) quality -= 0.3f;
        else if (ping > 50) quality -= 0.1f;
        
        // 패킷 로스 기반 품질 계산
        quality -= packetLoss * 2f; // 패킷 로스 1%당 품질 2% 감소
        
        quality = Mathf.Clamp01(quality);
        
        connectionQualityImage.fillAmount = quality;
        connectionQualityImage.color = Color.Lerp(qualityColors[0], qualityColors[2], quality);
    }
}
```

---

## 플러그인 시스템 구축

### 플러그인 인터페이스 정의

```csharp
public interface IGamePlugin
{
    string PluginId { get; }
    string PluginName { get; }
    Version PluginVersion { get; }
    
    Task<bool> InitializeAsync(IServiceProvider serviceProvider);
    Task ShutdownAsync();
    void OnUpdate();
    void OnFixedUpdate();
}

public interface INetworkPlugin : IGamePlugin
{
    Task<bool> HandleNetworkMessageAsync(NetworkMessage message);
    void RegisterNetworkHandlers(INetworkManager networkManager);
}

public interface IGameModePlugin : IGamePlugin
{
    GameMode CreateGameMode(GameRules rules);
    bool SupportsGameMode(string modeId);
}

public abstract class BasePlugin : IGamePlugin
{
    public abstract string PluginId { get; }
    public abstract string PluginName { get; }
    public abstract Version PluginVersion { get; }
    
    protected IServiceProvider ServiceProvider { get; private set; }
    protected ILogger Logger { get; private set; }
    
    public virtual async Task<bool> InitializeAsync(IServiceProvider serviceProvider)
    {
        ServiceProvider = serviceProvider;
        Logger = serviceProvider.GetService<ILogger>();
        
        Logger?.Log(LogLevel.Info, $"플러그인 초기화: {PluginName} v{PluginVersion}", "Plugin");
        
        return await OnInitializeAsync();
    }
    
    public virtual async Task ShutdownAsync()
    {
        await OnShutdownAsync();
        Logger?.Log(LogLevel.Info, $"플러그인 종료: {PluginName}", "Plugin");
    }
    
    public virtual void OnUpdate() { }
    public virtual void OnFixedUpdate() { }
    
    protected abstract Task<bool> OnInitializeAsync();
    protected abstract Task OnShutdownAsync();
}
```

### 플러그인 매니저

```csharp
public class PluginManager : MonoBehaviour
{
    [Header("Plugin Settings")]
    [SerializeField] private string pluginsDirectory = "Plugins";
    [SerializeField] private bool enableHotReload = false;
    
    private Dictionary<string, IGamePlugin> _loadedPlugins = new();
    private List<Assembly> _pluginAssemblies = new();
    private IServiceProvider _serviceProvider;
    
    public static PluginManager Instance { get; private set; }
    
    void Awake()
    {
        if (Instance == null)
        {
            Instance = this;
            DontDestroyOnLoad(gameObject);
        }
        else
        {
            Destroy(gameObject);
        }
    }
    
    async void Start()
    {
        _serviceProvider = CreateServiceProvider();
        await LoadPluginsAsync();
    }
    
    void Update()
    {
        foreach (var plugin in _loadedPlugins.Values)
        {
            try
            {
                plugin.OnUpdate();
            }
            catch (Exception ex)
            {
                Log.Error($"플러그인 업데이트 오류 ({plugin.PluginId}): {ex.Message}", "Plugin");
            }
        }
    }
    
    void FixedUpdate()
    {
        foreach (var plugin in _loadedPlugins.Values)
        {
            try
            {
                plugin.OnFixedUpdate();
            }
            catch (Exception ex)
            {
                Log.Error($"플러그인 FixedUpdate 오류 ({plugin.PluginId}): {ex.Message}", "Plugin");
            }
        }
    }
    
    public async Task LoadPluginsAsync()
    {
        var pluginPath = Path.Combine(Application.streamingAssetsPath, pluginsDirectory);
        
        if (!Directory.Exists(pluginPath))
        {
            Directory.CreateDirectory(pluginPath);
            return;
        }
        
        var dllFiles = Directory.GetFiles(pluginPath, "*.dll");
        
        foreach (var dllFile in dllFiles)
        {
            try
            {
                await LoadPluginFromAssemblyAsync(dllFile);
            }
            catch (Exception ex)
            {
                Log.Error($"플러그인 로드 실패 ({Path.GetFileName(dllFile)}): {ex.Message}", "Plugin");
            }
        }
        
        Log.Info($"총 {_loadedPlugins.Count}개 플러그인 로드됨", "Plugin");
    }
    
    private async Task LoadPluginFromAssemblyAsync(string assemblyPath)
    {
        var assembly = Assembly.LoadFrom(assemblyPath);
        _pluginAssemblies.Add(assembly);
        
        var pluginTypes = assembly.GetTypes()
            .Where(t => typeof(IGamePlugin).IsAssignableFrom(t) && !t.IsAbstract);
        
        foreach (var pluginType in pluginTypes)
        {
            var plugin = Activator.CreateInstance(pluginType) as IGamePlugin;
            if (plugin != null)
            {
                var success = await plugin.InitializeAsync(_serviceProvider);
                if (success)
                {
                    _loadedPlugins[plugin.PluginId] = plugin;
                    
                    // 특수 플러그인 타입 처리
                    if (plugin is INetworkPlugin networkPlugin)
                    {
                        RegisterNetworkPlugin(networkPlugin);
                    }
                    
                    if (plugin is IGameModePlugin gameModePlugin)
                    {
                        RegisterGameModePlugin(gameModePlugin);
                    }
                    
                    Log.Info($"플러그인 로드됨: {plugin.PluginName} v{plugin.PluginVersion}", "Plugin");
                }
                else
                {
                    Log.Warning($"플러그인 초기화 실패: {plugin.PluginName}", "Plugin");
                }
            }
        }
    }
    
    public T GetPlugin<T>(string pluginId) where T : class, IGamePlugin
    {
        return _loadedPlugins.TryGetValue(pluginId, out var plugin) ? plugin as T : null;
    }
    
    public async Task UnloadPluginAsync(string pluginId)
    {
        if (_loadedPlugins.TryGetValue(pluginId, out var plugin))
        {
            await plugin.ShutdownAsync();
            _loadedPlugins.Remove(pluginId);
            
            Log.Info($"플러그인 언로드됨: {plugin.PluginName}", "Plugin");
        }
    }
}
```

### 예제 플러그인: 채팅 시스템

```csharp
public class ChatSystemPlugin : BasePlugin, INetworkPlugin
{
    public override string PluginId => "chat_system";
    public override string PluginName => "Chat System";
    public override Version PluginVersion => new Version(1, 0, 0);
    
    private IChatUI _chatUI;
    private IChatStorage _chatStorage;
    private List<ChatFilter> _chatFilters;
    
    protected override async Task<bool> OnInitializeAsync()
    {
        try
        {
            // UI 컴포넌트 생성
            _chatUI = CreateChatUI();
            
            // 채팅 저장소 초기화
            _chatStorage = new InMemoryChatStorage();
            
            // 채팅 필터 초기화
            InitializeChatFilters();
            
            // 이벤트 구독
            EventBusOptimized.Instance.Subscribe<PlayerJoinedEvent>(OnPlayerJoined);
            EventBusOptimized.Instance.Subscribe<PlayerLeftEvent>(OnPlayerLeft);
            
            Logger.Log(LogLevel.Info, "채팅 시스템 초기화 완료", "Chat");
            return true;
        }
        catch (Exception ex)
        {
            Logger.Log(LogLevel.Error, $"채팅 시스템 초기화 실패: {ex.Message}", "Chat");
            return false;
        }
    }
    
    public async Task<bool> HandleNetworkMessageAsync(NetworkMessage message)
    {
        if (message.Type == MessageType.Chat)
        {
            try
            {
                var chatMessage = JsonUtility.FromJson<ChatMessage>(
                    System.Text.Encoding.UTF8.GetString(message.Data));
                
                // 채팅 필터링
                var filteredMessage = await ApplyChatFiltersAsync(chatMessage);
                
                if (filteredMessage != null)
                {
                    // 채팅 저장
                    await _chatStorage.StoreChatMessageAsync(filteredMessage);
                    
                    // UI 업데이트
                    _chatUI.DisplayChatMessage(filteredMessage);
                    
                    // 이벤트 발행
                    EventBusOptimized.Instance.Publish(new ChatMessageReceivedEvent
                    {
                        Message = filteredMessage,
                        Timestamp = DateTime.UtcNow
                    });
                }
                
                return true;
            }
            catch (Exception ex)
            {
                Logger.Log(LogLevel.Error, $"채팅 메시지 처리 오류: {ex.Message}", "Chat");
                return false;
            }
        }
        
        return false;
    }
    
    public void RegisterNetworkHandlers(INetworkManager networkManager)
    {
        // 네트워크 핸들러 등록은 HandleNetworkMessageAsync에서 처리됨
    }
    
    private async Task<ChatMessage> ApplyChatFiltersAsync(ChatMessage message)
    {
        var filteredMessage = message;
        
        foreach (var filter in _chatFilters)
        {
            filteredMessage = await filter.FilterAsync(filteredMessage);
            if (filteredMessage == null)
            {
                Logger.Log(LogLevel.Debug, $"채팅 메시지 필터링됨: {filter.GetType().Name}", "Chat");
                break;
            }
        }
        
        return filteredMessage;
    }
}
```

---

## 플랫폼별 확장

### 모바일 플랫폼 최적화

```csharp
public class MobilePlatformPlugin : BasePlugin
{
    public override string PluginId => "mobile_platform";
    public override string PluginName => "Mobile Platform Optimization";
    public override Version PluginVersion => new Version(1, 0, 0);
    
    private TouchInputManager _touchInputManager;
    private BatteryOptimizer _batteryOptimizer;
    private NetworkAdaptation _networkAdaptation;
    
    protected override async Task<bool> OnInitializeAsync()
    {
        if (!IsMobilePlatform())
        {
            return false; // 모바일 플랫폼이 아니면 비활성화
        }
        
        InitializeTouchInput();
        InitializeBatteryOptimization();
        InitializeNetworkAdaptation();
        
        // 모바일 특화 설정 적용
        ApplyMobileSettings();
        
        return true;
    }
    
    private void InitializeTouchInput()
    {
        var inputManager = new GameObject("TouchInputManager");
        _touchInputManager = inputManager.AddComponent<TouchInputManager>();
        
        _touchInputManager.Initialize(new TouchSettings
        {
            MultiTouchEnabled = true,
            GestureRecognition = true,
            HapticFeedback = true
        });
        
        // 터치 입력 이벤트 매핑
        _touchInputManager.OnTap += OnTouchTap;
        _touchInputManager.OnSwipe += OnTouchSwipe;
        _touchInputManager.OnPinch += OnTouchPinch;
    }
    
    private void InitializeBatteryOptimization()
    {
        _batteryOptimizer = new BatteryOptimizer();
        
        // 배터리 상태 모니터링
        _batteryOptimizer.OnBatteryLevelChanged += OnBatteryLevelChanged;
        _batteryOptimizer.OnBatteryStateChanged += OnBatteryStateChanged;
        
        _batteryOptimizer.StartMonitoring();
    }
    
    private void InitializeNetworkAdaptation()
    {
        _networkAdaptation = new NetworkAdaptation();
        
        // 네트워크 상태에 따른 적응
        _networkAdaptation.OnConnectionTypeChanged += OnConnectionTypeChanged;
        _networkAdaptation.OnConnectionQualityChanged += OnConnectionQualityChanged;
        
        _networkAdaptation.StartMonitoring();
    }
    
    private void ApplyMobileSettings()
    {
        // 프레임레이트 제한
        Application.targetFrameRate = 60;
        
        // 절전 모드 비활성화 (게임 중)
        Screen.sleepTimeout = SleepTimeout.NeverSleep;
        
        // 모바일 렌더링 최적화
        QualitySettings.SetQualityLevel(GetOptimalQualityLevel());
        
        // 해상도 조정
        SetOptimalResolution();
        
        Logger.Log(LogLevel.Info, "모바일 플랫폼 설정 적용 완료", "Mobile");
    }
    
    private void OnBatteryLevelChanged(float batteryLevel)
    {
        if (batteryLevel < 0.2f) // 20% 미만
        {
            // 절전 모드 활성화
            EnablePowerSaveMode();
        }
        else if (batteryLevel > 0.5f) // 50% 이상
        {
            // 일반 모드로 복구
            DisablePowerSaveMode();
        }
    }
    
    private void EnablePowerSaveMode()
    {
        // 프레임레이트 낮추기
        Application.targetFrameRate = 30;
        
        // 품질 설정 낮추기
        QualitySettings.DecreaseLevel();
        
        // 네트워크 업데이트 빈도 줄이기
        var networkManager = ServiceLocator.Instance.Get<INetworkManager>();
        networkManager?.SetUpdateRate(10); // 10Hz로 감소
        
        EventBusOptimized.Instance.Publish(new PowerSaveModeEnabledEvent());
        
        Logger.Log(LogLevel.Info, "절전 모드 활성화", "Mobile");
    }
    
    private void OnConnectionTypeChanged(ConnectionType connectionType)
    {
        var networkConfig = ServiceLocator.Instance.Get<NetworkConfig>();
        
        switch (connectionType)
        {
            case ConnectionType.WiFi:
                // Wi-Fi에서는 고품질 설정
                networkConfig.EnableHighQuality();
                break;
                
            case ConnectionType.Cellular:
                // 셀룰러에서는 데이터 절약 모드
                networkConfig.EnableDataSaver();
                break;
                
            case ConnectionType.None:
                // 오프라인 모드 활성화
                EnableOfflineMode();
                break;
        }
        
        Logger.Log(LogLevel.Info, $"연결 타입 변경: {connectionType}", "Mobile");
    }
}
```

### Console 플랫폼 확장

```csharp
public class ConsolePlatformPlugin : BasePlugin
{
    public override string PluginId => "console_platform";
    public override string PluginName => "Console Platform Support";
    public override Version PluginVersion => new Version(1, 0, 0);
    
    private ConsoleInputManager _consoleInputManager;
    private ConsoleTrophySystem _trophySystem;
    private ConsoleOnlineService _onlineService;
    
    protected override async Task<bool> OnInitializeAsync()
    {
        var platform = GetConsolePlatform();
        if (platform == ConsolePlatform.None)
        {
            return false;
        }
        
        switch (platform)
        {
            case ConsolePlatform.PlayStation:
                await InitializePlayStationSupport();
                break;
                
            case ConsolePlatform.Xbox:
                await InitializeXboxSupport();
                break;
                
            case ConsolePlatform.NintendoSwitch:
                await InitializeNintendoSwitchSupport();
                break;
        }
        
        return true;
    }
    
    private async Task InitializePlayStationSupport()
    {
        // PlayStation 특화 기능 초기화
        _trophySystem = new PlayStationTrophySystem();
        await _trophySystem.InitializeAsync();
        
        _onlineService = new PlayStationNetworkService();
        await _onlineService.InitializeAsync();
        
        // 컨트롤러 설정
        _consoleInputManager = new PlayStationInputManager();
        _consoleInputManager.EnableHapticFeedback(true);
        _consoleInputManager.EnableAdaptiveTriggers(true);
        
        Logger.Log(LogLevel.Info, "PlayStation 플랫폼 지원 초기화 완료", "Console");
    }
    
    private async Task InitializeXboxSupport()
    {
        // Xbox 특화 기능 초기화
        _trophySystem = new XboxAchievementSystem();
        await _trophySystem.InitializeAsync();
        
        _onlineService = new XboxLiveService();
        await _onlineService.InitializeAsync();
        
        // 컨트롤러 설정
        _consoleInputManager = new XboxInputManager();
        _consoleInputManager.EnableRumble(true);
        
        Logger.Log(LogLevel.Info, "Xbox 플랫폼 지원 초기화 완료", "Console");
    }
}
```

---

## AI 시스템 통합

### AI Bot 시스템

```csharp
public class AIBotPlugin : BasePlugin, IGameModePlugin
{
    public override string PluginId => "ai_bot_system";
    public override string PluginName => "AI Bot System";
    public override Version PluginVersion => new Version(1, 0, 0);
    
    private List<AIBot> _activeBots;
    private AIBehaviorTree _behaviorTree;
    private AIPathfinding _pathfinding;
    
    protected override async Task<bool> OnInitializeAsync()
    {
        _activeBots = new List<AIBot>();
        
        // 행동 트리 초기화
        _behaviorTree = new AIBehaviorTree();
        await _behaviorTree.LoadBehaviorTreesAsync("AI/BehaviorTrees");
        
        // 길찾기 시스템 초기화
        _pathfinding = new AIPathfinding();
        await _pathfinding.InitializeAsync();
        
        // 이벤트 구독
        EventBusOptimized.Instance.Subscribe<GameModeStartedEvent>(OnGameStarted);
        EventBusOptimized.Instance.Subscribe<PlayerLeftEvent>(OnPlayerLeft);
        
        return true;
    }
    
    public GameMode CreateGameMode(GameRules rules)
    {
        // AI Bot을 포함한 게임 모드 생성
        if (rules is AIBotGameRules botRules)
        {
            return new AIBotGameMode(botRules, this);
        }
        
        return null;
    }
    
    public bool SupportsGameMode(string modeId)
    {
        return modeId.Contains("_with_bots") || modeId.Contains("_ai");
    }
    
    public async Task<AIBot> CreateBotAsync(BotDifficulty difficulty, TeamType team)
    {
        var bot = new AIBot();
        
        // Bot 설정
        bot.Initialize(new BotConfig
        {
            Difficulty = difficulty,
            Team = team,
            BehaviorTree = _behaviorTree.GetBehaviorTree(difficulty),
            Pathfinding = _pathfinding
        });
        
        // 게임에 Bot 추가
        await AddBotToGameAsync(bot);
        
        _activeBots.Add(bot);
        
        Logger.Log(LogLevel.Info, $"AI Bot 생성됨: {difficulty} 난이도, {team} 팀", "AI");
        
        return bot;
    }
    
    public override void OnUpdate()
    {
        foreach (var bot in _activeBots)
        {
            if (bot.IsActive)
            {
                bot.UpdateAI();
            }
        }
    }
    
    private void OnGameStarted(GameModeStartedEvent eventData)
    {
        // 게임 시작 시 필요한 만큼 Bot 추가
        var gameRules = ServiceLocator.Instance.Get<GameRules>();
        if (gameRules is AIBotGameRules botRules)
        {
            _ = FillWithBotsAsync(botRules);
        }
    }
    
    private async Task FillWithBotsAsync(AIBotGameRules rules)
    {
        var currentPlayerCount = ServiceLocator.Instance.Get<IGameManager>().GetPlayerCount();
        var botsNeeded = rules.maxPlayers - currentPlayerCount;
        
        for (int i = 0; i < botsNeeded && i < rules.maxBots; i++)
        {
            var difficulty = GetRandomDifficulty(rules.botDifficultyDistribution);
            var team = GetBalancedTeam();
            
            await CreateBotAsync(difficulty, team);
            
            // Bot 생성 간격 (부하 분산)
            await Task.Delay(100);
        }
    }
}

public class AIBot : MonoBehaviour, IPlayer
{
    public string Id { get; private set; }
    public string Name { get; private set; }
    public TeamType TeamType { get; private set; }
    public Vector3 Position => transform.position;
    public bool IsActive { get; private set; }
    
    private BotConfig _config;
    private IBehaviorTree _behaviorTree;
    private AIPathfinding _pathfinding;
    private AIMemory _memory;
    private AISensors _sensors;
    
    public void Initialize(BotConfig config)
    {
        _config = config;
        Id = Guid.NewGuid().ToString();
        Name = GenerateBotName();
        TeamType = config.Team;
        
        _behaviorTree = config.BehaviorTree;
        _pathfinding = config.Pathfinding;
        
        // AI 컴포넌트 초기화
        _memory = new AIMemory();
        _sensors = GetComponent<AISensors>();
        _sensors.Initialize(config.SensorRange, config.SensorFOV);
        
        IsActive = true;
        
        Log.Debug($"AI Bot 초기화 완료: {Name}", "AI");
    }
    
    public void UpdateAI()
    {
        if (!IsActive) return;
        
        try
        {
            // 센서 업데이트
            _sensors.UpdateSensors();
            
            // 메모리 업데이트
            _memory.UpdateMemory(_sensors.GetDetectedObjects());
            
            // 행동 트리 실행
            _behaviorTree.Execute(this);
        }
        catch (Exception ex)
        {
            Log.Error($"AI Bot 업데이트 오류 ({Name}): {ex.Message}", "AI");
        }
    }
    
    public async Task<bool> MoveToAsync(Vector3 targetPosition)
    {
        var path = await _pathfinding.FindPathAsync(Position, targetPosition);
        
        if (path != null && path.Count > 0)
        {
            await FollowPathAsync(path);
            return true;
        }
        
        return false;
    }
    
    public void Attack(IPlayer target)
    {
        if (target == null || target.TeamType == TeamType) return;
        
        // 공격 로직
        var weapon = GetComponent<WeaponController>();
        if (weapon != null && IsInRange(target))
        {
            weapon.Fire(target.Position);
            
            // 공격 기억
            _memory.RecordEvent(new AIMemoryEvent
            {
                Type = AIEventType.Attack,
                Target = target.Id,
                Position = target.Position,
                Timestamp = DateTime.UtcNow
            });
        }
    }
}
```

---

## 데이터베이스 통합

### 데이터베이스 플러그인

```csharp
public class DatabasePlugin : BasePlugin
{
    public override string PluginId => "database_system";
    public override string PluginName => "Database Integration";
    public override Version PluginVersion => new Version(1, 0, 0);
    
    private IDbConnectionFactory _connectionFactory;
    private Dictionary<Type, IRepository> _repositories;
    
    protected override async Task<bool> OnInitializeAsync()
    {
        try
        {
            // 데이터베이스 연결 팩토리 초기화
            _connectionFactory = CreateConnectionFactory();
            
            // Repository 등록
            _repositories = new Dictionary<Type, IRepository>();
            RegisterRepositories();
            
            // 데이터베이스 마이그레이션 실행
            await RunMigrationsAsync();
            
            // ServiceLocator에 Repository들 등록
            RegisterServicesInLocator();
            
            Logger.Log(LogLevel.Info, "데이터베이스 시스템 초기화 완료", "Database");
            return true;
        }
        catch (Exception ex)
        {
            Logger.Log(LogLevel.Error, $"데이터베이스 초기화 실패: {ex.Message}", "Database");
            return false;
        }
    }
    
    private IDbConnectionFactory CreateConnectionFactory()
    {
        var dbConfig = ServiceLocator.Instance.Get<DatabaseConfig>();
        
        return dbConfig.DatabaseType switch
        {
            DatabaseType.SQLite => new SQLiteConnectionFactory(dbConfig.ConnectionString),
            DatabaseType.MySQL => new MySQLConnectionFactory(dbConfig.ConnectionString),
            DatabaseType.PostgreSQL => new PostgreSQLConnectionFactory(dbConfig.ConnectionString),
            _ => throw new NotSupportedException($"지원되지 않는 데이터베이스 타입: {dbConfig.DatabaseType}")
        };
    }
    
    private void RegisterRepositories()
    {
        // 게임 관련 Repository
        _repositories[typeof(Player)] = new PlayerRepository(_connectionFactory);
        _repositories[typeof(GameSession)] = new GameSessionRepository(_connectionFactory);
        _repositories[typeof(GameStats)] = new GameStatsRepository(_connectionFactory);
        
        // 설정 관련 Repository
        _repositories[typeof(UserSettings)] = new UserSettingsRepository(_connectionFactory);
        
        // 로그 관련 Repository
        _repositories[typeof(GameLog)] = new GameLogRepository(_connectionFactory);
    }
    
    private async Task RunMigrationsAsync()
    {
        var migrationManager = new DatabaseMigrationManager(_connectionFactory);
        await migrationManager.MigrateAsync();
        
        Logger.Log(LogLevel.Info, "데이터베이스 마이그레이션 완료", "Database");
    }
    
    public T GetRepository<T>() where T : class, IRepository
    {
        var entityType = typeof(T).GetGenericArguments()[0];
        return _repositories.TryGetValue(entityType, out var repository) ? repository as T : null;
    }
}

public interface IRepository<T> : IRepository where T : class
{
    Task<T> GetByIdAsync(object id);
    Task<IEnumerable<T>> GetAllAsync();
    Task<T> AddAsync(T entity);
    Task<T> UpdateAsync(T entity);
    Task<bool> DeleteAsync(object id);
    Task<IEnumerable<T>> FindAsync(Expression<Func<T, bool>> predicate);
}

public class PlayerRepository : Repository<Player>, IPlayerRepository
{
    public PlayerRepository(IDbConnectionFactory connectionFactory) 
        : base(connectionFactory)
    {
    }
    
    public async Task<Player> GetByUsernameAsync(string username)
    {
        const string sql = @"
            SELECT * FROM Players 
            WHERE Username = @Username";
        
        using var connection = _connectionFactory.CreateConnection();
        return await connection.QuerySingleOrDefaultAsync<Player>(sql, new { Username = username });
    }
    
    public async Task<IEnumerable<Player>> GetTopPlayersByScoreAsync(int count)
    {
        const string sql = @"
            SELECT * FROM Players 
            ORDER BY TotalScore DESC 
            LIMIT @Count";
        
        using var connection = _connectionFactory.CreateConnection();
        return await connection.QueryAsync<Player>(sql, new { Count = count });
    }
    
    public async Task<bool> UpdatePlayerStatsAsync(string playerId, GameStats stats)
    {
        const string sql = @"
            UPDATE Players SET 
                TotalScore = TotalScore + @Score,
                GamesPlayed = GamesPlayed + 1,
                Wins = Wins + @Wins,
                LastPlayedAt = @LastPlayedAt
            WHERE Id = @PlayerId";
        
        using var connection = _connectionFactory.CreateConnection();
        var affected = await connection.ExecuteAsync(sql, new 
        { 
            PlayerId = playerId,
            Score = stats.Score,
            Wins = stats.IsWin ? 1 : 0,
            LastPlayedAt = DateTime.UtcNow
        });
        
        return affected > 0;
    }
}

// 사용 예제
public class GameStatsManager : MonoBehaviour
{
    private IPlayerRepository _playerRepository;
    private IGameSessionRepository _sessionRepository;
    
    void Start()
    {
        var dbPlugin = PluginManager.Instance.GetPlugin<DatabasePlugin>("database_system");
        _playerRepository = dbPlugin.GetRepository<IPlayerRepository>();
        _sessionRepository = dbPlugin.GetRepository<IGameSessionRepository>();
        
        // 게임 이벤트 구독
        EventBusOptimized.Instance.Subscribe<GameEndedEvent>(OnGameEnded);
        EventBusOptimized.Instance.Subscribe<PlayerKilledEvent>(OnPlayerKilled);
    }
    
    private async void OnGameEnded(GameEndedEvent eventData)
    {
        try
        {
            // 게임 세션 저장
            var session = new GameSession
            {
                Id = eventData.SessionId,
                GameMode = eventData.GameMode,
                StartTime = eventData.StartTime,
                EndTime = eventData.EndTime,
                PlayerCount = eventData.PlayerStats.Count,
                WinnerId = eventData.WinnerId
            };
            
            await _sessionRepository.AddAsync(session);
            
            // 플레이어 통계 업데이트
            foreach (var playerStats in eventData.PlayerStats)
            {
                await _playerRepository.UpdatePlayerStatsAsync(playerStats.PlayerId, playerStats.Stats);
            }
            
            Log.Info($"게임 통계 저장 완료: {eventData.SessionId}", "Stats");
        }
        catch (Exception ex)
        {
            Log.Error($"게임 통계 저장 실패: {ex.Message}", "Stats");
        }
    }
}
```

## 결론

이 확장 가이드를 통해 Police-Thief 프로젝트의 다양한 확장 방법을 학습할 수 있습니다:

1. **네트워크 프로토콜 확장**: 새로운 통신 프로토콜 추가
2. **게임 모드 확장**: 다양한 게임 모드 구현
3. **UI/UX 시스템 확장**: 새로운 사용자 인터페이스 구성요소
4. **플러그인 시스템**: 모듈화된 기능 확장
5. **플랫폼별 확장**: 다양한 플랫폼 지원
6. **AI 시스템 통합**: 인공지능 기반 기능
7. **데이터베이스 통합**: 영구 데이터 저장

각 확장 방법은 기존 시스템과의 호환성을 유지하면서도 새로운 기능을 쉽게 추가할 수 있도록 설계되었습니다. 모듈러 아키텍처의 장점을 최대한 활용하여 확장 가능하고 유지보수가 용이한 시스템을 구축할 수 있습니다.