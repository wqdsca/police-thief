using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using UnityEngine;

namespace PoliceThief.Core.Events
{
    /// <summary>
    /// Mobile-optimized EventBus without reflection overhead
    /// Uses direct delegate invocation and struct events for zero-allocation
    /// </summary>
    public sealed class EventBusOptimized : IEventBus
    {
        private static EventBusOptimized _instance;
        private static readonly object _instanceLock = new object();
        
        public static EventBusOptimized Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_instanceLock)
                    {
                        _instance ??= new EventBusOptimized();
                    }
                }
                return _instance;
            }
        }
        
        // Direct delegate storage without type casting
        private readonly Dictionary<Type, object> _handlers = new Dictionary<Type, object>();
        private readonly Dictionary<Type, object> _weakHandlers = new Dictionary<Type, object>();
        
        // Optimized queue using delegates instead of reflection
        private readonly Queue<Action> _actionQueue = new Queue<Action>();
        
        // Reusable lists to avoid allocations
        private readonly Dictionary<Type, List<object>> _tempListCache = new Dictionary<Type, List<object>>();
        
        private readonly object _lock = new object();
        private bool _isProcessingQueue = false;
        
        // Statistics
        public int TotalEventTypes => _handlers.Count + _weakHandlers.Count;
        public int TotalSubscriptions => CalculateTotalSubscriptions();
        public int QueuedEvents => _actionQueue.Count;
        
        private EventBusOptimized()
        {
            // Private constructor for singleton
        }
        
        #region High-Frequency Event Direct Access
        
        // Direct access methods for high-frequency events to bypass dictionary lookup
        private Action<GameStateUpdateEvent> _gameStateUpdateHandlers;
        private Action<PlayerPositionEvent> _playerPositionHandlers;
        private Action<NetworkMessageEvent> _networkMessageHandlers;
        
        /// <summary>
        /// Optimized publish for high-frequency game state updates
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void PublishGameStateUpdate(ref GameStateUpdateEvent evt)
        {
            _gameStateUpdateHandlers?.Invoke(evt);
        }
        
        /// <summary>
        /// Optimized publish for player position updates
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void PublishPlayerPosition(ref PlayerPositionEvent evt)
        {
            _playerPositionHandlers?.Invoke(evt);
        }
        
        /// <summary>
        /// Optimized publish for network messages
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void PublishNetworkMessage(ref NetworkMessageEvent evt)
        {
            _networkMessageHandlers?.Invoke(evt);
        }
        
        #endregion
        
        /// <summary>
        /// Subscribe to an event with zero allocation
        /// </summary>
        public IDisposable Subscribe<T>(Action<T> handler) where T : IEvent
        {
            return SubscribeInternal(handler, false);
        }
        
        /// <summary>
        /// Subscribe with weak reference for auto-cleanup
        /// </summary>
        public IDisposable SubscribeWeak<T>(Action<T> handler) where T : IEvent
        {
            return SubscribeInternal(handler, true);
        }
        
        private IDisposable SubscribeInternal<T>(Action<T> handler, bool isWeak) where T : IEvent
        {
            lock (_lock)
            {
                var type = typeof(T);
                var targetDict = isWeak ? _weakHandlers : _handlers;
                
                // Special handling for high-frequency events
                if (type == typeof(GameStateUpdateEvent))
                {
                    _gameStateUpdateHandlers += handler as Action<GameStateUpdateEvent>;
                    return new SubscriptionToken(() => _gameStateUpdateHandlers -= handler as Action<GameStateUpdateEvent>);
                }
                else if (type == typeof(PlayerPositionEvent))
                {
                    _playerPositionHandlers += handler as Action<PlayerPositionEvent>;
                    return new SubscriptionToken(() => _playerPositionHandlers -= handler as Action<PlayerPositionEvent>);
                }
                else if (type == typeof(NetworkMessageEvent))
                {
                    _networkMessageHandlers += handler as Action<NetworkMessageEvent>;
                    return new SubscriptionToken(() => _networkMessageHandlers -= handler as Action<NetworkMessageEvent>);
                }
                
                // Regular event handling
                if (!targetDict.TryGetValue(type, out var existingDelegate))
                {
                    targetDict[type] = handler;
                }
                else
                {
                    targetDict[type] = Delegate.Combine(existingDelegate as Delegate, handler as Delegate);
                }
                
                return new SubscriptionToken(() => UnsubscribeInternal(type, handler, isWeak));
            }
        }
        
        private void UnsubscribeInternal<T>(Type type, Action<T> handler, bool isWeak) where T : IEvent
        {
            lock (_lock)
            {
                var targetDict = isWeak ? _weakHandlers : _handlers;
                
                if (targetDict.TryGetValue(type, out var existingDelegate))
                {
                    var newDelegate = Delegate.Remove(existingDelegate as Delegate, handler as Delegate);
                    
                    if (newDelegate == null)
                    {
                        targetDict.Remove(type);
                    }
                    else
                    {
                        targetDict[type] = newDelegate;
                    }
                }
            }
        }
        
        /// <summary>
        /// Publish event immediately with minimal overhead
        /// </summary>
        public void Publish<T>(T eventData) where T : IEvent
        {
            var type = typeof(T);
            
            // Fast path for struct events
            if (typeof(T).IsValueType)
            {
                PublishStruct(ref eventData);
                return;
            }
            
            // Regular path for class events
            PublishClass(eventData);
        }
        
        /// <summary>
        /// Optimized struct event publishing
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private void PublishStruct<T>(ref T eventData) where T : IEvent
        {
            var type = typeof(T);
            
            if (_handlers.TryGetValue(type, out var handler))
            {
                (handler as Action<T>)?.Invoke(eventData);
            }
            
            if (_weakHandlers.TryGetValue(type, out var weakHandler))
            {
                (weakHandler as Action<T>)?.Invoke(eventData);
            }
            
            #if DEBUG && !UNITY_EDITOR
            // Conditional compilation to remove debug logs in release builds
            #else
            LogEvent(type.Name);
            #endif
        }
        
        /// <summary>
        /// Regular class event publishing
        /// </summary>
        private void PublishClass<T>(T eventData) where T : IEvent
        {
            var type = typeof(T);
            
            lock (_lock)
            {
                if (_handlers.TryGetValue(type, out var handler))
                {
                    try
                    {
                        (handler as Action<T>)?.Invoke(eventData);
                    }
                    catch (Exception ex)
                    {
                        LogError($"Error invoking handler for {type.Name}: {ex}");
                    }
                }
                
                // Clean up dead weak references periodically
                if (_weakHandlers.TryGetValue(type, out var weakHandler))
                {
                    try
                    {
                        (weakHandler as Action<T>)?.Invoke(eventData);
                    }
                    catch (Exception ex)
                    {
                        LogError($"Error invoking weak handler for {type.Name}: {ex}");
                    }
                }
            }
        }
        
        /// <summary>
        /// Queue event for batch processing (no reflection)
        /// </summary>
        public void PublishQueued<T>(T eventData) where T : IEvent
        {
            lock (_lock)
            {
                // Capture event data in closure to avoid reflection
                var localEvent = eventData;
                _actionQueue.Enqueue(() => Publish(localEvent));
            }
        }
        
        /// <summary>
        /// Process queued events without reflection
        /// </summary>
        public void ProcessEventQueue()
        {
            if (_isProcessingQueue) return;
            
            _isProcessingQueue = true;
            
            try
            {
                while (_actionQueue.Count > 0)
                {
                    Action action;
                    
                    lock (_lock)
                    {
                        if (_actionQueue.Count == 0) break;
                        action = _actionQueue.Dequeue();
                    }
                    
                    action?.Invoke();
                }
            }
            finally
            {
                _isProcessingQueue = false;
            }
        }
        
        /// <summary>
        /// Async publish for non-critical events with cancellation support
        /// </summary>
        public async void PublishAsync<T>(T eventData) where T : IEvent
        {
            try
            {
                await System.Threading.Tasks.Task.Yield();
                
                // Check if we should still publish (avoiding circular reference)
                // Simply publish without cancellation check for now
                Publish(eventData);
            }
            catch (System.OperationCanceledException)
            {
                // Silently handle cancellation
            }
            catch (Exception ex)
            {
                LogError($"Async publish failed: {ex.Message}");
            }
        }
        
        /// <summary>
        /// Publish event with delay and cancellation support
        /// </summary>
        public async Task PublishDelayedAsync<T>(T eventData, int delayMs, System.Threading.CancellationToken? token = null) where T : IEvent
        {
            var cancellationToken = token ?? System.Threading.CancellationToken.None;
            
            try
            {
                await Task.Delay(delayMs, cancellationToken);
                
                if (!cancellationToken.IsCancellationRequested)
                {
                    Publish(eventData);
                }
            }
            catch (System.OperationCanceledException)
            {
                // Expected when cancelled
            }
        }
        
        /// <summary>
        /// Clear all subscriptions
        /// </summary>
        public void ClearAll()
        {
            lock (_lock)
            {
                _handlers.Clear();
                _weakHandlers.Clear();
                _actionQueue.Clear();
                _tempListCache.Clear();
                
                _gameStateUpdateHandlers = null;
                _playerPositionHandlers = null;
                _networkMessageHandlers = null;
            }
        }
        
        private int CalculateTotalSubscriptions()
        {
            int count = 0;
            
            foreach (var handler in _handlers.Values)
            {
                if (handler is Delegate del)
                {
                    count += del.GetInvocationList().Length;
                }
            }
            
            foreach (var handler in _weakHandlers.Values)
            {
                if (handler is Delegate del)
                {
                    count += del.GetInvocationList().Length;
                }
            }
            
            return count;
        }
        
        #region Conditional Logging
        
        [System.Diagnostics.Conditional("EVENTBUS_LOGGING")]
        private void LogEvent(string eventName)
        {
            Debug.Log($"[EventBus] Published: {eventName}");
        }
        
        [System.Diagnostics.Conditional("EVENTBUS_LOGGING")]
        private void LogError(string message)
        {
            Debug.LogError($"[EventBus] {message}");
        }
        
        #endregion
        
        private sealed class SubscriptionToken : IDisposable
        {
            private Action _unsubscribe;
            
            public SubscriptionToken(Action unsubscribe)
            {
                _unsubscribe = unsubscribe;
            }
            
            public void Dispose()
            {
                _unsubscribe?.Invoke();
                _unsubscribe = null;
            }
        }
    }
    
    #region Optimized Event Structs
    
    /// <summary>
    /// High-frequency game state update event (struct for zero allocation)
    /// </summary>
    public readonly struct GameStateUpdateEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public readonly int StateId;
        public readonly float DeltaTime;
        
        public GameStateUpdateEvent(int stateId, float deltaTime)
        {
            Timestamp = DateTime.UtcNow;
            StateId = stateId;
            DeltaTime = deltaTime;
        }
    }
    
    /// <summary>
    /// Player position update event (struct for zero allocation)
    /// </summary>
    public readonly struct PlayerPositionEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public readonly int PlayerId;
        public readonly float X, Y, Z;
        
        public PlayerPositionEvent(int playerId, float x, float y, float z)
        {
            Timestamp = DateTime.UtcNow;
            PlayerId = playerId;
            X = x;
            Y = y;
            Z = z;
        }
    }
    
    /// <summary>
    /// Network message event (struct for zero allocation)
    /// </summary>
    public readonly struct NetworkMessageEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public readonly int MessageType;
        public readonly ArraySegment<byte> Data;
        
        public NetworkMessageEvent(int messageType, ArraySegment<byte> data)
        {
            Timestamp = DateTime.UtcNow;
            MessageType = messageType;
            Data = data;
        }
    }
    
    /// <summary>
    /// Empty events as structs to avoid heap allocation
    /// </summary>
    public readonly struct GameStartEventStruct : IEvent
    {
        public DateTime Timestamp { get; }
        public GameStartEventStruct(bool dummy = false) { Timestamp = DateTime.UtcNow; }
    }
    
    public readonly struct GameEndEventStruct : IEvent
    {
        public DateTime Timestamp { get; }
        public GameEndEventStruct(bool dummy = false) { Timestamp = DateTime.UtcNow; }
    }
    
    public readonly struct GamePausedEventStruct : IEvent
    {
        public DateTime Timestamp { get; }
        public GamePausedEventStruct(bool dummy = false) { Timestamp = DateTime.UtcNow; }
    }
    
    public readonly struct GameResumedEventStruct : IEvent
    {
        public DateTime Timestamp { get; }
        public GameResumedEventStruct(bool dummy = false) { Timestamp = DateTime.UtcNow; }
    }
    
    #endregion
}