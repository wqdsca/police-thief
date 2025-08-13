using System;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.DI;
using PoliceThief.Core.Logging;
using PoliceThief.Core.Config;
using PoliceThief.Core.Events;
using PoliceThief.Core.Pool;
using PoliceThief.Infrastructure.Network.Grpc;
using PoliceThief.Infrastructure.Network.Interfaces;
using PoliceThief.Infrastructure.Network.Core;
using PoliceThief.Presentation;

namespace PoliceThief
{
    /// <summary>
    /// Bootstrap class to initialize all systems in correct order
    /// 게임 시작 시 모든 시스템을 올바른 순서로 초기화
    /// </summary>
    public class Bootstrap : MonoBehaviour
    {
        [Header("Configuration")]
        [SerializeField] private bool autoInitialize = true;
        [SerializeField] private bool enableLogging = true;
        [SerializeField] private string serverUrl = "http://localhost:50051";
        
        private ServiceLocator _serviceLocator;
        private GrpcClientOptimized _grpcClient;
        
        private void Awake()
        {
            if (autoInitialize)
            {
                InitializeSystems();
            }
        }
        
        /// <summary>
        /// Initialize all core systems with improved DI architecture
        /// 개선된 DI 아키텍처로 모든 핵심 시스템 초기화
        /// </summary>
        public void InitializeSystems()
        {
            UnityEngine.Debug.Log("[Bootstrap] Starting system initialization with new architecture...");
            
            try
            {
                // 1. Initialize Service Locator (DI Container)
                InitializeServiceLocator();
                
                // 2. Initialize Core Services
                InitializeCoreServices();
                
                // 3. Initialize Infrastructure Services
                InitializeInfrastructureServices();
                
                // 4. Initialize Application Services
                InitializeApplicationServices();
                
                // 5. Initialize Presentation Layer
                InitializePresentationLayer();
                
                UnityEngine.Debug.Log("[Bootstrap] All systems initialized successfully with new architecture!");
            }
            catch (Exception ex)
            {
                UnityEngine.Debug.LogError($"[Bootstrap] System initialization failed: {ex.Message}");
                UnityEngine.Debug.LogError($"[Bootstrap] Stack trace: {ex.StackTrace}");
                throw;
            }
        }
        
        private void InitializeServiceLocator()
        {
            _serviceLocator = ServiceLocator.Instance;
            UnityEngine.Debug.Log("[Bootstrap] ServiceLocator initialized with new architecture");
        }
        
        private void InitializeCoreServices()
        {
            // Configuration Manager - converted to pure C# class
            var configManager = ConfigManager.Instance;
            _serviceLocator.RegisterSingleton<IConfigManager>(configManager);
            _serviceLocator.RegisterSingleton<ConfigManager>(configManager);
            
            if (enableLogging) Log.Info("ConfigManager registered as interface and concrete type", "Bootstrap");
            
            // Event Bus - converted to pure C# class
            var eventBus = EventBus.Instance;
            _serviceLocator.RegisterSingleton<IEventBus>(eventBus);
            _serviceLocator.RegisterSingleton<EventBus>(eventBus);
            
            if (enableLogging) Log.Info("EventBus registered as interface and concrete type", "Bootstrap");
            
            // Object Pool
            var objectPool = ObjectPool.Instance;
            _serviceLocator.RegisterSingleton<ObjectPool>(objectPool);
            
            if (enableLogging) Log.Info("ObjectPool registered to ServiceLocator", "Bootstrap");
            
            // Logging is handled via static Log class
            if (enableLogging) Log.Info("Core services initialized", "Bootstrap");
        }
        
        
        private void InitializeInfrastructureServices()
        {
            // Create optimized gRPC client with production configuration
            var config = new GrpcClientOptimized.ConnectionConfig
            {
                ServerUrl = serverUrl,
                ConnectTimeoutMs = 5000,
                MaxRetryAttempts = 3,
                RetryDelayMs = 1000,
                EnableKeepAlive = true,
                KeepAliveIntervalMs = 30000,
                EnableAutoReconnect = true,
                ReconnectDelayMs = 5000
            };
            
            _grpcClient = new GrpcClientOptimized(config);
            _serviceLocator.RegisterSingleton<IGrpcClient>(_grpcClient);
            _serviceLocator.RegisterSingleton<GrpcClientOptimized>(_grpcClient);
            
            Log.Info("GrpcClient registered as interface and concrete type", "Bootstrap");
            
            // Subscribe to connection events
            _grpcClient.OnConnected += OnGrpcConnected;
            _grpcClient.OnDisconnected += OnGrpcDisconnected;
            _grpcClient.OnError += OnGrpcError;
            
            // Initialize new Network Connection Manager
            var networkConnectionManager = new NetworkConnectionManager();
            _serviceLocator.RegisterSingleton<INetworkManager>(networkConnectionManager);
            _serviceLocator.RegisterSingleton<NetworkConnectionManager>(networkConnectionManager);
            Log.Info("NetworkConnectionManager registered as interface and concrete type", "Bootstrap");
            
            Log.Info("Infrastructure services initialized", "Bootstrap");
        }
        
        private void InitializeApplicationServices()
        {
            // Application layer services can be added here
            // For now, we'll keep the GameManager registration in presentation layer
            Log.Info("Application services initialized", "Bootstrap");
        }
        
        private void InitializePresentationLayer()
        {
            var gameManager = GameManager.Instance;
            if (gameManager != null)
            {
                _serviceLocator.RegisterSingleton<GameManager>(gameManager);
                Log.Info("GameManager registered to ServiceLocator", "Bootstrap");
            }
            
            Log.Info("Presentation layer initialized", "Bootstrap");
        }
        
        private void OnGrpcConnected()
        {
            Log.Info("Successfully connected to gRPC server", "Bootstrap");
            EventBus.Instance.Publish(new NetworkConnectedEvent(_grpcClient.ServerUrl));
        }
        
        private void OnGrpcDisconnected()
        {
            Log.Warning("Disconnected from gRPC server", "Bootstrap");
            EventBus.Instance.Publish(new NetworkDisconnectedEvent("gRPC connection lost"));
        }
        
        private void OnGrpcError(string error)
        {
            Log.Error($"gRPC error: {error}", "Bootstrap");
            EventBus.Instance.Publish(new NetworkErrorEvent(error));
        }
        
        private void OnDestroy()
        {
            if (_grpcClient != null)
            {
                _grpcClient.OnConnected -= OnGrpcConnected;
                _grpcClient.OnDisconnected -= OnGrpcDisconnected;
                _grpcClient.OnError -= OnGrpcError;
                _grpcClient.Dispose();
            }
        }
        
        private void OnApplicationPause(bool pauseStatus)
        {
            if (pauseStatus)
            {
                Log.Info("Application paused", "Bootstrap");
            }
            else
            {
                Log.Info("Application resumed", "Bootstrap");
            }
        }
        
        private void OnApplicationQuit()
        {
            Log.Info("Application quitting", "Bootstrap");
            _grpcClient?.Dispose();
        }
    }
}