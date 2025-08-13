using UnityEngine;

namespace PoliceThief.Core.Logging
{
    /// <summary>
    /// Simple static logging wrapper for Unity Debug
    /// </summary>
    public static class Log
    {
        public static void Debug(string message, string category = "General")
        {
            UnityEngine.Debug.Log($"[{category}] {message}");
        }
        
        public static void Info(string message, string category = "General")
        {
            UnityEngine.Debug.Log($"[{category}] {message}");
        }
        
        public static void Warning(string message, string category = "General")
        {
            UnityEngine.Debug.LogWarning($"[{category}] {message}");
        }
        
        public static void Error(string message, string category = "General")
        {
            UnityEngine.Debug.LogError($"[{category}] {message}");
        }
        
        public static void Fatal(string message, string category = "General")
        {
            UnityEngine.Debug.LogError($"[FATAL][{category}] {message}");
        }
    }
}