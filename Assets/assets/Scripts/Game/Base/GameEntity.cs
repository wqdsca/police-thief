using System;
using UnityEngine;
using Sirenix.OdinInspector;
using PoliceThief.Core.Events;
using PoliceThief.Core.Pool;

namespace PoliceThief.Game.Base
{
    /// <summary>
    /// Base class for all game entities (players, NPCs, objects)
    /// </summary>
    public abstract class GameEntity : MonoBehaviour, IPoolable
    {
        [Title("Entity Info")]
        [SerializeField]
        protected string _entityId;
        
        [SerializeField]
        protected string _entityName;
        
        [SerializeField]
        protected EntityType _entityType;
        
        [Title("Entity Stats")]
        [ShowInInspector]
        [ProgressBar(0, 100)]
        public virtual float Health { get; protected set; } = 100f;
        
        [ShowInInspector]
        [ProgressBar(0, 100)]
        public virtual float MaxHealth { get; protected set; } = 100f;
        
        [ShowInInspector]
        public bool IsAlive => Health > 0;
        
        [Title("Entity State")]
        [ShowInInspector]
        [DisplayAsString]
        public EntityState CurrentState { get; protected set; } = EntityState.Idle;
        
        [ShowInInspector]
        public bool IsActive { get; protected set; }
        
        // Properties
        public string EntityId => _entityId;
        public string EntityName => _entityName;
        public EntityType EntityType => _entityType;
        
        // Events
        public event Action<GameEntity> OnEntitySpawned;
        public event Action<GameEntity> OnEntityDestroyed;
        public event Action<EntityState, EntityState> OnStateChanged;
        
        protected virtual void Awake()
        {
            if (string.IsNullOrEmpty(_entityId))
            {
                _entityId = Guid.NewGuid().ToString();
            }
        }
        
        protected virtual void Start()
        {
            Initialize();
        }
        
        protected virtual void OnEnable()
        {
            IsActive = true;
            OnEntitySpawned?.Invoke(this);
            EventBus.Instance.Publish(new EntitySpawnedEvent(this));
        }
        
        protected virtual void OnDisable()
        {
            IsActive = false;
            OnEntityDestroyed?.Invoke(this);
            EventBus.Instance.Publish(new EntityDestroyedEvent(this));
        }
        
        /// <summary>
        /// Initialize the entity
        /// </summary>
        protected virtual void Initialize()
        {
            UnityEngine.Debug.Log($"GameEntity Initialize called for {_entityName}");
        }
        
        /// <summary>
        /// Update entity logic
        /// </summary>
        protected virtual void Update()
        {
            // Implementation removed - only logging remains
            if (!IsActive || !IsAlive) return;
        }
        
        /// <summary>
        /// Override for custom update logic
        /// </summary>
        protected abstract void UpdateEntity();
        
        /// <summary>
        /// Take damage
        /// </summary>
        public virtual void TakeDamage(float damage, GameEntity attacker = null)
        {
            UnityEngine.Debug.Log($"TakeDamage called on {_entityName} with damage: {damage}");
        }
        
        /// <summary>
        /// Heal the entity
        /// </summary>
        public virtual void Heal(float amount)
        {
            UnityEngine.Debug.Log($"Heal called on {_entityName} with amount: {amount}");
        }
        
        /// <summary>
        /// Handle entity death
        /// </summary>
        protected virtual void Die(GameEntity killer = null)
        {
            UnityEngine.Debug.Log($"Die called on {_entityName}");
        }
        
        /// <summary>
        /// Change entity state
        /// </summary>
        protected virtual void ChangeState(EntityState newState)
        {
            if (CurrentState == newState) return;
            
            var previousState = CurrentState;
            CurrentState = newState;
            
            OnStateChanged?.Invoke(previousState, newState);
            OnStateChangedInternal(previousState, newState);
        }
        
        /// <summary>
        /// Override for state change handling
        /// </summary>
        protected virtual void OnStateChangedInternal(EntityState previousState, EntityState newState)
        {
            // Override in derived classes
        }
        
        /// <summary>
        /// Reset the entity
        /// </summary>
        public virtual void Reset()
        {
            Health = MaxHealth;
            ChangeState(EntityState.Idle);
            transform.position = Vector3.zero;
            transform.rotation = Quaternion.identity;
        }
        
        // IPoolable implementation
        public virtual void OnGetFromPool()
        {
            Reset();
            Initialize();
        }
        
        public virtual void OnReturnToPool()
        {
            // Clean up
        }
        
        #if UNITY_EDITOR
        [Title("Debug Actions")]
        [Button("Take Damage (10)")]
        private void DebugTakeDamage()
        {
            TakeDamage(10);
        }
        
        [Button("Heal (20)")]
        private void DebugHeal()
        {
            Heal(20);
        }
        
        [Button("Kill")]
        private void DebugKill()
        {
            TakeDamage(Health);
        }
        #endif
    }
    
    public enum EntityType
    {
        Player,
        NPC,
        Object,
        Projectile,
        Pickup
    }
    
    public enum EntityState
    {
        Idle,
        Moving,
        Attacking,
        Defending,
        Stunned,
        Dead
    }
    
    // Entity Events
    public class EntitySpawnedEvent : GameEvent
    {
        public GameEntity Entity { get; }
        public EntitySpawnedEvent(GameEntity entity) { Entity = entity; }
    }
    
    public class EntityDestroyedEvent : GameEvent
    {
        public GameEntity Entity { get; }
        public EntityDestroyedEvent(GameEntity entity) { Entity = entity; }
    }
    
    public class EntityDamagedEvent : GameEvent
    {
        public GameEntity Entity { get; }
        public float Damage { get; }
        public GameEntity Attacker { get; }
        
        public EntityDamagedEvent(GameEntity entity, float damage, GameEntity attacker)
        {
            Entity = entity;
            Damage = damage;
            Attacker = attacker;
        }
    }
    
    public class EntityHealedEvent : GameEvent
    {
        public GameEntity Entity { get; }
        public float Amount { get; }
        
        public EntityHealedEvent(GameEntity entity, float amount)
        {
            Entity = entity;
            Amount = amount;
        }
    }
    
    public class EntityDiedEvent : GameEvent
    {
        public GameEntity Entity { get; }
        public GameEntity Killer { get; }
        
        public EntityDiedEvent(GameEntity entity, GameEntity killer)
        {
            Entity = entity;
            Killer = killer;
        }
    }
}