using System;
using System.Threading.Tasks;
using UnityEngine;
using Sirenix.OdinInspector;
using PoliceThief.Infrastructure.Network.Grpc;
using PoliceThief.Core.DI;
using PoliceThief.Core.Logging;

namespace PoliceThief.Test
{
    /// <summary>
    /// gRPC 연결 테스트 스크립트
    /// </summary>
    public class GrpcConnectionTest : MonoBehaviour
    {
        [Title("gRPC 연결 테스트")]
        [SerializeField] private bool _autoConnectOnStart = true;
        
        [ShowInInspector]
        [DisplayAsString]
        public string ConnectionStatus => _grpcClient?.ConnectionStatus ?? "클라이언트 없음";
        
        [ShowInInspector]
        [DisplayAsString] 
        public string ServerUrl => _grpcClient?.ServerUrl ?? "N/A";
        
        [ShowInInspector]
        [DisplayAsString]
        public float AverageLatency => _grpcClient?.AverageLatency ?? 0f;
        
        private GrpcClientOptimized _grpcClient;
        
        private void Start()
        {
            InitializeGrpcClient();
            
            if (_autoConnectOnStart)
            {
                ConnectToServer();
            }
        }
        
        /// <summary>
        /// gRPC 클라이언트 초기화
        /// </summary>
        private void InitializeGrpcClient()
        {
            try
            {
                _grpcClient = ServiceLocator.Instance.Get<GrpcClientOptimized>();
                
                // 이벤트 구독
                _grpcClient.OnConnected += OnConnected;
                _grpcClient.OnDisconnected += OnDisconnected;
                _grpcClient.OnError += OnError;
                
                Log.Info("gRPC 클라이언트 초기화 완료", "GrpcTest");
            }
            catch (Exception ex)
            {
                Log.Error($"gRPC 클라이언트 초기화 실패: {ex.Message}", "GrpcTest");
            }
        }
        
        /// <summary>
        /// 서버 연결 시작
        /// </summary>
        [Button("서버 연결", ButtonSizes.Large)]
        [EnableIf("@_grpcClient != null && !_grpcClient.IsConnected")]
        public async void ConnectToServer()
        {
            if (_grpcClient == null)
            {
                Log.Error("gRPC 클라이언트가 초기화되지 않았습니다", "GrpcTest");
                return;
            }
            
            try
            {
                Log.Info("서버 연결 시도 중...", "GrpcTest");
                var success = await _grpcClient.ConnectAsync();
                
                if (success)
                {
                    Log.Info("서버 연결 성공!", "GrpcTest");
                }
                else
                {
                    Log.Error("서버 연결 실패", "GrpcTest");
                }
            }
            catch (Exception ex)
            {
                Log.Error($"서버 연결 중 오류: {ex.Message}", "GrpcTest");
            }
        }
        
        /// <summary>
        /// 서버 연결 해제
        /// </summary>
        [Button("연결 해제", ButtonSizes.Large)]
        [EnableIf("@_grpcClient != null && _grpcClient.IsConnected")]
        public async void DisconnectFromServer()
        {
            if (_grpcClient == null) return;
            
            try
            {
                Log.Info("서버 연결 해제 중...", "GrpcTest");
                await _grpcClient.DisconnectAsync();
                Log.Info("서버 연결 해제 완료", "GrpcTest");
            }
            catch (Exception ex)
            {
                Log.Error($"연결 해제 중 오류: {ex.Message}", "GrpcTest");
            }
        }
        
        /// <summary>
        /// 연결 상태 확인
        /// </summary>
        [Button("연결 상태 확인")]
        public async void CheckConnectionHealth()
        {
            if (_grpcClient == null)
            {
                Log.Warning("gRPC 클라이언트가 없습니다", "GrpcTest");
                return;
            }
            
            try
            {
                var isHealthy = await _grpcClient.CheckHealthAsync();
                var status = isHealthy ? "정상" : "비정상";
                Log.Info($"연결 상태: {status}", "GrpcTest");
            }
            catch (Exception ex)
            {
                Log.Error($"상태 확인 실패: {ex.Message}", "GrpcTest");
            }
        }
        
        /// <summary>
        /// 서비스 테스트 호출
        /// </summary>
        [Button("서비스 테스트", ButtonSizes.Medium)]
        [EnableIf("@_grpcClient != null && _grpcClient.IsConnected")]
        public async void TestServiceCall()
        {
            if (_grpcClient == null || !_grpcClient.IsConnected)
            {
                Log.Warning("서버에 연결되지 않았습니다", "GrpcTest");
                return;
            }
            
            try
            {
                Log.Info("서비스 호출 테스트 시작...", "GrpcTest");
                
                // 간단한 재시도 로직 테스트
                var result = await _grpcClient.ExecuteWithRetryAsync(async () =>
                {
                    await Task.Delay(100); // 가짜 서비스 호출
                    return "테스트 성공";
                });
                
                Log.Info($"서비스 호출 결과: {result}", "GrpcTest");
            }
            catch (Exception ex)
            {
                Log.Error($"서비스 호출 실패: {ex.Message}", "GrpcTest");
            }
        }
        
        #region 이벤트 핸들러
        
        private void OnConnected()
        {
            Log.Info("🟢 gRPC 서버 연결됨", "GrpcTest");
        }
        
        private void OnDisconnected()
        {
            Log.Warning("🔴 gRPC 서버 연결 해제됨", "GrpcTest");
        }
        
        private void OnError(string errorMessage)
        {
            Log.Error($"⚠️ gRPC 오류: {errorMessage}", "GrpcTest");
        }
        
        #endregion
        
        private void OnDestroy()
        {
            if (_grpcClient != null)
            {
                _grpcClient.OnConnected -= OnConnected;
                _grpcClient.OnDisconnected -= OnDisconnected;
                _grpcClient.OnError -= OnError;
            }
        }
        
        #region 디버그 정보
        
        [Title("디버그 정보")]
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("총 연결 횟수")]
        public int TotalConnections => _grpcClient?.TotalConnections ?? 0;
        
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("총 오류 수")]
        public int TotalErrors => _grpcClient?.TotalErrors ?? 0;
        
        [ShowInInspector]
        [DisplayAsString]
        [LabelText("연결 시간")]
        public string ConnectionUptime => _grpcClient?.ConnectionUptime ?? "N/A";
        
        #endregion
    }
}