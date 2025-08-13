using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using UnityEngine;

namespace PoliceThief.Core.Async
{
    /// <summary>
    /// Centralized async operation manager with proper cancellation and error handling
    /// Prevents memory leaks from orphaned tasks in mobile environments
    /// </summary>
    public sealed class AsyncManager : IDisposable
    {
        private static AsyncManager _instance;
        private static readonly object _instanceLock = new object();
        
        public static AsyncManager Instance
        {
            get
            {
                if (_instance == null)
                {
                    lock (_instanceLock)
                    {
                        _instance ??= new AsyncManager();
                    }
                }
                return _instance;
            }
        }
        
        // Master cancellation source for application lifetime
        private CancellationTokenSource _applicationCts;
        
        // Scene-specific cancellation (reset on scene change)
        private CancellationTokenSource _sceneCts;
        
        // Track active operations for cleanup
        private readonly HashSet<IAsyncOperation> _activeOperations = new HashSet<IAsyncOperation>();
        private readonly object _operationsLock = new object();
        
        // Reusable task completion sources pool
        private readonly Queue<TaskCompletionSource<bool>> _tcsPool = new Queue<TaskCompletionSource<bool>>(10);
        
        private AsyncManager()
        {
            _applicationCts = new CancellationTokenSource();
            _sceneCts = new CancellationTokenSource();
            
            // Register for scene changes
            #if UNITY_2019_1_OR_NEWER
            UnityEngine.SceneManagement.SceneManager.sceneUnloaded += OnSceneUnloaded;
            #endif
            
            // Register for application quit
            Application.quitting += OnApplicationQuit;
        }
        
        /// <summary>
        /// Get application-wide cancellation token
        /// </summary>
        public CancellationToken ApplicationToken => _applicationCts.Token;
        
        /// <summary>
        /// Get scene-specific cancellation token
        /// </summary>
        public CancellationToken SceneToken => _sceneCts.Token;
        
        /// <summary>
        /// Create a linked token that cancels with either application or scene
        /// </summary>
        public CancellationToken GetLinkedToken()
        {
            return CancellationTokenSource.CreateLinkedTokenSource(
                _applicationCts.Token,
                _sceneCts.Token
            ).Token;
        }
        
        /// <summary>
        /// Run an async operation with automatic cancellation and error handling
        /// </summary>
        public async Task RunAsync(
            Func<CancellationToken, Task> operation,
            string operationName = "AsyncOperation",
            bool sceneScoped = true)
        {
            var token = sceneScoped ? GetLinkedToken() : ApplicationToken;
            var asyncOp = new AsyncOperation(operationName);
            
            lock (_operationsLock)
            {
                _activeOperations.Add(asyncOp);
            }
            
            try
            {
                await operation(token).ConfigureAwait(false);
                asyncOp.Complete();
            }
            catch (OperationCanceledException)
            {
                asyncOp.Cancel();
                Debug.Log($"[AsyncManager] Operation cancelled: {operationName}");
            }
            catch (Exception ex)
            {
                asyncOp.Fail(ex);
                Debug.LogError($"[AsyncManager] Operation failed: {operationName} - {ex.Message}");
                throw;
            }
            finally
            {
                lock (_operationsLock)
                {
                    _activeOperations.Remove(asyncOp);
                }
            }
        }
        
        /// <summary>
        /// Run an async operation with result
        /// </summary>
        public async Task<T> RunAsync<T>(
            Func<CancellationToken, Task<T>> operation,
            string operationName = "AsyncOperation",
            bool sceneScoped = true)
        {
            var token = sceneScoped ? GetLinkedToken() : ApplicationToken;
            var asyncOp = new AsyncOperation(operationName);
            
            lock (_operationsLock)
            {
                _activeOperations.Add(asyncOp);
            }
            
            try
            {
                var result = await operation(token).ConfigureAwait(false);
                asyncOp.Complete();
                return result;
            }
            catch (OperationCanceledException)
            {
                asyncOp.Cancel();
                Debug.Log($"[AsyncManager] Operation cancelled: {operationName}");
                return default(T);
            }
            catch (Exception ex)
            {
                asyncOp.Fail(ex);
                Debug.LogError($"[AsyncManager] Operation failed: {operationName} - {ex.Message}");
                throw;
            }
            finally
            {
                lock (_operationsLock)
                {
                    _activeOperations.Remove(asyncOp);
                }
            }
        }
        
        /// <summary>
        /// Delay with cancellation support
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public Task DelayAsync(int milliseconds, CancellationToken? token = null)
        {
            var cancellationToken = token ?? GetLinkedToken();
            return Task.Delay(milliseconds, cancellationToken);
        }
        
        /// <summary>
        /// Delay with TimeSpan and cancellation support
        /// </summary>
        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public Task DelayAsync(TimeSpan delay, CancellationToken? token = null)
        {
            var cancellationToken = token ?? GetLinkedToken();
            return Task.Delay(delay, cancellationToken);
        }
        
        /// <summary>
        /// Run operation with timeout
        /// </summary>
        public async Task<T> RunWithTimeoutAsync<T>(
            Func<CancellationToken, Task<T>> operation,
            int timeoutMs,
            string operationName = "TimeoutOperation")
        {
            using (var timeoutCts = new CancellationTokenSource(timeoutMs))
            using (var linkedCts = CancellationTokenSource.CreateLinkedTokenSource(
                GetLinkedToken(), timeoutCts.Token))
            {
                try
                {
                    return await RunAsync(operation, operationName + "_Timeout", false)
                        .ConfigureAwait(false);
                }
                catch (OperationCanceledException) when (timeoutCts.IsCancellationRequested)
                {
                    throw new TimeoutException($"Operation '{operationName}' timed out after {timeoutMs}ms");
                }
            }
        }
        
        /// <summary>
        /// Fire and forget with error handling
        /// </summary>
        public void FireAndForget(
            Func<CancellationToken, Task> operation,
            string operationName = "FireAndForget",
            Action<Exception> onError = null)
        {
            _ = Task.Run(async () =>
            {
                try
                {
                    await RunAsync(operation, operationName).ConfigureAwait(false);
                }
                catch (Exception ex)
                {
                    onError?.Invoke(ex);
                }
            });
        }
        
        /// <summary>
        /// Cancel all scene-scoped operations
        /// </summary>
        public void CancelSceneOperations()
        {
            _sceneCts?.Cancel();
            _sceneCts?.Dispose();
            _sceneCts = new CancellationTokenSource();
            
            Debug.Log("[AsyncManager] Scene operations cancelled");
        }
        
        /// <summary>
        /// Cancel all operations
        /// </summary>
        public void CancelAllOperations()
        {
            _applicationCts?.Cancel();
            _sceneCts?.Cancel();
            
            lock (_operationsLock)
            {
                foreach (var op in _activeOperations)
                {
                    op.Cancel();
                }
                _activeOperations.Clear();
            }
            
            Debug.Log("[AsyncManager] All async operations cancelled");
        }
        
        /// <summary>
        /// Get a pooled TaskCompletionSource
        /// </summary>
        public TaskCompletionSource<bool> GetPooledTcs()
        {
            lock (_tcsPool)
            {
                if (_tcsPool.Count > 0)
                {
                    var tcs = _tcsPool.Dequeue();
                    // Reset the TCS for reuse
                    return tcs;
                }
            }
            
            return new TaskCompletionSource<bool>();
        }
        
        /// <summary>
        /// Return a TaskCompletionSource to pool
        /// </summary>
        public void ReturnTcs(TaskCompletionSource<bool> tcs)
        {
            if (tcs == null) return;
            
            lock (_tcsPool)
            {
                if (_tcsPool.Count < 20) // Limit pool size
                {
                    _tcsPool.Enqueue(tcs);
                }
            }
        }
        
        private void OnSceneUnloaded(UnityEngine.SceneManagement.Scene scene)
        {
            CancelSceneOperations();
        }
        
        private void OnApplicationQuit()
        {
            CancelAllOperations();
        }
        
        public void Dispose()
        {
            CancelAllOperations();
            
            _applicationCts?.Dispose();
            _sceneCts?.Dispose();
            
            #if UNITY_2019_1_OR_NEWER
            UnityEngine.SceneManagement.SceneManager.sceneUnloaded -= OnSceneUnloaded;
            #endif
            
            Application.quitting -= OnApplicationQuit;
            
            _instance = null;
        }
        
        /// <summary>
        /// Interface for tracking async operations
        /// </summary>
        private interface IAsyncOperation
        {
            string Name { get; }
            void Complete();
            void Cancel();
            void Fail(Exception ex);
        }
        
        /// <summary>
        /// Async operation tracker
        /// </summary>
        private class AsyncOperation : IAsyncOperation
        {
            public string Name { get; }
            public OperationStatus Status { get; private set; }
            public Exception Error { get; private set; }
            public DateTime StartTime { get; }
            public DateTime? EndTime { get; private set; }
            
            public AsyncOperation(string name)
            {
                Name = name;
                Status = OperationStatus.Running;
                StartTime = DateTime.UtcNow;
            }
            
            public void Complete()
            {
                Status = OperationStatus.Completed;
                EndTime = DateTime.UtcNow;
            }
            
            public void Cancel()
            {
                Status = OperationStatus.Cancelled;
                EndTime = DateTime.UtcNow;
            }
            
            public void Fail(Exception ex)
            {
                Status = OperationStatus.Failed;
                Error = ex;
                EndTime = DateTime.UtcNow;
            }
        }
        
        private enum OperationStatus
        {
            Running,
            Completed,
            Cancelled,
            Failed
        }
    }
    
    /// <summary>
    /// Extension methods for Task with cancellation
    /// </summary>
    public static class TaskExtensions
    {
        /// <summary>
        /// Safely fire and forget with error handling
        /// </summary>
        public static void SafeFireAndForget(
            this Task task,
            Action<Exception> onException = null,
            bool continueOnCapturedContext = false)
        {
            _ = SafeFireAndForgetAsync(task, onException, continueOnCapturedContext);
        }
        
        private static async Task SafeFireAndForgetAsync(
            Task task,
            Action<Exception> onException,
            bool continueOnCapturedContext)
        {
            try
            {
                await task.ConfigureAwait(continueOnCapturedContext);
            }
            catch (Exception ex) when (onException != null)
            {
                onException(ex);
            }
        }
        
        /// <summary>
        /// Add timeout to any task
        /// </summary>
        public static async Task<T> WithTimeout<T>(
            this Task<T> task,
            int timeoutMs)
        {
            using (var cts = new CancellationTokenSource(timeoutMs))
            {
                var completedTask = await Task.WhenAny(task, Task.Delay(timeoutMs, cts.Token));
                
                if (completedTask == task)
                {
                    cts.Cancel(); // Cancel the delay task
                    return await task;
                }
                else
                {
                    throw new TimeoutException($"Operation timed out after {timeoutMs}ms");
                }
            }
        }
        
        /// <summary>
        /// Add cancellation to any task
        /// </summary>
        public static async Task<T> WithCancellation<T>(
            this Task<T> task,
            CancellationToken cancellationToken)
        {
            var tcs = new TaskCompletionSource<bool>();
            
            using (cancellationToken.Register(s => ((TaskCompletionSource<bool>)s).TrySetResult(true), tcs))
            {
                if (task != await Task.WhenAny(task, tcs.Task))
                {
                    throw new OperationCanceledException(cancellationToken);
                }
            }
            
            return await task;
        }
    }
}