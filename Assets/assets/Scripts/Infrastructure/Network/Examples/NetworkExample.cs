using System;
using System.Threading.Tasks;
using UnityEngine;
using Sirenix.OdinInspector;
using PoliceThief.Core.DI;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Core;
using PoliceThief.Infrastructure.Network.Interfaces;

namespace PoliceThief.Infrastructure.Network.Examples
{
    /// <summary>
    /// 새로운 아키텍처 기반 네트워크 연결 테스트 예제
    /// </summary>
    public class NetworkExample : MonoBehaviour
    {
        [Title("네트워크 테스트")]
        [SerializeField] private string testMessage = "Unity에서 안녕하세요!";
        [SerializeField] private bool autoConnect = false;
        
        private INetworkManager _networkManager;
        
        private void Start()
        {
            InitializeExample();
            
            if (autoConnect)
            {
                ConnectToGrpcServer();
            }
        }
        
        private void InitializeExample()
        {
            _networkManager = ServiceLocator.Instance.Get<INetworkManager>();
            
            if (_networkManager == null)
            {
                Log.Error("ServiceLocator에서 INetworkManager를 찾을 수 없습니다", "NetworkExample");
                return;
            }
            
            Log.Info("NetworkExample이 새로운 아키텍처로 초기화되었습니다", "NetworkExample");
        }
        
        [Button("gRPC 서버에 연결", ButtonSizes.Large)]
        public async void ConnectToGrpcServer()
        {
            if (_networkManager == null)
            {
                Log.Error("NetworkManager is null", "NetworkExample");
                return;
            }
            
            try
            {
                var result = await _networkManager.ConnectAsync(NetworkProtocol.GRPC);
                if (result)
                {
                    Log.Info("gRPC 서버 연결 성공", "NetworkExample");
                }
                else
                {
                    Log.Error("gRPC 서버 연결 실패", "NetworkExample");
                }
            }
            catch (Exception ex)
            {
                Log.Error($"gRPC 연결 중 오류: {ex.Message}", "NetworkExample");
            }
        }
        
        [Button("연결 해제")]
        public async void DisconnectFromServer()
        {
            try
            {
                Log.Info("연결을 해제합니다", "NetworkExample");
                // TODO: Disconnect 기능 구현 필요
                await Task.Delay(100);
            }
            catch (Exception ex)
            {
                Log.Error($"연결 해제 중 오류: {ex.Message}", "NetworkExample");
            }
        }
        
        [Button("네트워크 상태 확인")]
        public void CheckNetworkStatus()
        {
            if (_networkManager == null)
            {
                Log.Warning("NetworkManager가 초기화되지 않았습니다", "NetworkExample");
                return;
            }
            
            // TODO: 연결 상태 확인 기능 추가
            Log.Info("네트워크 상태 확인 - 구현 예정", "NetworkExample");
        }
        
        [Button("테스트 데이터 전송")]
        public async void SendTestData()
        {
            if (_networkManager == null)
            {
                Log.Error("NetworkManager is null", "NetworkExample");
                return;
            }
            
            try
            {
                var gameData = new
                {
                    playerId = "player123",
                    position = new { x = 10.5f, y = 2.3f, z = -5.1f },
                    health = 85,
                    message = testMessage,
                    timestamp = DateTime.UtcNow.ToString("yyyy-MM-dd HH:mm:ss")
                };
                
                var json = JsonUtility.ToJson(gameData);
                Log.Info($"테스트 데이터 준비: {json}", "NetworkExample");
                
                // TODO: 실제 데이터 전송 구현 필요
                await Task.Delay(100);
                Log.Info("테스트 데이터 전송 완료", "NetworkExample");
            }
            catch (Exception ex)
            {
                Log.Error($"데이터 전송 중 오류: {ex.Message}", "NetworkExample");
            }
        }
    }
}