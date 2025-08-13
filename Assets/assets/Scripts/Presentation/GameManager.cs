using System;
using UnityEngine;
using Sirenix.OdinInspector;
using PoliceThief.Infrastructure.Network.Grpc;
using System.Threading.Tasks;

namespace PoliceThief.Presentation
{
    /// <summary>
    /// 게임 매니저 - gRPC 연결 관리 + 디버깅
    /// 네트워크 연결은 유지하되 게임 로직은 제거
    /// </summary>
    public class GameManager : MonoBehaviour
    {
        private static GameManager _instance;
        public static GameManager Instance => _instance;
        
        [Title("Network Settings")]
        [BoxGroup("Network")]
        [SerializeField]
        [LabelText("Server URL")]
        private string _serverUrl = "http://localhost:50051";
        
        [BoxGroup("Network")]
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("Connection Status")]
        private string ConnectionStatus => _grpcClient != null && _grpcClient.IsConnected ? 
            "🟢 Connected" : "🔴 Disconnected";
        
        [BoxGroup("Network")]
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("Server")]
        private string CurrentServer => _serverUrl;
        
        // Infrastructure
        private GrpcClientOptimized _grpcClient;
        
        [Title("Debug Settings")]
        [BoxGroup("Debug")]
        [LabelText("Enable Debug Logging")]
        [ToggleLeft]
        public bool enableDebugLogging = true;
        
        [BoxGroup("Debug")]
        [ShowIf("enableDebugLogging")]
        [Range(1, 5)]
        [LabelText("Log Verbosity Level")]
        public int logVerbosityLevel = 3;
        
        [Title("Runtime Information")]
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("Application State")]
        private string ApplicationState => Application.isPlaying ? "Playing" : "Editor";
        
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("Time Since Startup")]
        private string TimeSinceStartup => $"{Time.realtimeSinceStartup:F2} seconds";
        
        [ShowInInspector]
        [ProgressBar(0, 60)]
        [LabelText("Current FPS")]
        private float CurrentFPS => 1.0f / Time.deltaTime;
        
        // Properties
        public GrpcClientOptimized GrpcClient => _grpcClient;
        public bool IsConnected => _grpcClient != null && _grpcClient.IsConnected;
        
        // Events
        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnConnectionError;
        
        private void Awake()
        {
            // 싱글톤 패턴
            if (_instance != null && _instance != this)
            {
                Destroy(gameObject);
                return;
            }
            
            _instance = this;
            DontDestroyOnLoad(gameObject);
            
            Initialize();
        }
        
        private void Initialize()
        {
            LogDebug("[GameManager] Initializing with gRPC support", 1);
            LogDebug($"[GameManager] Server URL: {_serverUrl}", 2);
            
            // gRPC 클라이언트 초기화만 수행
            var config = new GrpcClientOptimized.ConnectionConfig { serverUrl = _serverUrl };
            _grpcClient = new GrpcClientOptimized(config);
            
            LogDebug("[GameManager] Ready for connection. Use Connect button to connect to server.", 1);
        }
        
        [Title("Network Actions")]
        [BoxGroup("Network Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.4f, 0.8f, 0.4f)]
        [EnableIf("@!IsConnected")]
        public async void ConnectToServer()
        {
            LogDebug($"[GameManager] Connecting to server: {_serverUrl}", 1);
            
            try
            {
                bool connected = await _grpcClient.ConnectAsync();
                
                if (connected)
                {
                    LogDebug("[GameManager] Successfully connected to server!", 1);
                    OnConnected?.Invoke();
                }
                else
                {
                    LogDebug("[GameManager] Failed to connect to server", 1);
                    OnConnectionError?.Invoke("Connection failed");
                }
            }
            catch (Exception ex)
            {
                LogDebug($"[GameManager] Connection error: {ex.Message}", 1);
                OnConnectionError?.Invoke(ex.Message);
            }
        }
        
        [BoxGroup("Network Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.8f, 0.4f, 0.4f)]
        [EnableIf("IsConnected")]
        public async void DisconnectFromServer()
        {
            LogDebug("[GameManager] Disconnecting from server", 1);
            
            try
            {
                await _grpcClient.DisconnectAsync();
                LogDebug("[GameManager] Disconnected from server", 1);
                OnDisconnected?.Invoke();
            }
            catch (Exception ex)
            {
                LogDebug($"[GameManager] Disconnect error: {ex.Message}", 1);
            }
        }
        
        [BoxGroup("Network Actions")]
        [Button(ButtonSizes.Medium)]
        [GUIColor(0.8f, 0.8f, 0.4f)]
        [EnableIf("IsConnected")]
        public async void CheckConnection()
        {
            if (_grpcClient != null)
            {
                bool isConnected = await _grpcClient.CheckHealthAsync();
                LogDebug($"[GameManager] Connection check: {(isConnected ? "Connected" : "Disconnected")}", 1);
            }
        }
        
        [BoxGroup("Network Actions")]
        [Button(ButtonSizes.Medium)]
        public void UpdateServerUrl(string newUrl)
        {
            if (!IsConnected)
            {
                _serverUrl = newUrl;
                var config = new GrpcClientOptimized.ConnectionConfig { serverUrl = _serverUrl };
                _grpcClient = new GrpcClientOptimized(config);
                LogDebug($"[GameManager] Server URL updated to: {_serverUrl}", 1);
            }
            else
            {
                LogDebug("[GameManager] Cannot change server URL while connected", 1);
            }
        }
        
        [Title("Debug Actions")]
        [BoxGroup("Debug Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.4f, 0.8f, 0.4f)]
        public void TestDebugLog()
        {
            LogDebug("Test debug log message", 1);
        }
        
        [BoxGroup("Debug Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.8f, 0.8f, 0.4f)]
        public void TestWarningLog()
        {
            if (enableDebugLogging)
                UnityEngine.Debug.LogWarning("[GameManager] Test warning message");
        }
        
        [BoxGroup("Debug Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.8f, 0.4f, 0.4f)]
        public void TestErrorLog()
        {
            if (enableDebugLogging)
                UnityEngine.Debug.LogError("[GameManager] Test error message");
        }
        
        private void LogDebug(string message, int requiredVerbosity)
        {
            if (enableDebugLogging && logVerbosityLevel >= requiredVerbosity)
            {
                UnityEngine.Debug.Log(message);
            }
        }
        
        private async void OnDestroy()
        {
            LogDebug("[GameManager] Destroying GameManager", 1);
            
            if (_grpcClient != null)
            {
                await _grpcClient.DisconnectAsync();
                _grpcClient.Dispose();
            }
        }
        
        private void OnApplicationPause(bool pauseStatus)
        {
            LogDebug($"[GameManager] Application pause state: {pauseStatus}", 3);
        }
        
        private void OnApplicationFocus(bool hasFocus)
        {
            LogDebug($"[GameManager] Application focus: {hasFocus}", 3);
            
            // 포커스를 다시 얻었을 때 연결 상태 확인
            if (hasFocus && IsConnected)
            {
                _ = CheckConnectionAsync();
            }
        }
        
        private async Task CheckConnectionAsync()
        {
            if (_grpcClient != null)
            {
                bool isConnected = await _grpcClient.CheckHealthAsync();
                if (!isConnected)
                {
                    LogDebug("[GameManager] Connection lost, disconnecting...", 1);
                    OnDisconnected?.Invoke();
                }
            }
        }
        
        [Title("Inspector Debugging")]
        [InfoBox("GameManager with gRPC connection support. Game logic should be implemented separately.")]
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("Debug Status")]
        private string DebugStatus => enableDebugLogging ? "Active" : "Inactive";
        
        [ShowInInspector]
        [Button("Clear Console")]
        private void ClearConsole()
        {
            // Note: ClearDeveloperConsole is only available in Unity Editor
            #if UNITY_EDITOR
            var logEntries = System.Type.GetType("UnityEditor.LogEntries, UnityEditor.dll");
            if (logEntries != null)
            {
                var clearMethod = logEntries.GetMethod("Clear", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public);
                clearMethod?.Invoke(null, null);
            }
            #endif
            LogDebug("[GameManager] Console cleared", 1);
        }
        
        [Title("Memory Monitoring")]
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("Memory Usage (MB)")]
        private float MemoryUsage => System.GC.GetTotalMemory(false) / (1024f * 1024f);
        
        [ShowInInspector]
        [Button("Force Garbage Collection")]
        private void ForceGC()
        {
            System.GC.Collect();
            System.GC.WaitForPendingFinalizers();
            System.GC.Collect();
            LogDebug($"[GameManager] GC Forced. Memory: {MemoryUsage:F2} MB", 2);
        }
    }
}