# 🎮 Police-Thief 개발 가이드 (핵심 요약)

## 📋 **기본 구조**

```
Police-Thief/
├── Core/           # 핵심 시스템 (DI, Events, Logging, Pool)
├── Infrastructure/ # 네트워크 계층 (QUIC, gRPC, TCP)
├── Game/          # 게임 로직 (Entity, Managers)
├── Presentation/  # Unity UI 계층
└── Test/         # 테스트 코드
```

## 🏗️ **핵심 시스템 사용법**

### **1. ServiceLocator (의존성 주입)**
```csharp
// 서비스 등록 (Bootstrap.cs에서)
ServiceLocator.Instance.RegisterSingleton<IService>(implementation);

// 서비스 사용
var service = ServiceLocator.Instance.Get<IService>();
```

### **2. EventBus (이벤트 시스템)**
```csharp
// 이벤트 발행
EventBus.Instance.Publish(new PlayerJoinedEvent());

// 이벤트 구독
EventBus.Instance.Subscribe<PlayerJoinedEvent>(OnPlayerJoined);
```

### **3. 로깅 시스템**
```csharp
Log.Info("정보 메시지", "카테고리");
Log.Warning("경고 메시지", "카테고리");  
Log.Error("에러 메시지", "카테고리");
```

### **4. ObjectPool (성능 최적화)**
```csharp
// 오브젝트 가져오기
var obj = ObjectPool.Instance.Get(prefab);

// 오브젝트 반환
ObjectPool.Instance.Return(obj);
```

## 🌐 **네트워크 프로토콜 선택**

| 프로토콜 | 용도 | 예시 |
|---------|------|------|
| **QUIC** | 실시간 데이터 | 음성채팅, 게임상태 동기화 |
| **gRPC** | API 호출 | 로그인, 랭킹, 매치메이킹 |
| **TCP** | 신뢰성 메시지 | 채팅, 친구시스템, 파일전송 |

### **네트워크 클라이언트 사용**
```csharp
// gRPC 사용
var grpcClient = ServiceLocator.Instance.Get<IGrpcClient>();
await grpcClient.ConnectAsync();

// QUIC 사용 (실시간)
var quicClient = new QuicClientNonMono(config);
await quicClient.ConnectAsync(serverUrl);
```

## 🎯 **게임 엔티티 생성**

### **GameEntity 상속**
```csharp
public class Player : GameEntity
{
    protected override void UpdateEntity()
    {
        Log.Info("Player update called", "Game");
    }
    
    public override void OnPoolGet()
    {
        Log.Info("Player spawned", "Game");  
    }
}
```

## 🚀 **성능 최적화 핵심 규칙**

### **메모리 관리**
- ObjectPool 사용으로 가비지 컬렉션 최소화
- `string.Format()` 대신 `$""` 사용  
- 자주 생성되는 객체는 풀링 적용

### **네트워크 최적화**
- 메시지 배칭: 50ms 간격, 최대 10개
- 압축: gRPC >4MB, TCP >512bytes, QUIC >128bytes
- 연결 풀링으로 재사용

### **Unity 최적화**  
- MonoBehaviour는 Unity 전용 기능만 사용
- 순수 C# 클래스로 서비스 구현
- Update 루프 최소화

## 🛠️ **개발 도구**

### **Odin Inspector 주요 기능**
```csharp
[Title("섹션 제목")]
[SerializeField] private int value;

[Button("테스트 실행")]
public void TestMethod() { }

[ShowInInspector]
public string Status => IsConnected ? "연결됨" : "연결 안됨";
```

## 📱 **모바일 최적화**

### **성능 타겟**
- **로드 타임**: 3G에서 <3초, WiFi에서 <1초
- **메모리**: 모바일 <100MB, 데스크톱 <500MB  
- **배터리**: CPU 사용률 <30% 평균

### **빌드 설정**
```csharp
// 릴리즈 빌드용 매크로
#if !DEBUG
    Log.DisableDebugLogs();
#endif
```

## ⚡ **빠른 시작**

### **1. 새 기능 추가**
1. `Game/Logic/` 에 매니저 클래스 생성
2. `Bootstrap.cs` 에 서비스 등록
3. EventBus로 다른 시스템과 통신

### **2. 네트워크 기능 추가**  
1. 프로토콜 선택 (QUIC/gRPC/TCP)
2. 해당 클라이언트 사용
3. 연결 상태 이벤트 처리

### **3. 성능 문제 해결**
1. Unity Profiler로 병목 지점 확인
2. ObjectPool 적용 검토
3. 불필요한 Update 루프 제거

## 🐛 **일반적인 문제 해결**

- **초기화 순서 문제** → Bootstrap.cs에서 올바른 순서로 등록
- **메모리 누수** → ObjectPool 사용, 이벤트 구독 해제  
- **네트워크 끊김** → 자동 재연결 로직 확인
- **성능 저하** → Profiler로 가비지 컬렉션 확인

---

**💡 Tip**: 복잡한 기능 개발 시 `Test/` 폴더에 테스트 코드 작성하여 검증!