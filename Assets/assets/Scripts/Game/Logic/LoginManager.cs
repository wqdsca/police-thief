using System;
using System.Threading.Tasks;
using PoliceThief.Core.DI;
using PoliceThief.Core.Events;
using PoliceThief.Core.Logging;
using PoliceThief.Infrastructure.Network.Grpc;

namespace PoliceThief.Game.Logic
{
    /// <summary>
    /// Login service interface
    /// 로그인 서비스 인터페이스
    /// </summary>
    public interface ILoginService
    {
        bool IsLoggedIn { get; }
        string login_token { get; }
        string login_type { get; }
        
        Task<bool> LoginAsync(string login_type, string login_token);
        Task LogoutAsync();
    }
    
    /// <summary>
    /// Pure C# login service implementation
    /// 순수 C# 로그인 서비스 구현
    /// </summary>
    public class LoginService : ILoginService, IDisposable
    {
        private static LoginService _instance;
        private static readonly object _lock = new object();
        
        public static LoginService Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_lock)
                    {
                        if (_instance == null)
                        {
                            _instance = new LoginService();
                        }
                    }
                }
                return _instance;
            }
        }
        
        // Configuration
        private readonly string _defaultUsername = "test1";
        
        // Status properties
        public bool IsLoggedIn => _isLoggedIn;
        public string login_token => _currentUserId; // Interface property
        public string login_type => "guest"; // Interface property
        public string CurrentUsername => _currentUsername;
        public string CurrentUserId => _currentUserId;
        public string LoginStatus => _isLoggedIn ? $"Logged in as {_currentUsername}" : "Not logged in";
        
        private IGrpcClient _grpcClient;
        private IEventBus _eventBus;
        private bool _isLoggedIn;
        private string _currentUsername;
        private string _currentUserId;
        private IDisposable _networkConnectedSubscription;
        private IDisposable _networkDisconnectedSubscription;
        
        private LoginService()
        {
            Initialize();
        }
        
        private void Initialize()
        {
            // Get services from DI container using interfaces
            _grpcClient = ServiceLocator.Instance.Get<IGrpcClient>();
            _eventBus = ServiceLocator.Instance.Get<IEventBus>();
            
            // Subscribe to network events
            _networkConnectedSubscription = _eventBus.Subscribe<NetworkConnectedEvent>(OnNetworkConnected);
            _networkDisconnectedSubscription = _eventBus.Subscribe<NetworkDisconnectedEvent>(OnNetworkDisconnected);
            
            Log.Info("LoginService initialized with DI architecture", "Login");
        }
        
        /// <summary>
        /// Login with username
        /// 사용자 이름으로 로그인 - 여기에 실제 로직 구현
        /// </summary>
        public async Task<bool> LoginAsync(string username = null)
        {
            username = string.IsNullOrEmpty(username) ? _defaultUsername : username;
            
            Log.Info($"LoginAsync called with username: {username}", "Login");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return false;
        }
        
        /// <summary>
        /// Interface method implementation
        /// </summary>
        public async Task<bool> LoginAsync(string login_type, string login_token)
        {
            Log.Info($"LoginAsync called with login_type: {login_type}, login_token: {login_token}", "Login");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return false;
        }
        
        /// <summary>
        /// Logout
        /// 로그아웃 - 여기에 실제 로직 구현
        /// </summary>
        public async Task LogoutAsync()
        {
            Log.Info("LogoutAsync called", "Login");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
        }
        
        /// <summary>
        /// Login with specific type (for UI integration)
        /// 로그인 타입별 로그인 - UI에서 호출용
        /// </summary>
        public async Task<bool> AsyncLogin(int loginType, string nickname = null)
        {
            Log.Info($"AsyncLogin called with loginType: {loginType}, nickname: {nickname}", "Login");
            
            // Implementation removed - only logging remains
            await Task.CompletedTask;
            return false;
        }
        
        private void OnNetworkConnected(NetworkConnectedEvent evt)
        {
            Log.Info("OnNetworkConnected called", "Login");
        }
        
        private void OnNetworkDisconnected(NetworkDisconnectedEvent evt)
        {
            Log.Info("OnNetworkDisconnected called", "Login");
        }
        
        public void Dispose()
        {
            _networkConnectedSubscription?.Dispose();
            _networkDisconnectedSubscription?.Dispose();
        }
    }
    
    // Login Events
    public class UserLoggedInEvent : IEvent
    {
        public DateTime Timestamp { get; }
        public string Username { get; set; }
        public string UserId { get; set; }
        
        public UserLoggedInEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
    
    public class UserLoggedOutEvent : IEvent 
    { 
        public DateTime Timestamp { get; }
        
        public UserLoggedOutEvent()
        {
            Timestamp = DateTime.UtcNow;
        }
    }
}