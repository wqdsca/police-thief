using System;

namespace PoliceThief.Core.Events
{
    /// <summary>
    /// Network related events
    /// </summary>
    public class NetworkConnectedEvent : GameEvent 
    { 
        public string ServerUrl { get; }
        
        public NetworkConnectedEvent(string serverUrl = null)
        {
            ServerUrl = serverUrl;
        }
    }
    
    public class NetworkDisconnectedEvent : GameEvent 
    { 
        public string Reason { get; }
        
        public NetworkDisconnectedEvent(string reason = null)
        {
            Reason = reason;
        }
    }
    
    public class NetworkErrorEvent : GameEvent 
    { 
        public string ErrorMessage { get; }
        public Exception Exception { get; }
        
        public NetworkErrorEvent(string errorMessage, Exception exception = null)
        {
            ErrorMessage = errorMessage;
            Exception = exception;
        }
    }
    
    public class NetworkLatencyUpdateEvent : GameEvent
    {
        public float Latency { get; }
        
        public NetworkLatencyUpdateEvent(float latency)
        {
            Latency = latency;
        }
    }
}