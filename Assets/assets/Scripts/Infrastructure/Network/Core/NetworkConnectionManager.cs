using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using PoliceThief.Core.Logging;
using PoliceThief.Core.DI;
using PoliceThief.Core.Config;
using PoliceThief.Infrastructure.Network.Interfaces;
using PoliceThief.Infrastructure.Network.Grpc;
using PoliceThief.Infrastructure.Network.QUIC;

namespace PoliceThief.Infrastructure.Network.Core
{
    /// <summary>
    /// 네트워크 연결 관리 구현체
    /// MonoBehaviour에서 분리된 순수 C# 클래스
    /// </summary>
    public class NetworkConnectionManager : INetworkManager
    {
        private readonly Dictionary<NetworkProtocol, IProtocolManager> _protocolManagers;
        private readonly object _lock = new object();
        
        public event Action<NetworkProtocol> OnProtocolConnected;
        public event Action<NetworkProtocol> OnProtocolDisconnected;
        public event Action<NetworkProtocol, string> OnProtocolError;
        
        public int TotalConnections { get; private set; }
        public int ActiveConnections => GetActiveConnectionCount();

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

        public void RegisterProtocolManager(NetworkProtocol protocol, IProtocolManager manager)
        {
            lock (_lock)
            {
                if (_protocolManagers.ContainsKey(protocol))
                {
                    Log.Warning($"Protocol manager for {protocol} already exists, replacing...", "Network");
                }

                _protocolManagers[protocol] = manager;
                
                // 이벤트 구독
                manager.OnConnected += () => HandleProtocolConnected(protocol);
                manager.OnDisconnected += () => HandleProtocolDisconnected(protocol);
                manager.OnError += (error) => HandleProtocolError(protocol, error);
                
                Log.Info($"Registered protocol manager for {protocol}", "Network");
            }
        }

        public async Task<bool> ConnectAsync(NetworkProtocol protocol)
        {
            if (_protocolManagers.TryGetValue(protocol, out var manager))
            {
                Log.Info($"Connecting to {protocol}...", "Network");
                var success = await manager.ConnectAsync();
                
                if (success)
                {
                    TotalConnections++;
                    Log.Info($"Successfully connected to {protocol}", "Network");
                }
                else
                {
                    Log.Error($"Failed to connect to {protocol}", "Network");
                }
                
                return success;
            }
            
            Log.Error($"No protocol manager found for {protocol}", "Network");
            return false;
        }

        public async Task DisconnectAsync(NetworkProtocol protocol)
        {
            if (_protocolManagers.TryGetValue(protocol, out var manager))
            {
                Log.Info($"Disconnecting from {protocol}...", "Network");
                await manager.DisconnectAsync();
                Log.Info($"Disconnected from {protocol}", "Network");
            }
        }

        public async Task DisconnectAllAsync()
        {
            Log.Info("Disconnecting all protocols...", "Network");
            
            var tasks = new List<Task>();
            foreach (var kvp in _protocolManagers)
            {
                tasks.Add(kvp.Value.DisconnectAsync());
            }
            
            await Task.WhenAll(tasks);
            Log.Info("All protocols disconnected", "Network");
        }

        public bool IsConnected(NetworkProtocol protocol)
        {
            return _protocolManagers.TryGetValue(protocol, out var manager) && manager.IsConnected;
        }

        public ConnectionState GetConnectionState(NetworkProtocol protocol)
        {
            return _protocolManagers.TryGetValue(protocol, out var manager) ? manager.State : ConnectionState.Disconnected;
        }

        public T GetProtocolManager<T>() where T : class
        {
            foreach (var manager in _protocolManagers.Values)
            {
                if (manager is T typedManager)
                {
                    return typedManager;
                }
            }
            return null;
        }

        private void HandleProtocolConnected(NetworkProtocol protocol)
        {
            Log.Info($"Protocol {protocol} connected", "Network");
            OnProtocolConnected?.Invoke(protocol);
        }

        private void HandleProtocolDisconnected(NetworkProtocol protocol)
        {
            Log.Info($"Protocol {protocol} disconnected", "Network");
            OnProtocolDisconnected?.Invoke(protocol);
        }

        private void HandleProtocolError(NetworkProtocol protocol, string error)
        {
            Log.Error($"Protocol {protocol} error: {error}", "Network");
            OnProtocolError?.Invoke(protocol, error);
        }

        private int GetActiveConnectionCount()
        {
            int count = 0;
            foreach (var manager in _protocolManagers.Values)
            {
                if (manager.IsConnected)
                {
                    count++;
                }
            }
            return count;
        }
    }

    /// <summary>
    /// QUIC 프로토콜 매니저
    /// </summary>
    public class QuicProtocolManager : IProtocolManager
    {
        private readonly QuicClientNonMono _quicClient;
        private readonly NetworkConfig _config;
        private ClientConnectionState _state = ClientConnectionState.Disconnected;

        public NetworkProtocol Protocol => NetworkProtocol.QUIC;
        public ConnectionState State => _state switch
        {
            ClientConnectionState.Disconnected => ConnectionState.Disconnected,
            ClientConnectionState.Connecting => ConnectionState.Connecting,
            ClientConnectionState.Connected => ConnectionState.Connected,
            ClientConnectionState.Disconnecting => ConnectionState.Disconnected,
            ClientConnectionState.Error => ConnectionState.Error,
            _ => ConnectionState.Disconnected
        };
        public bool IsConnected => _state == ClientConnectionState.Connected;

        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnError;

        public QuicProtocolManager(QuicClientNonMono quicClient, NetworkConfig config)
        {
            _quicClient = quicClient ?? throw new ArgumentNullException(nameof(quicClient));
            _config = config ?? throw new ArgumentNullException(nameof(config));
            
            // QUIC 클라이언트 이벤트 연결
            _quicClient.OnConnected += HandleConnected;
            _quicClient.OnDisconnected += HandleDisconnected;
            _quicClient.OnError += HandleError;
        }

        private void HandleConnected()
        {
            _state = ClientConnectionState.Connected;
            OnConnected?.Invoke();
        }

        private void HandleDisconnected()
        {
            _state = ClientConnectionState.Disconnected;
            OnDisconnected?.Invoke();
        }

        private void HandleError(string error)
        {
            _state = ClientConnectionState.Error;
            OnError?.Invoke(error);
        }

        public async Task<bool> ConnectAsync()
        {
            _state = ClientConnectionState.Connecting;
            var success = await _quicClient.ConnectAsync(_config.GetQuicEndpoint());
            _state = success ? ClientConnectionState.Connected : ClientConnectionState.Error;
            return success;
        }

        public async Task DisconnectAsync()
        {
            _state = ClientConnectionState.Disconnecting;
            await _quicClient.DisconnectAsync();
            _state = ClientConnectionState.Disconnected;
        }

        public async Task<bool> CheckHealthAsync()
        {
            if (!IsConnected) return false;
            
            // Send a heartbeat message
            var heartbeat = new NetworkMessage
            {
                messageType = MessageType.Heartbeat,
                payload = System.BitConverter.GetBytes(System.DateTime.UtcNow.Ticks)
            };
            
            return await _quicClient.SendAsync(heartbeat);
        }

        public QuicClientNonMono GetClient() => _quicClient;
    }

    /// <summary>
    /// gRPC 프로토콜 매니저
    /// </summary>
    public class GrpcProtocolManager : IProtocolManager
    {
        private readonly IGrpcClient _grpcClient;

        public NetworkProtocol Protocol => NetworkProtocol.GRPC;
        public ConnectionState State => _grpcClient.IsConnecting ? ConnectionState.Connecting : 
                                       (_grpcClient.IsConnected ? ConnectionState.Connected : ConnectionState.Disconnected);
        public bool IsConnected => _grpcClient.IsConnected;

        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnError;

        public GrpcProtocolManager(IGrpcClient grpcClient)
        {
            _grpcClient = grpcClient ?? throw new ArgumentNullException(nameof(grpcClient));
            
            // gRPC 클라이언트 이벤트 연결
            _grpcClient.OnConnected += () => OnConnected?.Invoke();
            _grpcClient.OnDisconnected += () => OnDisconnected?.Invoke();
            _grpcClient.OnError += (error) => OnError?.Invoke(error);
        }

        public async Task<bool> ConnectAsync()
        {
            return await _grpcClient.ConnectAsync();
        }

        public async Task DisconnectAsync()
        {
            await _grpcClient.DisconnectAsync();
        }

        public async Task<bool> CheckHealthAsync()
        {
            return await _grpcClient.CheckHealthAsync();
        }

        public T CreateClient<T>() where T : class
        {
            return _grpcClient.CreateClient<T>();
        }
    }
}