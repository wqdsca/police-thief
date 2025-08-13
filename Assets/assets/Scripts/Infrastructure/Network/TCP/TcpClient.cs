using System;
using System.Collections.Concurrent;
using System.IO;
using System.Linq;
using System.Net.Sockets;
using System.Threading;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.Config;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Core;

namespace PoliceThief.Infrastructure.Network.TCP
{
    /// <summary>
    /// 연결 관리 및 성능 기능을 갖춘 최적화된 TCP 클라이언트
    /// </summary>
    public class TcpClientOptimized : IDisposable
    {
        #region 이벤트
        public event Action OnConnected;  // 연결 완료
        public event Action OnDisconnected;  // 연결 해제
        public event Action<string> OnError;  // 오류 발생
        public event Action<NetworkMessage> OnMessageReceived;  // 메시지 수신
        #endregion
        
        #region 필드
        private readonly NetworkConfig _config;
        private TcpClient _tcpClient;
        private NetworkStream _networkStream;
        private ClientConnectionState _connectionState = ClientConnectionState.Disconnected;
        private CancellationTokenSource _cancellationTokenSource;
        
        // 메시지 관리
        private readonly ConcurrentQueue<NetworkMessage> _outgoingMessages = new();
        private readonly byte[] _receiveBuffer;
        
        // 통계
        private readonly NetworkStats _stats = new();
        private readonly object _statsLock = new object();
        
        // 작업
        private Task _sendTask;
        private Task _receiveTask;
        private Task _keepAliveTask;
        
        private readonly object _connectionLock = new object();
        private int _currentRetryCount = 0;
        #endregion
        
        public ClientConnectionState State => _connectionState;
        public NetworkStats Statistics => _stats;
        
        public TcpClientOptimized(NetworkConfig config)
        {
            _config = config ?? throw new ArgumentNullException(nameof(config));
            _receiveBuffer = new byte[_config.tcpBufferSize];
        }
        
        public async Task<bool> ConnectAsync()
        {
            lock (_connectionLock)
            {
                if (_connectionState != ClientConnectionState.Disconnected)
                {
                    Log.Warning($"이미 연결되었거나 연결 시도 중입니다. 상태: {_connectionState}", "TCP");
                    return false;
                }
                _connectionState = ClientConnectionState.Connecting;
            }
            
            try
            {
                _cancellationTokenSource = new CancellationTokenSource();
                _tcpClient = new TcpClient();
                
                // 소켓 옵션 설정 (NetworkConfig 값 사용)
                _tcpClient.ReceiveTimeout = _config.tcpTimeoutMs;
                _tcpClient.SendTimeout = _config.tcpTimeoutMs;
                _tcpClient.ReceiveBufferSize = _config.tcpBufferSize;
                _tcpClient.SendBufferSize = _config.tcpBufferSize;
                
                // TCP 전용 소켓 옵션
                _tcpClient.Client.SetSocketOption(SocketOptionLevel.Tcp, SocketOptionName.NoDelay, true);
                _tcpClient.Client.LingerState = new LingerOption(true, 0);
                
                // 타임아웃과 함께 연결 시도
                var connectTask = _tcpClient.ConnectAsync(_config.tcpHost, _config.tcpPort);
                var timeoutTask = Task.Delay(_config.tcpTimeoutMs);
                
                var completedTask = await Task.WhenAny(connectTask, timeoutTask);
                
                if (completedTask == timeoutTask || !_tcpClient.Connected)
                {
                    throw new TimeoutException($"{_config.GetTcpEndpoint()}에 연결 타임아웃");
                }
                
                _networkStream = _tcpClient.GetStream();
                
                // Keep-alive 활성화 시 설정
                if (_config.tcpEnableKeepalive)
                {
                    _tcpClient.Client.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.KeepAlive, true);
                }
                
                lock (_connectionLock)
                {
                    _connectionState = ClientConnectionState.Connected;
                    _currentRetryCount = 0; // 연결 성공 시 재시도 카운터 리셋
                }
                
                // 백그라운드 작업 시작
                StartBackgroundTasks();
                
                // 연결 메시지 전송
                var connectMessage = new NetworkMessage
                {
                    messageType = MessageType.Connect
                };
                await SendMessageAsync(connectMessage);
                
                Log.Info($"TCP 서버 {_config.GetTcpEndpoint()}에 성공적으로 연결되었습니다", "TCP");
                UnityMainThreadDispatcher.Instance.Enqueue(() => OnConnected?.Invoke());
                
                return true;
            }
            catch (Exception ex)
            {
                Log.Error($"TCP 연결 오류: {ex.Message}", "TCP");
                lock (_connectionLock)
                {
                    _connectionState = ClientConnectionState.Error;
                }
                
                // 재시도 로직
                _currentRetryCount++;
                if (_config.enableAutoReconnect && _currentRetryCount < _config.tcpMaxRetries)
                {
                    Log.Info($"TCP 연결 재시도 {_currentRetryCount}/{_config.tcpMaxRetries}", "TCP");
                    await Task.Delay(_config.reconnectDelayMs);
                    return await ConnectAsync();
                }
                
                UnityMainThreadDispatcher.Instance.Enqueue(() => OnError?.Invoke(ex.Message));
                await DisconnectAsync();
                return false;
            }
        }
        
        public async Task DisconnectAsync()
        {
            lock (_connectionLock)
            {
                if (_connectionState == ClientConnectionState.Disconnected)
                    return;
                    
                _connectionState = ClientConnectionState.Disconnecting;
            }
            
            try
            {
                // Send disconnect message if connected
                if (_networkStream != null)
                {
                    var disconnectMessage = new NetworkMessage
                    {
                        messageType = MessageType.Disconnect
                    };
                    
                    try
                    {
                        await SendMessageInternalAsync(disconnectMessage);
                        await Task.Delay(100); // Brief delay for message to be sent
                    }
                    catch
                    {
                        // Ignore errors during disconnect
                    }
                }
                
                // Cancel all background tasks
                _cancellationTokenSource?.Cancel();
                
                // Wait for tasks to complete with timeout
                var tasks = new[] { _sendTask, _receiveTask, _keepAliveTask }.Where(t => t != null).ToArray();
                if (tasks.Length > 0)
                {
                    try
                    {
                        await Task.WhenAll(tasks).ConfigureAwait(false);
                    }
                    catch
                    {
                        // Tasks may throw due to cancellation
                    }
                }
                
                // Cleanup network resources
                _networkStream?.Close();
                _networkStream?.Dispose();
                _networkStream = null;
                
                _tcpClient?.Close();
                _tcpClient?.Dispose();
                _tcpClient = null;
                
                // Clear message queues
                while (_outgoingMessages.TryDequeue(out _)) { }
                
                lock (_connectionLock)
                {
                    _connectionState = ClientConnectionState.Disconnected;
                }
                
                Log.Info("Disconnected from TCP server", "TCP");
                UnityMainThreadDispatcher.Instance.Enqueue(() => OnDisconnected?.Invoke());
            }
            catch (Exception ex)
            {
                Log.Error($"TCP disconnect error: {ex.Message}", "TCP");
                UnityMainThreadDispatcher.Instance.Enqueue(() => OnError?.Invoke(ex.Message));
            }
        }
        
        public Task<bool> SendMessageAsync(NetworkMessage message)
        {
            if (_connectionState != ClientConnectionState.Connected)
            {
                Log.Warning("메시지 전송 불가: 연결되지 않음", "TCP");
                return Task.FromResult(false);
            }
            
            _outgoingMessages.Enqueue(message);
            return Task.FromResult(true);
        }
        
        private void StartBackgroundTasks()
        {
            var token = _cancellationTokenSource.Token;
            
            _sendTask = Task.Run(async () => await SendLoop(token), token);
            _receiveTask = Task.Run(async () => await ReceiveLoop(token), token);
            
            if (_config.tcpEnableKeepalive)
            {
                _keepAliveTask = Task.Run(async () => await KeepAliveLoop(token), token);
            }
        }
        
        private async Task SendLoop(CancellationToken cancellationToken)
        {
            while (!cancellationToken.IsCancellationRequested && _connectionState == ClientConnectionState.Connected)
            {
                try
                {
                    if (_outgoingMessages.TryDequeue(out var message))
                    {
                        await SendMessageInternalAsync(message);
                        
                        lock (_statsLock)
                        {
                            _stats.totalMessagesSent++;
                            _stats.lastActivity = DateTime.UtcNow;
                        }
                    }
                    else
                    {
                        await Task.Delay(1, cancellationToken);
                    }
                }
                catch (Exception ex)
                {
                    if (!cancellationToken.IsCancellationRequested)
                    {
                        Log.Error($"TCP send loop error: {ex.Message}", "TCP");
                        await HandleConnectionError(ex.Message);
                    }
                    break;
                }
            }
        }
        
        private async Task ReceiveLoop(CancellationToken cancellationToken)
        {
            while (!cancellationToken.IsCancellationRequested && _connectionState == ClientConnectionState.Connected)
            {
                try
                {
                    if (_networkStream != null && _networkStream.CanRead)
                    {
                        // Read message length (4 bytes)
                        var lengthBuffer = new byte[4];
                        var bytesRead = await ReadExactAsync(_networkStream, lengthBuffer, 4, cancellationToken);
                        
                        if (bytesRead == 4)
                        {
                            var messageLength = BitConverter.ToInt32(lengthBuffer, 0);
                            
                            if (messageLength > 0 && messageLength <= _config.tcpBufferSize)
                            {
                                // Read message data
                                var messageBuffer = new byte[messageLength];
                                bytesRead = await ReadExactAsync(_networkStream, messageBuffer, messageLength, cancellationToken);
                                
                                if (bytesRead == messageLength)
                                {
                                    var message = DeserializeMessage(messageBuffer);
                                    if (message != null)
                                    {
                                        await HandleReceivedMessage(message);
                                    }
                                }
                            }
                        }
                    }
                    else
                    {
                        await Task.Delay(1, cancellationToken);
                    }
                }
                catch (Exception ex)
                {
                    if (!cancellationToken.IsCancellationRequested)
                    {
                        Log.Error($"TCP receive loop error: {ex.Message}", "TCP");
                        await HandleConnectionError(ex.Message);
                    }
                    break;
                }
            }
        }
        
        private async Task KeepAliveLoop(CancellationToken cancellationToken)
        {
            while (!cancellationToken.IsCancellationRequested && _connectionState == ClientConnectionState.Connected)
            {
                try
                {
                    var heartbeat = new NetworkMessage
                    {
                        messageType = MessageType.Heartbeat
                    };
                    
                    _outgoingMessages.Enqueue(heartbeat);
                    
                    await Task.Delay(_config.tcpKeepaliveIntervalMs, cancellationToken);
                }
                catch (Exception ex)
                {
                    if (!cancellationToken.IsCancellationRequested)
                        Log.Error($"TCP keep-alive error: {ex.Message}", "TCP");
                }
            }
        }
        
        private async Task HandleReceivedMessage(NetworkMessage message)
        {
            lock (_statsLock)
            {
                _stats.totalMessagesReceived++;
                _stats.lastActivity = DateTime.UtcNow;
            }
            
            switch (message.messageType)
            {
                case MessageType.Disconnect:
                    await DisconnectAsync();
                    break;
                    
                case MessageType.Heartbeat:
                    // Keep-alive received, connection is healthy
                    break;
                    
                default:
                    // Process the message on main thread
                    UnityMainThreadDispatcher.Instance.Enqueue(() => OnMessageReceived?.Invoke(message));
                    break;
            }
        }
        
        private async Task SendMessageInternalAsync(NetworkMessage message)
        {
            if (_networkStream == null || !_networkStream.CanWrite)
                throw new InvalidOperationException("Network stream is not writable");
                
            var messageData = SerializeMessage(message);
            var lengthBytes = BitConverter.GetBytes(messageData.Length);
            
            // Send length first (4 bytes), then message data
            await _networkStream.WriteAsync(lengthBytes, 0, 4);
            await _networkStream.WriteAsync(messageData, 0, messageData.Length);
            await _networkStream.FlushAsync();
        }
        
        private async Task<int> ReadExactAsync(NetworkStream stream, byte[] buffer, int count, CancellationToken cancellationToken)
        {
            int totalRead = 0;
            while (totalRead < count && !cancellationToken.IsCancellationRequested)
            {
                var bytesRead = await stream.ReadAsync(buffer, totalRead, count - totalRead, cancellationToken);
                if (bytesRead == 0)
                    break; // Connection closed
                    
                totalRead += bytesRead;
            }
            return totalRead;
        }
        
        private byte[] SerializeMessage(NetworkMessage message)
        {
            var json = JsonUtility.ToJson(message);
            var data = System.Text.Encoding.UTF8.GetBytes(json);
            
            // Enable compression for large messages (>512 bytes)
            if (_config.tcpEnableCompression && data.Length > 512)
            {
                return CompressData(data);
            }
            
            return data;
        }
        
        private byte[] CompressData(byte[] data)
        {
            using (var output = new System.IO.MemoryStream())
            {
                using (var gzip = new System.IO.Compression.GZipStream(output, System.IO.Compression.CompressionMode.Compress))
                {
                    gzip.Write(data, 0, data.Length);
                }
                return output.ToArray();
            }
        }
        
        private NetworkMessage DeserializeMessage(byte[] data)
        {
            try
            {
                // Try decompression if compression is enabled
                if (_config.tcpEnableCompression && IsCompressed(data))
                {
                    data = DecompressData(data);
                }
                
                var json = System.Text.Encoding.UTF8.GetString(data);
                return JsonUtility.FromJson<NetworkMessage>(json);
            }
            catch (Exception ex)
            {
                Log.Error($"Failed to deserialize TCP message: {ex.Message}", "TCP");
                return null;
            }
        }
        
        private byte[] DecompressData(byte[] data)
        {
            using (var input = new System.IO.MemoryStream(data))
            using (var gzip = new System.IO.Compression.GZipStream(input, System.IO.Compression.CompressionMode.Decompress))
            using (var output = new System.IO.MemoryStream())
            {
                gzip.CopyTo(output);
                return output.ToArray();
            }
        }
        
        private bool IsCompressed(byte[] data)
        {
            // GZip magic number: 0x1f, 0x8b
            return data.Length >= 2 && data[0] == 0x1f && data[1] == 0x8b;
        }
        
        private async Task HandleConnectionError(string errorMessage)
        {
            lock (_connectionLock)
            {
                if (_connectionState != ClientConnectionState.Connected)
                    return; // Already handling disconnection
                    
                _connectionState = ClientConnectionState.Error;
            }
            
            UnityMainThreadDispatcher.Instance.Enqueue(() => OnError?.Invoke(errorMessage));
            
            if (_config.enableAutoReconnect)
            {
                Log.Info("Attempting to reconnect...", "TCP");
                await Task.Delay(_config.reconnectDelayMs);
                await ConnectAsync();
            }
            else
            {
                await DisconnectAsync();
            }
        }
        
        public void Dispose()
        {
            DisconnectAsync().Wait(5000); // Wait max 5 seconds for cleanup
            _cancellationTokenSource?.Dispose();
        }
    }
}