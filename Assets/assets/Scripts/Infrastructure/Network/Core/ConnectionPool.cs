using System;
using System.Collections.Concurrent;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.Config;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.TCP;
using PoliceThief.Infrastructure.Network.QUIC;

namespace PoliceThief.Infrastructure.Network.Core
{
    /// <summary>
    /// 최적화된 네트워크 리소스 관리를 위한 연결 풀
    /// </summary>
    public class ConnectionPool : IDisposable
    {
        private readonly NetworkConfig _config;
        private readonly ConcurrentQueue<TcpClientOptimized> _tcpPool = new();
        private readonly ConcurrentQueue<QuicClientNonMono> _quicPool = new();
        private readonly object _poolLock = new object();
        
        private int _tcpPoolSize = 0;
        private int _quicPoolSize = 0;
        
        public ConnectionPool(NetworkConfig config)
        {
            _config = config ?? throw new ArgumentNullException(nameof(config));
        }
        
        public async Task<TcpClientOptimized> GetTcpClientAsync()
        {
            if (_tcpPool.TryDequeue(out var client))
            {
                if (client.State == ClientConnectionState.Connected)
                {
                    return client;
                }
                else
                {
                    client.Dispose();
                    lock (_poolLock) _tcpPoolSize--;
                }
            }
            
            // 새로운 클라이언트 생성
            client = new TcpClientOptimized(_config);
            var connected = await client.ConnectAsync();
            
            if (connected)
            {
                Log.Info("새로운 TCP 클라이언트가 생성되고 연결되었습니다", "ConnectionPool");
                return client;
            }
            
            client.Dispose();
            return null;
        }
        
        public async Task<QuicClientNonMono> GetQuicClientAsync()
        {
            if (_quicPool.TryDequeue(out var client))
            {
                // QUIC doesn't have State property in same way, check if it's not null
                // and rely on connection test
                if (client != null)
                {
                    return client;
                }
                else
                {
                    client?.Dispose();
                    lock (_poolLock) _quicPoolSize--;
                }
            }
            
            // 새로운 클라이언트 생성
            client = new QuicClientNonMono(_config);
            var connected = await client.ConnectAsync(_config.GetQuicEndpoint());
            
            if (connected)
            {
                Log.Info("새로운 QUIC 클라이언트가 생성되고 연결되었습니다", "ConnectionPool");
                return client;
            }
            
            client.Dispose();
            return null;
        }
        
        public void ReturnTcpClient(TcpClientOptimized client)
        {
            if (client?.State == ClientConnectionState.Connected)
            {
                lock (_poolLock)
                {
                    if (_tcpPoolSize < _config.connectionPoolSize)
                    {
                        _tcpPool.Enqueue(client);
                        _tcpPoolSize++;
                        return;
                    }
                }
            }
            
            client?.Dispose();
        }
        
        public void ReturnQuicClient(QuicClientNonMono client)
        {
            if (client != null)
            {
                lock (_poolLock)
                {
                    if (_quicPoolSize < _config.connectionPoolSize)
                    {
                        _quicPool.Enqueue(client);
                        _quicPoolSize++;
                        return;
                    }
                }
            }
            
            client?.Dispose();
        }
        
        public void Dispose()
        {
            // Dispose all pooled connections
            while (_tcpPool.TryDequeue(out var tcpClient))
            {
                tcpClient.Dispose();
            }
            
            while (_quicPool.TryDequeue(out var quicClient))
            {
                quicClient.Dispose();
            }
            
            Log.Info("연결 풀이 해제되었습니다", "ConnectionPool");
        }
    }
}