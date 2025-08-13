using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using UnityEngine;
using PoliceThief.Core.Pool;

namespace PoliceThief.Infrastructure.Network.Core
{
    /// <summary>
    /// Mobile-optimized network message using ArraySegment and pooling
    /// Zero-allocation design for high-frequency network communication
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct NetworkMessageOptimized : IPoolable
    {
        public uint messageId;
        public MessageType messageType;
        public uint sequenceNumber;
        public long timestampTicks; // Using ticks instead of DateTime to avoid allocation
        public ArraySegment<byte> payload; // Zero-copy payload reference
        
        private static uint _messageIdCounter;
        
        /// <summary>
        /// Get timestamp as DateTime (allocates)
        /// </summary>
        public DateTime Timestamp => new DateTime(timestampTicks, DateTimeKind.Utc);
        
        /// <summary>
        /// Initialize message with minimal overhead
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void Initialize(MessageType type, ArraySegment<byte> data, uint sequence = 0)
        {
            messageId = GenerateMessageId();
            messageType = type;
            sequenceNumber = sequence;
            timestampTicks = DateTime.UtcNow.Ticks;
            payload = data;
        }
        
        /// <summary>
        /// Reset message for pool return
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void Reset()
        {
            messageId = 0;
            messageType = MessageType.Connect;
            sequenceNumber = 0;
            timestampTicks = 0;
            payload = default;
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private static uint GenerateMessageId()
        {
            return ++_messageIdCounter;
        }
        
        public void OnGetFromPool()
        {
            // Message will be initialized when used
        }
        
        public void OnReturnToPool()
        {
            Reset();
        }
    }
    
    /// <summary>
    /// Pooled network message wrapper for heap-allocated scenarios
    /// </summary>
    public sealed class PooledNetworkMessage : IPoolable, IDisposable
    {
        private static readonly GenericPool<PooledNetworkMessage> _pool = 
            new GenericPool<PooledNetworkMessage>(initialSize: 20, maxSize: 100);
        
        // Reusable buffer pool for payload data
        private static readonly GenericPool<ByteBuffer> _bufferPool = 
            new GenericPool<ByteBuffer>(initialSize: 20, maxSize: 100);
        
        public uint MessageId { get; private set; }
        public MessageType MessageType { get; set; }
        public uint SequenceNumber { get; set; }
        public long TimestampTicks { get; private set; }
        
        private ByteBuffer _buffer;
        public ArraySegment<byte> Payload => _buffer?.Segment ?? default;
        
        /// <summary>
        /// Get a pooled message instance
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static PooledNetworkMessage Get()
        {
            var message = _pool.Get() ?? new PooledNetworkMessage();
            message.Initialize();
            return message;
        }
        
        /// <summary>
        /// Get a pooled message with data
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public static PooledNetworkMessage Get(MessageType type, byte[] data, int offset, int count)
        {
            var message = Get();
            message.SetData(type, data, offset, count);
            return message;
        }
        
        private void Initialize()
        {
            MessageId = GenerateMessageId();
            TimestampTicks = DateTime.UtcNow.Ticks;
            MessageType = MessageType.Connect;
            SequenceNumber = 0;
        }
        
        /// <summary>
        /// Set message data without allocation
        /// </summary>
        public void SetData(MessageType type, byte[] data, int offset, int count)
        {
            MessageType = type;
            
            if (_buffer == null)
            {
                _buffer = _bufferPool.Get() ?? new ByteBuffer();
            }
            
            _buffer.SetData(data, offset, count);
        }
        
        /// <summary>
        /// Set message data from existing ArraySegment
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void SetData(MessageType type, ArraySegment<byte> data)
        {
            MessageType = type;
            
            if (_buffer == null)
            {
                _buffer = _bufferPool.Get() ?? new ByteBuffer();
            }
            
            _buffer.SetData(data);
        }
        
        public void OnGetFromPool()
        {
            Initialize();
        }
        
        public void OnReturnToPool()
        {
            MessageId = 0;
            MessageType = MessageType.Connect;
            SequenceNumber = 0;
            TimestampTicks = 0;
            
            if (_buffer != null)
            {
                _bufferPool.Return(_buffer);
                _buffer = null;
            }
        }
        
        public void Dispose()
        {
            OnReturnToPool();
            _pool.Return(this);
        }
        
        private static uint _messageIdCounter;
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private static uint GenerateMessageId()
        {
            return ++_messageIdCounter;
        }
    }
    
    /// <summary>
    /// Reusable byte buffer for network messages
    /// </summary>
    internal sealed class ByteBuffer : IPoolable
    {
        private const int DEFAULT_BUFFER_SIZE = 1024;
        private const int MAX_BUFFER_SIZE = 65536; // 64KB max
        
        private byte[] _internalBuffer;
        private int _dataLength;
        
        public ArraySegment<byte> Segment => new ArraySegment<byte>(_internalBuffer, 0, _dataLength);
        
        public ByteBuffer()
        {
            _internalBuffer = new byte[DEFAULT_BUFFER_SIZE];
        }
        
        /// <summary>
        /// Set data without allocation if possible
        /// </summary>
        public void SetData(byte[] data, int offset, int count)
        {
            if (count > MAX_BUFFER_SIZE)
            {
                throw new ArgumentException($"Data size {count} exceeds maximum buffer size {MAX_BUFFER_SIZE}");
            }
            
            // Resize buffer if needed
            if (count > _internalBuffer.Length)
            {
                var newSize = Math.Min(GetNextPowerOfTwo(count), MAX_BUFFER_SIZE);
                _internalBuffer = new byte[newSize];
            }
            
            Buffer.BlockCopy(data, offset, _internalBuffer, 0, count);
            _dataLength = count;
        }
        
        /// <summary>
        /// Set data from ArraySegment
        /// </summary>
        public void SetData(ArraySegment<byte> data)
        {
            if (data.Count > MAX_BUFFER_SIZE)
            {
                throw new ArgumentException($"Data size {data.Count} exceeds maximum buffer size {MAX_BUFFER_SIZE}");
            }
            
            // Resize buffer if needed
            if (data.Count > _internalBuffer.Length)
            {
                var newSize = Math.Min(GetNextPowerOfTwo(data.Count), MAX_BUFFER_SIZE);
                _internalBuffer = new byte[newSize];
            }
            
            Buffer.BlockCopy(data.Array, data.Offset, _internalBuffer, 0, data.Count);
            _dataLength = data.Count;
        }
        
        public void OnGetFromPool()
        {
            _dataLength = 0;
        }
        
        public void OnReturnToPool()
        {
            _dataLength = 0;
            
            // Clear sensitive data
            if (_internalBuffer.Length > 0)
            {
                Array.Clear(_internalBuffer, 0, Math.Min(_dataLength, _internalBuffer.Length));
            }
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private static int GetNextPowerOfTwo(int value)
        {
            value--;
            value |= value >> 1;
            value |= value >> 2;
            value |= value >> 4;
            value |= value >> 8;
            value |= value >> 16;
            return value + 1;
        }
    }
    
    /// <summary>
    /// Optimized network statistics with reduced allocations
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct NetworkStatsOptimized
    {
        public int totalMessagesSent;
        public int totalMessagesReceived;
        public int messagesLost;
        public int messagesRetransmitted;
        public float averageLatency;
        public float packetLossRate;
        public long lastActivityTicks;
        
        // Add bytes tracking
        private long totalBytesSent;
        private long totalBytesReceived;
        
        public DateTime LastActivity => new DateTime(lastActivityTicks, DateTimeKind.Utc);
        
        // Unity compatibility methods
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public long GetTotalBytesSent() => totalBytesSent;
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public long GetTotalBytesReceived() => totalBytesReceived;
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public float GetAverageRTT() => averageLatency;
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public float GetPacketLossRate() => packetLossRate;
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public bool Is0RTTAvailable() => false; // Simplified for Unity
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public int GetMigrationCount() => 0; // Simplified for Unity
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void Reset()
        {
            totalMessagesSent = 0;
            totalMessagesReceived = 0;
            messagesLost = 0;
            messagesRetransmitted = 0;
            averageLatency = 0;
            packetLossRate = 0;
            lastActivityTicks = DateTime.UtcNow.Ticks;
            totalBytesSent = 0;
            totalBytesReceived = 0;
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void UpdateActivity()
        {
            lastActivityTicks = DateTime.UtcNow.Ticks;
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void IncrementSent(int bytes = 0)
        {
            totalMessagesSent++;
            totalBytesSent += bytes;
            UpdateActivity();
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void IncrementReceived(int bytes = 0)
        {
            totalMessagesReceived++;
            totalBytesReceived += bytes;
            UpdateActivity();
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void UpdateLatency(float newLatency)
        {
            // Simple moving average
            averageLatency = (averageLatency * 0.9f) + (newLatency * 0.1f);
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void CalculatePacketLoss()
        {
            var total = totalMessagesSent;
            if (total > 0)
            {
                packetLossRate = (messagesLost / (float)total) * 100f;
            }
        }
    }
    
    /// <summary>
    /// Network message batch for efficient bulk operations
    /// </summary>
    public sealed class NetworkMessageBatch : IDisposable
    {
        private readonly PooledNetworkMessage[] _messages;
        private int _count;
        private readonly int _capacity;
        
        public int Count => _count;
        public bool IsFull => _count >= _capacity;
        
        public NetworkMessageBatch(int capacity = 10)
        {
            _capacity = capacity;
            _messages = new PooledNetworkMessage[capacity];
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public bool TryAdd(MessageType type, byte[] data, int offset, int count)
        {
            if (IsFull) return false;
            
            var message = PooledNetworkMessage.Get(type, data, offset, count);
            _messages[_count++] = message;
            return true;
        }
        
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public PooledNetworkMessage GetMessage(int index)
        {
            if (index < 0 || index >= _count)
                throw new IndexOutOfRangeException();
            
            return _messages[index];
        }
        
        public void Clear()
        {
            for (int i = 0; i < _count; i++)
            {
                _messages[i]?.Dispose();
                _messages[i] = null;
            }
            _count = 0;
        }
        
        public void Dispose()
        {
            Clear();
        }
    }
}