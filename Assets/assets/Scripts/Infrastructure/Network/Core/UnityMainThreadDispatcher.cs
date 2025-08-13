using System;
using System.Collections.Generic;
using UnityEngine;

namespace PoliceThief.Infrastructure.Network.Core
{
    /// <summary>
    /// Dispatcher to execute actions on Unity's main thread from background threads
    /// </summary>
    public class UnityMainThreadDispatcher : MonoBehaviour
    {
        private static UnityMainThreadDispatcher _instance;
        private readonly Queue<Action> _executionQueue = new Queue<Action>();
        private readonly object _queueLock = new object();

        public static UnityMainThreadDispatcher Instance
        {
            get
            {
                if (_instance == null)
                {
                    var go = new GameObject("[UnityMainThreadDispatcher]");
                    _instance = go.AddComponent<UnityMainThreadDispatcher>();
                    DontDestroyOnLoad(go);
                }
                return _instance;
            }
        }

        private void Awake()
        {
            if (_instance != null && _instance != this)
            {
                Destroy(gameObject);
                return;
            }
            
            _instance = this;
            DontDestroyOnLoad(gameObject);
        }

        private void Update()
        {
            lock (_queueLock)
            {
                while (_executionQueue.Count > 0)
                {
                    var action = _executionQueue.Dequeue();
                    try
                    {
                        action?.Invoke();
                    }
                    catch (Exception ex)
                    {
                        Debug.LogError($"Error executing action on main thread: {ex}");
                    }
                }
            }
        }

        public void Enqueue(Action action)
        {
            if (action == null) return;
            
            lock (_queueLock)
            {
                _executionQueue.Enqueue(action);
            }
        }
    }
}