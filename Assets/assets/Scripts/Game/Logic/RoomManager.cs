using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
using PoliceThief.Core.DI;
using PoliceThief.Core.Events;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Grpc;
using Sirenix.OdinInspector;

namespace PoliceThief.Game.Logic
{
    /// <summary>
    /// Example room manager showing how to implement room creation and joining
    /// 방 생성 및 참가 핵심 로직 구현 예제
    /// </summary>
    public class RoomManager : MonoBehaviour
    {
        [Title("Room Configuration")]
        [SerializeField] private int minPlayersToStart = 2;
        
        [Title("Current Room Status")]
        [ShowInInspector]
        [DisplayAsString]
        public string RoomStatus => _currentRoom != null ? 
            $"In room: {_currentRoom.RoomId} ({_currentRoom.PlayerCount}/{_currentRoom.MaxPlayers})" : 
            "Not in a room";
        
        [ShowInInspector]
        [TableList]
        public List<RoomInfo> AvailableRooms => _availableRooms;
        
        [ShowInInspector]
        [TableList]
        public List<PlayerInfo> CurrentRoomPlayers => _currentRoom?.Players ?? new List<PlayerInfo>();
        
        private GrpcClientOptimized _grpcClient;
        private EventBus _eventBus;
        private RoomInfo _currentRoom;
        private List<RoomInfo> _availableRooms = new List<RoomInfo>();
        private bool _isHost;
        private IDisposable _loginSubscription;
        private IDisposable _logoutSubscription;
        
        private void Start()
        {
            Log.Info("RoomManager Start called", "Room");
            // Implementation removed - only logging remains
        }
        
        /// <summary>
        /// Create a new room
        /// 새 방 생성 - 여기에 실제 로직 구현
        /// </summary>
        [Button("Create Room", ButtonSizes.Large)]
        public async Task<RoomInfo> CreateRoomAsync(string roomName = null)
        {
            Log.Info($"CreateRoomAsync called with roomName: {roomName}", "Room");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return null;
        }
        
        /// <summary>
        /// Join an existing room
        /// 기존 방 참가 - 여기에 실제 로직 구현
        /// </summary>
        [Button("Join Room")]
        public async Task<bool> JoinRoomAsync(string roomId)
        {
            Log.Info($"JoinRoomAsync called with roomId: {roomId}", "Room");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return false;
        }
        
        /// <summary>
        /// Leave current room
        /// 현재 방 나가기 - 여기에 실제 로직 구현
        /// </summary>
        [Button("Leave Room")]
        [EnableIf("@_currentRoom != null")]
        public async Task LeaveRoomAsync()
        {
            Log.Info("LeaveRoomAsync called", "Room");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
        }
        
        /// <summary>
        /// Get list of available rooms
        /// 사용 가능한 방 목록 가져오기 - 여기에 실제 로직 구현
        /// </summary>
        [Button("Refresh Room List")]
        public async Task<List<RoomInfo>> GetAvailableRoomsAsync()
        {
            Log.Info("GetAvailableRoomsAsync called", "Room");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return new List<RoomInfo>();
        }
        
        /// <summary>
        /// Start game (host only)
        /// 게임 시작 (호스트만) - 여기에 실제 로직 구현
        /// </summary>
        [Button("Start Game", ButtonSizes.Large)]
        [EnableIf("@_isHost && _currentRoom != null && _currentRoom.PlayerCount >= minPlayersToStart")]
        public async Task<bool> StartGameAsync()
        {
            Log.Info("StartGameAsync called", "Room");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return false;
        }
        
        private void OnUserLoggedIn(UserLoggedInEvent evt)
        {
            Log.Info("OnUserLoggedIn called", "Room");
        }
        
        private void OnUserLoggedOut(UserLoggedOutEvent evt)
        {
            Log.Info("OnUserLoggedOut called", "Room");
        }
        
        private void OnDestroy()
        {
            _loginSubscription?.Dispose();
            _logoutSubscription?.Dispose();
        }
        
        [Serializable]
        public class RoomInfo
        {
            [TableColumnWidth(80)]
            public string RoomId { get; set; }
            
            [TableColumnWidth(150)]
            public string RoomName { get; set; }
            
            public string HostId { get; set; }
            
            [TableColumnWidth(80)]
            [ProgressBar(0, 8)]
            public int PlayerCount { get; set; }
            
            [TableColumnWidth(80)]
            public int MaxPlayers { get; set; }
            
            [HideInTables]
            public List<PlayerInfo> Players { get; set; } = new List<PlayerInfo>();
        }
        
        [Serializable]
        public class PlayerInfo
        {
            [TableColumnWidth(100)]
            public string PlayerId { get; set; }
            
            [TableColumnWidth(150)]
            public string PlayerName { get; set; }
            
            [TableColumnWidth(60)]
            public bool IsHost { get; set; }
        }
    }
    
    // Room Events
    public class RoomCreatedEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public RoomManager.RoomInfo Room { get; set; }
        
        public RoomCreatedEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
    
    public class RoomJoinedEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public RoomManager.RoomInfo Room { get; set; }
        
        public RoomJoinedEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
    
    public class RoomLeftEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public string RoomId { get; set; }
        
        public RoomLeftEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
    
    public class GameStartedEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public string RoomId { get; set; }
        
        public GameStartedEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
}