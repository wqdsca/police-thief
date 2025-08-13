using System;
using System.Collections.Concurrent;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using UnityEngine;
using UnityEngine.Networking;
using System.Collections;
using System.Text;
using PoliceThief.Core.Config;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Core;

namespace PoliceThief.Infrastructure.Network.QUIC
{
    /// <summary>
    /// Unity-compatible QUIC client implementation
    /// Uses UnityWebRequest for compatibility and HTTP/2 as fallback
    /// </summary>
    public class QuicClient : MonoBehaviour, IDisposable
    {
        #region Events
        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnError;
        public event Action<NetworkMessage> OnMessageReceived;
        #endregion
        
        #region Fields
        private NetworkConfig _config;
        private string _serverUrl;
        private ClientConnectionState _connectionState = ClientConnectionState.Disconnected;
        private CancellationTokenSource _cancellationTokenSource;
        
        // Message management
        private int _sequenceNumber = 0;
        private readonly ConcurrentQueue<NetworkMessage> _outgoingMessages = new();
        
        // Statistics
        public NetworkStatsOptimized Statistics { get; private set; } = new NetworkStatsOptimized();
        
        // Connection info
        private string _connectionId;
        private string _sessionTicket;
        
        // Coroutine management
        private Coroutine _receiveCoroutine;
        private Coroutine _keepAliveCoroutine;
        #endregion
        
        #region Unity Lifecycle
        
        void Awake()
        {
            Statistics.Reset();
        }
        
        void OnDestroy()
        {
            Dispose();
        }
        
        #endregion
        
        #region Constructor Alternative
        
        public void Initialize(NetworkConfig config)
        {
            _config = config ?? throw new ArgumentNullException(nameof(config));
        }
        
        #endregion
        
        #region Connection Management
        
        public async Task<bool> ConnectAsync(string serverUrl)
        {
            if (_connectionState != ClientConnectionState.Disconnected)
            {
                Debug.LogWarning("[QuicClient] Already connected or connecting");
                return false;
            }
            
            try
            {
                _connectionState = ClientConnectionState.Connecting;
                _serverUrl = serverUrl;
                _cancellationTokenSource = new CancellationTokenSource();
                
                // Try 0-RTT connection if we have a session ticket
                bool connected = false;
                if (!string.IsNullOrEmpty(_sessionTicket))
                {
                    connected = await TryZeroRttConnection();
                }
                
                // Fall back to 1-RTT connection
                if (!connected)
                {
                    connected = await PerformHandshake();
                }
                
                if (connected)
                {
                    _connectionState = ClientConnectionState.Connected;
                    StartBackgroundTasks();
                    OnConnected?.Invoke();
                    Debug.Log($"[QuicClient] Connected to {serverUrl}");
                    return true;
                }
                
                _connectionState = ClientConnectionState.Error;
                return false;
            }
            catch (Exception ex)
            {
                _connectionState = ClientConnectionState.Error;
                OnError?.Invoke($"Connection failed: {ex.Message}");
                Debug.LogError($"[QuicClient] Connection failed: {ex}");
                return false;
            }
        }
        
        private async Task<bool> TryZeroRttConnection()
        {
            try
            {
                // Unity-compatible version using UnityWebRequest
                var tcs = new TaskCompletionSource<bool>();
                StartCoroutine(SendConnectRequest($"{_serverUrl}/connect", true, tcs));
                return await tcs.Task;
            }
            catch (Exception ex)
            {
                Debug.LogWarning($"[QuicClient] 0-RTT failed, falling back to 1-RTT: {ex.Message}");
            }
            
            return false;
        }
        
        private async Task<bool> PerformHandshake()
        {
            try
            {
                var tcs = new TaskCompletionSource<bool>();
                StartCoroutine(SendConnectRequest($"{_serverUrl}/connect", false, tcs));
                return await tcs.Task;
            }
            catch (Exception ex)
            {
                Debug.LogError($"[QuicClient] Handshake error: {ex}");
                return false;
            }
        }
        
        private IEnumerator SendConnectRequest(string url, bool earlyData, TaskCompletionSource<bool> tcs)
        {
            var requestData = new ConnectionRequest
            {
                sessionTicket = earlyData ? _sessionTicket : null,
                earlyData = earlyData,
                version = Application.version,
                platform = Application.platform.ToString()
            };
            
            string json = JsonUtility.ToJson(requestData);
            byte[] bodyRaw = Encoding.UTF8.GetBytes(json);
            
            using (UnityWebRequest www = new UnityWebRequest(url, "POST"))
            {
                www.uploadHandler = new UploadHandlerRaw(bodyRaw);
                www.downloadHandler = new DownloadHandlerBuffer();
                www.SetRequestHeader("Content-Type", "application/json");
                
                if (earlyData)
                {
                    www.SetRequestHeader("Early-Data", "1");
                }
                
                yield return www.SendWebRequest();
                
                if (www.result == UnityWebRequest.Result.Success)
                {
                    var response = JsonUtility.FromJson<ConnectionResponse>(www.downloadHandler.text);
                    _connectionId = response.connectionId;
                    _sessionTicket = response.sessionTicket;
                    
                    // Store session ticket for 0-RTT next time
                    StoreSessionTicket();
                    
                    Debug.Log($"[QuicClient] Connection successful (0-RTT: {earlyData})");
                    tcs.SetResult(true);
                }
                else
                {
                    Debug.LogError($"[QuicClient] Connect failed: {www.error}");
                    tcs.SetResult(false);
                }
            }
        }
        
        public async Task DisconnectAsync()
        {
            if (_connectionState == ClientConnectionState.Disconnected)
                return;
            
            try
            {
                _connectionState = ClientConnectionState.Disconnecting;
                
                // Send disconnect message
                if (!string.IsNullOrEmpty(_connectionId))
                {
                    StartCoroutine(SendDisconnectRequest());
                }
                
                _cancellationTokenSource?.Cancel();
                
                // Stop coroutines
                if (_receiveCoroutine != null)
                {
                    StopCoroutine(_receiveCoroutine);
                    _receiveCoroutine = null;
                }
                
                if (_keepAliveCoroutine != null)
                {
                    StopCoroutine(_keepAliveCoroutine);
                    _keepAliveCoroutine = null;
                }
                
                _connectionState = ClientConnectionState.Disconnected;
                OnDisconnected?.Invoke();
                
                Debug.Log("[QuicClient] Disconnected");
                
                await Task.CompletedTask;
            }
            catch (Exception ex)
            {
                Debug.LogError($"[QuicClient] Disconnect error: {ex}");
            }
        }
        
        private IEnumerator SendDisconnectRequest()
        {
            string url = $"{_serverUrl}/disconnect";
            
            using (UnityWebRequest www = new UnityWebRequest(url, "POST"))
            {
                byte[] bodyRaw = Encoding.UTF8.GetBytes(_connectionId);
                www.uploadHandler = new UploadHandlerRaw(bodyRaw);
                www.downloadHandler = new DownloadHandlerBuffer();
                
                yield return www.SendWebRequest();
            }
        }
        
        #endregion
        
        #region Message Handling
        
        public async Task<bool> SendAsync(NetworkMessage message)
        {
            if (_connectionState != ClientConnectionState.Connected)
                return false;
            
            try
            {
                message.sequenceNumber = (uint)Interlocked.Increment(ref _sequenceNumber);
                
                var tcs = new TaskCompletionSource<bool>();
                StartCoroutine(SendMessageCoroutine(message, tcs));
                return await tcs.Task;
            }
            catch (Exception ex)
            {
                OnError?.Invoke($"Send failed: {ex.Message}");
                return false;
            }
        }
        
        private IEnumerator SendMessageCoroutine(NetworkMessage message, TaskCompletionSource<bool> tcs)
        {
            var messageData = new MessageData
            {
                connectionId = _connectionId,
                messageId = message.messageId,
                messageType = message.messageType.ToString(),
                sequenceNumber = message.sequenceNumber,
                payload = message.payload != null ? Convert.ToBase64String(message.payload) : null
            };
            
            string json = JsonUtility.ToJson(messageData);
            byte[] bodyRaw = Encoding.UTF8.GetBytes(json);
            
            string url = $"{_serverUrl}/message";
            
            using (UnityWebRequest www = new UnityWebRequest(url, "POST"))
            {
                www.uploadHandler = new UploadHandlerRaw(bodyRaw);
                www.downloadHandler = new DownloadHandlerBuffer();
                www.SetRequestHeader("Content-Type", "application/json");
                
                yield return www.SendWebRequest();
                
                if (www.result == UnityWebRequest.Result.Success)
                {
                    var stats = Statistics;
                    stats.IncrementSent(bodyRaw.Length);
                    Statistics = stats;
                    tcs.SetResult(true);
                }
                else
                {
                    Debug.LogWarning($"[QuicClient] Send failed: {www.error}");
                    tcs.SetResult(false);
                }
            }
        }
        
        public void Send(NetworkMessage message)
        {
            _ = SendAsync(message);
        }
        
        #endregion
        
        #region Background Tasks
        
        private void StartBackgroundTasks()
        {
            // Start receive loop
            _receiveCoroutine = StartCoroutine(ReceiveLoop());
            
            // Start keep-alive
            _keepAliveCoroutine = StartCoroutine(KeepAliveLoop());
        }
        
        private IEnumerator ReceiveLoop()
        {
            while (_connectionState == ClientConnectionState.Connected)
            {
                string url = $"{_serverUrl}/stream/{_connectionId}";
                
                using (UnityWebRequest www = UnityWebRequest.Get(url))
                {
                    www.timeout = 30;
                    yield return www.SendWebRequest();
                    
                    if (www.result == UnityWebRequest.Result.Success)
                    {
                        ProcessMessages(www.downloadHandler.text);
                    }
                    else if (!_cancellationTokenSource.Token.IsCancellationRequested)
                    {
                        OnError?.Invoke($"Receive error: {www.error}");
                        yield return new WaitForSeconds(1f); // Retry delay
                    }
                }
                
                yield return null;
            }
        }
        
        private IEnumerator KeepAliveLoop()
        {
            while (_connectionState == ClientConnectionState.Connected)
            {
                yield return new WaitForSeconds(30f);
                
                var message = new NetworkMessage
                {
                    messageType = MessageType.Heartbeat,
                    payload = BitConverter.GetBytes(DateTime.UtcNow.Ticks)
                };
                
                _ = SendAsync(message);
            }
        }
        
        private void ProcessMessages(string data)
        {
            try
            {
                // Handle multiple messages in response
                string[] lines = data.Split('\n');
                foreach (string line in lines)
                {
                    if (!string.IsNullOrEmpty(line) && line.StartsWith("data: "))
                    {
                        ProcessMessage(line.Substring(6));
                    }
                }
            }
            catch (Exception ex)
            {
                Debug.LogError($"[QuicClient] Failed to process messages: {ex}");
            }
        }
        
        private void ProcessMessage(string json)
        {
            try
            {
                var data = JsonUtility.FromJson<MessageData>(json);
                
                var message = new NetworkMessage
                {
                    messageId = data.messageId,
                    messageType = Enum.Parse<MessageType>(data.messageType),
                    sequenceNumber = data.sequenceNumber,
                    timestamp = new DateTime(data.timestamp),
                    payload = !string.IsNullOrEmpty(data.payload) ? 
                        Convert.FromBase64String(data.payload) : null
                };
                
                var stats = Statistics;
                stats.IncrementReceived(json.Length);
                Statistics = stats;
                
                OnMessageReceived?.Invoke(message);
            }
            catch (Exception ex)
            {
                Debug.LogError($"[QuicClient] Failed to process message: {ex}");
            }
        }
        
        #endregion
        
        #region Helper Methods
        
        private void StoreSessionTicket()
        {
            if (!string.IsNullOrEmpty(_sessionTicket))
            {
                PlayerPrefs.SetString($"QUIC_SessionTicket_{_serverUrl}", _sessionTicket);
                PlayerPrefs.Save();
            }
        }
        
        private void LoadSessionTicket()
        {
            _sessionTicket = PlayerPrefs.GetString($"QUIC_SessionTicket_{_serverUrl}", "");
        }
        
        #endregion
        
        #region IDisposable
        
        public void Dispose()
        {
            _ = DisconnectAsync();
            _cancellationTokenSource?.Dispose();
        }
        
        #endregion
        
        #region Inner Types
        
        [Serializable]
        private class ConnectionRequest
        {
            public string sessionTicket;
            public bool earlyData;
            public string version;
            public string platform;
        }
        
        [Serializable]
        private class ConnectionResponse
        {
            public string connectionId;
            public string sessionTicket;
        }
        
        [Serializable]
        private class MessageData
        {
            public string connectionId;
            public uint messageId;
            public string messageType;
            public uint sequenceNumber;
            public long timestamp;
            public string payload;
        }
        
        #endregion
    }
    
    // Alternative non-MonoBehaviour version for ServiceLocator
    public class QuicClientNonMono : IDisposable
    {
        private QuicClient _internalClient;
        private GameObject _clientObject;
        
        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnError;
        public event Action<NetworkMessage> OnMessageReceived;
        
        public NetworkStatsOptimized Statistics => _internalClient?.Statistics ?? new NetworkStatsOptimized();
        
        public QuicClientNonMono(NetworkConfig config)
        {
            // Create GameObject to host MonoBehaviour
            _clientObject = new GameObject("QuicClient");
            UnityEngine.Object.DontDestroyOnLoad(_clientObject);
            
            _internalClient = _clientObject.AddComponent<QuicClient>();
            _internalClient.Initialize(config);
            
            // Forward events
            _internalClient.OnConnected += () => OnConnected?.Invoke();
            _internalClient.OnDisconnected += () => OnDisconnected?.Invoke();
            _internalClient.OnError += (error) => OnError?.Invoke(error);
            _internalClient.OnMessageReceived += (msg) => OnMessageReceived?.Invoke(msg);
        }
        
        public Task<bool> ConnectAsync(string serverUrl)
        {
            return _internalClient.ConnectAsync(serverUrl);
        }
        
        public Task DisconnectAsync()
        {
            return _internalClient.DisconnectAsync();
        }
        
        public Task<bool> SendAsync(NetworkMessage message)
        {
            return _internalClient.SendAsync(message);
        }
        
        public void Send(NetworkMessage message)
        {
            _internalClient.Send(message);
        }
        
        public void Dispose()
        {
            if (_clientObject != null)
            {
                UnityEngine.Object.Destroy(_clientObject);
                _clientObject = null;
                _internalClient = null;
            }
        }
    }
}