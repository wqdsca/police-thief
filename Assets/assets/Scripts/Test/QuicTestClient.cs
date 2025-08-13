using System;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.Config;
using PoliceThief.Core.DI;
using PoliceThief.Infrastructure.Network.Core;
using PoliceThief.Infrastructure.Network.QUIC;
using PoliceThief.Infrastructure.Network.Interfaces;

namespace PoliceThief.Test
{
    /// <summary>
    /// QUIC 프로토콜 테스트 클라이언트
    /// UDP를 QUIC로 완전히 대체한 구현 예제
    /// </summary>
    public class QuicTestClient : MonoBehaviour
    {
        [Header("QUIC Connection Settings")]
        [SerializeField] private string serverUrl = "https://localhost:5000";
        [SerializeField] private bool autoConnect = false;
        
        private QuicClientNonMono _quicClient;
        private NetworkConfig _config;
        private INetworkManager _networkManager;
        
        private void Start()
        {
            InitializeQuicClient();
            
            if (autoConnect)
            {
                ConnectToServer();
            }
        }
        
        private void InitializeQuicClient()
        {
            // NetworkConfig 가져오기
            _config = ServiceLocator.Instance.Get<NetworkConfig>();
            if (_config == null)
            {
                _config = new NetworkConfig
                {
                    quicHost = "localhost",
                    quicPort = 5000,
                    connectTimeoutMs = 5000,
                    enable0Rtt = true,
                    enableConnectionMigration = true
                };
            }
            
            // NetworkManager 가져오기
            _networkManager = ServiceLocator.Instance.Get<INetworkManager>();
        }
        
        [ContextMenu("Connect to QUIC Server")]
        public async void ConnectToServer()
        {
            try
            {
                Debug.Log($"[QuicTest] QUIC 서버 연결 시작... URL: {serverUrl}");
                
                // NetworkManager를 통한 연결 (serverUrl은 NetworkConfig에서 사용)
                var connected = await _networkManager.ConnectAsync(NetworkProtocol.QUIC);
                
                if (connected)
                {
                    Debug.Log("[QuicTest] QUIC 서버 연결 성공!");
                    
                    // QuicClient 직접 접근이 필요한 경우
                    var quicManager = _networkManager.GetProtocolManager<QuicProtocolManager>();
                    if (quicManager != null)
                    {
                        _quicClient = quicManager.GetClient();
                        SetupEventHandlers();
                    }
                }
                else
                {
                    Debug.LogError("[QuicTest] QUIC 서버 연결 실패");
                }
            }
            catch (Exception ex)
            {
                Debug.LogError($"[QuicTest] 연결 오류: {ex.Message}");
            }
        }
        
        [ContextMenu("Disconnect from QUIC Server")]
        public async void DisconnectFromServer()
        {
            try
            {
                Debug.Log("[QuicTest] QUIC 서버 연결 해제 중...");
                await _networkManager.DisconnectAsync(NetworkProtocol.QUIC);
                Debug.Log("[QuicTest] QUIC 서버 연결 해제 완료");
            }
            catch (Exception ex)
            {
                Debug.LogError($"[QuicTest] 연결 해제 오류: {ex.Message}");
            }
        }
        
        [ContextMenu("Send Test Message")]
        public async void SendTestMessage()
        {
            if (_quicClient == null)
            {
                Debug.LogWarning("[QuicTest] QUIC 클라이언트가 연결되지 않았습니다");
                return;
            }
            
            var message = new NetworkMessage
            {
                messageType = MessageType.GameData,
                payload = System.Text.Encoding.UTF8.GetBytes($"Test message at {DateTime.Now}")
            };
            
            var sent = await _quicClient.SendAsync(message);
            if (sent)
            {
                Debug.Log($"[QuicTest] 메시지 전송 성공: {message.messageId}");
            }
            else
            {
                Debug.LogError("[QuicTest] 메시지 전송 실패");
            }
        }
        
        [ContextMenu("Send Bulk Messages")]
        public async void SendBulkMessages()
        {
            if (_quicClient == null)
            {
                Debug.LogWarning("[QuicTest] QUIC 클라이언트가 연결되지 않았습니다");
                return;
            }
            
            Debug.Log("[QuicTest] 대량 메시지 전송 시작...");
            
            var tasks = new Task<bool>[100];
            for (int i = 0; i < tasks.Length; i++)
            {
                var message = new NetworkMessage
                {
                    messageType = MessageType.GameData,
                    payload = System.Text.Encoding.UTF8.GetBytes($"Bulk message {i}")
                };
                
                tasks[i] = _quicClient.SendAsync(message);
            }
            
            var results = await Task.WhenAll(tasks);
            var successCount = 0;
            foreach (var result in results)
            {
                if (result) successCount++;
            }
            
            Debug.Log($"[QuicTest] 대량 메시지 전송 완료: {successCount}/{tasks.Length} 성공");
        }
        
        [ContextMenu("Check Connection Status")]
        public void CheckConnectionStatus()
        {
            if (_networkManager == null)
            {
                Debug.LogWarning("[QuicTest] NetworkManager가 초기화되지 않았습니다");
                return;
            }
            
            var isConnected = _networkManager.IsConnected(NetworkProtocol.QUIC);
            var state = _networkManager.GetConnectionState(NetworkProtocol.QUIC);
            
            Debug.Log($"[QuicTest] QUIC 연결 상태: {state}, 연결됨: {isConnected}");
            
            if (_quicClient != null)
            {
                var stats = _quicClient.Statistics;
                Debug.Log($"[QuicTest] 통계 - 전송: {stats.GetTotalBytesSent():N0}, 수신: {stats.GetTotalBytesReceived():N0}");
            }
        }
        
        private void SetupEventHandlers()
        {
            if (_quicClient == null) return;
            
            _quicClient.OnConnected += OnConnected;
            _quicClient.OnDisconnected += OnDisconnected;
            _quicClient.OnError += OnError;
            _quicClient.OnMessageReceived += OnMessageReceived;
        }
        
        private void OnConnected()
        {
            Debug.Log("[QuicTest] ✅ QUIC 연결 이벤트 수신");
        }
        
        private void OnDisconnected()
        {
            Debug.Log("[QuicTest] ❌ QUIC 연결 해제 이벤트 수신");
        }
        
        private void OnError(string error)
        {
            Debug.LogError($"[QuicTest] ⚠️ QUIC 오류: {error}");
        }
        
        private void OnMessageReceived(NetworkMessage message)
        {
            var payload = message.payload != null ? 
                System.Text.Encoding.UTF8.GetString(message.payload) : "No payload";
            Debug.Log($"[QuicTest] 📥 메시지 수신 - Type: {message.messageType}, Payload: {payload}");
        }
        
        private void OnDestroy()
        {
            if (_quicClient != null)
            {
                _quicClient.OnConnected -= OnConnected;
                _quicClient.OnDisconnected -= OnDisconnected;
                _quicClient.OnError -= OnError;
                _quicClient.OnMessageReceived -= OnMessageReceived;
            }
            
            // Disconnect on destroy
            DisconnectFromServer();
        }
        
        private void OnApplicationPause(bool pauseStatus)
        {
            // 모바일에서 앱이 백그라운드로 가는 경우 처리
            if (pauseStatus)
            {
                Debug.Log("[QuicTest] 앱이 백그라운드로 전환됨 - QUIC는 연결 마이그레이션 지원");
            }
            else
            {
                Debug.Log("[QuicTest] 앱이 포그라운드로 복귀 - QUIC 연결 상태 확인");
                CheckConnectionStatus();
            }
        }
    }
}