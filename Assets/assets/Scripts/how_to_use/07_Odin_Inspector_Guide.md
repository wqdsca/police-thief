# Odin Inspector 사용 가이드

## 개요

Odin Inspector는 Unity의 기본 Inspector를 강력하게 확장하여 개발 효율성을 크게 향상시키는 도구입니다. 다양한 어트리뷰트를 통해 Inspector를 커스터마이징하고, 복잡한 데이터 구조를 시각적으로 관리할 수 있습니다.

## 목차

1. [기본 설정](#기본-설정)
2. [기본 어트리뷰트](#기본-어트리뷰트)
3. [레이아웃 어트리뷰트](#레이아웃-어트리뷰트)
4. [조건부 어트리뷰트](#조건부-어트리뷰트)
5. [검증 어트리뷰트](#검증-어트리뷰트)
6. [버튼과 메서드](#버튼과-메서드)
7. [고급 기능](#고급-기능)
8. [Police-Thief 프로젝트 적용](#police-thief-프로젝트-적용)

---

## 기본 설정

### NetworkConfig에 Odin Inspector 적용

```csharp
using Sirenix.OdinInspector;
using UnityEngine;

[System.Serializable]
public class NetworkConfig
{
    [TabGroup("QUIC")]
    [Title("QUIC Protocol Settings", "고성능 UDP 기반 프로토콜", TitleAlignments.Centered)]
    [BoxGroup("QUIC/Connection")]
    [LabelWidth(120)]
    public string quicHost = "localhost";
    
    [BoxGroup("QUIC/Connection")]
    [Range(1000, 65535)]
    [SuffixLabel("Port")]
    public int quicPort = 5000;
    
    [BoxGroup("QUIC/Connection")]
    [SuffixLabel("ms", true)]
    [Range(1000, 30000)]
    public int connectTimeoutMs = 5000;
    
    [TabGroup("QUIC")]
    [BoxGroup("QUIC/Advanced")]
    [InfoBox("0-RTT 연결을 사용하면 연결 시간을 단축할 수 있습니다.", InfoMessageType.Info)]
    public bool enable0Rtt = true;
    
    [BoxGroup("QUIC/Advanced")]
    [EnableIf("enable0Rtt")]
    public bool enableConnectionMigration = true;
    
    [BoxGroup("QUIC/Advanced")]
    [ShowIf("enableConnectionMigration")]
    [SuffixLabel("ms")]
    [Range(5000, 120000)]
    public int keepAliveIntervalMs = 30000;
    
    [TabGroup("gRPC")]
    [Title("gRPC Settings", "HTTP/2 기반 RPC 프로토콜")]
    [BoxGroup("gRPC/Connection")]
    [ValidateInput("ValidateGrpcEndpoint", "올바른 gRPC 엔드포인트를 입력해주세요")]
    public string grpcEndpoint = "https://localhost:5001";
    
    [BoxGroup("gRPC/Connection")]
    [SuffixLabel("ms")]
    [Range(1000, 60000)]
    public int grpcTimeoutMs = 10000;
    
    [BoxGroup("gRPC/Connection")]
    [InfoBox("재시도 기능을 활성화하면 네트워크 불안정 시 자동으로 재시도합니다.", InfoMessageType.Warning)]
    public bool grpcEnableRetry = true;
    
    [TabGroup("TCP")]
    [Title("TCP Settings", "전통적인 TCP 프로토콜")]
    [BoxGroup("TCP/Connection")]
    public string tcpHost = "localhost";
    
    [BoxGroup("TCP/Connection")]
    [Range(1000, 65535)]
    public int tcpPort = 5002;
    
    [BoxGroup("TCP/Connection")]
    [SuffixLabel("bytes")]
    [Range(1024, 65536)]
    public int tcpBufferSize = 8192;
    
    [FoldoutGroup("Debug")]
    [ReadOnly]
    [ShowInInspector]
    public string QuicEndpoint => $"https://{quicHost}:{quicPort}";
    
    [FoldoutGroup("Debug")]
    [ReadOnly]
    [ShowInInspector]
    public string TcpEndpoint => $"{tcpHost}:{tcpPort}";
    
    [Button("Test All Connections", ButtonSizes.Large)]
    [GUIColor(0.4f, 0.8f, 1f)]
    public async void TestConnections()
    {
        Debug.Log("연결 테스트 시작...");
        
        // QUIC 연결 테스트
        var quicTest = await TestQuicConnection();
        Debug.Log($"QUIC 연결: {(quicTest ? "성공" : "실패")}");
        
        // gRPC 연결 테스트
        var grpcTest = await TestGrpcConnection();
        Debug.Log($"gRPC 연결: {(grpcTest ? "성공" : "실패")}");
        
        // TCP 연결 테스트
        var tcpTest = await TestTcpConnection();
        Debug.Log($"TCP 연결: {(tcpTest ? "성공" : "실패")}");
    }
    
    [FoldoutGroup("Advanced")]
    [Button("Reset to Default")]
    [GUIColor(1f, 0.6f, 0.6f)]
    public void ResetToDefault()
    {
        quicHost = "localhost";
        quicPort = 5000;
        connectTimeoutMs = 5000;
        enable0Rtt = true;
        enableConnectionMigration = true;
        keepAliveIntervalMs = 30000;
        
        grpcEndpoint = "https://localhost:5001";
        grpcTimeoutMs = 10000;
        grpcEnableRetry = true;
        
        tcpHost = "localhost";
        tcpPort = 5002;
        tcpBufferSize = 8192;
        
        Debug.Log("설정이 기본값으로 초기화되었습니다.");
    }
    
    private bool ValidateGrpcEndpoint(string endpoint)
    {
        return System.Uri.TryCreate(endpoint, System.UriKind.Absolute, out var uri) && 
               (uri.Scheme == "http" || uri.Scheme == "https");
    }
    
    private async System.Threading.Tasks.Task<bool> TestQuicConnection()
    {
        // QUIC 연결 테스트 로직
        await System.Threading.Tasks.Task.Delay(1000);
        return true; // 임시
    }
    
    private async System.Threading.Tasks.Task<bool> TestGrpcConnection()
    {
        // gRPC 연결 테스트 로직
        await System.Threading.Tasks.Task.Delay(1000);
        return true; // 임시
    }
    
    private async System.Threading.Tasks.Task<bool> TestTcpConnection()
    {
        // TCP 연결 테스트 로직
        await System.Threading.Tasks.Task.Delay(1000);
        return true; // 임시
    }
}
```

---

## 기본 어트리뷰트

### 입력 검증 및 제한

```csharp
using Sirenix.OdinInspector;

public class PlayerSettings : MonoBehaviour
{
    [Title("Player Configuration")]
    
    [Required("플레이어 이름은 필수입니다")]
    [ValidateInput("ValidatePlayerName", "플레이어 이름은 3-20자여야 합니다")]
    public string playerName = "";
    
    [Range(1, 100)]
    [SuffixLabel("Level", true)]
    public int playerLevel = 1;
    
    [MinValue(0)]
    [MaxValue(1000)]
    [SuffixLabel("HP")]
    public float maxHealth = 100f;
    
    [MinMaxSlider(0, 100, true)]
    [SuffixLabel("%")]
    public Vector2 damageRange = new Vector2(10, 25);
    
    [PropertyRange(0, "maxHealth")]
    public float currentHealth = 100f;
    
    [Unit(Units.Second)]
    [Range(0.1f, 10f)]
    public float respawnTime = 3f;
    
    [Unit(Units.MetersPerSecond)]
    public float movementSpeed = 5f;
    
    [Multiline(3)]
    [InfoBox("플레이어에 대한 설명을 입력하세요")]
    public string description = "";
    
    [ColorUsage(true, true)] // HDR 지원
    public Color playerColor = Color.white;
    
    [AssetsOnly]
    [Required]
    public GameObject playerPrefab;
    
    [SceneObjectsOnly]
    public Transform spawnPoint;
    
    private bool ValidatePlayerName(string name)
    {
        return !string.IsNullOrEmpty(name) && name.Length >= 3 && name.Length <= 20;
    }
}
```

### 컬렉션과 딕셔너리

```csharp
public class GameItemDatabase : MonoBehaviour
{
    [Title("Game Items Database")]
    
    [TableList(ShowIndexLabels = true)]
    [InfoBox("아이템 목록을 테이블 형태로 관리합니다")]
    public List<GameItem> items = new List<GameItem>();
    
    [DictionaryDrawerSettings(DisplayMode = DictionaryDisplayOptions.OneLine)]
    [InfoBox("아이템 ID로 빠른 검색이 가능합니다")]
    public Dictionary<string, GameItem> itemLookup = new Dictionary<string, GameItem>();
    
    [ListDrawerSettings(
        NumberOfItemsPerPage = 5,
        ShowIndexLabels = true,
        ShowPaging = true,
        ShowItemCount = true,
        DraggableItems = true
    )]
    public List<WeaponConfig> weapons = new List<WeaponConfig>();
    
    [TableMatrix(HorizontalTitle = "Damage Types", VerticalTitle = "Armor Types")]
    public float[,] damageMatrix = new float[4, 4];
    
    [Button("Populate Sample Data")]
    [GUIColor(0.7f, 1f, 0.7f)]
    public void PopulateSampleData()
    {
        items.Clear();
        items.AddRange(new[]
        {
            new GameItem { id = "pistol", name = "권총", damage = 25, price = 500 },
            new GameItem { id = "rifle", name = "소총", damage = 45, price = 1200 },
            new GameItem { id = "shotgun", name = "산탄총", damage = 60, price = 800 }
        });
        
        // Dictionary 자동 생성
        itemLookup = items.ToDictionary(item => item.id);
        
        Debug.Log($"샘플 데이터 {items.Count}개 생성 완료");
    }
}

[System.Serializable]
public class GameItem
{
    [TableColumnWidth(80)]
    [ReadOnly]
    public string id;
    
    [TableColumnWidth(150)]
    public string name;
    
    [TableColumnWidth(80)]
    [SuffixLabel("dmg")]
    public int damage;
    
    [TableColumnWidth(80)]
    [SuffixLabel("$")]
    public int price;
    
    [TableColumnWidth(60)]
    [PreviewField(60)]
    public Sprite icon;
    
    [TableColumnWidth(120)]
    [EnumToggleButtons]
    public ItemRarity rarity;
}

public enum ItemRarity
{
    Common,
    Rare,
    Epic,
    Legendary
}
```

---

## 레이아웃 어트리뷰트

### 그룹화와 탭

```csharp
public class NetworkManager : MonoBehaviour
{
    [TabGroup("Connection")]
    [BoxGroup("Connection/Server")]
    [LabelWidth(100)]
    public string serverAddress = "127.0.0.1";
    
    [BoxGroup("Connection/Server")]
    [Range(1000, 65535)]
    public int serverPort = 7777;
    
    [TabGroup("Connection")]
    [BoxGroup("Connection/Client")]
    public int maxConnections = 100;
    
    [BoxGroup("Connection/Client")]
    [SuffixLabel("seconds")]
    public float connectionTimeout = 10f;
    
    [TabGroup("Security")]
    [ToggleGroup("useEncryption")]
    public bool useEncryption = true;
    
    [TabGroup("Security")]
    [ToggleGroup("useEncryption")]
    [Indent]
    public string encryptionKey = "default-key";
    
    [TabGroup("Security")]
    [ToggleGroup("useEncryption")]
    [Indent]
    [Range(128, 4096)]
    public int keyLength = 256;
    
    [TabGroup("Debug")]
    [FoldoutGroup("Debug/Network Stats", expanded: false)]
    [ReadOnly]
    [ShowInInspector]
    public float BytesSent { get; private set; }
    
    [FoldoutGroup("Debug/Network Stats")]
    [ReadOnly]
    [ShowInInspector]
    public float BytesReceived { get; private set; }
    
    [FoldoutGroup("Debug/Network Stats")]
    [ReadOnly]
    [ShowInInspector]
    [DisplayAsString]
    public string NetworkStatus => IsConnected ? "연결됨" : "연결 안됨";
    
    [FoldoutGroup("Debug/Actions")]
    [Button("Start Server", ButtonSizes.Large)]
    [GUIColor(0.7f, 1f, 0.7f)]
    public void StartServer()
    {
        Debug.Log("서버 시작");
    }
    
    [FoldoutGroup("Debug/Actions")]
    [Button("Connect to Server")]
    [EnableIf("@!IsConnected")]
    public void ConnectToServer()
    {
        Debug.Log("서버에 연결 중...");
    }
    
    [FoldoutGroup("Debug/Actions")]
    [Button("Disconnect")]
    [EnableIf("IsConnected")]
    [GUIColor(1f, 0.7f, 0.7f)]
    public void Disconnect()
    {
        Debug.Log("연결 해제");
    }
    
    [ShowInInspector]
    [ReadOnly]
    private bool IsConnected => Application.isPlaying; // 예시
}
```

### 수평/수직 그룹

```csharp
public class UILayoutExample : MonoBehaviour
{
    [Title("UI Layout Configuration")]
    
    [HorizontalGroup("Row1")]
    [BoxGroup("Row1/Position")]
    [LabelWidth(50)]
    public float posX = 0f;
    
    [BoxGroup("Row1/Position")]
    [LabelWidth(50)]
    public float posY = 0f;
    
    [HorizontalGroup("Row1")]
    [BoxGroup("Row1/Size")]
    [LabelWidth(50)]
    public float width = 100f;
    
    [BoxGroup("Row1/Size")]
    [LabelWidth(50)]
    public float height = 100f;
    
    [HorizontalGroup("Row2", 0.5f)]
    [VerticalGroup("Row2/Left")]
    [BoxGroup("Row2/Left/Colors")]
    public Color primaryColor = Color.white;
    
    [BoxGroup("Row2/Left/Colors")]
    public Color secondaryColor = Color.gray;
    
    [VerticalGroup("Row2/Right")]
    [BoxGroup("Row2/Right/Settings")]
    public bool isActive = true;
    
    [BoxGroup("Row2/Right/Settings")]
    public bool isVisible = true;
    
    [Space(10)]
    [HorizontalGroup("Buttons")]
    [Button("Apply Settings", ButtonSizes.Medium)]
    [GUIColor(0.8f, 1f, 0.8f)]
    public void ApplySettings()
    {
        Debug.Log("설정 적용됨");
    }
    
    [HorizontalGroup("Buttons")]
    [Button("Reset", ButtonSizes.Medium)]
    [GUIColor(1f, 0.8f, 0.8f)]
    public void ResetSettings()
    {
        posX = posY = 0f;
        width = height = 100f;
        primaryColor = Color.white;
        secondaryColor = Color.gray;
        isActive = isVisible = true;
        Debug.Log("설정 초기화됨");
    }
}
```

---

## 조건부 어트리뷰트

### 조건부 표시/활성화

```csharp
public class WeaponSystem : MonoBehaviour
{
    [Title("Weapon Configuration")]
    
    [EnumToggleButtons]
    public WeaponType weaponType = WeaponType.Pistol;
    
    [ShowIf("weaponType", WeaponType.Pistol)]
    [BoxGroup("Pistol Settings")]
    [Range(10, 50)]
    public int pistolDamage = 25;
    
    [ShowIf("weaponType", WeaponType.Pistol)]
    [BoxGroup("Pistol Settings")]
    [Range(0.1f, 2f)]
    public float pistolFireRate = 0.5f;
    
    [ShowIf("weaponType", WeaponType.Rifle)]
    [BoxGroup("Rifle Settings")]
    [Range(30, 100)]
    public int rifleDamage = 45;
    
    [ShowIf("weaponType", WeaponType.Rifle)]
    [BoxGroup("Rifle Settings")]
    [Range(0.05f, 0.3f)]
    public float rifleFireRate = 0.1f;
    
    [ShowIf("weaponType", WeaponType.Rifle)]
    [BoxGroup("Rifle Settings")]
    public bool hasScope = false;
    
    [ShowIf("@weaponType == WeaponType.Rifle && hasScope")]
    [BoxGroup("Rifle Settings")]
    [Range(2f, 8f)]
    public float scopeZoom = 4f;
    
    [Space(10)]
    [InfoBox("자동 사격 모드를 활성화하면 연사가 가능합니다", InfoMessageType.Info)]
    public bool autoFire = false;
    
    [EnableIf("autoFire")]
    [Range(50, 1000)]
    [SuffixLabel("RPM")]
    public int roundsPerMinute = 600;
    
    [DisableIf("@weaponType == WeaponType.Shotgun")]
    [Range(1, 10)]
    public int magazineSize = 30;
    
    [HideIf("@weaponType == WeaponType.Melee")]
    [Range(10f, 100f)]
    [SuffixLabel("meters")]
    public float range = 50f;
    
    [ShowIf("@weaponType != WeaponType.Melee && range > 75f")]
    [InfoBox("장거리 무기는 정확도가 중요합니다", InfoMessageType.Warning)]
    [Range(0.1f, 1f)]
    public float accuracy = 0.8f;
    
    [Button("Calculate DPS")]
    [InfoBox("@CalculateDPS().ToString(\"F1\") + \" DPS\"", InfoMessageType.None)]
    public float CalculateDPS()
    {
        return weaponType switch
        {
            WeaponType.Pistol => pistolDamage / pistolFireRate,
            WeaponType.Rifle => rifleDamage / rifleFireRate,
            WeaponType.Shotgun => 80f / 1.5f, // 예시
            WeaponType.Melee => 120f / 2f,    // 예시
            _ => 0f
        };
    }
}

public enum WeaponType
{
    Pistol,
    Rifle,
    Shotgun,
    Melee
}
```

---

## 검증 어트리뷰트

### 복합 검증 시스템

```csharp
public class GameBalance : MonoBehaviour
{
    [Title("Game Balance Configuration")]
    
    [ValidateInput("ValidatePlayerHealth", "체력은 0보다 커야 합니다")]
    [Range(1, 1000)]
    public float playerMaxHealth = 100f;
    
    [ValidateInput("ValidateHealthRegen", "체력 재생은 최대 체력의 10%를 넘을 수 없습니다")]
    [Range(0, 50)]
    public float healthRegenPerSecond = 2f;
    
    [Required("무기 데미지 목록은 비어있을 수 없습니다")]
    [ListDrawerSettings(ShowItemCount = true)]
    [ValidateInput("ValidateWeaponDamages", "모든 무기 데미지는 0보다 커야 합니다")]
    public List<float> weaponDamages = new List<float>();
    
    [AssetsOnly]
    [Required("플레이어 프리팹은 필수입니다")]
    [ValidateInput("ValidatePlayerPrefab", "플레이어 프리팹에는 Player 컴포넌트가 있어야 합니다")]
    public GameObject playerPrefab;
    
    [FilePath(Extensions = "json,xml")]
    [InfoBox("설정 파일 경로를 지정하세요")]
    public string configFilePath = "";
    
    [FolderPath]
    public string saveDataFolder = "";
    
    [ValueDropdown("GetAvailableLevels")]
    [InfoBox("드롭다운에서 레벨을 선택하세요")]
    public string selectedLevel = "";
    
    [ValueDropdown("GetDifficultyOptions")]
    public DifficultyLevel difficulty = DifficultyLevel.Normal;
    
    [OnValueChanged("OnBalanceChanged")]
    [InfoBox("밸런스가 변경되면 자동으로 재계산됩니다", InfoMessageType.Info)]
    public bool autoRecalculateBalance = true;
    
    // 검증 메서드들
    private bool ValidatePlayerHealth(float health)
    {
        return health > 0;
    }
    
    private bool ValidateHealthRegen(float regen)
    {
        return regen <= playerMaxHealth * 0.1f;
    }
    
    private bool ValidateWeaponDamages(List<float> damages)
    {
        return damages.All(damage => damage > 0);
    }
    
    private bool ValidatePlayerPrefab(GameObject prefab)
    {
        return prefab != null && prefab.GetComponent<Player>() != null;
    }
    
    private IEnumerable<string> GetAvailableLevels()
    {
        return new[] { "Level_01", "Level_02", "Level_03", "Boss_Level" };
    }
    
    private IEnumerable<ValueDropdownItem<DifficultyLevel>> GetDifficultyOptions()
    {
        return new[]
        {
            new ValueDropdownItem<DifficultyLevel>("쉬움", DifficultyLevel.Easy),
            new ValueDropdownItem<DifficultyLevel>("보통", DifficultyLevel.Normal),
            new ValueDropdownItem<DifficultyLevel>("어려움", DifficultyLevel.Hard),
            new ValueDropdownItem<DifficultyLevel>("극한", DifficultyLevel.Extreme)
        };
    }
    
    private void OnBalanceChanged()
    {
        if (autoRecalculateBalance)
        {
            RecalculateBalance();
        }
    }
    
    [Button("Recalculate Balance", ButtonSizes.Large)]
    [GUIColor(0.8f, 0.8f, 1f)]
    public void RecalculateBalance()
    {
        Debug.Log("게임 밸런스 재계산 중...");
        
        // 밸런스 계산 로직
        float totalDPS = weaponDamages.Sum();
        float survivalTime = playerMaxHealth / (totalDPS * 0.1f); // 예시 계산
        
        Debug.Log($"총 DPS: {totalDPS:F1}, 예상 생존 시간: {survivalTime:F1}초");
    }
}

public enum DifficultyLevel
{
    Easy,
    Normal,
    Hard,
    Extreme
}
```

---

## 버튼과 메서드

### 인터랙티브 Inspector

```csharp
public class GameDebugTools : MonoBehaviour
{
    [Title("Game Debug Tools", "개발 및 디버깅을 위한 도구들", TitleAlignments.Centered)]
    
    [BoxGroup("Player Controls")]
    [Button("Spawn Player", ButtonSizes.Large)]
    [GUIColor(0.7f, 1f, 0.7f)]
    public void SpawnPlayer()
    {
        Debug.Log("플레이어 스폰");
        // 플레이어 스폰 로직
    }
    
    [BoxGroup("Player Controls")]
    [Button("Kill Player")]
    [GUIColor(1f, 0.7f, 0.7f)]
    [EnableIf("@Application.isPlaying")]
    public void KillPlayer()
    {
        Debug.Log("플레이어 사망");
        // 플레이어 사망 처리
    }
    
    [BoxGroup("Player Controls")]
    [HorizontalGroup("Player Controls/Row1")]
    [Button("Heal")]
    [GUIColor(0.7f, 1f, 0.7f)]
    public void HealPlayer()
    {
        Debug.Log("플레이어 치료");
    }
    
    [HorizontalGroup("Player Controls/Row1")]
    [Button("Damage")]
    [GUIColor(1f, 1f, 0.7f)]
    public void DamagePlayer()
    {
        Debug.Log("플레이어 데미지");
    }
    
    [BoxGroup("Game Controls")]
    [Button("Start Game")]
    [DisableIf("@Application.isPlaying && IsGameRunning")]
    public void StartGame()
    {
        Debug.Log("게임 시작");
        IsGameRunning = true;
    }
    
    [BoxGroup("Game Controls")]
    [Button("Pause/Resume Game")]
    [EnableIf("IsGameRunning")]
    public void ToggleGamePause()
    {
        Time.timeScale = Time.timeScale > 0 ? 0 : 1;
        Debug.Log($"게임 {(Time.timeScale > 0 ? "재개" : "일시정지")}");
    }
    
    [BoxGroup("Game Controls")]
    [Button("Stop Game")]
    [EnableIf("IsGameRunning")]
    [GUIColor(1f, 0.6f, 0.6f)]
    public void StopGame()
    {
        Debug.Log("게임 정지");
        IsGameRunning = false;
        Time.timeScale = 1;
    }
    
    [BoxGroup("Utility")]
    [Button("Clear Console")]
    public void ClearConsole()
    {
        var logEntries = System.Type.GetType("UnityEditor.LogEntries,UnityEditor.dll");
        var clearMethod = logEntries?.GetMethod("Clear", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public);
        clearMethod?.Invoke(null, null);
    }
    
    [BoxGroup("Utility")]
    [Button("Take Screenshot")]
    [FolderPath]
    public string screenshotPath = "Screenshots";
    
    [BoxGroup("Utility")]
    [Button("Capture")]
    [HorizontalGroup("Utility/Screenshot")]
    public void TakeScreenshot()
    {
        if (!System.IO.Directory.Exists(screenshotPath))
        {
            System.IO.Directory.CreateDirectory(screenshotPath);
        }
        
        string fileName = $"Screenshot_{System.DateTime.Now:yyyyMMdd_HHmmss}.png";
        string fullPath = System.IO.Path.Combine(screenshotPath, fileName);
        
        ScreenCapture.CaptureScreenshot(fullPath);
        Debug.Log($"스크린샷 저장: {fullPath}");
    }
    
    [BoxGroup("Network Debug")]
    [Button("Simulate Network Lag")]
    [PropertyRange(0, 1000)]
    public int simulatedLag = 100;
    
    [BoxGroup("Network Debug")]
    [Button("Apply Lag")]
    [HorizontalGroup("Network Debug/Lag")]
    public void ApplyNetworkLag()
    {
        Debug.Log($"네트워크 지연 {simulatedLag}ms 적용");
        // 네트워크 지연 시뮬레이션 로직
    }
    
    [HorizontalGroup("Network Debug/Lag")]
    [Button("Reset")]
    public void ResetNetworkLag()
    {
        simulatedLag = 0;
        Debug.Log("네트워크 지연 초기화");
    }
    
    // 메서드 매개변수를 가진 버튼
    [Button("Add Score")]
    public void AddScore(
        [InfoBox("점수를 입력하세요")] int score = 100,
        [InfoBox("플레이어 이름")] string playerName = "Player1"
    )
    {
        Debug.Log($"{playerName}에게 {score}점 추가");
    }
    
    [ShowInInspector]
    [ReadOnly]
    public bool IsGameRunning { get; private set; }
    
    [ShowInInspector]
    [ReadOnly]
    [DisplayAsString]
    public string GameStatus => IsGameRunning ? "실행 중" : "정지됨";
    
    [ShowInInspector]
    [ReadOnly]
    [ProgressBar(0, 100, ColorGetter = "GetHealthBarColor")]
    public float PlayerHealth { get; set; } = 100f;
    
    private Color GetHealthBarColor(float value)
    {
        return Color.Lerp(Color.red, Color.green, value / 100f);
    }
}
```

---

## 고급 기능

### 커스텀 드로어와 속성

```csharp
using Sirenix.OdinInspector;

public class AdvancedInspectorFeatures : MonoBehaviour
{
    [Title("Advanced Odin Inspector Features")]
    
    [PreviewField(100, ObjectFieldAlignment.Left)]
    [HorizontalGroup("Preview")]
    public Texture2D profileImage;
    
    [HorizontalGroup("Preview")]
    [VerticalGroup("Preview/Info")]
    [LabelWidth(80)]
    public string playerName = "";
    
    [VerticalGroup("Preview/Info")]
    [LabelWidth(80)]
    [ReadOnly]
    public int playerLevel = 1;
    
    [ProgressBar(0, 1000, ColorGetter = "GetXPColor")]
    [LabelText("Experience")]
    public float experience = 250f;
    
    [PropertySpace(20)]
    
    [SearchableEnum]
    [InfoBox("검색 가능한 Enum입니다. 많은 옵션이 있을 때 유용합니다.")]
    public KeyCode primaryAttackKey = KeyCode.Mouse0;
    
    [PropertySpace(10)]
    
    [InlineEditor(InlineEditorModes.FullEditor)]
    [InfoBox("인라인 에디터로 ScriptableObject를 바로 편집할 수 있습니다")]
    public PlayerData playerData;
    
    [PropertySpace(10)]
    
    [ShowInInspector]
    [TableMatrix(SquareCells = true, DrawElementMethod = "DrawColoredEnumElement")]
    [InfoBox("색상으로 구분되는 매트릭스입니다")]
    public TerrainType[,] terrainMap = new TerrainType[5, 5];
    
    [PropertySpace(10)]
    
    [OnCollectionChanged("OnInventoryChanged")]
    [ListDrawerSettings(
        ShowItemCount = true,
        ShowPaging = true,
        NumberOfItemsPerPage = 10,
        CustomAddFunction = "AddNewItem"
    )]
    public List<InventoryItem> inventory = new List<InventoryItem>();
    
    [PropertySpace(10)]
    
    [ShowInInspector]
    [DictionaryDrawerSettings(
        KeyLabel = "Stat Type",
        ValueLabel = "Value",
        DisplayMode = DictionaryDisplayOptions.Foldout
    )]
    [InfoBox("플레이어 스탯을 동적으로 관리할 수 있습니다")]
    public Dictionary<StatType, float> playerStats = new Dictionary<StatType, float>
    {
        { StatType.Strength, 10 },
        { StatType.Agility, 8 },
        { StatType.Intelligence, 12 },
        { StatType.Luck, 5 }
    };
    
    [PropertySpace(20)]
    [Title("Custom Validation & Info")]
    
    [MultiLineProperty(5)]
    [InfoBox("$GetInventoryInfo", InfoMessageType.Info, "HasInventoryItems")]
    public string inventoryNotes = "";
    
    [ShowInInspector]
    [ProgressBar(0, "@inventory.Count", ColorGetter = "GetInventoryProgressColor")]
    [LabelText("Inventory Fullness")]
    public int InventoryCount => inventory.Count;
    
    // 커스텀 메서드들
    private Color GetXPColor(float value)
    {
        return Color.Lerp(Color.blue, Color.yellow, value / 1000f);
    }
    
    private Color GetInventoryProgressColor(float value)
    {
        float maxSlots = 50f; // 최대 인벤토리 슬롯
        float ratio = value / maxSlots;
        
        if (ratio < 0.7f) return Color.green;
        if (ratio < 0.9f) return Color.yellow;
        return Color.red;
    }
    
    private TerrainType DrawColoredEnumElement(Rect rect, TerrainType value)
    {
        if (Event.current.type == EventType.MouseDown && rect.Contains(Event.current.mousePosition))
        {
            value = (TerrainType)(((int)value + 1) % System.Enum.GetValues(typeof(TerrainType)).Length);
            GUI.changed = true;
            Event.current.Use();
        }
        
        var color = value switch
        {
            TerrainType.Grass => Color.green,
            TerrainType.Water => Color.blue,
            TerrainType.Mountain => Color.gray,
            TerrainType.Desert => Color.yellow,
            _ => Color.white
        };
        
        UnityEditor.EditorGUI.DrawRect(rect, color);
        return value;
    }
    
    private InventoryItem AddNewItem()
    {
        return new InventoryItem
        {
            itemName = "New Item",
            quantity = 1,
            rarity = ItemRarity.Common
        };
    }
    
    private void OnInventoryChanged()
    {
        Debug.Log($"인벤토리 변경됨. 현재 아이템 수: {inventory.Count}");
    }
    
    private string GetInventoryInfo()
    {
        if (inventory.Count == 0)
            return "인벤토리가 비어있습니다.";
        
        var rarityCount = inventory.GroupBy(item => item.rarity)
                                 .ToDictionary(g => g.Key, g => g.Count());
        
        return $"총 {inventory.Count}개 아이템 - " +
               string.Join(", ", rarityCount.Select(kvp => $"{kvp.Key}: {kvp.Value}"));
    }
    
    private bool HasInventoryItems()
    {
        return inventory.Count > 0;
    }
    
    [Button("Randomize Terrain")]
    [GUIColor(0.8f, 0.8f, 1f)]
    public void RandomizeTerrain()
    {
        var terrainTypes = System.Enum.GetValues(typeof(TerrainType)).Cast<TerrainType>().ToArray();
        
        for (int x = 0; x < terrainMap.GetLength(0); x++)
        {
            for (int y = 0; y < terrainMap.GetLength(1); y++)
            {
                terrainMap[x, y] = terrainTypes[UnityEngine.Random.Range(0, terrainTypes.Length)];
            }
        }
        
        Debug.Log("지형 맵이 랜덤으로 생성되었습니다.");
    }
    
    [Button("Add Random Items")]
    public void AddRandomItems()
    {
        var itemNames = new[] { "검", "방패", "물약", "화살", "보석" };
        var rarities = System.Enum.GetValues(typeof(ItemRarity)).Cast<ItemRarity>().ToArray();
        
        for (int i = 0; i < 5; i++)
        {
            inventory.Add(new InventoryItem
            {
                itemName = itemNames[UnityEngine.Random.Range(0, itemNames.Length)],
                quantity = UnityEngine.Random.Range(1, 10),
                rarity = rarities[UnityEngine.Random.Range(0, rarities.Length)]
            });
        }
        
        Debug.Log("랜덤 아이템 5개가 추가되었습니다.");
    }
}

[System.Serializable]
public class InventoryItem
{
    [HorizontalGroup("Item")]
    [LabelWidth(60)]
    public string itemName;
    
    [HorizontalGroup("Item")]
    [LabelWidth(60)]
    [Range(1, 999)]
    public int quantity;
    
    [HorizontalGroup("Item")]
    [LabelWidth(60)]
    [EnumToggleButtons]
    public ItemRarity rarity;
}

public enum TerrainType
{
    Grass,
    Water,
    Mountain,
    Desert
}

public enum StatType
{
    Strength,
    Agility,
    Intelligence,
    Luck
}

[CreateAssetMenu(fileName = "PlayerData", menuName = "Game/Player Data")]
public class PlayerData : ScriptableObject
{
    [Title("Player Profile")]
    
    [PreviewField(80)]
    public Sprite avatar;
    
    [Required]
    public string displayName;
    
    [TextArea(3, 5)]
    public string biography;
    
    [Title("Stats")]
    [Range(1, 100)]
    public int level = 1;
    
    [ProgressBar(0, 1000)]
    public int experience = 0;
    
    [MinMaxSlider(0, 100, true)]
    public Vector2 statRange = new Vector2(10, 90);
}
```

---

## Police-Thief 프로젝트 적용

### NetworkConnectionManager에 Odin Inspector 적용

```csharp
using Sirenix.OdinInspector;

public class NetworkConnectionManager : MonoBehaviour, INetworkManager
{
    [Title("Network Connection Manager", "네트워크 연결 관리 시스템", TitleAlignments.Centered)]
    
    [TabGroup("Configuration")]
    [Required]
    [InlineEditor]
    public NetworkConfig networkConfig;
    
    [TabGroup("Configuration")]
    [BoxGroup("Configuration/Pool Settings")]
    [Range(1, 20)]
    [InfoBox("연결 풀 크기를 설정합니다. 높을수록 성능이 좋지만 메모리를 더 사용합니다.")]
    public int poolSize = 5;
    
    [BoxGroup("Configuration/Pool Settings")]
    [SuffixLabel("seconds")]
    [Range(5, 60)]
    public int poolCleanupInterval = 30;
    
    [TabGroup("Status")]
    [ShowInInspector]
    [ReadOnly]
    [LabelText("Current Protocol")]
    [DisplayAsString]
    public NetworkProtocol CurrentProtocol { get; private set; }
    
    [TabGroup("Status")]
    [ShowInInspector]
    [ReadOnly]
    [ProgressBar(0, 100, ColorGetter = "GetConnectionQualityColor")]
    [LabelText("Connection Quality")]
    public float ConnectionQuality { get; private set; } = 100f;
    
    [TabGroup("Status")]
    [ShowInInspector]
    [ReadOnly]
    [TableList(ShowIndexLabels = true)]
    [ListDrawerSettings(ShowItemCount = true, ShowPaging = false)]
    public List<ConnectionStatus> ActiveConnections { get; private set; } = new();
    
    [TabGroup("Debug")]
    [FoldoutGroup("Debug/Network Statistics", expanded: true)]
    [ShowInInspector]
    [ReadOnly]
    [DisplayAsString]
    public string NetworkStats => GetNetworkStatsString();
    
    [FoldoutGroup("Debug/Network Statistics")]
    [ShowInInspector]
    [ReadOnly]
    [ProgressBar(0, 1000, ColorGetter = "GetLatencyColor")]
    [LabelText("Average Latency (ms)")]
    public float AverageLatency { get; private set; }
    
    [FoldoutGroup("Debug/Debug Controls")]
    [Button("Test All Protocols", ButtonSizes.Large)]
    [GUIColor(0.7f, 0.9f, 1f)]
    public async void TestAllProtocols()
    {
        Debug.Log("모든 프로토콜 연결 테스트 시작...");
        
        var protocols = new[] { NetworkProtocol.QUIC, NetworkProtocol.GRPC, NetworkProtocol.TCP };
        
        foreach (var protocol in protocols)
        {
            try
            {
                var success = await ConnectAsync(protocol);
                Debug.Log($"{protocol} 연결: {(success ? "성공" : "실패")}");
            }
            catch (System.Exception ex)
            {
                Debug.LogError($"{protocol} 연결 오류: {ex.Message}");
            }
        }
    }
    
    [FoldoutGroup("Debug/Debug Controls")]
    [HorizontalGroup("Debug/Debug Controls/Row1")]
    [Button("Connect QUIC")]
    [GUIColor(0.8f, 1f, 0.8f)]
    [EnableIf("@!IsConnected(NetworkProtocol.QUIC)")]
    public async void ConnectQuic()
    {
        await ConnectAsync(NetworkProtocol.QUIC);
    }
    
    [HorizontalGroup("Debug/Debug Controls/Row1")]
    [Button("Connect gRPC")]
    [GUIColor(0.8f, 1f, 0.8f)]
    [EnableIf("@!IsConnected(NetworkProtocol.GRPC)")]
    public async void ConnectGrpc()
    {
        await ConnectAsync(NetworkProtocol.GRPC);
    }
    
    [HorizontalGroup("Debug/Debug Controls/Row1")]
    [Button("Connect TCP")]
    [GUIColor(0.8f, 1f, 0.8f)]
    [EnableIf("@!IsConnected(NetworkProtocol.TCP)")]
    public async void ConnectTcp()
    {
        await ConnectAsync(NetworkProtocol.TCP);
    }
    
    [FoldoutGroup("Debug/Debug Controls")]
    [Button("Disconnect All", ButtonSizes.Medium)]
    [GUIColor(1f, 0.8f, 0.8f)]
    [EnableIf("@ActiveConnections.Count > 0")]
    public async void DisconnectAll()
    {
        Debug.Log("모든 연결 해제 중...");
        await DisconnectAllAsync();
    }
    
    [FoldoutGroup("Debug/Debug Controls")]
    [Button("Clear Statistics")]
    public void ClearStatistics()
    {
        // 통계 초기화 로직
        AverageLatency = 0f;
        ConnectionQuality = 100f;
        Debug.Log("네트워크 통계가 초기화되었습니다.");
    }
    
    [FoldoutGroup("Debug/Simulation")]
    [InfoBox("테스트를 위한 네트워크 상태 시뮬레이션", InfoMessageType.Info)]
    [Range(0, 1000)]
    [OnValueChanged("OnLatencyChanged")]
    public float simulatedLatency = 0f;
    
    [FoldoutGroup("Debug/Simulation")]
    [Range(0, 100)]
    [SuffixLabel("%")]
    [OnValueChanged("OnPacketLossChanged")]
    public float simulatedPacketLoss = 0f;
    
    private Color GetConnectionQualityColor(float value)
    {
        if (value >= 80) return Color.green;
        if (value >= 60) return Color.yellow;
        if (value >= 40) return new Color(1f, 0.5f, 0f); // Orange
        return Color.red;
    }
    
    private Color GetLatencyColor(float value)
    {
        if (value <= 50) return Color.green;
        if (value <= 100) return Color.yellow;
        if (value <= 200) return new Color(1f, 0.5f, 0f); // Orange
        return Color.red;
    }
    
    private string GetNetworkStatsString()
    {
        return $"Active: {ActiveConnections.Count} | Protocol: {CurrentProtocol} | Quality: {ConnectionQuality:F1}%";
    }
    
    private bool IsConnected(NetworkProtocol protocol)
    {
        return ActiveConnections.Any(conn => conn.Protocol == protocol && conn.IsConnected);
    }
    
    private void OnLatencyChanged()
    {
        AverageLatency = simulatedLatency;
        UpdateConnectionQuality();
    }
    
    private void OnPacketLossChanged()
    {
        UpdateConnectionQuality();
    }
    
    private void UpdateConnectionQuality()
    {
        // 간단한 연결 품질 계산
        float latencyPenalty = Mathf.Min(AverageLatency / 10f, 50f);
        float packetLossPenalty = simulatedPacketLoss * 2f;
        
        ConnectionQuality = Mathf.Max(0f, 100f - latencyPenalty - packetLossPenalty);
    }
    
    // INetworkManager 인터페이스 구현
    public async Task<bool> ConnectAsync(NetworkProtocol protocol)
    {
        // 실제 연결 로직은 기존 코드 사용
        // 여기서는 시뮬레이션만 추가
        
        await Task.Delay(1000); // 연결 시뮬레이션
        
        var connectionStatus = new ConnectionStatus
        {
            Protocol = protocol,
            IsConnected = true,
            ConnectedAt = System.DateTime.Now,
            EndpointUrl = GetEndpointUrl(protocol)
        };
        
        ActiveConnections.Add(connectionStatus);
        CurrentProtocol = protocol;
        
        return true;
    }
    
    private string GetEndpointUrl(NetworkProtocol protocol)
    {
        return protocol switch
        {
            NetworkProtocol.QUIC => networkConfig.GetQuicEndpoint(),
            NetworkProtocol.GRPC => networkConfig.grpcEndpoint,
            NetworkProtocol.TCP => networkConfig.GetTcpEndpoint(),
            _ => "Unknown"
        };
    }
    
    private async Task DisconnectAllAsync()
    {
        ActiveConnections.Clear();
        CurrentProtocol = NetworkProtocol.QUIC; // 기본값
        await Task.Delay(500); // 해제 시뮬레이션
    }
}

[System.Serializable]
public class ConnectionStatus
{
    [ReadOnly]
    [TableColumnWidth(80)]
    public NetworkProtocol Protocol;
    
    [ReadOnly]
    [TableColumnWidth(60)]
    public bool IsConnected;
    
    [ReadOnly]
    [TableColumnWidth(130)]
    [DisplayAsString]
    public System.DateTime ConnectedAt;
    
    [ReadOnly]
    [TableColumnWidth(200)]
    public string EndpointUrl;
    
    [ReadOnly]
    [TableColumnWidth(80)]
    [ProgressBar(0, 200, ColorGetter = "GetPingColor")]
    [LabelText("Ping")]
    public float Ping => UnityEngine.Random.Range(10f, 100f); // 시뮬레이션
    
    private Color GetPingColor(float value)
    {
        if (value <= 50) return Color.green;
        if (value <= 100) return Color.yellow;
        return Color.red;
    }
}
```

### 게임 매니저에 Odin Inspector 적용

```csharp
using Sirenix.OdinInspector;

public class GameManager : MonoBehaviour
{
    [Title("Police-Thief Game Manager", "게임 전체 상태 관리", TitleAlignments.Centered)]
    
    [TabGroup("Game State")]
    [ShowInInspector]
    [ReadOnly]
    [EnumToggleButtons]
    [LabelText("Current State")]
    public GameState CurrentState { get; private set; } = GameState.MainMenu;
    
    [TabGroup("Game State")]
    [ShowInInspector]
    [ReadOnly]
    [ProgressBar(0, 300, ColorGetter = "GetGameTimeColor")]
    [LabelText("Game Time (seconds)")]
    public float GameTime { get; private set; }
    
    [TabGroup("Players")]
    [ShowInInspector]
    [ReadOnly]
    [TableList(ShowIndexLabels = true, ShowPaging = false)]
    [ListDrawerSettings(ShowItemCount = true)]
    public List<PlayerInfo> ConnectedPlayers { get; private set; } = new();
    
    [TabGroup("Players")]
    [ShowInInspector]
    [ReadOnly]
    [HorizontalGroup("Players/Stats")]
    [LabelText("Police Count")]
    [GUIColor(0.7f, 0.7f, 1f)]
    public int PoliceCount => ConnectedPlayers.Count(p => p.Team == TeamType.Police);
    
    [HorizontalGroup("Players/Stats")]
    [ShowInInspector]
    [ReadOnly]
    [LabelText("Thief Count")]
    [GUIColor(1f, 0.7f, 0.7f)]
    public int ThiefCount => ConnectedPlayers.Count(p => p.Team == TeamType.Thief);
    
    [TabGroup("Game Settings")]
    [BoxGroup("Game Settings/Match Configuration")]
    [Range(60, 1800)]
    [SuffixLabel("seconds")]
    [InfoBox("게임 제한 시간을 설정합니다")]
    public float matchDuration = 300f;
    
    [BoxGroup("Game Settings/Match Configuration")]
    [Range(2, 20)]
    [InfoBox("최대 플레이어 수")]
    public int maxPlayers = 10;
    
    [BoxGroup("Game Settings/Team Balance")]
    [Range(0.3f, 0.7f)]
    [SuffixLabel("ratio")]
    [InfoBox("경찰 팀 비율 (나머지는 도둑 팀)")]
    [OnValueChanged("UpdateTeamBalance")]
    public float policeRatio = 0.4f;
    
    [BoxGroup("Game Settings/Team Balance")]
    [ShowInInspector]
    [ReadOnly]
    [LabelText("Police Slots")]
    public int PoliceSlots => Mathf.RoundToInt(maxPlayers * policeRatio);
    
    [BoxGroup("Game Settings/Team Balance")]
    [ShowInInspector]
    [ReadOnly]
    [LabelText("Thief Slots")]
    public int ThiefSlots => maxPlayers - PoliceSlots;
    
    [TabGroup("Debug")]
    [FoldoutGroup("Debug/Game Controls", expanded: true)]
    [Button("Start Match", ButtonSizes.Large)]
    [GUIColor(0.7f, 1f, 0.7f)]
    [EnableIf("@CurrentState == GameState.Lobby && ConnectedPlayers.Count >= 2")]
    public void StartMatch()
    {
        CurrentState = GameState.InGame;
        GameTime = 0f;
        Debug.Log("매치가 시작되었습니다!");
        
        // 게임 시작 이벤트 발행
        EventBusOptimized.Instance.Publish(new GameStartEvent
        {
            RoomId = System.Guid.NewGuid().ToString(),
            PlayerIds = ConnectedPlayers.Select(p => p.PlayerId).ToList()
        });
    }
    
    [FoldoutGroup("Debug/Game Controls")]
    [Button("End Match")]
    [GUIColor(1f, 0.8f, 0.8f)]
    [EnableIf("@CurrentState == GameState.InGame")]
    public void EndMatch()
    {
        CurrentState = GameState.GameOver;
        Debug.Log("매치가 종료되었습니다!");
    }
    
    [FoldoutGroup("Debug/Game Controls")]
    [Button("Reset Game")]
    [GUIColor(1f, 0.6f, 0.6f)]
    public void ResetGame()
    {
        CurrentState = GameState.MainMenu;
        GameTime = 0f;
        ConnectedPlayers.Clear();
        Debug.Log("게임이 초기화되었습니다!");
    }
    
    [FoldoutGroup("Debug/Player Management")]
    [Button("Add Test Players")]
    [EnableIf("@CurrentState == GameState.Lobby")]
    public void AddTestPlayers()
    {
        var testPlayers = new[]
        {
            new PlayerInfo { PlayerId = "P001", PlayerName = "Officer Kim", Team = TeamType.Police, Score = 0, IsAlive = true },
            new PlayerInfo { PlayerId = "P002", PlayerName = "Detective Lee", Team = TeamType.Police, Score = 0, IsAlive = true },
            new PlayerInfo { PlayerId = "T001", PlayerName = "Phantom", Team = TeamType.Thief, Score = 0, IsAlive = true },
            new PlayerInfo { PlayerId = "T002", PlayerName = "Shadow", Team = TeamType.Thief, Score = 0, IsAlive = true }
        };
        
        foreach (var player in testPlayers)
        {
            if (!ConnectedPlayers.Any(p => p.PlayerId == player.PlayerId))
            {
                ConnectedPlayers.Add(player);
            }
        }
        
        Debug.Log($"테스트 플레이어 {testPlayers.Length}명 추가됨");
    }
    
    [FoldoutGroup("Debug/Player Management")]
    [Button("Clear All Players")]
    [GUIColor(1f, 0.7f, 0.7f)]
    public void ClearAllPlayers()
    {
        ConnectedPlayers.Clear();
        Debug.Log("모든 플레이어가 제거되었습니다.");
    }
    
    void Update()
    {
        if (CurrentState == GameState.InGame)
        {
            GameTime += Time.deltaTime;
            
            // 제한 시간 체크
            if (GameTime >= matchDuration)
            {
                EndMatch();
            }
        }
    }
    
    private Color GetGameTimeColor(float value)
    {
        float ratio = value / matchDuration;
        if (ratio < 0.5f) return Color.green;
        if (ratio < 0.8f) return Color.yellow;
        return Color.red;
    }
    
    private void UpdateTeamBalance()
    {
        Debug.Log($"팀 밸런스 업데이트: 경찰 {PoliceSlots}명, 도둑 {ThiefSlots}명");
    }
}

[System.Serializable]
public class PlayerInfo
{
    [TableColumnWidth(80)]
    [ReadOnly]
    public string PlayerId;
    
    [TableColumnWidth(120)]
    [ReadOnly]
    public string PlayerName;
    
    [TableColumnWidth(80)]
    [ReadOnly]
    [EnumToggleButtons]
    public TeamType Team;
    
    [TableColumnWidth(60)]
    [ReadOnly]
    public int Score;
    
    [TableColumnWidth(60)]
    [ReadOnly]
    public bool IsAlive;
    
    [TableColumnWidth(100)]
    [ReadOnly]
    [ProgressBar(0, 100, ColorGetter = "GetHealthColor")]
    [LabelText("HP")]
    public float Health = 100f;
    
    private Color GetHealthColor(float value)
    {
        return Color.Lerp(Color.red, Color.green, value / 100f);
    }
}

public enum TeamType
{
    Police,
    Thief
}
```

## 결론

Odin Inspector를 Police-Thief 프로젝트에 적용하면 다음과 같은 이점을 얻을 수 있습니다:

1. **개발 효율성 향상**: 복잡한 설정을 시각적으로 관리
2. **디버깅 편의성**: 실시간 상태 모니터링과 테스트 버튼
3. **팀 협업 개선**: 비프로그래머도 쉽게 설정 조정 가능
4. **코드 품질**: 검증과 조건부 표시로 오류 방지
5. **유지보수성**: 직관적인 Inspector로 빠른 문제 해결

Odin Inspector의 다양한 어트리뷰트를 활용하여 프로젝트의 모든 설정과 상태를 효과적으로 관리할 수 있습니다.