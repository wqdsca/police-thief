using System;
using System.Threading.Tasks;

namespace PoliceThief.Infrastructure.Network.Interfaces
{
    public enum NetworkProtocol
    {
        QUIC,
        TCP,
        GRPC
    }

    public enum ConnectionState
    {
        Disconnected,
        Connecting,
        Connected,
        Reconnecting,
        Error
    }

    /// <summary>
    /// 네트워크 연결 관리 인터페이스
    /// </summary>
    public interface INetworkManager
    {
        // Connection Management
        Task<bool> ConnectAsync(NetworkProtocol protocol);
        Task DisconnectAsync(NetworkProtocol protocol);
        Task DisconnectAllAsync();
        bool IsConnected(NetworkProtocol protocol);
        ConnectionState GetConnectionState(NetworkProtocol protocol);
        
        // Protocol Specific Access
        T GetProtocolManager<T>() where T : class;
        
        // Events
        event Action<NetworkProtocol> OnProtocolConnected;
        event Action<NetworkProtocol> OnProtocolDisconnected;
        event Action<NetworkProtocol, string> OnProtocolError;
        
        // Statistics
        int TotalConnections { get; }
        int ActiveConnections { get; }
    }

    /// <summary>
    /// 프로토콜별 네트워크 매니저 인터페이스
    /// </summary>
    public interface IProtocolManager
    {
        NetworkProtocol Protocol { get; }
        ConnectionState State { get; }
        bool IsConnected { get; }
        
        Task<bool> ConnectAsync();
        Task DisconnectAsync();
        Task<bool> CheckHealthAsync();
        
        event Action OnConnected;
        event Action OnDisconnected;
        event Action<string> OnError;
    }

    /// <summary>
    /// 메시지 송수신 인터페이스
    /// </summary>
    public interface IMessageHandler
    {
        Task SendMessageAsync<T>(T message) where T : class;
        event Action<object> OnMessageReceived;
    }
}