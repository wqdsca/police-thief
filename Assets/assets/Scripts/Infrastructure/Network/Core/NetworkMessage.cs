using System;
using UnityEngine;

namespace PoliceThief.Infrastructure.Network.Core
{
    /// <summary>
    /// QUIC/TCP 통신을 위한 기본 네트워크 메시지 프로토콜
    /// </summary>
    [Serializable]
    public class NetworkMessage
    {
        public uint messageId;
        public MessageType messageType;
        public uint sequenceNumber;
        public DateTime timestamp;
        public byte[] payload;
        
        public NetworkMessage()
        {
            messageId = GenerateMessageId();
            timestamp = DateTime.UtcNow;
        }
        
        private static uint _messageIdCounter = 0;
        private static uint GenerateMessageId() => ++_messageIdCounter;
    }
    
    /// <summary>
    /// 네트워크 통신용 메시지 타입
    /// </summary>
    public enum MessageType : byte
    {
        // 연결 관리
        Connect = 0,
        ConnectAck = 1,
        Disconnect = 2,
        Heartbeat = 3,
        
        // 게임 메시지
        GameData = 10,
        PlayerAction = 11,
        StateSync = 12,
        
        // QUIC 관련
        Acknowledgment = 20,
        Retransmission = 21,
        
        // 시스템 메시지
        Error = 255
    }
    
    /// <summary>
    /// 네트워크 클라이언트의 연결 상태
    /// </summary>
    public enum ClientConnectionState : byte
    {
        Disconnected = 0,
        Connecting = 1,
        Connected = 2,
        Disconnecting = 3,
        Error = 4
    }
    
    /// <summary>
    /// 모니터링용 네트워크 통계
    /// </summary>
    [Serializable]
    public class NetworkStats
    {
        public int totalMessagesSent;
        public int totalMessagesReceived;
        public int messagesLost;
        public int messagesRetransmitted;
        public float averageLatency;
        public float packetLossRate;
        public DateTime lastActivity;
        
        public void Reset()
        {
            totalMessagesSent = 0;
            totalMessagesReceived = 0;
            messagesLost = 0;
            messagesRetransmitted = 0;
            averageLatency = 0;
            packetLossRate = 0;
            lastActivity = DateTime.UtcNow;
        }
    }
}