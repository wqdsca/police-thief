using System;
using System.Diagnostics;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.Events;
using PoliceThief.Core.Pool;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Core;
using Debug = UnityEngine.Debug;

namespace PoliceThief.Test
{
    /// <summary>
    /// Performance benchmark to validate mobile optimizations
    /// Compares original vs optimized implementations
    /// </summary>
    public class PerformanceBenchmark : MonoBehaviour
    {
        [Header("Benchmark Configuration")]
        [SerializeField] private int eventIterations = 10000;
        [SerializeField] private int poolIterations = 1000;
        [SerializeField] private int messageIterations = 5000;
        [SerializeField] private bool runOnStart = false;
        
        [Header("Test Objects")]
        [SerializeField] private GameObject testPrefab;
        
        private void Start()
        {
            if (runOnStart)
            {
                RunAllBenchmarks();
            }
        }
        
        [ContextMenu("Run All Benchmarks")]
        public async void RunAllBenchmarks()
        {
            Debug.Log("========== PERFORMANCE BENCHMARK START ==========");
            
            // Warm up
            System.GC.Collect();
            await Task.Delay(100);
            
            // Run benchmarks
            BenchmarkEventBus();
            await Task.Delay(100);
            
            BenchmarkObjectPool();
            await Task.Delay(100);
            
            BenchmarkNetworkMessage();
            await Task.Delay(100);
            
            BenchmarkLogging();
            await Task.Delay(100);
            
            BenchmarkAsyncOperations();
            
            Debug.Log("========== PERFORMANCE BENCHMARK COMPLETE ==========");
        }
        
        #region EventBus Benchmark
        
        [ContextMenu("Benchmark EventBus")]
        public void BenchmarkEventBus()
        {
            Debug.Log("--- EventBus Benchmark ---");
            
            // Original EventBus with reflection
            var originalBus = EventBus.Instance;
            var originalTime = BenchmarkOriginalEventBus(originalBus);
            
            // Optimized EventBus without reflection
            var optimizedBus = EventBusOptimized.Instance;
            var optimizedTime = BenchmarkOptimizedEventBus(optimizedBus);
            
            // Memory comparison
            var gcBefore = System.GC.CollectionCount(0);
            PublishManyEvents(originalBus);
            var gcAfterOriginal = System.GC.CollectionCount(0);
            
            PublishManyEventsOptimized(optimizedBus);
            var gcAfterOptimized = System.GC.CollectionCount(0);
            
            // Results
            Debug.Log($"Original EventBus: {originalTime}ms");
            Debug.Log($"Optimized EventBus: {optimizedTime}ms");
            Debug.Log($"Performance Improvement: {((originalTime - optimizedTime) / originalTime * 100):F1}%");
            Debug.Log($"GC Collections - Original: {gcAfterOriginal - gcBefore}, Optimized: {gcAfterOptimized - gcAfterOriginal}");
        }
        
        private long BenchmarkOriginalEventBus(EventBus bus)
        {
            var sw = Stopwatch.StartNew();
            
            // Subscribe
            var subscription = bus.Subscribe<TestEvent>(OnTestEvent);
            
            // Publish many events
            for (int i = 0; i < eventIterations; i++)
            {
                bus.Publish(new TestEvent { Value = i });
            }
            
            // Cleanup
            subscription.Dispose();
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        private long BenchmarkOptimizedEventBus(EventBusOptimized bus)
        {
            var sw = Stopwatch.StartNew();
            
            // Subscribe
            var subscription = bus.Subscribe<TestEvent>(OnTestEvent);
            
            // Publish many events
            for (int i = 0; i < eventIterations; i++)
            {
                bus.Publish(new TestEvent { Value = i });
            }
            
            // Test struct events (zero allocation)
            var structEvent = new GameStateUpdateEvent(1, 0.016f);
            for (int i = 0; i < eventIterations; i++)
            {
                bus.PublishGameStateUpdate(ref structEvent);
            }
            
            // Cleanup
            subscription.Dispose();
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        private void PublishManyEvents(EventBus bus)
        {
            for (int i = 0; i < 1000; i++)
            {
                bus.Publish(new GameStartEvent());
            }
        }
        
        private void PublishManyEventsOptimized(EventBusOptimized bus)
        {
            var structEvent = new GameStartEventStruct(false);
            for (int i = 0; i < 1000; i++)
            {
                bus.Publish(structEvent);
            }
        }
        
        private void OnTestEvent(TestEvent evt) { }
        
        #endregion
        
        #region ObjectPool Benchmark
        
        [ContextMenu("Benchmark ObjectPool")]
        public void BenchmarkObjectPool()
        {
            if (testPrefab == null)
            {
                Debug.LogWarning("Test prefab not assigned for ObjectPool benchmark");
                return;
            }
            
            Debug.Log("--- ObjectPool Benchmark ---");
            
            // Original ObjectPool
            var originalPool = ObjectPool.Instance;
            var originalTime = BenchmarkOriginalObjectPool(originalPool);
            
            // Optimized ObjectPool
            var optimizedPool = ObjectPoolOptimized.Instance;
            var optimizedTime = BenchmarkOptimizedObjectPool(optimizedPool);
            
            // Results
            Debug.Log($"Original ObjectPool: {originalTime}ms");
            Debug.Log($"Optimized ObjectPool: {optimizedTime}ms");
            Debug.Log($"Performance Improvement: {((originalTime - optimizedTime) / originalTime * 100):F1}%");
            
            // Cleanup
            originalPool.ClearAllPools();
            optimizedPool.ClearAllPools();
        }
        
        private long BenchmarkOriginalObjectPool(ObjectPool pool)
        {
            var sw = Stopwatch.StartNew();
            
            // Pre-warm
            pool.PreWarm(testPrefab, 10);
            
            // Get and return objects
            for (int i = 0; i < poolIterations; i++)
            {
                var obj = pool.Get(testPrefab);
                pool.Return(obj);
            }
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        private long BenchmarkOptimizedObjectPool(ObjectPoolOptimized pool)
        {
            var sw = Stopwatch.StartNew();
            
            // Pre-warm
            pool.PreWarm(testPrefab, 10);
            
            // Get and return objects (using int keys)
            for (int i = 0; i < poolIterations; i++)
            {
                var obj = pool.Get(testPrefab);
                pool.Return(obj);
            }
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        #endregion
        
        #region NetworkMessage Benchmark
        
        [ContextMenu("Benchmark NetworkMessage")]
        public void BenchmarkNetworkMessage()
        {
            Debug.Log("--- NetworkMessage Benchmark ---");
            
            var originalTime = BenchmarkOriginalNetworkMessage();
            var optimizedTime = BenchmarkOptimizedNetworkMessage();
            
            // Results
            Debug.Log($"Original NetworkMessage: {originalTime}ms");
            Debug.Log($"Optimized NetworkMessage: {optimizedTime}ms");
            Debug.Log($"Performance Improvement: {((originalTime - optimizedTime) / originalTime * 100):F1}%");
        }
        
        private long BenchmarkOriginalNetworkMessage()
        {
            var sw = Stopwatch.StartNew();
            var data = new byte[1024];
            
            for (int i = 0; i < messageIterations; i++)
            {
                var message = new NetworkMessage
                {
                    messageType = MessageType.GameData,
                    payload = data // Allocates new array reference
                };
            }
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        private long BenchmarkOptimizedNetworkMessage()
        {
            var sw = Stopwatch.StartNew();
            var data = new byte[1024];
            var segment = new ArraySegment<byte>(data);
            
            for (int i = 0; i < messageIterations; i++)
            {
                // Using pooled messages
                using (var message = PooledNetworkMessage.Get(MessageType.GameData, data, 0, data.Length))
                {
                    // Message is automatically returned to pool on dispose
                }
            }
            
            // Also test struct version (stack allocated)
            for (int i = 0; i < messageIterations; i++)
            {
                var message = new NetworkMessageOptimized();
                message.Initialize(MessageType.GameData, segment);
            }
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        #endregion
        
        #region Logging Benchmark
        
        [ContextMenu("Benchmark Logging")]
        public void BenchmarkLogging()
        {
            Debug.Log("--- Logging Benchmark ---");
            
            var originalTime = BenchmarkOriginalLogging();
            var optimizedTime = BenchmarkOptimizedLogging();
            
            // Results
            Debug.Log($"Original Logging: {originalTime}ms");
            Debug.Log($"Optimized Logging: {optimizedTime}ms");
            Debug.Log($"Performance Improvement: {((originalTime - optimizedTime) / originalTime * 100):F1}%");
        }
        
        private long BenchmarkOriginalLogging()
        {
            var sw = Stopwatch.StartNew();
            
            for (int i = 0; i < 1000; i++)
            {
                Log.Debug($"Debug message {i}", "Benchmark");
                Log.Info($"Info message {i}", "Benchmark");
            }
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        private long BenchmarkOptimizedLogging()
        {
            var sw = Stopwatch.StartNew();
            
            // These will be completely removed in release builds
            for (int i = 0; i < 1000; i++)
            {
                LogOptimized.Debug($"Debug message {i}", "Benchmark");
                LogOptimized.Info($"Info message {i}", "Benchmark");
            }
            
            sw.Stop();
            return sw.ElapsedMilliseconds;
        }
        
        #endregion
        
        #region Async Operations Benchmark
        
        [ContextMenu("Benchmark Async Operations")]
        public async void BenchmarkAsyncOperations()
        {
            Debug.Log("--- Async Operations Benchmark ---");
            
            // Test cancellation
            var asyncManager = Core.Async.AsyncManager.Instance;
            
            var sw = Stopwatch.StartNew();
            
            // Run multiple async operations with proper cancellation
            var tasks = new Task[10];
            for (int i = 0; i < tasks.Length; i++)
            {
                int index = i;
                tasks[i] = asyncManager.RunAsync(async (token) =>
                {
                    await Task.Delay(100, token);
                    Debug.Log($"Task {index} completed");
                }, $"BenchmarkTask_{i}");
            }
            
            await Task.WhenAll(tasks);
            
            sw.Stop();
            Debug.Log($"Async operations completed in: {sw.ElapsedMilliseconds}ms");
            
            // Test cancellation
            var cancelTask = asyncManager.RunAsync(async (token) =>
            {
                await Task.Delay(5000, token);
            }, "CancelTest");
            
            await Task.Delay(100);
            asyncManager.CancelSceneOperations();
            
            Debug.Log("Async cancellation test completed");
        }
        
        #endregion
        
        // Test event class
        private class TestEvent : GameEvent
        {
            public int Value { get; set; }
        }
    }
}