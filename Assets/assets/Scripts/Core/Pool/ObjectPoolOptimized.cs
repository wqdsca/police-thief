using System;
using System.Collections.Generic;
using UnityEngine;
using System.Runtime.CompilerServices;

namespace PoliceThief.Core.Pool
{
    /// <summary>
    /// Mobile-optimized object pooling system
    /// - Uses int keys instead of string for zero allocation
    /// - Reduced default pool sizes for mobile memory constraints
    /// - No Update() loop overhead
    /// - Lazy initialization without FindFirstObjectByType
    /// </summary>
    public sealed class ObjectPoolOptimized
    {
        private static ObjectPoolOptimized _instance;
        private static readonly object _instanceLock = new object();
        
        public static ObjectPoolOptimized Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_instanceLock)
                    {
                        _instance ??= new ObjectPoolOptimized();
                    }
                }
                return _instance;
            }
        }
        
        // Mobile-optimized pool configuration
        private const int MOBILE_DEFAULT_POOL_SIZE = 5;  // Reduced from 10
        private const int MOBILE_MAX_POOL_SIZE = 30;     // Reduced from 100
        private const bool AUTO_EXPAND = false;          // Disabled for mobile to prevent memory spikes
        
        // Use int keys for zero string allocation
        private readonly Dictionary<int, Pool> _pools = new Dictionary<int, Pool>();
        private readonly Dictionary<int, GameObject> _prefabLookup = new Dictionary<int, GameObject>();
        
        // Reusable lists for zero allocation
        private readonly List<int> _poolsToRemove = new List<int>(8);
        
        // GameObject container (created lazily)
        private GameObject _poolContainer;
        private Transform _poolTransform;
        
        // Statistics
        public int TotalPools => _pools.Count;
        public int TotalObjects { get; private set; }
        public int ActiveObjects { get; private set; }
        public float PoolUtilization => TotalObjects > 0 ? (ActiveObjects / (float)TotalObjects) * 100f : 0f;
        
        private ObjectPoolOptimized()
        {
            // Private constructor for singleton
        }
        
        private void EnsurePoolContainer()
        {
            if (_poolContainer == null)
            {
                _poolContainer = new GameObject("[ObjectPoolOptimized]");
                _poolTransform = _poolContainer.transform;
                UnityEngine.Object.DontDestroyOnLoad(_poolContainer);
            }
        }
        
        /// <summary>
        /// Pre-warm a pool with specified number of objects
        /// </summary>
        public void PreWarm(GameObject prefab, int count)
        {
            if (prefab == null) return;
            
            var pool = GetOrCreatePool(prefab);
            pool.PreWarm(Math.Min(count, MOBILE_MAX_POOL_SIZE));
        }
        
        /// <summary>
        /// Get an object from pool with minimal overhead
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public GameObject Get(GameObject prefab, Vector3 position = default, Quaternion rotation = default, Transform parent = null)
        {
            if (prefab == null) return null;
            
            var pool = GetOrCreatePool(prefab);
            var obj = pool.Get();
            
            if (obj != null)
            {
                var transform = obj.transform;
                transform.position = position;
                transform.rotation = rotation;
                
                if (parent != null)
                {
                    transform.SetParent(parent, false);
                }
            }
            
            return obj;
        }
        
        /// <summary>
        /// Get a component from pooled object
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public T Get<T>(GameObject prefab, Vector3 position = default, Quaternion rotation = default, Transform parent = null) where T : Component
        {
            var obj = Get(prefab, position, rotation, parent);
            return obj != null ? obj.GetComponent<T>() : null;
        }
        
        /// <summary>
        /// Return an object to pool
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void Return(GameObject obj)
        {
            if (obj == null) return;
            
            // Try to get pool ID from object name or component
            var poolId = GetPoolId(obj);
            
            if (_pools.TryGetValue(poolId, out var pool))
            {
                if (pool.Return(obj))
                {
                    ActiveObjects--;
                    return;
                }
            }
            
            // If no pool found, just deactivate
            obj.SetActive(false);
        }
        
        /// <summary>
        /// Return an object to pool after delay (using async instead of coroutine)
        /// </summary>
        public async void ReturnDelayed(GameObject obj, float delay)
        {
            if (obj == null || delay <= 0)
            {
                Return(obj);
                return;
            }
            
            await System.Threading.Tasks.Task.Delay((int)(delay * 1000));
            Return(obj);
        }
        
        /// <summary>
        /// Clear a specific pool
        /// </summary>
        public void ClearPool(GameObject prefab)
        {
            if (prefab == null) return;
            
            var key = prefab.GetInstanceID();
            
            if (_pools.TryGetValue(key, out var pool))
            {
                TotalObjects -= pool.TotalCount;
                ActiveObjects -= pool.ActiveCount;
                
                pool.Clear();
                _pools.Remove(key);
                _prefabLookup.Remove(key);
            }
        }
        
        /// <summary>
        /// Clear all pools
        /// </summary>
        public void ClearAllPools()
        {
            foreach (var pool in _pools.Values)
            {
                pool.Clear();
            }
            
            _pools.Clear();
            _prefabLookup.Clear();
            TotalObjects = 0;
            ActiveObjects = 0;
            
            #if !UNITY_EDITOR && DEBUG
            Debug.Log("[ObjectPoolOptimized] All pools cleared");
            #endif
        }
        
        /// <summary>
        /// Manual cleanup of unused pools (call periodically if needed)
        /// </summary>
        public void CleanupUnusedPools(float unusedThresholdSeconds = 60f)
        {
            _poolsToRemove.Clear();
            
            foreach (var kvp in _pools)
            {
                if (kvp.Value.IsEmpty && kvp.Value.GetTimeSinceLastUse() > unusedThresholdSeconds)
                {
                    _poolsToRemove.Add(kvp.Key);
                }
            }
            
            foreach (var key in _poolsToRemove)
            {
                if (_pools.TryGetValue(key, out var pool))
                {
                    TotalObjects -= pool.TotalCount;
                    pool.Clear();
                    _pools.Remove(key);
                    _prefabLookup.Remove(key);
                }
            }
            
            #if !UNITY_EDITOR && DEBUG
            if (_poolsToRemove.Count > 0)
            {
                Debug.Log($"[ObjectPoolOptimized] Cleaned up {_poolsToRemove.Count} unused pools");
            }
            #endif
        }
        
        private Pool GetOrCreatePool(GameObject prefab)
        {
            var key = prefab.GetInstanceID();
            
            if (!_pools.TryGetValue(key, out var pool))
            {
                EnsurePoolContainer();
                
                var poolContainer = new GameObject($"Pool_{prefab.name}_{key}");
                poolContainer.transform.SetParent(_poolTransform, false);
                
                pool = new Pool(prefab, poolContainer.transform, key, MOBILE_DEFAULT_POOL_SIZE, MOBILE_MAX_POOL_SIZE, AUTO_EXPAND);
                _pools[key] = pool;
                _prefabLookup[key] = prefab;
                
                TotalObjects += pool.TotalCount;
            }
            
            return pool;
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private int GetPoolId(GameObject obj)
        {
            // Try to extract pool ID from object name (fastest)
            var name = obj.name;
            var lastUnderscore = name.LastIndexOf('_');
            
            if (lastUnderscore > 0 && int.TryParse(name.Substring(lastUnderscore + 1).Replace("(Clone)", ""), out var id))
            {
                return id;
            }
            
            // Fallback: check if object has a PoolableObject component
            var poolable = obj.GetComponent<PoolableObject>();
            if (poolable != null)
            {
                return poolable.PoolId;
            }
            
            return 0;
        }
        
        /// <summary>
        /// Get statistics for debugging (conditional compilation)
        /// </summary>
        [System.Diagnostics.Conditional("POOL_STATISTICS")]
        public void LogPoolStatistics()
        {
            Debug.Log($"[ObjectPoolOptimized] Pools: {TotalPools}, Total: {TotalObjects}, Active: {ActiveObjects}, Utilization: {PoolUtilization:F1}%");
            
            foreach (var kvp in _pools)
            {
                var pool = kvp.Value;
                Debug.Log($"  Pool {kvp.Key}: Total: {pool.TotalCount}, Active: {pool.ActiveCount}");
            }
        }
        
        /// <summary>
        /// Internal pool implementation
        /// </summary>
        private sealed class Pool
        {
            private readonly GameObject _prefab;
            private readonly Transform _container;
            private readonly int _poolId;
            private readonly int _maxSize;
            private readonly bool _autoExpand;
            
            private readonly Queue<GameObject> _available = new Queue<GameObject>();
            private readonly HashSet<GameObject> _active = new HashSet<GameObject>();
            
            private float _lastUsedTime;
            
            public int TotalCount => _available.Count + _active.Count;
            public int ActiveCount => _active.Count;
            public bool IsEmpty => _active.Count == 0;
            
            public Pool(GameObject prefab, Transform container, int poolId, int initialSize, int maxSize, bool autoExpand)
            {
                _prefab = prefab;
                _container = container;
                _poolId = poolId;
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
                    #if UNITY_EDITOR || DEBUG
                    Debug.LogWarning($"[ObjectPoolOptimized] Pool {_poolId} reached max size ({_maxSize})");
                    #endif
                    obj = UnityEngine.Object.Instantiate(_prefab);
                }
                
                if (obj != null)
                {
                    obj.SetActive(true);
                    _active.Add(obj);
                    Instance.ActiveObjects++;
                    
                    // Set pool ID for fast return
                    var poolable = obj.GetComponent<PoolableObject>();
                    if (poolable != null)
                    {
                        poolable.PoolId = _poolId;
                        poolable.OnGetFromPool();
                    }
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
                obj.transform.SetParent(_container, false);
                
                // Reset before returning to pool
                var poolable = obj.GetComponent<PoolableObject>();
                poolable?.OnReturnToPool();
                
                _available.Enqueue(obj);
                
                return true;
            }
            
            public void Clear()
            {
                foreach (var obj in _available)
                {
                    if (obj != null)
                        UnityEngine.Object.Destroy(obj);
                }
                
                foreach (var obj in _active)
                {
                    if (obj != null)
                        UnityEngine.Object.Destroy(obj);
                }
                
                _available.Clear();
                _active.Clear();
                
                if (_container != null)
                    UnityEngine.Object.Destroy(_container.gameObject);
            }
            
            private GameObject CreateObject()
            {
                var obj = UnityEngine.Object.Instantiate(_prefab, _container);
                obj.name = $"{_prefab.name}_{_poolId}";
                obj.SetActive(false);
                
                // Add PoolableObject component if not present
                if (obj.GetComponent<PoolableObject>() == null)
                {
                    var poolable = obj.AddComponent<PoolableObject>();
                    poolable.PoolId = _poolId;
                }
                
                _available.Enqueue(obj);
                return obj;
            }
            
            public float GetTimeSinceLastUse()
            {
                return Time.time - _lastUsedTime;
            }
        }
    }
    
    /// <summary>
    /// Optimized poolable object component
    /// </summary>
    public class PoolableObject : MonoBehaviour, IPoolable
    {
        [HideInInspector]
        public int PoolId { get; set; }
        
        public virtual void OnGetFromPool()
        {
            // Override in derived classes
        }
        
        public virtual void OnReturnToPool()
        {
            // Override in derived classes
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void ReturnToPool()
        {
            ObjectPoolOptimized.Instance.Return(gameObject);
        }
        
        public void ReturnToPoolDelayed(float delay)
        {
            ObjectPoolOptimized.Instance.ReturnDelayed(gameObject, delay);
        }
    }
    
    /// <summary>
    /// Generic object pool for non-GameObject types
    /// </summary>
    public sealed class GenericPool<T> where T : class, new()
    {
        private readonly Queue<T> _pool = new Queue<T>();
        private readonly int _maxSize;
        private int _currentSize;
        
        public GenericPool(int initialSize = 10, int maxSize = 50)
        {
            _maxSize = maxSize;
            
            for (int i = 0; i < initialSize; i++)
            {
                _pool.Enqueue(new T());
                _currentSize++;
            }
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public T Get()
        {
            if (_pool.Count > 0)
            {
                return _pool.Dequeue();
            }
            
            if (_currentSize < _maxSize)
            {
                _currentSize++;
                return new T();
            }
            
            return null;
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void Return(T item)
        {
            if (item == null || _pool.Count >= _maxSize) return;
            
            // Reset if IPoolable
            if (item is IPoolable poolable)
            {
                poolable.OnReturnToPool();
            }
            
            _pool.Enqueue(item);
        }
        
        public void Clear()
        {
            _pool.Clear();
            _currentSize = 0;
        }
    }
}