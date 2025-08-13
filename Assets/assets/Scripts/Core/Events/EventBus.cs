using System;
using System.Collections.Generic;
using UnityEngine;
using Sirenix.OdinInspector;

namespace PoliceThief.Core.Events
{
    /// <summary>
    /// Event bus interface
    /// </summary>
    public interface IEventBus
    {
        IDisposable Subscribe<T>(Action<T> handler) where T : IEvent;
        void Publish<T>(T eventData) where T : IEvent;
        void PublishAsync<T>(T eventData) where T : IEvent;
        void ClearAll();
        int TotalEventTypes { get; }
        int TotalSubscriptions { get; }
        int QueuedEvents { get; }
    }
    
    /// <summary>
    /// High-performance event bus with type safety and weak references
    /// Converted from MonoBehaviour to pure C# class
    /// </summary>
    public sealed class EventBus : IEventBus
    {
        private static EventBus _instance;
        private static readonly object _instanceLock = new object();
        
        public static EventBus Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_instanceLock)
                    {
                        if (_instance == null)
                        {
                            _instance = new EventBus();
                        }
                    }
                }
                return _instance;
            }
        }
        
        private readonly Dictionary<Type, List<IEventSubscription>> _subscriptions = new Dictionary<Type, List<IEventSubscription>>();
        private readonly Queue<QueuedEvent> _eventQueue = new Queue<QueuedEvent>();
        private readonly object _lock = new object();
        
        // Event Statistics - removed Odin attributes as this is no longer a MonoBehaviour
        public int TotalEventTypes => _subscriptions.Count;
        public int TotalSubscriptions => GetTotalSubscriptionCount();
        public int QueuedEvents => _eventQueue.Count;
        public Dictionary<string, int> EventTypeSubscriptions => GetEventTypeSubscriptions();
        
        private bool _isProcessingQueue = false;
        
        private EventBus()
        {
            // Private constructor for singleton pattern
        }
        
        // Note: Event queue processing needs to be called manually or through a game loop
        // since this is no longer a MonoBehaviour
        
        /// <summary>
        /// Subscribe to an event (interface implementation)
        /// </summary>
        public IDisposable Subscribe<T>(Action<T> handler) where T : IEvent
        {
            return Subscribe(handler, 0);
        }
        
        /// <summary>
        /// Subscribe to an event with a strong reference
        /// </summary>
        public IDisposable Subscribe<T>(Action<T> handler, int priority = 0) where T : IEvent
        {
            lock (_lock)
            {
                var type = typeof(T);
                if (!_subscriptions.TryGetValue(type, out var list))
                {
                    list = new List<IEventSubscription>();
                    _subscriptions[type] = list;
                }
                
                var subscription = new EventSubscription<T>(handler, priority);
                
                // Insert based on priority
                int insertIndex = list.Count;
                for (int i = 0; i < list.Count; i++)
                {
                    if (list[i].Priority < priority)
                    {
                        insertIndex = i;
                        break;
                    }
                }
                
                list.Insert(insertIndex, subscription);
                
                return new SubscriptionToken(() => Unsubscribe(type, subscription));
            }
        }
        
        /// <summary>
        /// Subscribe with a weak reference (automatically removed when handler is GC'd)
        /// </summary>
        public IDisposable SubscribeWeak<T>(Action<T> handler, int priority = 0) where T : IEvent
        {
            lock (_lock)
            {
                var type = typeof(T);
                if (!_subscriptions.TryGetValue(type, out var list))
                {
                    list = new List<IEventSubscription>();
                    _subscriptions[type] = list;
                }
                
                var subscription = new WeakEventSubscription<T>(handler, priority);
                
                // Insert based on priority
                int insertIndex = list.Count;
                for (int i = 0; i < list.Count; i++)
                {
                    if (list[i].Priority < priority)
                    {
                        insertIndex = i;
                        break;
                    }
                }
                
                list.Insert(insertIndex, subscription);
                
                return new SubscriptionToken(() => Unsubscribe(type, subscription));
            }
        }
        
        /// <summary>
        /// Publish an event immediately
        /// </summary>
        public void Publish<T>(T evt) where T : IEvent
        {
            lock (_lock)
            {
                var type = typeof(T);
                
                if (_subscriptions.TryGetValue(type, out var list))
                {
                    // Create a copy to avoid modification during iteration
                    var subscriptions = new List<IEventSubscription>(list);
                    
                    // Remove dead weak references
                    list.RemoveAll(s => !s.IsAlive);
                    
                    foreach (var subscription in subscriptions)
                    {
                        if (subscription.IsAlive)
                        {
                            try
                            {
                                subscription.Invoke(evt);
                            }
                            catch (Exception ex)
                            {
                                UnityEngine.Debug.LogError($"[EventBus] Error invoking handler for {type.Name}: {ex}");
                            }
                        }
                    }
                }
                
                // Log event if debugging
                #if UNITY_EDITOR
                UnityEngine.Debug.Log($"[EventBus] Published: {type.Name}");
                #endif
            }
        }
        
        /// <summary>
        /// Publish an event asynchronously
        /// </summary>
        public void PublishAsync<T>(T eventData) where T : IEvent
        {
            // For now, just publish immediately - can be enhanced with async handling later
            Publish(eventData);
        }
        
        /// <summary>
        /// Queue an event to be published at the end of frame
        /// </summary>
        public void PublishQueued<T>(T evt) where T : IEvent
        {
            lock (_lock)
            {
                _eventQueue.Enqueue(new QueuedEvent(evt, typeof(T)));
            }
        }
        
        /// <summary>
        /// Publish an event after a delay (using Task instead of Coroutine)
        /// </summary>
        public async void PublishDelayed<T>(T evt, int delayMs) where T : IEvent
        {
            await System.Threading.Tasks.Task.Delay(delayMs);
            Publish(evt);
        }
        
        private void ProcessEventQueue()
        {
            if (_isProcessingQueue) return;
            
            _isProcessingQueue = true;
            
            while (_eventQueue.Count > 0)
            {
                QueuedEvent queuedEvent;
                
                lock (_lock)
                {
                    if (_eventQueue.Count == 0) break;
                    queuedEvent = _eventQueue.Dequeue();
                }
                
                // Use reflection to call Publish with the correct type
                var publishMethod = GetType().GetMethod(nameof(Publish)).MakeGenericMethod(queuedEvent.EventType);
                publishMethod.Invoke(this, new[] { queuedEvent.Event });
            }
            
            _isProcessingQueue = false;
        }
        
        private void Unsubscribe(Type eventType, IEventSubscription subscription)
        {
            lock (_lock)
            {
                if (_subscriptions.TryGetValue(eventType, out var list))
                {
                    list.Remove(subscription);
                    
                    if (list.Count == 0)
                    {
                        _subscriptions.Remove(eventType);
                    }
                }
            }
        }
        
        /// <summary>
        /// Clear all subscriptions for a specific event type
        /// </summary>
        public void ClearSubscriptions<T>() where T : IEvent
        {
            lock (_lock)
            {
                _subscriptions.Remove(typeof(T));
            }
        }
        
        /// <summary>
        /// Clear all subscriptions (interface implementation)
        /// </summary>
        public void ClearAll()
        {
            ClearAllSubscriptions();
        }
        
        /// <summary>
        /// Clear all subscriptions
        /// </summary>
        public void ClearAllSubscriptions()
        {
            lock (_lock)
            {
                _subscriptions.Clear();
                _eventQueue.Clear();
            }
            
            UnityEngine.Debug.Log("[EventBus] All subscriptions cleared");
        }
        
        private int GetTotalSubscriptionCount()
        {
            int count = 0;
            foreach (var list in _subscriptions.Values)
            {
                count += list.Count;
            }
            return count;
        }
        
        private Dictionary<string, int> GetEventTypeSubscriptions()
        {
            var result = new Dictionary<string, int>();
            foreach (var kvp in _subscriptions)
            {
                result[kvp.Key.Name] = kvp.Value.Count;
            }
            return result;
        }
        
        public void LogEventStatistics()
        {
            UnityEngine.Debug.Log($"[EventBus] Total Event Types: {TotalEventTypes}");
            UnityEngine.Debug.Log($"[EventBus] Total Subscriptions: {TotalSubscriptions}");
            UnityEngine.Debug.Log($"[EventBus] Queued Events: {QueuedEvents}");
            
            foreach (var kvp in EventTypeSubscriptions)
            {
                UnityEngine.Debug.Log($"  {kvp.Key}: {kvp.Value} subscriptions");
            }
        }
        
        private interface IEventSubscription
        {
            int Priority { get; }
            bool IsAlive { get; }
            void Invoke(object evt);
        }
        
        private class EventSubscription<T> : IEventSubscription where T : IEvent
        {
            private readonly Action<T> _handler;
            public int Priority { get; }
            public bool IsAlive => true;
            
            public EventSubscription(Action<T> handler, int priority)
            {
                _handler = handler;
                Priority = priority;
            }
            
            public void Invoke(object evt)
            {
                _handler((T)evt);
            }
        }
        
        private class WeakEventSubscription<T> : IEventSubscription where T : IEvent
        {
            private readonly WeakReference _weakHandler;
            public int Priority { get; }
            public bool IsAlive => _weakHandler.IsAlive;
            
            public WeakEventSubscription(Action<T> handler, int priority)
            {
                _weakHandler = new WeakReference(handler);
                Priority = priority;
            }
            
            public void Invoke(object evt)
            {
                if (_weakHandler.Target is Action<T> handler)
                {
                    handler((T)evt);
                }
            }
        }
        
        private class SubscriptionToken : IDisposable
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
        
        private struct QueuedEvent
        {
            public object Event;
            public Type EventType;
            
            public QueuedEvent(object evt, Type eventType)
            {
                Event = evt;
                EventType = eventType;
            }
        }
    }
    
    /// <summary>
    /// Base interface for all events
    /// </summary>
    public interface IEvent
    {
        DateTime Timestamp { get; }
    }
    
    /// <summary>
    /// Base class for events
    /// </summary>
    public abstract class GameEvent : IEvent
    {
        public DateTime Timestamp { get; }
        
        protected GameEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
    
    /// <summary>
    /// Common game events
    /// </summary>
    public class GameStartEvent : GameEvent { }
    public class GameEndEvent : GameEvent { }
    public class GamePausedEvent : GameEvent { }
    public class GameResumedEvent : GameEvent { }
    
    public class PlayerConnectedEvent : GameEvent
    {
        public string PlayerId { get; }
        public PlayerConnectedEvent(string playerId) { PlayerId = playerId; }
    }
    
    public class PlayerDisconnectedEvent : GameEvent
    {
        public string PlayerId { get; }
        public PlayerDisconnectedEvent(string playerId) { PlayerId = playerId; }
    }
}