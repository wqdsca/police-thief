using System;
using UnityEngine;

namespace PoliceThief.Core.Config
{
    /// <summary>
    /// Network configuration for QUIC and TCP connections
    /// </summary>
    [Serializable]
    public class NetworkConfig
    {
        [Header("QUIC Configuration")]
        public string quicHost = "localhost";
        public int quicPort = 5000;
        public int connectTimeoutMs = 5000;
        public int requestTimeoutMs = 30000;
        public int keepAliveIntervalMs = 30000;
        public bool enable0Rtt = true;
        public int maxStreamsPerConnection = 100;
        public int maxDataPerStream = 10485760; // 10MB
        public bool enableConnectionMigration = true;
        
        [Header("TCP Configuration")]
        public string tcpHost = "localhost";
        public int tcpPort = 4000;
        public int tcpTimeoutMs = 5000;
        public int tcpMaxRetries = 3;
        public int tcpBufferSize = 8192;
        public bool tcpEnableKeepalive = true;
        public int tcpKeepaliveIntervalMs = 30000;
        public bool tcpEnableCompression = true;
        
        [Header("General Settings")]
        public bool enableAutoReconnect = true;
        public int reconnectDelayMs = 3000;
        public int maxConcurrentConnections = 10;
        public bool enableLogging = true;
        
        [Header("Performance Settings")]
        public bool enableBatching = true;
        public int batchIntervalMs = 50;
        public int maxBatchSize = 10;
        public bool enableAdaptiveProtocol = true;
        public int connectionPoolSize = 5;
        
        public string GetQuicEndpoint() => $"https://{quicHost}:{quicPort}";
        public string GetTcpEndpoint() => $"{tcpHost}:{tcpPort}";
    }
}