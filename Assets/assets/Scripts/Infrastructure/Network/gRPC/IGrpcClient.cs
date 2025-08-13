using System;
using System.Threading.Tasks;
using Grpc.Net.Client;

namespace PoliceThief.Infrastructure.Network.Grpc
{
    /// <summary>
    /// gRPC 클라이언트 인터페이스
    /// 의존성 주입과 테스트를 위한 추상화
    /// </summary>
    public interface IGrpcClient : IDisposable
    {
        // Connection Management
        bool IsConnected { get; }
        bool IsConnecting { get; }
        string ServerUrl { get; }
        string ConnectionStatus { get; }
        
        // Connection Operations
        Task<bool> ConnectAsync();
        Task DisconnectAsync();
        Task<bool> CheckHealthAsync();
        
        // Service Access
        GrpcChannel GetChannel();
        T CreateClient<T>() where T : class;
        Task<T> ExecuteWithRetryAsync<T>(Func<Task<T>> grpcCall, int maxRetries = 3);
        
        // Events
        event Action OnConnected;
        event Action OnDisconnected;
        event Action<string> OnError;
        event Action<float> OnLatencyMeasured;
        
        // Metrics
        int TotalConnections { get; }
        int TotalErrors { get; }
        float AverageLatency { get; }
    }
    
    /// <summary>
    /// gRPC 클라이언트 설정
    /// </summary>
    public interface IGrpcClientConfig
    {
        string ServerUrl { get; set; }
        int ConnectTimeoutMs { get; set; }
        int MaxRetryAttempts { get; set; }
        int RetryDelayMs { get; set; }
        bool EnableKeepAlive { get; set; }
        int KeepAliveIntervalMs { get; set; }
        bool EnableAutoReconnect { get; set; }
        int ReconnectDelayMs { get; set; }
    }
}