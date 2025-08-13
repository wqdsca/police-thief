using System;
using System.Threading.Tasks;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Interfaces;

namespace PoliceThief.Infrastructure.Network.Grpc
{
    /// <summary>
    /// gRPC protocol manager implementation
    /// </summary>
    public class GrpcProtocolManager : IProtocolManager
    {
        private readonly IGrpcClient _grpcClient;
        
        public NetworkProtocol Protocol => NetworkProtocol.GRPC;
        public ConnectionState State => _grpcClient.IsConnected ? ConnectionState.Connected : 
                                       _grpcClient.IsConnecting ? ConnectionState.Connecting : 
                                       ConnectionState.Disconnected;
        public bool IsConnected => _grpcClient.IsConnected;
        
        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnError;
        
        public GrpcProtocolManager(IGrpcClient grpcClient)
        {
            _grpcClient = grpcClient ?? throw new ArgumentNullException(nameof(grpcClient));
            
            // Subscribe to gRPC client events
            _grpcClient.OnConnected += () => OnConnected?.Invoke();
            _grpcClient.OnDisconnected += () => OnDisconnected?.Invoke();
            _grpcClient.OnError += (error) => OnError?.Invoke(error);
            
            Log.Info("GrpcProtocolManager initialized", "Network");
        }
        
        public async Task<bool> ConnectAsync()
        {
            try
            {
                Log.Info("Connecting via gRPC protocol manager", "Network");
                return await _grpcClient.ConnectAsync();
            }
            catch (Exception ex)
            {
                Log.Error($"gRPC connection failed: {ex.Message}", "Network");
                OnError?.Invoke($"Connection failed: {ex.Message}");
                return false;
            }
        }
        
        public async Task DisconnectAsync()
        {
            try
            {
                Log.Info("Disconnecting via gRPC protocol manager", "Network");
                await _grpcClient.DisconnectAsync();
            }
            catch (Exception ex)
            {
                Log.Error($"gRPC disconnect failed: {ex.Message}", "Network");
                OnError?.Invoke($"Disconnect failed: {ex.Message}");
            }
        }
        
        public async Task<bool> CheckHealthAsync()
        {
            try
            {
                return await _grpcClient.CheckHealthAsync();
            }
            catch (Exception ex)
            {
                Log.Error($"gRPC health check failed: {ex.Message}", "Network");
                OnError?.Invoke($"Health check failed: {ex.Message}");
                return false;
            }
        }
    }
}