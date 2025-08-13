using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using UnityEngine;
using Sirenix.OdinInspector;
using PoliceThief.Infrastructure.Network.Grpc;

namespace PoliceThief.Core.DI
{
    /// <summary>
    /// Service locator interface
    /// </summary>
    public interface IServiceLocator
    {
        void RegisterSingleton<T>(T service) where T : class;
        void RegisterTransient<T>(Func<T> factory) where T : class;
        void RegisterScoped<T>(Func<T> factory) where T : class;
        T Get<T>() where T : class;
        bool TryGet<T>(out T service) where T : class;
        bool IsRegistered<T>() where T : class;
        void Unregister<T>() where T : class;
        void ClearAll();
        IServiceScope CreateScope();
    }
    
    /// <summary>
    /// Advanced Service Locator with dependency injection capabilities
    /// Converted from MonoBehaviour to pure C# class for better performance
    /// </summary>
    public sealed class ServiceLocator : IServiceLocator
    {
        private static ServiceLocator _instance;
        private static readonly object _lock = new object();
        private readonly ConcurrentDictionary<Type, object> _services = new ConcurrentDictionary<Type, object>();
        private readonly ConcurrentDictionary<Type, Func<object>> _factories = new ConcurrentDictionary<Type, Func<object>>();
        private readonly ConcurrentDictionary<Type, ServiceLifetime> _lifetimes = new ConcurrentDictionary<Type, ServiceLifetime>();
        private readonly HashSet<Type> _initializing = new HashSet<Type>();
        
        public static ServiceLocator Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_lock)
                    {
                        if (_instance == null)
                        {
                            _instance = new ServiceLocator();
                        }
                    }
                }
                return _instance;
            }
        }
        
        [Title("Service Registry")]
        [ShowInInspector]
        [DictionaryDrawerSettings(KeyLabel = "Service Type", ValueLabel = "Instance")]
        private Dictionary<string, string> RegisteredServices => GetRegisteredServicesDisplay();
        
        [Title("Performance Metrics")]
        [ShowInInspector]
        [DisplayAsString]
        public int TotalServices => _services.Count + _factories.Count;
        
        [ShowInInspector]
        [DisplayAsString]
        public int ActiveInstances => _services.Count;
        
        [ShowInInspector]
        [DisplayAsString]
        public int RegisteredFactories => _factories.Count;
        
        private ServiceLocator()
        {
            // 기본 서비스들 등록
            RegisterDefaultServices();
            
            UnityEngine.Debug.Log("[ServiceLocator] 초기화 완료");
        }
        
        /// <summary>
        /// Register a singleton service
        /// </summary>
        public void RegisterSingleton<T>(T service) where T : class
        {
            var type = typeof(T);
            
            if (_services.ContainsKey(type))
            {
                UnityEngine.Debug.LogWarning($"[ServiceLocator] Service {type.Name} already registered, replacing...");
            }
            
            _services[type] = service;
            _lifetimes[type] = ServiceLifetime.Singleton;
            
            UnityEngine.Debug.Log($"[ServiceLocator] Registered singleton: {type.Name}");
        }
        
        /// <summary>
        /// Register a transient service with factory
        /// </summary>
        public void RegisterTransient<T>(Func<T> factory) where T : class
        {
            var type = typeof(T);
            _factories[type] = () => factory();
            _lifetimes[type] = ServiceLifetime.Transient;
            
            UnityEngine.Debug.Log($"[ServiceLocator] Registered transient: {type.Name}");
        }
        
        /// <summary>
        /// Register a scoped service (singleton per request scope)
        /// </summary>
        public void RegisterScoped<T>(Func<T> factory) where T : class
        {
            var type = typeof(T);
            _factories[type] = () => factory();
            _lifetimes[type] = ServiceLifetime.Scoped;
            
            UnityEngine.Debug.Log($"[ServiceLocator] Registered scoped: {type.Name}");
        }
        
        /// <summary>
        /// Get a service with circular dependency detection
        /// </summary>
        public T Get<T>() where T : class
        {
            var type = typeof(T);
            
            // Check for circular dependency
            if (_initializing.Contains(type))
            {
                throw new InvalidOperationException($"Circular dependency detected for {type.Name}");
            }
            
            try
            {
                _initializing.Add(type);
                
                // Check if singleton exists
                if (_services.TryGetValue(type, out var service))
                {
                    return (T)service;
                }
                
                // Check if factory exists
                if (_factories.TryGetValue(type, out var factory))
                {
                    var instance = factory() as T;
                    
                    // Cache if scoped
                    if (_lifetimes.TryGetValue(type, out var lifetime) && lifetime == ServiceLifetime.Scoped)
                    {
                        _services[type] = instance;
                    }
                    
                    return instance;
                }
                
                // Skip MonoBehaviour auto-creation - use factory registration instead
                // MonoBehaviour components should be explicitly registered
                
                // Try to create with default constructor
                try
                {
                    var instance = Activator.CreateInstance<T>();
                    _services[type] = instance;
                    return instance;
                }
                catch
                {
                    throw new InvalidOperationException($"Service {type.Name} not registered and cannot be auto-created");
                }
            }
            finally
            {
                _initializing.Remove(type);
            }
        }
        
        /// <summary>
        /// Try to get a service without throwing
        /// </summary>
        public bool TryGet<T>(out T service) where T : class
        {
            try
            {
                service = Get<T>();
                return service != null;
            }
            catch
            {
                service = null;
                return false;
            }
        }
        
        /// <summary>
        /// Check if a service is registered
        /// </summary>
        public bool IsRegistered<T>() where T : class
        {
            var type = typeof(T);
            return _services.ContainsKey(type) || _factories.ContainsKey(type);
        }
        
        /// <summary>
        /// Unregister a service
        /// </summary>
        public void Unregister<T>() where T : class
        {
            var type = typeof(T);
            _services.TryRemove(type, out _);
            _factories.TryRemove(type, out _);
            _lifetimes.TryRemove(type, out _);
            
            UnityEngine.Debug.Log($"[ServiceLocator] Unregistered: {type.Name}");
        }
        
        /// <summary>
        /// Clear all services
        /// </summary>
        [Button("Clear All Services")]
        public void ClearAll()
        {
            // Dispose disposable services
            foreach (var service in _services.Values)
            {
                if (service is IDisposable disposable)
                {
                    disposable.Dispose();
                }
            }
            
            _services.Clear();
            _factories.Clear();
            _lifetimes.Clear();
            _initializing.Clear();
            
            UnityEngine.Debug.Log("[ServiceLocator] All services cleared");
        }
        
        /// <summary>
        /// Create a scoped container for isolated service resolution
        /// </summary>
        public IServiceScope CreateScope()
        {
            return new ServiceScope(this);
        }
        
        private Dictionary<string, string> GetRegisteredServicesDisplay()
        {
            var display = new Dictionary<string, string>();
            
            foreach (var kvp in _services)
            {
                display[kvp.Key.Name] = kvp.Value?.GetType().Name ?? "null";
            }
            
            foreach (var kvp in _factories)
            {
                if (!display.ContainsKey(kvp.Key.Name))
                {
                    var lifetime = _lifetimes.TryGetValue(kvp.Key, out var lt) ? lt.ToString() : "Unknown";
                    display[kvp.Key.Name] = $"Factory ({lifetime})";
                }
            }
            
            return display;
        }
        
        /// <summary>
        /// 기본 서비스들을 등록 - Bootstrap에서 서비스 등록을 담당하도록 변경
        /// </summary>
        private void RegisterDefaultServices()
        {
            // Bootstrap에서 모든 서비스 등록을 담당하므로 여기서는 초기화만 수행
            UnityEngine.Debug.Log("[ServiceLocator] 초기화 완료 - Bootstrap에서 서비스를 등록합니다");
        }

        // Helper method for ServiceScope
        internal bool IsScoped(Type type)
        {
            return _lifetimes.TryGetValue(type, out var lifetime) && lifetime == ServiceLifetime.Scoped;
        }
        
        // Dispose method for cleanup when needed
        public void Dispose()
        {
            ClearAll();
        }
    }
    
    /// <summary>
    /// Service lifetime enumeration
    /// </summary>
    public enum ServiceLifetime
    {
        Singleton,  // One instance for entire app lifetime
        Transient,  // New instance every time
        Scoped      // One instance per scope
    }
    
    /// <summary>
    /// Service scope interface for scoped service management
    /// </summary>
    public interface IServiceScope : IDisposable
    {
        T Get<T>() where T : class;
    }
    
    /// <summary>
    /// Internal service scope implementation
    /// </summary>
    internal class ServiceScope : IServiceScope
    {
        private readonly ServiceLocator _parent;
        private readonly Dictionary<Type, object> _scopedServices = new Dictionary<Type, object>();
        
        public ServiceScope(ServiceLocator parent)
        {
            _parent = parent;
        }
        
        public T Get<T>() where T : class
        {
            var type = typeof(T);
            
            // Check scoped cache first
            if (_scopedServices.TryGetValue(type, out var service))
            {
                return (T)service;
            }
            
            // Get from parent
            var instance = _parent.Get<T>();
            
            // Cache if scoped - access via public method
            if (_parent.IsScoped(type))
            {
                _scopedServices[type] = instance;
            }
            
            return instance;
        }
        
        public void Dispose()
        {
            foreach (var service in _scopedServices.Values)
            {
                if (service is IDisposable disposable)
                {
                    disposable.Dispose();
                }
            }
            _scopedServices.Clear();
        }
    }
    
    /// <summary>
    /// Service locator extensions for easy access
    /// </summary>
    public static class ServiceLocatorExtensions
    {
        public static T GetService<T>(this MonoBehaviour mono) where T : class
        {
            return ServiceLocator.Instance.Get<T>();
        }
        
        public static bool TryGetService<T>(this MonoBehaviour mono, out T service) where T : class
        {
            return ServiceLocator.Instance.TryGet(out service);
        }
    }
}