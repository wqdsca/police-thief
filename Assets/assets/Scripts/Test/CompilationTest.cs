using System;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.DI;
using PoliceThief.Core.Events;
using PoliceThief.Core.Logging;
using PoliceThief.Core.Config;
using PoliceThief.Core.Pool;
using PoliceThief.Infrastructure.Network.Grpc;
using PoliceThief.Infrastructure.Network.Interfaces;
using PoliceThief.Infrastructure.Network.Core;
using PoliceThief.Game.Logic;
using PoliceThief.Presentation;

namespace PoliceThief.Test
{
    /// <summary>
    /// Compilation test to verify all dependencies are properly resolved
    /// </summary>
    public class CompilationTest : MonoBehaviour
    {
        public void TestAllSystems()
        {
            try
            {
                // Test ServiceLocator
                var serviceLocator = ServiceLocator.Instance;
                Log.Info("ServiceLocator test passed", "Test");
                
                // Test EventBus
                var eventBus = EventBus.Instance;
                eventBus.Publish(new NetworkConnectedEvent("test-url"));
                Log.Info("EventBus test passed", "Test");
                
                // Test ConfigManager
                var configManager = ConfigManager.Instance;
                var networkConfig = configManager.GetNetworkConfig();
                var grpcConfig = configManager.GetGrpcConfig();
                Log.Info("ConfigManager test passed", "Test");
                
                // Test ObjectPool
                var objectPool = ObjectPool.Instance;
                Log.Info("ObjectPool test passed", "Test");
                
                // Test gRPC Client (with mock config)
                var grpcClientConfig = new GrpcClientOptimized.ConnectionConfig
                {
                    ServerUrl = "http://localhost:50051",
                    ConnectTimeoutMs = 5000
                };
                var grpcClient = new GrpcClientOptimized(grpcClientConfig);
                Log.Info("GrpcClient test passed", "Test");
                
                // Test NetworkConnectionManager
                var networkManager = new NetworkConnectionManager();
                Log.Info("NetworkConnectionManager test passed", "Test");
                
                // Test GrpcProtocolManager
                var grpcProtocolManager = new PoliceThief.Infrastructure.Network.Core.GrpcProtocolManager(grpcClient);
                Log.Info("GrpcProtocolManager test passed", "Test");
                
                // Test GameManager access
                var gameManager = GameManager.Instance;
                if (gameManager != null)
                {
                    Log.Info("GameManager test passed", "Test");
                }
                
                Log.Info("All compilation tests passed successfully!", "Test");
            }
            catch (Exception ex)
            {
                Log.Error($"Compilation test failed: {ex.Message}", "Test");
                Log.Error($"Stack trace: {ex.StackTrace}", "Test");
            }
        }
    }
}