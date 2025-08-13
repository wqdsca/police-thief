using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using Sirenix.OdinInspector;
using System.Collections.Concurrent;
using Cysharp.Threading.Tasks;
using System.Threading;

namespace PoliceThief.Core.Pool
{
    /// <summary>
    /// High-performance object pooling system with automatic management
    /// Converted from MonoBehaviour to pure C# class for better performance
    /// </summary>
    public sealed class ObjectPool
    {
        private static ObjectPool _instance;
        private static readonly object _lock = new object();
        
        public static ObjectPool Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_lock)
                    {
                        if (_instance == null)
                        {
                            _instance = new ObjectPool();
                        }
                    }
                }
                return _instance;
            }
        }
        
        [Title("Pool Configuration")]
        private int _defaultPoolSize = 10;
        
        private int _maxPoolSize = 100;
        
        private bool _autoExpand = true;
        
        private float _cleanupInterval = 60f; // Cleanup unused objects every 60 seconds
        
        [Title("Pool Statistics")]
        [ShowInInspector]
        [DisplayAsString]
        private int TotalPools => _pools.Count;
        
        [ShowInInspector]
        [DisplayAsString]
        private int TotalObjects => GetTotalObjectCount();
        
        [ShowInInspector]
        [DisplayAsString]
        private int ActiveObjects => GetActiveObjectCount();
        
        [ShowInInspector]
        [ProgressBar(0, 100)]
        private float PoolUtilization => TotalObjects > 0 ? (ActiveObjects / (float)TotalObjects) * 100f : 0f;
        
        private readonly ConcurrentDictionary<string, Pool> _pools = new ConcurrentDictionary<string, Pool>();
        private float _lastCleanupTime;
        
        private ObjectPool()
        {
            UnityEngine.Debug.Log("[ObjectPool] Pure C# ObjectPool initialized");
        }
        
        /// <summary>
        /// Manual cleanup method - call periodically from a manager or coroutine
        /// </summary>
        public void PeriodicCleanup()
        {
            if (Time.time - _lastCleanupTime > _cleanupInterval)
            {
                CleanupUnusedPools();
                _lastCleanupTime = Time.time;
            }
        }
        
        /// <summary>
        /// Pre-warm a pool with specified number of objects
        /// </summary>
        public void PreWarm(GameObject prefab, int count, Transform parent = null)
        {
            var pool = GetOrCreatePool(prefab, parent);
            pool.PreWarm(count);
        }
        
        /// <summary>
        /// Get an object from pool
        /// </summary>
        public GameObject Get(GameObject prefab, Vector3 position = default, Quaternion rotation = default, Transform parent = null)
        {
            var pool = GetOrCreatePool(prefab, parent);
            var obj = pool.Get();
            
            if (obj != null)
            {
                obj.transform.position = position;
                obj.transform.rotation = rotation;
                if (parent != null)
                {
                    obj.transform.SetParent(parent);
                }
            }
            
            return obj;
        }
        
        /// <summary>
        /// Get a component from pooled object
        /// </summary>
        public T Get<T>(GameObject prefab, Vector3 position = default, Quaternion rotation = default, Transform parent = null) where T : Component
        {
            var obj = Get(prefab, position, rotation, parent);
            return obj?.GetComponent<T>();
        }
        
        /// <summary>
        /// Return an object to pool
        /// </summary>
        public void Return(GameObject obj)
        {
            if (obj == null) return;
            
            var poolable = obj.GetComponent<IPoolable>();
            poolable?.OnReturnToPool();
            
            // Find the pool this object belongs to
            foreach (var pool in _pools.Values)
            {
                if (pool.Return(obj))
                {
                    return;
                }
            }
            
            // If no pool found, just deactivate
            obj.SetActive(false);
        }
        
        /// <summary>
        /// Return an object to pool after delay
        /// </summary>
        public async void ReturnDelayed(GameObject obj, float delay)
        {
            await ReturnDelayedAsync(obj, delay);
        }
        
        private async UniTask ReturnDelayedAsync(GameObject obj, float delay)
        {
            await UniTask.Delay(TimeSpan.FromSeconds(delay), cancellationToken: CancellationToken.None);
            Return(obj);
        }
        
        /// <summary>
        /// Clear a specific pool
        /// </summary>
        public void ClearPool(GameObject prefab)
        {
            var key = GetPoolKey(prefab);
            if (_pools.TryRemove(key, out var pool))
            {
                pool.Clear();
            }
        }
        
        /// <summary>
        /// Clear all pools
        /// </summary>
        [Button("Clear All Pools")]
        public void ClearAllPools()
        {
            foreach (var pool in _pools.Values)
            {
                pool.Clear();
            }
            _pools.Clear();
            
            UnityEngine.Debug.Log("[ObjectPool] All pools cleared");
        }
        
        private Pool GetOrCreatePool(GameObject prefab, Transform parent)
        {
            var key = GetPoolKey(prefab);
            
            if (!_pools.TryGetValue(key, out var pool))
            {
                var poolContainer = new GameObject($"Pool_{prefab.name}");
                // Since we're not a MonoBehaviour, set parent to provided parent or null
                if (parent != null)
                {
                    poolContainer.transform.SetParent(parent);
                }
                
                pool = new Pool(prefab, poolContainer.transform, _defaultPoolSize, _maxPoolSize, _autoExpand);
                _pools[key] = pool;
            }
            
            return pool;
        }
        
        private string GetPoolKey(GameObject prefab)
        {
            return prefab.GetInstanceID().ToString();
        }
        
        private void CleanupUnusedPools()
        {
            var poolsToRemove = new List<string>();
            
            foreach (var kvp in _pools)
            {
                if (kvp.Value.IsEmpty && kvp.Value.LastUsedTime > _cleanupInterval)
                {
                    kvp.Value.Clear();
                    poolsToRemove.Add(kvp.Key);
                }
            }
            
            foreach (var key in poolsToRemove)
            {
                _pools.TryRemove(key, out _);
            }
            
            if (poolsToRemove.Count > 0)
            {
                UnityEngine.Debug.Log($"[ObjectPool] Cleaned up {poolsToRemove.Count} unused pools");
            }
        }
        
        private int GetTotalObjectCount()
        {
            int count = 0;
            foreach (var pool in _pools.Values)
            {
                count += pool.TotalCount;
            }
            return count;
        }
        
        private int GetActiveObjectCount()
        {
            int count = 0;
            foreach (var pool in _pools.Values)
            {
                count += pool.ActiveCount;
            }
            return count;
        }
        
        [Title("Pool Debug")]
        [Button("Log Pool Statistics")]
        private void LogPoolStatistics()
        {
            UnityEngine.Debug.Log($"[ObjectPool] Total Pools: {TotalPools}");
            UnityEngine.Debug.Log($"[ObjectPool] Total Objects: {TotalObjects}");
            UnityEngine.Debug.Log($"[ObjectPool] Active Objects: {ActiveObjects}");
            UnityEngine.Debug.Log($"[ObjectPool] Pool Utilization: {PoolUtilization:F1}%");
            
            foreach (var kvp in _pools)
            {
                var pool = kvp.Value;
                UnityEngine.Debug.Log($"  Pool: {pool.PrefabName} - Total: {pool.TotalCount}, Active: {pool.ActiveCount}");
            }
        }
        
        /// <summary>
        /// Internal pool implementation
        /// </summary>
        private class Pool
        {
            private GameObject _prefab;
            private Transform _container;
            private Queue<GameObject> _available = new Queue<GameObject>();
            private HashSet<GameObject> _active = new HashSet<GameObject>();
            private int _maxSize;
            private bool _autoExpand;
            private float _lastUsedTime;
            
            public string PrefabName => _prefab.name;
            public int TotalCount => _available.Count + _active.Count;
            public int ActiveCount => _active.Count;
            public bool IsEmpty => _active.Count == 0;
            public float LastUsedTime => Time.time - _lastUsedTime;
            
            public Pool(GameObject prefab, Transform container, int initialSize, int maxSize, bool autoExpand)
            {
                _prefab = prefab;
                _container = container;
                _maxSize = maxSize;
                _autoExpand = autoExpand;
                _lastUsedTime = Time.time;
                
                PreWarm(initialSize);
            }
            
            public void PreWarm(int count)
            {
                for (int i = 0; i < count && TotalCount < _maxSize; i++)
                {
                    CreateObject();
                }
            }
            
            public GameObject Get()
            {
                _lastUsedTime = Time.time;
                
                GameObject obj = null;
                
                if (_available.Count > 0)
                {
                    obj = _available.Dequeue();
                }
                else if (_autoExpand && TotalCount < _maxSize)
                {
                    obj = CreateObject();
                }
                else if (_autoExpand)
                {
                    UnityEngine.Debug.LogWarning($"[ObjectPool] Pool for {_prefab.name} reached max size ({_maxSize})");
                    obj = GameObject.Instantiate(_prefab);
                }
                
                if (obj != null)
                {
                    obj.SetActive(true);
                    _active.Add(obj);
                    
                    var poolable = obj.GetComponent<IPoolable>();
                    poolable?.OnGetFromPool();
                }
                
                return obj;
            }
            
            public bool Return(GameObject obj)
            {
                if (!_active.Contains(obj))
                {
                    return false;
                }
                
                _active.Remove(obj);
                obj.SetActive(false);
                obj.transform.SetParent(_container);
                _available.Enqueue(obj);
                
                return true;
            }
            
            public void Clear()
            {
                foreach (var obj in _available)
                {
                    if (obj != null)
                        GameObject.Destroy(obj);
                }
                
                foreach (var obj in _active)
                {
                    if (obj != null)
                        GameObject.Destroy(obj);
                }
                
                _available.Clear();
                _active.Clear();
                
                if (_container != null)
                    GameObject.Destroy(_container.gameObject);
            }
            
            private GameObject CreateObject()
            {
                var obj = GameObject.Instantiate(_prefab, _container);
                obj.SetActive(false);
                _available.Enqueue(obj);
                return obj;
            }
        }
    }
    
    /// <summary>
    /// Interface for poolable objects
    /// </summary>
    public interface IPoolable
    {
        void OnGetFromPool();
        void OnReturnToPool();
    }
    
    /// <summary>
    /// Base class for poolable MonoBehaviours
    /// </summary>
    public abstract class PoolableMonoBehaviour : MonoBehaviour, IPoolable
    {
        public virtual void OnGetFromPool()
        {
            // Override in derived classes
        }
        
        public virtual void OnReturnToPool()
        {
            // Override in derived classes
        }
        
        public void ReturnToPool()
        {
            ObjectPool.Instance.Return(gameObject);
        }
        
        public void ReturnToPoolDelayed(float delay)
        {
            ObjectPool.Instance.ReturnDelayed(gameObject, delay);
        }
    }
}