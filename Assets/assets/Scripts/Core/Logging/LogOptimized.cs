using System;
using System.Diagnostics;
using System.Runtime.CompilerServices;
using UnityEngine;
using Debug = UnityEngine.Debug;

namespace PoliceThief.Core.Logging
{
    /// <summary>
    /// Mobile-optimized logging system with zero allocation in release builds
    /// Uses conditional compilation to completely remove logging overhead
    /// </summary>
    public static class LogOptimized
    {
        // Log levels for runtime filtering
        public enum LogLevel
        {
            Debug = 0,
            Info = 1,
            Warning = 2,
            Error = 3,
            Fatal = 4,
            None = 5
        }
        
        // Current log level (can be changed at runtime)
        #if UNITY_EDITOR
        private static LogLevel _currentLevel = LogLevel.Debug;
        #elif DEBUG
        private static LogLevel _currentLevel = LogLevel.Info;
        #else
        private static LogLevel _currentLevel = LogLevel.Error;
        #endif
        
        public static LogLevel CurrentLevel
        {
            get => _currentLevel;
            set => _currentLevel = value;
        }
        
        // String pooling for categories to avoid allocation
        private static class CategoryPool
        {
            public const string General = "General";
            public const string Network = "Network";
            public const string Game = "Game";
            public const string UI = "UI";
            public const string Audio = "Audio";
            public const string Input = "Input";
            public const string Physics = "Physics";
            public const string AI = "AI";
            public const string Performance = "Performance";
            public const string Security = "Security";
        }
        
        #region Debug Logging (Completely removed in release)
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Debug(string message, string category = CategoryPool.General)
        {
            if (_currentLevel <= LogLevel.Debug)
            {
                UnityEngine.Debug.Log(FormatMessage(category, message));
            }
        }
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void DebugFormat(string format, params object[] args)
        {
            if (_currentLevel <= LogLevel.Debug)
            {
                UnityEngine.Debug.LogFormat(format, args);
            }
        }
        
        #endregion
        
        #region Info Logging (Removed in release)
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [Conditional("ENABLE_INFO_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Info(string message, string category = CategoryPool.General)
        {
            if (_currentLevel <= LogLevel.Info)
            {
                UnityEngine.Debug.Log(FormatMessage(category, message));
            }
        }
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [Conditional("ENABLE_INFO_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void InfoFormat(string format, params object[] args)
        {
            if (_currentLevel <= LogLevel.Info)
            {
                UnityEngine.Debug.LogFormat(format, args);
            }
        }
        
        #endregion
        
        #region Warning Logging (Can be kept in release)
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [Conditional("ENABLE_WARNING_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Warning(string message, string category = CategoryPool.General)
        {
            if (_currentLevel <= LogLevel.Warning)
            {
                UnityEngine.Debug.LogWarning(FormatMessage(category, message));
            }
        }
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [Conditional("ENABLE_WARNING_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void WarningFormat(string format, params object[] args)
        {
            if (_currentLevel <= LogLevel.Warning)
            {
                UnityEngine.Debug.LogWarningFormat(format, args);
            }
        }
        
        #endregion
        
        #region Error Logging (Always kept)
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Error(string message, string category = CategoryPool.General)
        {
            if (_currentLevel <= LogLevel.Error)
            {
                UnityEngine.Debug.LogError(FormatMessage(category, message));
            }
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void ErrorFormat(string format, params object[] args)
        {
            if (_currentLevel <= LogLevel.Error)
            {
                UnityEngine.Debug.LogErrorFormat(format, args);
            }
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Exception(Exception exception, string category = CategoryPool.General)
        {
            if (_currentLevel <= LogLevel.Error)
            {
                UnityEngine.Debug.LogException(exception);
            }
        }
        
        #endregion
        
        #region Fatal Logging (Always kept)
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Fatal(string message, string category = CategoryPool.General)
        {
            if (_currentLevel <= LogLevel.Fatal)
            {
                UnityEngine.Debug.LogError($"[FATAL][{category}] {message}");
                
                #if !UNITY_EDITOR
                // In release builds, fatal errors could trigger crash reporting
                SendCrashReport(message, category);
                #endif
            }
        }
        
        #endregion
        
        #region Performance Logging (Special conditional)
        
        [Conditional("ENABLE_PERFORMANCE_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Performance(string message, float value)
        {
            #if ENABLE_PERFORMANCE_LOGS
            UnityEngine.Debug.Log($"[PERF] {message}: {value:F2}ms");
            #endif
        }
        
        [Conditional("ENABLE_PERFORMANCE_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void PerformanceBegin(string marker)
        {
            #if ENABLE_PERFORMANCE_LOGS && UNITY_2019_1_OR_NEWER
            UnityEngine.Profiling.Profiler.BeginSample(marker);
            #endif
        }
        
        [Conditional("ENABLE_PERFORMANCE_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void PerformanceEnd()
        {
            #if ENABLE_PERFORMANCE_LOGS && UNITY_2019_1_OR_NEWER
            UnityEngine.Profiling.Profiler.EndSample();
            #endif
        }
        
        #endregion
        
        #region Network Logging (Special conditional)
        
        [Conditional("ENABLE_NETWORK_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Network(string message)
        {
            #if ENABLE_NETWORK_LOGS
            UnityEngine.Debug.Log($"[NET] {message}");
            #endif
        }
        
        [Conditional("ENABLE_NETWORK_LOGS")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void NetworkData(string message, byte[] data, int offset, int count)
        {
            #if ENABLE_NETWORK_LOGS && DEBUG
            var hex = BitConverter.ToString(data, offset, Math.Min(count, 32));
            UnityEngine.Debug.Log($"[NET] {message}: {hex}");
            #endif
        }
        
        #endregion
        
        #region Assertion (Debug only)
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void Assert(bool condition, string message)
        {
            if (!condition)
            {
                UnityEngine.Debug.LogAssertion($"Assertion failed: {message}");
            }
        }
        
        [Conditional("DEBUG")]
        [Conditional("UNITY_EDITOR")]
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static void AssertNotNull(object obj, string name)
        {
            if (obj == null)
            {
                UnityEngine.Debug.LogAssertion($"Assertion failed: {name} is null");
            }
        }
        
        #endregion
        
        #region Helper Methods
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private static string FormatMessage(string category, string message)
        {
            // Use string concatenation instead of interpolation for better performance
            return "[" + category + "] " + message;
        }
        
        [Conditional("ENABLE_CRASH_REPORTING")]
        private static void SendCrashReport(string message, string category)
        {
            // Implement crash reporting integration here
            // This would typically send to Firebase Crashlytics, Bugsnag, etc.
        }
        
        #endregion
        
        #region Profiling Helpers
        
        /// <summary>
        /// Disposable profiling scope for automatic Begin/End
        /// </summary>
        public readonly struct ProfilingScope : IDisposable
        {
            private readonly string _name;
            
            public ProfilingScope(string name)
            {
                _name = name;
                #if ENABLE_PERFORMANCE_LOGS && UNITY_2019_1_OR_NEWER
                UnityEngine.Profiling.Profiler.BeginSample(name);
                #endif
            }
            
            public void Dispose()
            {
                #if ENABLE_PERFORMANCE_LOGS && UNITY_2019_1_OR_NEWER
                UnityEngine.Profiling.Profiler.EndSample();
                #endif
            }
        }
        
        /// <summary>
        /// Create a profiling scope
        /// Usage: using (LogOptimized.Profile("MyOperation")) { ... }
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static ProfilingScope Profile(string name)
        {
            return new ProfilingScope(name);
        }
        
        #endregion
    }
    
    /// <summary>
    /// Static class for compile-time log configuration
    /// Add these symbols to Player Settings > Scripting Define Symbols
    /// </summary>
    public static class LogConfig
    {
        // Debug builds: DEBUG or UNITY_EDITOR
        // Release builds: Remove all symbols for zero overhead
        
        // Optional symbols for granular control:
        // ENABLE_INFO_LOGS - Enable info level logs
        // ENABLE_WARNING_LOGS - Enable warning logs
        // ENABLE_PERFORMANCE_LOGS - Enable performance profiling
        // ENABLE_NETWORK_LOGS - Enable network debugging
        // ENABLE_CRASH_REPORTING - Enable crash reporting
        
        /// <summary>
        /// Check if logging is enabled at compile time
        /// </summary>
        public static bool IsLoggingEnabled
        {
            get
            {
                #if DEBUG || UNITY_EDITOR
                return true;
                #else
                return false;
                #endif
            }
        }
    }
}