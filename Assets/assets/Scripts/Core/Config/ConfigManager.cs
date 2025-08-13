using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEngine;
using Sirenix.OdinInspector;
namespace PoliceThief.Core.Config
{
    /// <summary>
    /// Centralized configuration management interface
    /// </summary>
    public interface IConfigManager
    {
        T LoadConfig<T>(string fileName) where T : class, new();
        void SaveConfig<T>(string fileName, T config) where T : class;
        T GetConfig<T>() where T : class, new();
        void ReloadConfig<T>(string fileName) where T : class, new();
        NetworkConfig GetNetworkConfig();
        GrpcConfig GetGrpcConfig();
        void ClearCache();
        
        event Action<string> OnConfigLoaded;
        event Action<string> OnConfigReloaded;
        event Action<string, Exception> OnConfigError;
    }
    
    /// <summary>
    /// Centralized configuration management implementation
    /// Converted from MonoBehaviour to pure C# class for better performance
    /// </summary>
    public class ConfigManager : IConfigManager
    {
        private static ConfigManager _instance;
        private static readonly object _lock = new object();
        
        public static ConfigManager Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_lock)
                    {
                        if (_instance == null)
                        {
                            _instance = new ConfigManager();
                        }
                    }
                }
                return _instance;
            }
        }
        
        [Title("Configuration Settings")]
        [SerializeField]
        private string _configPath = "Config";
        
        [SerializeField]
        private bool _enableHotReload = false;
        
        [SerializeField]
        private float _hotReloadCheckInterval = 1f;
        
        [Title("Network Configuration")]
        [SerializeField]
        private NetworkConfig _networkConfig = new NetworkConfig();
        
        [Title("gRPC Configuration")]
        [SerializeField]
        private GrpcConfig _grpcConfig = new GrpcConfig();
        
        [Title("Loaded Configurations")]
        [ShowInInspector]
        private Dictionary<string, object> _loadedConfigs = new Dictionary<string, object>();
        
        [ShowInInspector]
        private Dictionary<string, DateTime> _configLastModified = new Dictionary<string, DateTime>();
        
        private Dictionary<Type, object> _configCache = new Dictionary<Type, object>();
        private DateTime _lastHotReloadCheckTime = DateTime.Now;
        
        // Events
        public event Action<string> OnConfigLoaded;
        public event Action<string> OnConfigReloaded;
        public event Action<string, Exception> OnConfigError;
        
        private ConfigManager()
        {
            LoadAllConfigs();
        }
        
        // Hot reload moved to explicit method call - no more Unity Update loop dependency
        public void CheckHotReload()
        {
            if (_enableHotReload && (DateTime.Now - _lastHotReloadCheckTime).TotalSeconds > _hotReloadCheckInterval)
            {
                CheckForConfigChanges();
                _lastHotReloadCheckTime = DateTime.Now;
            }
        }
        
        /// <summary>
        /// Load a configuration file
        /// </summary>
        public T LoadConfig<T>(string fileName) where T : class, new()
        {
            var type = typeof(T);
            
            // Check cache first
            if (_configCache.TryGetValue(type, out var cached))
            {
                return (T)cached;
            }
            
            try
            {
                // Try loading from Resources
                var textAsset = Resources.Load<TextAsset>($"{_configPath}/{fileName}");
                if (textAsset != null)
                {
                    var config = JsonUtility.FromJson<T>(textAsset.text);
                    _configCache[type] = config;
                    _loadedConfigs[fileName] = config;
                    
                    OnConfigLoaded?.Invoke(fileName);
                    UnityEngine.Debug.Log($"[ConfigManager] Loaded config: {fileName}");
                    
                    return config;
                }
                
                // Try loading from persistent data path
                var persistentPath = Path.Combine(Application.persistentDataPath, _configPath, $"{fileName}.json");
                if (File.Exists(persistentPath))
                {
                    var json = File.ReadAllText(persistentPath);
                    var config = JsonUtility.FromJson<T>(json);
                    _configCache[type] = config;
                    _loadedConfigs[fileName] = config;
                    _configLastModified[fileName] = File.GetLastWriteTime(persistentPath);
                    
                    OnConfigLoaded?.Invoke(fileName);
                    UnityEngine.Debug.Log($"[ConfigManager] Loaded config from persistent: {fileName}");
                    
                    return config;
                }
                
                // Create default config
                var defaultConfig = new T();
                SaveConfig(fileName, defaultConfig);
                _configCache[type] = defaultConfig;
                _loadedConfigs[fileName] = defaultConfig;
                
                UnityEngine.Debug.Log($"[ConfigManager] Created default config: {fileName}");
                return defaultConfig;
            }
            catch (Exception ex)
            {
                UnityEngine.Debug.LogError($"[ConfigManager] Failed to load config {fileName}: {ex.Message}");
                OnConfigError?.Invoke(fileName, ex);
                
                // Return default config on error
                var defaultConfig = new T();
                _configCache[type] = defaultConfig;
                return defaultConfig;
            }
        }
        
        /// <summary>
        /// Save a configuration file
        /// </summary>
        public void SaveConfig<T>(string fileName, T config) where T : class
        {
            try
            {
                var json = JsonUtility.ToJson(config, true);
                var directory = Path.Combine(Application.persistentDataPath, _configPath);
                
                if (!Directory.Exists(directory))
                {
                    Directory.CreateDirectory(directory);
                }
                
                var path = Path.Combine(directory, $"{fileName}.json");
                File.WriteAllText(path, json);
                
                _configLastModified[fileName] = DateTime.Now;
                
                UnityEngine.Debug.Log($"[ConfigManager] Saved config: {fileName}");
            }
            catch (Exception ex)
            {
                UnityEngine.Debug.LogError($"[ConfigManager] Failed to save config {fileName}: {ex.Message}");
                OnConfigError?.Invoke(fileName, ex);
            }
        }
        
        /// <summary>
        /// Get a configuration value
        /// </summary>
        public T GetConfig<T>() where T : class, new()
        {
            var type = typeof(T);
            
            if (_configCache.TryGetValue(type, out var config))
            {
                return (T)config;
            }
            
            // Auto-load based on type name
            var fileName = type.Name.Replace("Config", "").Replace("Settings", "");
            return LoadConfig<T>(fileName);
        }
        
        /// <summary>
        /// Reload a specific configuration
        /// </summary>
        public void ReloadConfig<T>(string fileName) where T : class, new()
        {
            var type = typeof(T);
            _configCache.Remove(type);
            _loadedConfigs.Remove(fileName);
            
            var config = LoadConfig<T>(fileName);
            OnConfigReloaded?.Invoke(fileName);
        }
        
        /// <summary>
        /// 네트워크 설정 반환
        /// </summary>
        public NetworkConfig GetNetworkConfig()
        {
            return _networkConfig;
        }
        
        /// <summary>
        /// gRPC 설정 반환
        /// </summary>
        public GrpcConfig GetGrpcConfig()
        {
            return _grpcConfig;
        }
        
        /// <summary>
        /// Clear configuration cache
        /// </summary>
        [Button("Clear Config Cache")]
        public void ClearCache()
        {
            _configCache.Clear();
            _loadedConfigs.Clear();
            _configLastModified.Clear();
            
            UnityEngine.Debug.Log("[ConfigManager] Configuration cache cleared");
        }
        
        private void LoadAllConfigs()
        {
            // Load all configs from Resources
            var configs = Resources.LoadAll<TextAsset>(_configPath);
            foreach (var config in configs)
            {
                UnityEngine.Debug.Log($"[ConfigManager] Found config resource: {config.name}");
            }
            
            // Load all configs from persistent path
            var persistentPath = Path.Combine(Application.persistentDataPath, _configPath);
            if (Directory.Exists(persistentPath))
            {
                var files = Directory.GetFiles(persistentPath, "*.json");
                foreach (var file in files)
                {
                    var fileName = Path.GetFileNameWithoutExtension(file);
                    UnityEngine.Debug.Log($"[ConfigManager] Found persistent config: {fileName}");
                }
            }
        }
        
        private void CheckForConfigChanges()
        {
            var persistentPath = Path.Combine(Application.persistentDataPath, _configPath);
            if (!Directory.Exists(persistentPath)) return;
            
            foreach (var kvp in _configLastModified.ToList())
            {
                var path = Path.Combine(persistentPath, $"{kvp.Key}.json");
                if (File.Exists(path))
                {
                    var lastModified = File.GetLastWriteTime(path);
                    if (lastModified > kvp.Value)
                    {
                        UnityEngine.Debug.Log($"[ConfigManager] Config changed: {kvp.Key}, reloading...");
                        
                        // Find the type and reload
                        foreach (var configKvp in _configCache)
                        {
                            if (_loadedConfigs.ContainsKey(kvp.Key) && _loadedConfigs[kvp.Key] == configKvp.Value)
                            {
                                var reloadMethod = GetType().GetMethod(nameof(ReloadConfig)).MakeGenericMethod(configKvp.Key);
                                reloadMethod.Invoke(this, new object[] { kvp.Key });
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        [Button("Log Loaded Configs")]
        private void LogLoadedConfigs()
        {
            UnityEngine.Debug.Log($"[ConfigManager] Loaded configs: {_loadedConfigs.Count}");
            foreach (var kvp in _loadedConfigs)
            {
                UnityEngine.Debug.Log($"  {kvp.Key}: {kvp.Value?.GetType().Name ?? "null"}");
            }
        }
    }
    
    /// <summary>
    /// Base class for configuration objects
    /// </summary>
    [Serializable]
    public abstract class ConfigBase
    {
        public string version = "1.0.0";
        public string lastModified = System.DateTime.UtcNow.ToString();
        
        public virtual void Validate()
        {
            // Override in derived classes for validation
        }
    }
    
    /// <summary>
    /// gRPC 서버 연결 설정
    /// </summary>
    [Serializable]
    public class GrpcConfig : ConfigBase
    {
        [Header("서버 연결")]
        public string serverUrl = "http://localhost:50051";
        public int connectTimeoutMs = 5000;
        public int maxRetryAttempts = 3;
        public int retryDelayMs = 1000;
        
        [Header("연결 유지")]
        public bool enableKeepAlive = true;
        public int keepAliveIntervalMs = 30000;
        public bool enableAutoReconnect = true;
        public int reconnectDelayMs = 5000;
        
        [Header("메시지 크기")]
        public int maxReceiveMessageSize = 4 * 1024 * 1024; // 4MB
        public int maxSendMessageSize = 4 * 1024 * 1024;    // 4MB
        
        public override void Validate()
        {
            if (string.IsNullOrEmpty(serverUrl))
                throw new System.ArgumentException("서버 URL이 설정되지 않았습니다");
                
            if (connectTimeoutMs <= 0)
                connectTimeoutMs = 5000;
                
            if (maxRetryAttempts <= 0)
                maxRetryAttempts = 3;
                
            if (retryDelayMs <= 0)
                retryDelayMs = 1000;
                
            if (keepAliveIntervalMs <= 0)
                keepAliveIntervalMs = 30000;
                
            if (reconnectDelayMs <= 0)
                reconnectDelayMs = 5000;
        }
    }
    
    /// <summary>
    /// Example game configuration
    /// </summary>
    [Serializable]
    public class GameConfig : ConfigBase
    {
        public int maxPlayers = 10;
        public float roundDuration = 300f;
        public float respawnTime = 5f;
        public DebugSettings debug = new DebugSettings();
        
        [Serializable]
        public class DebugSettings
        {
            public bool enabled = true;
            public string logLevel = "Info";
            public bool showFps = true;
        }
    }
}