using System;
using System.Collections.Generic;
using UnityEngine;
using Sirenix.OdinInspector;
using System.Threading.Tasks;

namespace PoliceThief.Core.State
{
    /// <summary>
    /// Advanced hierarchical state machine with async support
    /// </summary>
    public class StateMachine : MonoBehaviour
    {
        [Title("State Machine Info")]
        [ShowInInspector]
        [DisplayAsString]
        public string CurrentStateName => _currentState?.GetType().Name ?? "None";
        
        [ShowInInspector]
        [DisplayAsString]
        public string PreviousStateName => _previousState?.GetType().Name ?? "None";
        
        [ShowInInspector]
        [DisplayAsString]
        public float TimeInCurrentState => _currentState != null ? Time.time - _stateEnterTime : 0f;
        
        [ShowInInspector]
        private Dictionary<string, float> StateHistory => GetStateHistory();
        
        private IState _currentState;
        private IState _previousState;
        private float _stateEnterTime;
        private Dictionary<Type, IState> _states = new Dictionary<Type, IState>();
        private Dictionary<Type, float> _stateTimeHistory = new Dictionary<Type, float>();
        private Stack<IState> _stateStack = new Stack<IState>();
        private bool _isTransitioning = false;
        
        public IState CurrentState => _currentState;
        public bool IsTransitioning => _isTransitioning;
        
        // Events
        public event Action<IState, IState> OnStateChanged;
        public event Action<IState> OnStateEntered;
        public event Action<IState> OnStateExited;
        
        /// <summary>
        /// Register a state
        /// </summary>
        public void RegisterState<T>(T state) where T : IState
        {
            var type = typeof(T);
            if (_states.ContainsKey(type))
            {
                UnityEngine.Debug.LogWarning($"[StateMachine] State {type.Name} already registered");
                return;
            }
            
            _states[type] = state;
            state.Initialize(this);
            
            UnityEngine.Debug.Log($"[StateMachine] Registered state: {type.Name}");
        }
        
        /// <summary>
        /// Change to a specific state
        /// </summary>
        public async Task ChangeStateAsync<T>() where T : IState
        {
            await ChangeStateAsync(typeof(T));
        }
        
        /// <summary>
        /// Change to a specific state with data
        /// </summary>
        public async Task ChangeStateAsync<T>(object data) where T : IState
        {
            await ChangeStateAsync(typeof(T), data);
        }
        
        private async Task ChangeStateAsync(Type stateType, object data = null)
        {
            if (_isTransitioning)
            {
                UnityEngine.Debug.LogWarning($"[StateMachine] Already transitioning, ignoring request for {stateType.Name}");
                return;
            }
            
            if (!_states.TryGetValue(stateType, out var newState))
            {
                UnityEngine.Debug.LogError($"[StateMachine] State {stateType.Name} not registered");
                return;
            }
            
            if (_currentState == newState)
            {
                UnityEngine.Debug.Log($"[StateMachine] Already in state {stateType.Name}");
                return;
            }
            
            _isTransitioning = true;
            
            // Exit current state
            if (_currentState != null)
            {
                UnityEngine.Debug.Log($"[StateMachine] Exiting state: {_currentState.GetType().Name}");
                
                // Track time spent in state
                var timeInState = Time.time - _stateEnterTime;
                if (!_stateTimeHistory.ContainsKey(_currentState.GetType()))
                {
                    _stateTimeHistory[_currentState.GetType()] = 0;
                }
                _stateTimeHistory[_currentState.GetType()] += timeInState;
                
                OnStateExited?.Invoke(_currentState);
                await _currentState.OnExitAsync();
                _previousState = _currentState;
            }
            
            // Enter new state
            _currentState = newState;
            _stateEnterTime = Time.time;
            
            UnityEngine.Debug.Log($"[StateMachine] Entering state: {newState.GetType().Name}");
            
            OnStateChanged?.Invoke(_previousState, _currentState);
            await _currentState.OnEnterAsync(data);
            OnStateEntered?.Invoke(_currentState);
            
            _isTransitioning = false;
        }
        
        /// <summary>
        /// Push a state onto the stack (for nested states)
        /// </summary>
        public async Task PushStateAsync<T>(object data = null) where T : IState
        {
            if (_currentState != null)
            {
                _stateStack.Push(_currentState);
                await _currentState.OnPauseAsync();
            }
            
            await ChangeStateAsync<T>(data);
        }
        
        /// <summary>
        /// Pop the previous state from the stack
        /// </summary>
        public async Task PopStateAsync()
        {
            if (_stateStack.Count == 0)
            {
                UnityEngine.Debug.LogWarning("[StateMachine] State stack is empty");
                return;
            }
            
            var previousState = _stateStack.Pop();
            await ChangeStateAsync(previousState.GetType());
            await previousState.OnResumeAsync();
        }
        
        /// <summary>
        /// Check if a state is registered
        /// </summary>
        public bool HasState<T>() where T : IState
        {
            return _states.ContainsKey(typeof(T));
        }
        
        /// <summary>
        /// Get a registered state
        /// </summary>
        public T GetState<T>() where T : IState
        {
            if (_states.TryGetValue(typeof(T), out var state))
            {
                return (T)state;
            }
            return default(T);
        }
        
        private void Update()
        {
            if (!_isTransitioning && _currentState != null)
            {
                _currentState.OnUpdate();
            }
        }
        
        private void FixedUpdate()
        {
            if (!_isTransitioning && _currentState != null)
            {
                _currentState.OnFixedUpdate();
            }
        }
        
        private void LateUpdate()
        {
            if (!_isTransitioning && _currentState != null)
            {
                _currentState.OnLateUpdate();
            }
        }
        
        [Button("Log State History")]
        private void LogStateHistory()
        {
            UnityEngine.Debug.Log("[StateMachine] State History:");
            foreach (var kvp in _stateTimeHistory)
            {
                UnityEngine.Debug.Log($"  {kvp.Key.Name}: {kvp.Value:F2} seconds");
            }
        }
        
        private Dictionary<string, float> GetStateHistory()
        {
            var history = new Dictionary<string, float>();
            foreach (var kvp in _stateTimeHistory)
            {
                history[kvp.Key.Name] = kvp.Value;
            }
            return history;
        }
    }
    
    /// <summary>
    /// Interface for states
    /// </summary>
    public interface IState
    {
        void Initialize(StateMachine stateMachine);
        Task OnEnterAsync(object data = null);
        Task OnExitAsync();
        void OnUpdate();
        void OnFixedUpdate();
        void OnLateUpdate();
        Task OnPauseAsync();
        Task OnResumeAsync();
    }
    
    /// <summary>
    /// Base implementation of a state
    /// </summary>
    public abstract class StateBase : IState
    {
        protected StateMachine StateMachine { get; private set; }
        
        public virtual void Initialize(StateMachine stateMachine)
        {
            StateMachine = stateMachine;
        }
        
        public virtual Task OnEnterAsync(object data = null)
        {
            return Task.CompletedTask;
        }
        
        public virtual Task OnExitAsync()
        {
            return Task.CompletedTask;
        }
        
        public virtual void OnUpdate() { }
        public virtual void OnFixedUpdate() { }
        public virtual void OnLateUpdate() { }
        
        public virtual Task OnPauseAsync()
        {
            return Task.CompletedTask;
        }
        
        public virtual Task OnResumeAsync()
        {
            return Task.CompletedTask;
        }
        
        protected async Task ChangeStateAsync<T>() where T : IState
        {
            await StateMachine.ChangeStateAsync<T>();
        }
        
        protected async Task ChangeStateAsync<T>(object data) where T : IState
        {
            await StateMachine.ChangeStateAsync<T>(data);
        }
    }
    
    /// <summary>
    /// Hierarchical state machine for complex state management
    /// </summary>
    public class HierarchicalStateMachine : StateMachine
    {
        private Dictionary<IState, StateMachine> _subStateMachines = new Dictionary<IState, StateMachine>();
        
        /// <summary>
        /// Register a sub-state machine for a state
        /// </summary>
        public void RegisterSubStateMachine(IState parentState, StateMachine subStateMachine)
        {
            _subStateMachines[parentState] = subStateMachine;
        }
        
        /// <summary>
        /// Get the sub-state machine for the current state
        /// </summary>
        public StateMachine GetCurrentSubStateMachine()
        {
            if (CurrentState != null && _subStateMachines.TryGetValue(CurrentState, out var subMachine))
            {
                return subMachine;
            }
            return null;
        }
    }
}