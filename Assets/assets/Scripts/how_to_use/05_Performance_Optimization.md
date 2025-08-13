# Performance Optimization 상세 가이드

## 개요

Police-Thief 프로젝트는 Unity 환경에서 최적의 성능을 제공하기 위해 다양한 최적화 기법을 적용합니다. 특히 모바일 플랫폼과 실시간 멀티플레이어 환경을 고려하여 메모리, CPU, 네트워크, GPU 성능을 체계적으로 최적화합니다.

## 목차
1. [성능 최적화 전략](#성능-최적화-전략)
2. [메모리 최적화](#메모리-최적화)
3. [CPU 최적화](#cpu-최적화)
4. [네트워크 최적화](#네트워크-최적화)
5. [GPU 최적화](#gpu-최적화)
6. [Unity 특화 최적화](#unity-특화-최적화)
7. [모바일 최적화](#모바일-최적화)
8. [프로파일링 및 모니터링](#프로파일링-및-모니터링)
9. [빌드 최적화](#빌드-최적화)

---

## 성능 최적화 전략

### 최적화 우선순위

```
1. 메모리 관리 (가장 중요)
   ├── 가비지 컬렉션 최소화
   ├── 오브젝트 풀링
   └── 메모리 리크 방지

2. 네트워크 최적화
   ├── 대역폭 사용량 최소화
   ├── 지연시간 최소화
   └── 안정적인 연결 유지

3. CPU 최적화
   ├── 알고리즘 효율성
   ├── 멀티스레딩 활용
   └── 불필요한 연산 제거

4. GPU 최적화
   ├── 드로우 콜 최소화
   ├── 텍스처 최적화
   └── 셰이더 최적화
```

### 성능 목표

```csharp
public static class PerformanceTargets
{
    // 프레임레이트 목표
    public const int TARGET_FPS_MOBILE = 30;
    public const int TARGET_FPS_DESKTOP = 60;
    
    // 메모리 사용량 목표
    public const int MAX_HEAP_SIZE_MB_MOBILE = 150;
    public const int MAX_HEAP_SIZE_MB_DESKTOP = 500;
    
    // 네트워크 목표
    public const int MAX_LATENCY_MS = 100;
    public const float MAX_PACKET_LOSS_RATE = 0.01f; // 1%
    
    // GPU 목표
    public const int MAX_DRAW_CALLS = 100;
    public const int MAX_VERTICES_PER_FRAME = 50000;
}
```

---

## 메모리 최적화

### 1. 오브젝트 풀링 시스템

메모리 할당/해제 오버헤드를 최소화하기 위한 고성능 풀링 시스템입니다.

```csharp
public class ObjectPoolOptimized<T> : IDisposable where T : class, IPoolable, new()
{
    private readonly ConcurrentQueue<T> _objects = new ConcurrentQueue<T>();
    private readonly Func<T> _createFunc;
    private readonly Action<T> _resetAction;
    private readonly int _maxSize;
    private volatile int _currentSize;
    
    // 성능 메트릭
    private volatile int _totalGets;
    private volatile int _totalReturns;
    private volatile int _cacheHits;
    private volatile int _cacheMisses;
    
    public ObjectPoolOptimized(int initialSize = 10, int maxSize = 100, 
                              Func<T> createFunc = null, Action<T> resetAction = null)
    {
        _maxSize = maxSize;
        _createFunc = createFunc ?? (() => new T());
        _resetAction = resetAction;
        
        // 초기 오브젝트 생성
        for (int i = 0; i < initialSize; i++)
        {
            var obj = _createFunc();
            obj.OnReturnToPool();
            _objects.Enqueue(obj);
            _currentSize++;
        }
    }
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public T Get()
    {
        Interlocked.Increment(ref _totalGets);
        
        if (_objects.TryDequeue(out T obj))
        {
            Interlocked.Decrement(ref _currentSize);
            Interlocked.Increment(ref _cacheHits);
            obj.OnGetFromPool();
            return obj;
        }
        
        // 풀이 비었으면 새로 생성
        Interlocked.Increment(ref _cacheMisses);
        obj = _createFunc();
        obj.OnGetFromPool();
        return obj;
    }
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public void Return(T obj)
    {
        if (obj == null) return;
        
        Interlocked.Increment(ref _totalReturns);
        
        // 리셋 작업
        _resetAction?.Invoke(obj);
        obj.OnReturnToPool();
        
        // 풀 크기 제한 확인
        if (_currentSize < _maxSize)
        {
            _objects.Enqueue(obj);
            Interlocked.Increment(ref _currentSize);
        }
    }
    
    public PoolStats GetStats()
    {
        return new PoolStats
        {
            CurrentSize = _currentSize,
            MaxSize = _maxSize,
            TotalGets = _totalGets,
            TotalReturns = _totalReturns,
            CacheHitRate = _totalGets > 0 ? (float)_cacheHits / _totalGets : 0f
        };
    }
    
    public void Dispose()
    {
        while (_objects.TryDequeue(out var obj))
        {
            if (obj is IDisposable disposable)
                disposable.Dispose();
        }
        _currentSize = 0;
    }
}

public struct PoolStats
{
    public int CurrentSize;
    public int MaxSize;
    public int TotalGets;
    public int TotalReturns;
    public float CacheHitRate;
}
```

### 2. 메모리 효율적인 데이터 구조

```csharp
/// <summary>
/// 메모리 효율적인 리스트 (ArrayList 대신 사용)
/// </summary>
public class CompactList<T> : IDisposable where T : struct
{
    private T[] _items;
    private int _count;
    private int _capacity;
    
    public CompactList(int capacity = 4)
    {
        _capacity = capacity;
        _items = new T[capacity];
    }
    
    public int Count => _count;
    public int Capacity => _capacity;
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public void Add(T item)
    {
        if (_count >= _capacity)
        {
            Resize();
        }
        
        _items[_count++] = item;
    }
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public T this[int index]
    {
        get => _items[index];
        set => _items[index] = value;
    }
    
    private void Resize()
    {
        int newCapacity = _capacity * 2;
        var newItems = new T[newCapacity];
        Array.Copy(_items, newItems, _count);
        _items = newItems;
        _capacity = newCapacity;
    }
    
    public void Clear()
    {
        _count = 0;
    }
    
    public void Dispose()
    {
        _items = null;
        _count = 0;
        _capacity = 0;
    }
}

/// <summary>
/// 스택 할당 최적화를 위한 구조체 기반 버퍼
/// </summary>
public unsafe struct StackBuffer<T> where T : unmanaged
{
    private fixed byte _buffer[1024]; // 1KB 스택 버퍼
    private int _count;
    
    public int Count => _count;
    public int MaxCapacity => 1024 / sizeof(T);
    
    public void Add(T item)
    {
        if (_count >= MaxCapacity)
            throw new InvalidOperationException("Buffer overflow");
        
        fixed (byte* ptr = _buffer)
        {
            ((T*)ptr)[_count++] = item;
        }
    }
    
    public T this[int index]
    {
        get
        {
            if (index >= _count)
                throw new IndexOutOfRangeException();
            
            fixed (byte* ptr = _buffer)
            {
                return ((T*)ptr)[index];
            }
        }
    }
    
    public void Clear()
    {
        _count = 0;
    }
}
```

### 3. 스마트 가비지 컬렉션 관리

```csharp
public class GarbageCollectionManager : MonoBehaviour
{
    [Header("GC Settings")]
    [SerializeField] private float gcInterval = 30f; // 30초마다 GC 수행
    [SerializeField] private long memoryThreshold = 100 * 1024 * 1024; // 100MB
    
    private float _lastGcTime;
    private int _framesSinceLastGc;
    
    // GC 통계
    private int _totalGcCount;
    private float _totalGcTime;
    private long _peakMemoryUsage;
    
    void Update()
    {
        _framesSinceLastGc++;
        
        // 메모리 사용량 모니터링
        long currentMemory = GC.GetTotalMemory(false);
        _peakMemoryUsage = Math.Max(_peakMemoryUsage, currentMemory);
        
        // GC 조건 확인
        bool shouldGc = Time.time - _lastGcTime > gcInterval || 
                        currentMemory > memoryThreshold;
        
        if (shouldGc && _framesSinceLastGc > 60) // 최소 60프레임 대기
        {
            PerformOptimalGc();
        }
    }
    
    private void PerformOptimalGc()
    {
        var stopwatch = System.Diagnostics.Stopwatch.StartNew();
        
        // 프레임 드랍을 최소화하기 위한 단계적 GC
        if (Application.isMobilePlatform)
        {
            // 모바일에서는 더 조심스럽게
            GC.Collect(0, GCCollectionMode.Optimized);
        }
        else
        {
            // 데스크톱에서는 전체 GC
            GC.Collect();
        }
        
        stopwatch.Stop();
        
        _totalGcCount++;
        _totalGcTime += stopwatch.ElapsedMilliseconds;
        _lastGcTime = Time.time;
        _framesSinceLastGc = 0;
        
        Log.Debug($"GC performed - Time: {stopwatch.ElapsedMilliseconds}ms, " +
                 $"Memory freed: {_peakMemoryUsage - GC.GetTotalMemory(false):N0} bytes", "Performance");
    }
    
    public GcStats GetStats()
    {
        return new GcStats
        {
            TotalGcCount = _totalGcCount,
            AverageGcTime = _totalGcCount > 0 ? _totalGcTime / _totalGcCount : 0f,
            PeakMemoryUsage = _peakMemoryUsage,
            CurrentMemoryUsage = GC.GetTotalMemory(false)
        };
    }
}

public struct GcStats
{
    public int TotalGcCount;
    public float AverageGcTime;
    public long PeakMemoryUsage;
    public long CurrentMemoryUsage;
}
```

---

## CPU 최적화

### 1. 멀티스레딩 및 Job System

```csharp
public class ParallelTaskManager
{
    private readonly TaskScheduler _scheduler;
    private readonly ConcurrentQueue<ITask> _taskQueue;
    private readonly SemaphoreSlim _semaphore;
    private readonly int _maxConcurrency;
    
    public ParallelTaskManager(int maxConcurrency = Environment.ProcessorCount)
    {
        _maxConcurrency = maxConcurrency;
        _taskQueue = new ConcurrentQueue<ITask>();
        _semaphore = new SemaphoreSlim(maxConcurrency, maxConcurrency);
        _scheduler = TaskScheduler.Default;
    }
    
    public async Task<T> ExecuteAsync<T>(Func<T> work, CancellationToken cancellationToken = default)
    {
        await _semaphore.WaitAsync(cancellationToken);
        
        try
        {
            return await Task.Factory.StartNew(work, cancellationToken, 
                TaskCreationOptions.None, _scheduler);
        }
        finally
        {
            _semaphore.Release();
        }
    }
    
    public Task ExecuteParallelAsync<T>(IEnumerable<T> items, Action<T> work, 
                                        CancellationToken cancellationToken = default)
    {
        var parallelOptions = new ParallelOptions
        {
            MaxDegreeOfParallelism = _maxConcurrency,
            CancellationToken = cancellationToken
        };
        
        return Task.Run(() => Parallel.ForEach(items, parallelOptions, work), cancellationToken);
    }
}

// Unity Jobs System 통합
public struct NetworkMessageProcessJob : IJobParallelFor
{
    [ReadOnly] public NativeArray<NetworkMessageData> Messages;
    public NativeArray<ProcessedMessage> Results;
    
    public void Execute(int index)
    {
        var message = Messages[index];
        
        // 메시지 처리 로직
        var processed = new ProcessedMessage
        {
            originalId = message.id,
            processedData = ProcessMessage(message),
            timestamp = UnityEngine.Time.time
        };
        
        Results[index] = processed;
    }
    
    private ProcessedMessageData ProcessMessage(NetworkMessageData message)
    {
        // 실제 메시지 처리 로직
        return new ProcessedMessageData { };
    }
}

public class JobSystemNetworkProcessor : MonoBehaviour
{
    private NativeArray<NetworkMessageData> _messageBuffer;
    private NativeArray<ProcessedMessage> _resultBuffer;
    private JobHandle _currentJob;
    
    void Start()
    {
        _messageBuffer = new NativeArray<NetworkMessageData>(1000, Allocator.Persistent);
        _resultBuffer = new NativeArray<ProcessedMessage>(1000, Allocator.Persistent);
    }
    
    void Update()
    {
        if (_currentJob.IsCompleted)
        {
            _currentJob.Complete();
            
            // 결과 처리
            ProcessResults();
            
            // 다음 배치 시작
            if (HasPendingMessages())
            {
                StartNextBatch();
            }
        }
    }
    
    private void StartNextBatch()
    {
        var job = new NetworkMessageProcessJob
        {
            Messages = _messageBuffer,
            Results = _resultBuffer
        };
        
        _currentJob = job.Schedule(_messageBuffer.Length, 64); // 64개씩 배치
    }
    
    void OnDestroy()
    {
        _currentJob.Complete();
        
        if (_messageBuffer.IsCreated)
            _messageBuffer.Dispose();
        if (_resultBuffer.IsCreated)
            _resultBuffer.Dispose();
    }
}
```

### 2. 고성능 알고리즘 최적화

```csharp
public static class OptimizedAlgorithms
{
    /// <summary>
    /// SIMD 최적화된 벡터 연산
    /// </summary>
    public static void OptimizedVectorAdd(float[] a, float[] b, float[] result)
    {
        if (Vector.IsHardwareAccelerated && a.Length >= Vector<float>.Count)
        {
            int simdLength = a.Length - (a.Length % Vector<float>.Count);
            
            // SIMD 연산
            for (int i = 0; i < simdLength; i += Vector<float>.Count)
            {
                var va = new Vector<float>(a, i);
                var vb = new Vector<float>(b, i);
                var vr = va + vb;
                vr.CopyTo(result, i);
            }
            
            // 나머지 요소들 처리
            for (int i = simdLength; i < a.Length; i++)
            {
                result[i] = a[i] + b[i];
            }
        }
        else
        {
            // 일반 연산
            for (int i = 0; i < a.Length; i++)
            {
                result[i] = a[i] + b[i];
            }
        }
    }
    
    /// <summary>
    /// 캐시 친화적인 행렬 곱셈
    /// </summary>
    public static unsafe void OptimizedMatrixMultiply(float* a, float* b, float* c, 
                                                     int n, int blockSize = 64)
    {
        for (int i = 0; i < n; i += blockSize)
        {
            for (int j = 0; j < n; j += blockSize)
            {
                for (int k = 0; k < n; k += blockSize)
                {
                    // 블록 단위 계산으로 캐시 미스 최소화
                    int maxI = Math.Min(i + blockSize, n);
                    int maxJ = Math.Min(j + blockSize, n);
                    int maxK = Math.Min(k + blockSize, n);
                    
                    for (int ii = i; ii < maxI; ii++)
                    {
                        for (int jj = j; jj < maxJ; jj++)
                        {
                            float sum = 0f;
                            for (int kk = k; kk < maxK; kk++)
                            {
                                sum += a[ii * n + kk] * b[kk * n + jj];
                            }
                            c[ii * n + jj] += sum;
                        }
                    }
                }
            }
        }
    }
    
    /// <summary>
    /// 빠른 역제곱근 (Quake III 알고리즘 개선)
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static unsafe float FastInverseSqrt(float x)
    {
        if (x == 0f) return float.PositiveInfinity;
        
        float threehalfs = 1.5f;
        float x2 = x * 0.5f;
        float y = x;
        
        // 비트 조작을 통한 초기 추정값
        int i = *(int*)&y;
        i = 0x5f3759df - (i >> 1);
        y = *(float*)&i;
        
        // 뉴턴-랩슨 법으로 정밀도 향상 (2회 반복)
        y = y * (threehalfs - (x2 * y * y));
        y = y * (threehalfs - (x2 * y * y));
        
        return y;
    }
    
    /// <summary>
    /// 분기 없는 최솟값/최댓값 (브랜치 미스예측 방지)
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static int BranchlessMin(int a, int b)
    {
        return a + ((b - a) & ((b - a) >> 31));
    }
    
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static int BranchlessMax(int a, int b)
    {
        return a - ((a - b) & ((a - b) >> 31));
    }
}
```

### 3. 프로파일링 및 성능 모니터링

```csharp
public class PerformanceProfiler : MonoBehaviour
{
    private readonly Dictionary<string, ProfileData> _profiles = new();
    private readonly Queue<FrameData> _frameHistory = new(60); // 60프레임 히스토리
    
    // 성능 메트릭
    public float AverageFPS { get; private set; }
    public float AverageFrameTime { get; private set; }
    public long CurrentMemoryUsage { get; private set; }
    public int GCCollections { get; private set; }
    
    void Update()
    {
        CollectFrameData();
        UpdateAverages();
    }
    
    public void BeginProfile(string name)
    {
        if (!_profiles.ContainsKey(name))
        {
            _profiles[name] = new ProfileData();
        }
        
        _profiles[name].StartTime = System.Diagnostics.Stopwatch.GetTimestamp();
    }
    
    public void EndProfile(string name)
    {
        if (_profiles.TryGetValue(name, out var data))
        {
            long elapsed = System.Diagnostics.Stopwatch.GetTimestamp() - data.StartTime;
            double elapsedMs = (double)elapsed / System.Diagnostics.Stopwatch.Frequency * 1000.0;
            
            data.TotalTime += elapsedMs;
            data.CallCount++;
            data.LastTime = elapsedMs;
            data.MinTime = Math.Min(data.MinTime, elapsedMs);
            data.MaxTime = Math.Max(data.MaxTime, elapsedMs);
        }
    }
    
    public ProfileData GetProfileData(string name)
    {
        return _profiles.TryGetValue(name, out var data) ? data : new ProfileData();
    }
    
    private void CollectFrameData()
    {
        var frameData = new FrameData
        {
            DeltaTime = Time.unscaledDeltaTime,
            MemoryUsage = GC.GetTotalMemory(false),
            GCCollections = GC.CollectionCount(0),
            Timestamp = Time.unscaledTime
        };
        
        _frameHistory.Enqueue(frameData);
        
        if (_frameHistory.Count > 60)
        {
            _frameHistory.Dequeue();
        }
    }
    
    private void UpdateAverages()
    {
        if (_frameHistory.Count == 0) return;
        
        float totalDeltaTime = 0f;
        foreach (var frame in _frameHistory)
        {
            totalDeltaTime += frame.DeltaTime;
        }
        
        AverageFrameTime = totalDeltaTime / _frameHistory.Count;
        AverageFPS = 1f / AverageFrameTime;
        
        var latestFrame = _frameHistory.Last();
        CurrentMemoryUsage = latestFrame.MemoryUsage;
        GCCollections = latestFrame.GCCollections;
    }
}

// 확장 메서드로 간편한 프로파일링
public static class ProfilerExtensions
{
    public static void ProfileAction(this PerformanceProfiler profiler, string name, Action action)
    {
        profiler.BeginProfile(name);
        try
        {
            action();
        }
        finally
        {
            profiler.EndProfile(name);
        }
    }
    
    public static async Task ProfileActionAsync(this PerformanceProfiler profiler, string name, Func<Task> action)
    {
        profiler.BeginProfile(name);
        try
        {
            await action();
        }
        finally
        {
            profiler.EndProfile(name);
        }
    }
}

public class ProfileData
{
    public long StartTime;
    public double TotalTime;
    public double LastTime;
    public double MinTime = double.MaxValue;
    public double MaxTime = double.MinValue;
    public int CallCount;
    
    public double AverageTime => CallCount > 0 ? TotalTime / CallCount : 0;
}

public struct FrameData
{
    public float DeltaTime;
    public long MemoryUsage;
    public int GCCollections;
    public float Timestamp;
}
```

---

## 네트워크 최적화

### 1. 메시지 압축 및 직렬화

```csharp
public static class NetworkOptimization
{
    // 메시지 압축
    public static class MessageCompression
    {
        public static byte[] CompressMessage(byte[] data)
        {
            if (data.Length < 100) // 작은 데이터는 압축하지 않음
                return data;
            
            using var output = new MemoryStream();
            using (var gzip = new GZipStream(output, CompressionLevel.Fastest))
            {
                gzip.Write(data, 0, data.Length);
            }
            
            byte[] compressed = output.ToArray();
            
            // 압축률이 좋지 않으면 원본 반환
            return compressed.Length < data.Length * 0.8f ? compressed : data;
        }
        
        public static byte[] DecompressMessage(byte[] compressedData, bool isCompressed)
        {
            if (!isCompressed) return compressedData;
            
            using var input = new MemoryStream(compressedData);
            using var gzip = new GZipStream(input, CompressionMode.Decompress);
            using var output = new MemoryStream();
            
            gzip.CopyTo(output);
            return output.ToArray();
        }
    }
    
    // 효율적인 직렬화
    public static class BinarySerializer
    {
        public static byte[] SerializePlayerState(PlayerState state)
        {
            using var stream = new MemoryStream(64); // 예상 크기로 초기화
            using var writer = new BinaryWriter(stream);
            
            // 비트 패킹으로 공간 절약
            byte flags = 0;
            flags |= (byte)(state.IsMoving ? 1 : 0);
            flags |= (byte)(state.IsJumping ? 2 : 0);
            flags |= (byte)(state.IsCrouching ? 4 : 0);
            
            writer.Write(flags);
            writer.Write((ushort)(state.Position.x * 100)); // 0.01 정밀도
            writer.Write((ushort)(state.Position.y * 100));
            writer.Write((ushort)(state.Position.z * 100));
            writer.Write((byte)(state.Rotation.y / 360f * 255)); // 360도를 8비트로
            writer.Write((byte)(state.Health / state.MaxHealth * 255)); // 체력을 퍼센트로
            
            return stream.ToArray();
        }
        
        public static PlayerState DeserializePlayerState(byte[] data)
        {
            using var stream = new MemoryStream(data);
            using var reader = new BinaryReader(stream);
            
            byte flags = reader.ReadByte();
            var position = new Vector3(
                reader.ReadUInt16() / 100f,
                reader.ReadUInt16() / 100f,
                reader.ReadUInt16() / 100f
            );
            
            float rotationY = reader.ReadByte() / 255f * 360f;
            float healthPercent = reader.ReadByte() / 255f;
            
            return new PlayerState
            {
                IsMoving = (flags & 1) != 0,
                IsJumping = (flags & 2) != 0,
                IsCrouching = (flags & 4) != 0,
                Position = position,
                Rotation = Quaternion.Euler(0, rotationY, 0),
                Health = healthPercent * 100f // 최대 체력이 100이라고 가정
            };
        }
    }
}

// 델타 압축 (변경된 부분만 전송)
public class DeltaCompression
{
    private readonly Dictionary<string, PlayerState> _lastStates = new();
    
    public byte[] CreateDelta(string playerId, PlayerState currentState)
    {
        if (!_lastStates.TryGetValue(playerId, out var lastState))
        {
            // 첫 번째 상태는 전체 전송
            _lastStates[playerId] = currentState;
            return SerializeFull(currentState);
        }
        
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream);
        
        byte deltaFlags = 0;
        writer.Write(deltaFlags); // 플래그 자리 예약
        
        int flagPosition = 0;
        
        // 위치 변경 체크
        if (Vector3.Distance(lastState.Position, currentState.Position) > 0.01f)
        {
            deltaFlags |= (byte)(1 << flagPosition);
            writer.Write(currentState.Position.x);
            writer.Write(currentState.Position.y);
            writer.Write(currentState.Position.z);
        }
        flagPosition++;
        
        // 회전 변경 체크
        if (Quaternion.Angle(lastState.Rotation, currentState.Rotation) > 1f)
        {
            deltaFlags |= (byte)(1 << flagPosition);
            writer.Write(currentState.Rotation.x);
            writer.Write(currentState.Rotation.y);
            writer.Write(currentState.Rotation.z);
            writer.Write(currentState.Rotation.w);
        }
        flagPosition++;
        
        // 체력 변경 체크
        if (Mathf.Abs(lastState.Health - currentState.Health) > 0.1f)
        {
            deltaFlags |= (byte)(1 << flagPosition);
            writer.Write(currentState.Health);
        }
        flagPosition++;
        
        // 플래그 업데이트
        var data = stream.ToArray();
        data[0] = deltaFlags;
        
        // 상태 업데이트
        _lastStates[playerId] = currentState;
        
        return data;
    }
    
    private byte[] SerializeFull(PlayerState state)
    {
        // 전체 상태 직렬화
        return NetworkOptimization.BinarySerializer.SerializePlayerState(state);
    }
}
```

### 2. 적응형 네트워크 품질 관리

```csharp
public class AdaptiveNetworkQuality
{
    private readonly Queue<NetworkMetric> _metrics = new(60);
    private NetworkQualityLevel _currentQuality = NetworkQualityLevel.High;
    
    // 임계값
    private const float HIGH_QUALITY_RTT = 50f;
    private const float MEDIUM_QUALITY_RTT = 150f;
    private const float HIGH_QUALITY_LOSS = 0.01f;
    private const float MEDIUM_QUALITY_LOSS = 0.05f;
    
    public NetworkQualityLevel CurrentQuality => _currentQuality;
    
    public void UpdateMetrics(float rtt, float packetLoss, float bandwidth)
    {
        var metric = new NetworkMetric
        {
            RTT = rtt,
            PacketLoss = packetLoss,
            Bandwidth = bandwidth,
            Timestamp = Time.time
        };
        
        _metrics.Enqueue(metric);
        
        if (_metrics.Count > 60)
        {
            _metrics.Dequeue();
        }
        
        // 품질 레벨 업데이트
        UpdateQualityLevel();
    }
    
    private void UpdateQualityLevel()
    {
        if (_metrics.Count < 10) return; // 최소 10개 샘플 필요
        
        // 최근 메트릭 평균 계산
        float avgRtt = 0f;
        float avgLoss = 0f;
        float avgBandwidth = 0f;
        
        foreach (var metric in _metrics)
        {
            avgRtt += metric.RTT;
            avgLoss += metric.PacketLoss;
            avgBandwidth += metric.Bandwidth;
        }
        
        avgRtt /= _metrics.Count;
        avgLoss /= _metrics.Count;
        avgBandwidth /= _metrics.Count;
        
        // 품질 레벨 결정
        NetworkQualityLevel newQuality;
        
        if (avgRtt <= HIGH_QUALITY_RTT && avgLoss <= HIGH_QUALITY_LOSS)
        {
            newQuality = NetworkQualityLevel.High;
        }
        else if (avgRtt <= MEDIUM_QUALITY_RTT && avgLoss <= MEDIUM_QUALITY_LOSS)
        {
            newQuality = NetworkQualityLevel.Medium;
        }
        else
        {
            newQuality = NetworkQualityLevel.Low;
        }
        
        if (newQuality != _currentQuality)
        {
            _currentQuality = newQuality;
            OnQualityChanged(newQuality);
        }
    }
    
    private void OnQualityChanged(NetworkQualityLevel newQuality)
    {
        Log.Info($"Network quality changed to: {newQuality}", "Network");
        
        // 품질에 따른 설정 조정
        switch (newQuality)
        {
            case NetworkQualityLevel.High:
                SetHighQualitySettings();
                break;
            case NetworkQualityLevel.Medium:
                SetMediumQualitySettings();
                break;
            case NetworkQualityLevel.Low:
                SetLowQualitySettings();
                break;
        }
    }
    
    private void SetHighQualitySettings()
    {
        // 높은 품질 설정
        NetworkUpdateRate = 60; // 60Hz
        CompressionLevel = CompressionLevel.NoCompression;
        PredictionEnabled = true;
    }
    
    private void SetMediumQualitySettings()
    {
        // 중간 품질 설정
        NetworkUpdateRate = 30; // 30Hz
        CompressionLevel = CompressionLevel.Fastest;
        PredictionEnabled = true;
    }
    
    private void SetLowQualitySettings()
    {
        // 낮은 품질 설정
        NetworkUpdateRate = 15; // 15Hz
        CompressionLevel = CompressionLevel.SmallestSize;
        PredictionEnabled = false;
    }
    
    public int NetworkUpdateRate { get; private set; } = 60;
    public CompressionLevel CompressionLevel { get; private set; } = CompressionLevel.Fastest;
    public bool PredictionEnabled { get; private set; } = true;
}

public enum NetworkQualityLevel
{
    Low,
    Medium,
    High
}

public struct NetworkMetric
{
    public float RTT;
    public float PacketLoss;
    public float Bandwidth;
    public float Timestamp;
}
```

### 3. 연결 풀링 및 재사용 최적화

```csharp
public class OptimizedConnectionPool : IDisposable
{
    private readonly ConcurrentQueue<INetworkConnection> _availableConnections;
    private readonly Dictionary<string, INetworkConnection> _activeConnections;
    private readonly NetworkConfig _config;
    private readonly object _lockObject = new();
    
    private volatile int _totalConnections;
    private volatile int _activeCount;
    
    public OptimizedConnectionPool(NetworkConfig config, int initialSize = 5)
    {
        _config = config;
        _availableConnections = new ConcurrentQueue<INetworkConnection>();
        _activeConnections = new Dictionary<string, INetworkConnection>();
        
        // 초기 연결 생성
        for (int i = 0; i < initialSize; i++)
        {
            var connection = CreateConnection();
            if (connection != null)
            {
                _availableConnections.Enqueue(connection);
                _totalConnections++;
            }
        }
    }
    
    public async Task<INetworkConnection> GetConnectionAsync(string endpoint)
    {
        // 기존 활성 연결 확인
        lock (_lockObject)
        {
            if (_activeConnections.TryGetValue(endpoint, out var existingConnection))
            {
                if (existingConnection.IsConnected)
                {
                    return existingConnection;
                }
                else
                {
                    _activeConnections.Remove(endpoint);
                }
            }
        }
        
        // 풀에서 연결 가져오기
        INetworkConnection connection = null;
        
        while (_availableConnections.TryDequeue(out connection))
        {
            if (connection.IsConnected)
            {
                break;
            }
            else
            {
                connection.Dispose();
                Interlocked.Decrement(ref _totalConnections);
                connection = null;
            }
        }
        
        // 사용 가능한 연결이 없으면 새로 생성
        if (connection == null)
        {
            connection = CreateConnection();
            if (connection != null)
            {
                Interlocked.Increment(ref _totalConnections);
            }
        }
        
        if (connection != null)
        {
            // 연결 설정
            await connection.ConnectAsync(endpoint);
            
            lock (_lockObject)
            {
                _activeConnections[endpoint] = connection;
                _activeCount = _activeConnections.Count;
            }
        }
        
        return connection;
    }
    
    public void ReturnConnection(string endpoint)
    {
        lock (_lockObject)
        {
            if (_activeConnections.TryGetValue(endpoint, out var connection))
            {
                _activeConnections.Remove(endpoint);
                _activeCount = _activeConnections.Count;
                
                if (connection.IsConnected && _availableConnections.Count < 10)
                {
                    _availableConnections.Enqueue(connection);
                }
                else
                {
                    connection.Dispose();
                    Interlocked.Decrement(ref _totalConnections);
                }
            }
        }
    }
    
    private INetworkConnection CreateConnection()
    {
        // 팩토리 패턴으로 연결 생성
        return new OptimizedQuicConnection(_config);
    }
    
    public ConnectionPoolStats GetStats()
    {
        return new ConnectionPoolStats
        {
            TotalConnections = _totalConnections,
            ActiveConnections = _activeCount,
            AvailableConnections = _availableConnections.Count
        };
    }
    
    public void Dispose()
    {
        // 모든 연결 정리
        while (_availableConnections.TryDequeue(out var connection))
        {
            connection.Dispose();
        }
        
        lock (_lockObject)
        {
            foreach (var connection in _activeConnections.Values)
            {
                connection.Dispose();
            }
            _activeConnections.Clear();
        }
    }
}

public struct ConnectionPoolStats
{
    public int TotalConnections;
    public int ActiveConnections;
    public int AvailableConnections;
}
```

---

## GPU 최적화

### 1. 드로우 콜 최적화

```csharp
public class DrawCallOptimizer : MonoBehaviour
{
    [Header("Batching Settings")]
    [SerializeField] private bool enableDynamicBatching = true;
    [SerializeField] private bool enableStaticBatching = true;
    [SerializeField] private int maxBatchSize = 64000; // 64k vertices
    
    private readonly List<MeshRenderer> _renderers = new();
    private readonly Dictionary<Material, List<MeshRenderer>> _materialGroups = new();
    
    void Start()
    {
        OptimizeScene();
    }
    
    public void OptimizeScene()
    {
        // 씬의 모든 렌더러 수집
        CollectRenderers();
        
        // 머티리얼별로 그룹화
        GroupByMaterial();
        
        // 배칭 최적화
        if (enableStaticBatching)
        {
            OptimizeStaticBatching();
        }
        
        if (enableDynamicBatching)
        {
            OptimizeDynamicBatching();
        }
        
        // GPU 인스턴싱 설정
        SetupGPUInstancing();
    }
    
    private void CollectRenderers()
    {
        _renderers.Clear();
        _renderers.AddRange(FindObjectsOfType<MeshRenderer>());
        
        Log.Info($"Found {_renderers.Count} renderers in scene", "GPU");
    }
    
    private void GroupByMaterial()
    {
        _materialGroups.Clear();
        
        foreach (var renderer in _renderers)
        {
            var material = renderer.sharedMaterial;
            if (material != null)
            {
                if (!_materialGroups.ContainsKey(material))
                {
                    _materialGroups[material] = new List<MeshRenderer>();
                }
                
                _materialGroups[material].Add(renderer);
            }
        }
        
        Log.Info($"Materials grouped: {_materialGroups.Count} unique materials", "GPU");
    }
    
    private void OptimizeStaticBatching()
    {
        foreach (var group in _materialGroups)
        {
            var staticRenderers = group.Value.Where(r => !r.gameObject.transform.hasChanged).ToArray();
            
            if (staticRenderers.Length > 1)
            {
                // Unity의 StaticBatchingUtility 사용
                var gameObjects = staticRenderers.Select(r => r.gameObject).ToArray();
                StaticBatchingUtility.Combine(gameObjects, gameObjects[0]);
                
                Log.Debug($"Static batched {staticRenderers.Length} objects with material {group.Key.name}", "GPU");
            }
        }
    }
    
    private void OptimizeDynamicBatching()
    {
        // 동적 배칭을 위한 최적화 설정
        foreach (var group in _materialGroups)
        {
            foreach (var renderer in group.Value)
            {
                // 버텍스 수 확인 (300 이하일 때만 동적 배칭 효과적)
                var mesh = renderer.GetComponent<MeshFilter>()?.sharedMesh;
                if (mesh != null && mesh.vertexCount <= 300)
                {
                    // 동적 배칭에 적합한 오브젝트 표시
                    renderer.gameObject.name += "_DynamicBatch";
                }
            }
        }
    }
    
    private void SetupGPUInstancing()
    {
        // GPU 인스턴싱이 가능한 오브젝트들 설정
        foreach (var group in _materialGroups)
        {
            var material = group.Key;
            
            // 머티리얼이 GPU 인스턴싱을 지원하는지 확인
            if (material.enableInstancing)
            {
                var renderers = group.Value.Where(r => 
                    r.GetComponent<MeshFilter>()?.sharedMesh != null).ToList();
                
                if (renderers.Count > 1)
                {
                    SetupInstancedRendering(renderers, material);
                }
            }
        }
    }
    
    private void SetupInstancedRendering(List<MeshRenderer> renderers, Material material)
    {
        // GPU 인스턴싱 설정
        var instances = new List<Matrix4x4>();
        var colors = new List<Vector4>();
        
        foreach (var renderer in renderers)
        {
            instances.Add(renderer.transform.localToWorldMatrix);
            colors.Add(renderer.material.color); // 개별 색상 지원
        }
        
        // 인스턴싱 렌더링 컴포넌트 추가
        var parent = new GameObject($"InstancedGroup_{material.name}");
        var instancer = parent.AddComponent<GPUInstancer>();
        instancer.Setup(material, renderers[0].GetComponent<MeshFilter>().sharedMesh, 
                       instances.ToArray(), colors.ToArray());
        
        // 원본 렌더러들 비활성화
        foreach (var renderer in renderers)
        {
            renderer.enabled = false;
        }
        
        Log.Info($"GPU instancing setup for {renderers.Count} objects with material {material.name}", "GPU");
    }
}

public class GPUInstancer : MonoBehaviour
{
    private Material _material;
    private Mesh _mesh;
    private Matrix4x4[] _matrices;
    private Vector4[] _colors;
    
    private ComputeBuffer _matrixBuffer;
    private ComputeBuffer _colorBuffer;
    private MaterialPropertyBlock _propertyBlock;
    
    public void Setup(Material material, Mesh mesh, Matrix4x4[] matrices, Vector4[] colors)
    {
        _material = material;
        _mesh = mesh;
        _matrices = matrices;
        _colors = colors;
        
        InitializeBuffers();
    }
    
    private void InitializeBuffers()
    {
        _matrixBuffer = new ComputeBuffer(_matrices.Length, sizeof(float) * 16);
        _colorBuffer = new ComputeBuffer(_colors.Length, sizeof(float) * 4);
        
        _matrixBuffer.SetData(_matrices);
        _colorBuffer.SetData(_colors);
        
        _propertyBlock = new MaterialPropertyBlock();
        _propertyBlock.SetBuffer("_MatrixBuffer", _matrixBuffer);
        _propertyBlock.SetBuffer("_ColorBuffer", _colorBuffer);
    }
    
    void Update()
    {
        if (_material != null && _mesh != null)
        {
            Graphics.DrawMeshInstancedIndirect(_mesh, 0, _material, 
                new Bounds(Vector3.zero, Vector3.one * 1000f),
                _matrixBuffer, 0, _propertyBlock);
        }
    }
    
    void OnDestroy()
    {
        _matrixBuffer?.Release();
        _colorBuffer?.Release();
    }
}
```

### 2. 텍스처 아틀라스 및 압축

```csharp
public class TextureOptimizer
{
    public static void OptimizeTextures()
    {
        var textures = Resources.FindObjectsOfTypeAll<Texture2D>();
        
        foreach (var texture in textures)
        {
            OptimizeIndividualTexture(texture);
        }
        
        // 텍스처 아틀라스 생성
        CreateTextureAtlases();
    }
    
    private static void OptimizeIndividualTexture(Texture2D texture)
    {
        var path = AssetDatabase.GetAssetPath(texture);
        if (string.IsNullOrEmpty(path)) return;
        
        var importer = AssetImporter.GetAtPath(path) as TextureImporter;
        if (importer == null) return;
        
        // 플랫폼별 최적화
        OptimizeForPlatform(importer, BuildTarget.Android);
        OptimizeForPlatform(importer, BuildTarget.iOS);
        OptimizeForPlatform(importer, BuildTarget.StandaloneWindows64);
        
        AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate);
    }
    
    private static void OptimizeForPlatform(TextureImporter importer, BuildTarget platform)
    {
        var settings = new TextureImporterPlatformSettings
        {
            name = platform.ToString(),
            overridden = true
        };
        
        // 플랫폼별 압축 형식 설정
        switch (platform)
        {
            case BuildTarget.Android:
                settings.format = TextureImporterFormat.ASTC_6x6;
                break;
            case BuildTarget.iOS:
                settings.format = TextureImporterFormat.ASTC_6x6;
                break;
            case BuildTarget.StandaloneWindows64:
                settings.format = TextureImporterFormat.DXT5;
                break;
        }
        
        // 최대 텍스처 크기 제한
        settings.maxTextureSize = DetermineOptimalSize(importer.assetPath);
        
        importer.SetPlatformTextureSettings(settings);
    }
    
    private static int DetermineOptimalSize(string texturePath)
    {
        // 텍스처 용도에 따른 최적 크기 결정
        if (texturePath.Contains("UI"))
        {
            return 512; // UI 텍스처는 512x512 최대
        }
        else if (texturePath.Contains("Character"))
        {
            return 1024; // 캐릭터 텍스처는 1024x1024 최대
        }
        else if (texturePath.Contains("Environment"))
        {
            return 2048; // 환경 텍스처는 2048x2048 최대
        }
        
        return 1024; // 기본값
    }
    
    private static void CreateTextureAtlases()
    {
        // UI 텍스처 아틀라스 생성
        CreateUIAtlas();
        
        // 캐릭터 텍스처 아틀라스 생성
        CreateCharacterAtlas();
    }
    
    private static void CreateUIAtlas()
    {
        var uiTextures = Resources.FindObjectsOfTypeAll<Texture2D>()
            .Where(t => AssetDatabase.GetAssetPath(t).Contains("UI"))
            .ToArray();
        
        if (uiTextures.Length > 1)
        {
            var atlas = new Texture2D(2048, 2048);
            var rects = atlas.PackTextures(uiTextures, 2, 2048);
            
            // 아틀라스 저장
            var bytes = atlas.EncodeToPNG();
            File.WriteAllBytes("Assets/Textures/UI_Atlas.png", bytes);
            
            Log.Info($"UI Atlas created with {uiTextures.Length} textures", "GPU");
        }
    }
    
    private static void CreateCharacterAtlas()
    {
        // 캐릭터 텍스처 아틀라스 생성 로직
        // UI와 유사한 방식으로 구현
    }
}

// 런타임 텍스처 스트리밍
public class TextureStreaming : MonoBehaviour
{
    private readonly Dictionary<string, Texture2D> _textureCache = new();
    private readonly Queue<TextureRequest> _loadQueue = new();
    
    public async Task<Texture2D> LoadTextureAsync(string path, int quality = 1)
    {
        // 캐시에서 확인
        if (_textureCache.TryGetValue(path, out var cachedTexture))
        {
            return cachedTexture;
        }
        
        // 비동기 로딩
        var request = new TextureRequest { Path = path, Quality = quality };
        _loadQueue.Enqueue(request);
        
        var texture = await LoadTextureFromDisk(path, quality);
        
        if (texture != null)
        {
            _textureCache[path] = texture;
        }
        
        return texture;
    }
    
    private async Task<Texture2D> LoadTextureFromDisk(string path, int quality)
    {
        // 파일 시스템에서 비동기 로딩
        byte[] data = await File.ReadAllBytesAsync(path);
        
        var texture = new Texture2D(2, 2);
        if (texture.LoadImage(data))
        {
            // 품질에 따른 리사이징
            if (quality < 1)
            {
                texture = ResizeTexture(texture, quality);
            }
            
            return texture;
        }
        
        return null;
    }
    
    private Texture2D ResizeTexture(Texture2D source, float scale)
    {
        int newWidth = Mathf.RoundToInt(source.width * scale);
        int newHeight = Mathf.RoundToInt(source.height * scale);
        
        var resized = new Texture2D(newWidth, newHeight);
        
        // 바이큐빅 보간법 사용
        for (int y = 0; y < newHeight; y++)
        {
            for (int x = 0; x < newWidth; x++)
            {
                float sourceX = (float)x / newWidth * source.width;
                float sourceY = (float)y / newHeight * source.height;
                
                var color = source.GetPixelBilinear(sourceX / source.width, sourceY / source.height);
                resized.SetPixel(x, y, color);
            }
        }
        
        resized.Apply();
        return resized;
    }
}

public struct TextureRequest
{
    public string Path;
    public int Quality;
}
```

---

## Unity 특화 최적화

### 1. 컴포넌트 최적화

```csharp
// 효율적인 컴포넌트 캐싱
public class OptimizedComponent : MonoBehaviour
{
    // 컴포넌트 캐시
    private Transform _cachedTransform;
    private Rigidbody _cachedRigidbody;
    private Collider _cachedCollider;
    
    // 프로퍼티를 통한 지연 초기화
    public Transform CachedTransform 
    {
        get
        {
            if (_cachedTransform == null)
                _cachedTransform = transform;
            return _cachedTransform;
        }
    }
    
    public Rigidbody CachedRigidbody
    {
        get
        {
            if (_cachedRigidbody == null)
                _cachedRigidbody = GetComponent<Rigidbody>();
            return _cachedRigidbody;
        }
    }
    
    // 컴포넌트 존재 여부 캐시
    private bool? _hasRigidbody;
    public bool HasRigidbody
    {
        get
        {
            if (!_hasRigidbody.HasValue)
                _hasRigidbody = GetComponent<Rigidbody>() != null;
            return _hasRigidbody.Value;
        }
    }
}

// 오브젝트 풀링과 통합된 컴포넌트
public class PoolableGameObject : MonoBehaviour, IPoolable
{
    private readonly List<IPoolableComponent> _poolableComponents = new();
    private bool _isInPool;
    
    void Awake()
    {
        // 풀링 가능한 컴포넌트들 수집
        _poolableComponents.AddRange(GetComponentsInChildren<IPoolableComponent>());
    }
    
    public void OnGetFromPool()
    {
        _isInPool = false;
        gameObject.SetActive(true);
        
        foreach (var component in _poolableComponents)
        {
            component.OnGetFromPool();
        }
    }
    
    public void OnReturnToPool()
    {
        _isInPool = true;
        gameObject.SetActive(false);
        
        foreach (var component in _poolableComponents)
        {
            component.OnReturnToPool();
        }
    }
    
    void OnDestroy()
    {
        if (!_isInPool)
        {
            // 풀에서 관리되지 않는 경우에만 정리
            foreach (var component in _poolableComponents)
            {
                component.OnReturnToPool();
            }
        }
    }
}

public interface IPoolableComponent
{
    void OnGetFromPool();
    void OnReturnToPool();
}
```

### 2. Update 최적화

```csharp
// 프레임별 업데이트 분산
public class UpdateManager : MonoBehaviour
{
    private readonly Dictionary<int, List<IUpdatable>> _updateGroups = new();
    private int _currentFrame;
    
    public void RegisterUpdatable(IUpdatable updatable, int frameInterval = 1)
    {
        if (!_updateGroups.ContainsKey(frameInterval))
        {
            _updateGroups[frameInterval] = new List<IUpdatable>();
        }
        
        _updateGroups[frameInterval].Add(updatable);
    }
    
    public void UnregisterUpdatable(IUpdatable updatable)
    {
        foreach (var group in _updateGroups.Values)
        {
            group.Remove(updatable);
        }
    }
    
    void Update()
    {
        _currentFrame++;
        
        // 프레임 간격별로 업데이트 실행
        foreach (var kvp in _updateGroups)
        {
            int frameInterval = kvp.Key;
            var updatables = kvp.Value;
            
            if (_currentFrame % frameInterval == 0)
            {
                for (int i = updatables.Count - 1; i >= 0; i--)
                {
                    var updatable = updatables[i];
                    if (updatable != null && updatable.IsValid)
                    {
                        updatable.OnUpdate();
                    }
                    else
                    {
                        updatables.RemoveAt(i);
                    }
                }
            }
        }
    }
}

public interface IUpdatable
{
    bool IsValid { get; }
    void OnUpdate();
}

// 효율적인 컴포넌트 업데이트
public class EfficientBehaviour : MonoBehaviour, IUpdatable
{
    [SerializeField] private int updateInterval = 1;
    [SerializeField] private bool useUpdateManager = true;
    
    public bool IsValid => this != null && gameObject.activeInHierarchy;
    
    void Start()
    {
        if (useUpdateManager)
        {
            var updateManager = FindObjectOfType<UpdateManager>();
            updateManager?.RegisterUpdatable(this, updateInterval);
        }
    }
    
    void Update()
    {
        if (!useUpdateManager)
        {
            OnUpdate();
        }
    }
    
    public virtual void OnUpdate()
    {
        // 실제 업데이트 로직
    }
    
    void OnDestroy()
    {
        if (useUpdateManager)
        {
            var updateManager = FindObjectOfType<UpdateManager>();
            updateManager?.UnregisterUpdatable(this);
        }
    }
}
```

### 3. 씬 관리 최적화

```csharp
public class OptimizedSceneManager : MonoBehaviour
{
    private readonly Dictionary<string, Scene> _loadedScenes = new();
    private readonly Queue<SceneOperation> _sceneOperationQueue = new();
    
    public async Task<Scene> LoadSceneAsync(string sceneName, LoadSceneMode mode = LoadSceneMode.Single)
    {
        // 이미 로드된 씬 확인
        if (_loadedScenes.TryGetValue(sceneName, out var existingScene) && existingScene.isLoaded)
        {
            return existingScene;
        }
        
        // 씬 로딩 작업을 큐에 추가
        var operation = new SceneOperation
        {
            SceneName = sceneName,
            Mode = mode,
            OperationType = SceneOperationType.Load,
            CompletionSource = new TaskCompletionSource<Scene>()
        };
        
        _sceneOperationQueue.Enqueue(operation);
        ProcessSceneOperations();
        
        return await operation.CompletionSource.Task;
    }
    
    private async void ProcessSceneOperations()
    {
        if (_sceneOperationQueue.Count == 0) return;
        
        var operation = _sceneOperationQueue.Dequeue();
        
        try
        {
            AsyncOperation asyncOp = null;
            
            switch (operation.OperationType)
            {
                case SceneOperationType.Load:
                    asyncOp = SceneManager.LoadSceneAsync(operation.SceneName, operation.Mode);
                    break;
                case SceneOperationType.Unload:
                    asyncOp = SceneManager.UnloadSceneAsync(operation.SceneName);
                    break;
            }
            
            if (asyncOp != null)
            {
                // 로딩 진행률 모니터링
                while (!asyncOp.isDone)
                {
                    float progress = asyncOp.progress;
                    // UI 업데이트 등
                    await Task.Yield();
                }
                
                var scene = SceneManager.GetSceneByName(operation.SceneName);
                _loadedScenes[operation.SceneName] = scene;
                operation.CompletionSource.SetResult(scene);
            }
        }
        catch (Exception ex)
        {
            operation.CompletionSource.SetException(ex);
        }
    }
}

public enum SceneOperationType
{
    Load,
    Unload
}

public struct SceneOperation
{
    public string SceneName;
    public LoadSceneMode Mode;
    public SceneOperationType OperationType;
    public TaskCompletionSource<Scene> CompletionSource;
}
```

---

## 모바일 최적화

### 1. 배터리 최적화

```csharp
public class BatteryOptimizer : MonoBehaviour
{
    [Header("Power Management")]
    [SerializeField] private float batteryCheckInterval = 5f;
    [SerializeField] private float lowBatteryThreshold = 0.2f; // 20%
    [SerializeField] private float criticalBatteryThreshold = 0.1f; // 10%
    
    private PowerMode _currentPowerMode = PowerMode.Normal;
    private float _lastBatteryCheck;
    
    void Update()
    {
        if (Application.isMobilePlatform && Time.time - _lastBatteryCheck > batteryCheckInterval)
        {
            CheckBatteryStatus();
            _lastBatteryCheck = Time.time;
        }
    }
    
    private void CheckBatteryStatus()
    {
        float batteryLevel = SystemInfo.batteryLevel;
        BatteryStatus batteryStatus = SystemInfo.batteryStatus;
        
        PowerMode newMode = DeterminePowerMode(batteryLevel, batteryStatus);
        
        if (newMode != _currentPowerMode)
        {
            ChangePowerMode(newMode);
        }
    }
    
    private PowerMode DeterminePowerMode(float batteryLevel, BatteryStatus batteryStatus)
    {
        if (batteryStatus == BatteryStatus.Charging)
        {
            return PowerMode.Normal;
        }
        
        if (batteryLevel < criticalBatteryThreshold)
        {
            return PowerMode.UltraLowPower;
        }
        else if (batteryLevel < lowBatteryThreshold)
        {
            return PowerMode.LowPower;
        }
        
        return PowerMode.Normal;
    }
    
    private void ChangePowerMode(PowerMode newMode)
    {
        _currentPowerMode = newMode;
        
        switch (newMode)
        {
            case PowerMode.Normal:
                SetNormalPowerSettings();
                break;
            case PowerMode.LowPower:
                SetLowPowerSettings();
                break;
            case PowerMode.UltraLowPower:
                SetUltraLowPowerSettings();
                break;
        }
        
        Log.Info($"Power mode changed to: {newMode}", "Battery");
    }
    
    private void SetNormalPowerSettings()
    {
        Application.targetFrameRate = 60;
        QualitySettings.vSyncCount = 1;
        
        // 정상 품질 설정
        SetGraphicsQuality(QualityLevel.High);
        SetNetworkQuality(NetworkQualityLevel.High);
    }
    
    private void SetLowPowerSettings()
    {
        Application.targetFrameRate = 30;
        QualitySettings.vSyncCount = 0;
        
        // 중간 품질 설정
        SetGraphicsQuality(QualityLevel.Medium);
        SetNetworkQuality(NetworkQualityLevel.Medium);
        
        // 백그라운드 작업 줄이기
        ReduceBackgroundTasks();
    }
    
    private void SetUltraLowPowerSettings()
    {
        Application.targetFrameRate = 15;
        QualitySettings.vSyncCount = 0;
        
        // 최저 품질 설정
        SetGraphicsQuality(QualityLevel.Low);
        SetNetworkQuality(NetworkQualityLevel.Low);
        
        // 최대한 절전 모드
        MinimizeAllOperations();
    }
    
    private void SetGraphicsQuality(QualityLevel level)
    {
        switch (level)
        {
            case QualityLevel.High:
                QualitySettings.antiAliasing = 4;
                QualitySettings.anisotropicFiltering = AnisotropicFiltering.ForceEnable;
                break;
            case QualityLevel.Medium:
                QualitySettings.antiAliasing = 2;
                QualitySettings.anisotropicFiltering = AnisotropicFiltering.Enable;
                break;
            case QualityLevel.Low:
                QualitySettings.antiAliasing = 0;
                QualitySettings.anisotropicFiltering = AnisotropicFiltering.Disable;
                break;
        }
    }
    
    private void ReduceBackgroundTasks()
    {
        // 불필요한 업데이트 중단
        // 네트워크 폴링 간격 증가
        // 애니메이션 품질 낮추기
    }
    
    private void MinimizeAllOperations()
    {
        // 최소한의 기능만 유지
        // 네트워크 연결 최소화
        // UI 업데이트 줄이기
    }
}

public enum PowerMode
{
    Normal,
    LowPower,
    UltraLowPower
}

public enum QualityLevel
{
    Low,
    Medium,
    High
}
```

### 2. 메모리 압력 관리

```csharp
public class MobileMemoryManager : MonoBehaviour
{
    [Header("Memory Management")]
    [SerializeField] private long warningThreshold = 100 * 1024 * 1024; // 100MB
    [SerializeField] private long criticalThreshold = 150 * 1024 * 1024; // 150MB
    [SerializeField] private float memoryCheckInterval = 2f;
    
    private float _lastMemoryCheck;
    private readonly Queue<MemorySnapshot> _memoryHistory = new(10);
    
    void Update()
    {
        if (Time.time - _lastMemoryCheck > memoryCheckInterval)
        {
            CheckMemoryPressure();
            _lastMemoryCheck = Time.time;
        }
    }
    
    private void CheckMemoryPressure()
    {
        long currentMemory = GC.GetTotalMemory(false);
        
        var snapshot = new MemorySnapshot
        {
            TotalMemory = currentMemory,
            Timestamp = Time.time,
            FrameCount = Time.frameCount
        };
        
        _memoryHistory.Enqueue(snapshot);
        
        if (_memoryHistory.Count > 10)
        {
            _memoryHistory.Dequeue();
        }
        
        // 메모리 압력 레벨 결정
        MemoryPressureLevel pressureLevel = DetermineMemoryPressure(currentMemory);
        HandleMemoryPressure(pressureLevel);
    }
    
    private MemoryPressureLevel DetermineMemoryPressure(long currentMemory)
    {
        if (currentMemory > criticalThreshold)
        {
            return MemoryPressureLevel.Critical;
        }
        else if (currentMemory > warningThreshold)
        {
            return MemoryPressureLevel.High;
        }
        
        // 메모리 증가 추세 분석
        if (_memoryHistory.Count >= 5)
        {
            var trend = AnalyzeMemoryTrend();
            if (trend > 0.1f) // 10% 이상 증가
            {
                return MemoryPressureLevel.Medium;
            }
        }
        
        return MemoryPressureLevel.Normal;
    }
    
    private float AnalyzeMemoryTrend()
    {
        var snapshots = _memoryHistory.ToArray();
        if (snapshots.Length < 2) return 0f;
        
        long oldestMemory = snapshots[0].TotalMemory;
        long newestMemory = snapshots[snapshots.Length - 1].TotalMemory;
        
        return (float)(newestMemory - oldestMemory) / oldestMemory;
    }
    
    private void HandleMemoryPressure(MemoryPressureLevel level)
    {
        switch (level)
        {
            case MemoryPressureLevel.Critical:
                HandleCriticalMemoryPressure();
                break;
            case MemoryPressureLevel.High:
                HandleHighMemoryPressure();
                break;
            case MemoryPressureLevel.Medium:
                HandleMediumMemoryPressure();
                break;
        }
    }
    
    private void HandleCriticalMemoryPressure()
    {
        Log.Warning("Critical memory pressure detected", "Memory");
        
        // 즉시 GC 실행
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        
        // 텍스처 품질 최대한 낮추기
        ReduceTextureQuality();
        
        // 불필요한 오브젝트 해제
        UnloadUnusedAssets();
        
        // 네트워크 버퍼 크기 줄이기
        ReduceNetworkBuffers();
    }
    
    private void HandleHighMemoryPressure()
    {
        Log.Info("High memory pressure detected", "Memory");
        
        // 부드러운 GC 실행
        GC.Collect(0, GCCollectionMode.Optimized);
        
        // 텍스처 압축 강화
        CompressTextures();
        
        // 오브젝트 풀 크기 줄이기
        ReduceObjectPools();
    }
    
    private void HandleMediumMemoryPressure()
    {
        Log.Debug("Medium memory pressure detected", "Memory");
        
        // 예방적 정리
        CleanupPreventively();
    }
    
    private void ReduceTextureQuality()
    {
        // 동적으로 텍스처 해상도 낮추기
        var textures = Resources.FindObjectsOfTypeAll<Texture2D>();
        foreach (var texture in textures)
        {
            if (texture.width > 512)
            {
                // 텍스처 다운샘플링
                DownsampleTexture(texture, 0.5f);
            }
        }
    }
    
    private void DownsampleTexture(Texture2D texture, float scale)
    {
        // 텍스처 리사이징 로직
        int newWidth = Mathf.RoundToInt(texture.width * scale);
        int newHeight = Mathf.RoundToInt(texture.height * scale);
        
        // Unity의 Graphics.ConvertTexture 사용 또는 직접 구현
    }
    
    private void UnloadUnusedAssets()
    {
        Resources.UnloadUnusedAssets();
    }
}

public enum MemoryPressureLevel
{
    Normal,
    Medium,
    High,
    Critical
}

public struct MemorySnapshot
{
    public long TotalMemory;
    public float Timestamp;
    public int FrameCount;
}
```

---

## 프로파일링 및 모니터링

### 1. 통합 성능 모니터

```csharp
public class PerformanceMonitor : MonoBehaviour
{
    [Header("Monitoring Settings")]
    [SerializeField] private bool enableProfiling = true;
    [SerializeField] private bool showDebugGUI = true;
    [SerializeField] private KeyCode toggleKey = KeyCode.F3;
    
    // 성능 메트릭
    private PerformanceMetrics _metrics = new();
    private readonly Queue<FrameTimingData> _frameTimings = new(120);
    
    // 프로파일러
    private readonly Dictionary<string, ProfilerData> _profilers = new();
    
    void Update()
    {
        if (enableProfiling)
        {
            CollectMetrics();
        }
        
        if (Input.GetKeyDown(toggleKey))
        {
            showDebugGUI = !showDebugGUI;
        }
    }
    
    void OnGUI()
    {
        if (showDebugGUI)
        {
            DrawPerformanceGUI();
        }
    }
    
    private void CollectMetrics()
    {
        // 프레임 타이밍 수집
        var frameTiming = new FrameTimingData
        {
            DeltaTime = Time.unscaledDeltaTime,
            FPS = 1f / Time.unscaledDeltaTime,
            MemoryUsage = GC.GetTotalMemory(false),
            Timestamp = Time.unscaledTime
        };
        
        _frameTimings.Enqueue(frameTiming);
        
        if (_frameTimings.Count > 120)
        {
            _frameTimings.Dequeue();
        }
        
        // 메트릭 업데이트
        UpdateMetrics();
    }
    
    private void UpdateMetrics()
    {
        if (_frameTimings.Count == 0) return;
        
        var frames = _frameTimings.ToArray();
        
        _metrics.AverageFPS = frames.Average(f => f.FPS);
        _metrics.MinFPS = frames.Min(f => f.FPS);
        _metrics.MaxFPS = frames.Max(f => f.FPS);
        
        _metrics.AverageFrameTime = frames.Average(f => f.DeltaTime) * 1000f; // ms
        _metrics.MaxFrameTime = frames.Max(f => f.DeltaTime) * 1000f;
        
        _metrics.CurrentMemoryMB = frames.Last().MemoryUsage / (1024f * 1024f);
        _metrics.PeakMemoryMB = frames.Max(f => f.MemoryUsage) / (1024f * 1024f);
        
        // GPU 메트릭 (Unity 2019.1+에서 사용 가능)
        #if UNITY_2019_1_OR_NEWER
        _metrics.GPUFrameTime = UnityEngine.Profiling.Profiler.GetCounter("GPU Frame Time")?.LastValue ?? 0f;
        _metrics.DrawCalls = UnityEngine.Profiling.Profiler.GetCounter("Draw Calls")?.LastValue ?? 0;
        _metrics.Vertices = UnityEngine.Profiling.Profiler.GetCounter("Vertices")?.LastValue ?? 0;
        #endif
    }
    
    private void DrawPerformanceGUI()
    {
        int yOffset = 10;
        int lineHeight = 20;
        
        GUI.Box(new Rect(10, 10, 300, 400), "Performance Monitor");
        
        // FPS 정보
        GUI.Label(new Rect(20, yOffset += lineHeight * 2, 280, lineHeight), 
                 $"FPS: {_metrics.AverageFPS:F1} (Min: {_metrics.MinFPS:F1}, Max: {_metrics.MaxFPS:F1})");
        
        // 프레임 타임
        GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), 
                 $"Frame Time: {_metrics.AverageFrameTime:F1}ms (Max: {_metrics.MaxFrameTime:F1}ms)");
        
        // 메모리 사용량
        GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), 
                 $"Memory: {_metrics.CurrentMemoryMB:F1}MB (Peak: {_metrics.PeakMemoryMB:F1}MB)");
        
        // GPU 메트릭
        GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), 
                 $"GPU Frame: {_metrics.GPUFrameTime:F1}ms");
        GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), 
                 $"Draw Calls: {_metrics.DrawCalls}");
        GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), 
                 $"Vertices: {_metrics.Vertices:N0}");
        
        // 프로파일러 데이터
        yOffset += lineHeight;
        GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), "Profiler Data:");
        
        foreach (var kvp in _profilers)
        {
            var data = kvp.Value;
            GUI.Label(new Rect(20, yOffset += lineHeight, 280, lineHeight), 
                     $"{kvp.Key}: {data.AverageTime:F2}ms ({data.CallCount} calls)");
        }
    }
    
    // 프로파일러 API
    public void BeginProfileBlock(string blockName)
    {
        if (!_profilers.ContainsKey(blockName))
        {
            _profilers[blockName] = new ProfilerData();
        }
        
        _profilers[blockName].StartTime = System.Diagnostics.Stopwatch.GetTimestamp();
    }
    
    public void EndProfileBlock(string blockName)
    {
        if (_profilers.TryGetValue(blockName, out var data))
        {
            long elapsed = System.Diagnostics.Stopwatch.GetTimestamp() - data.StartTime;
            double elapsedMs = (double)elapsed / System.Diagnostics.Stopwatch.Frequency * 1000.0;
            
            data.TotalTime += elapsedMs;
            data.CallCount++;
            data.LastTime = elapsedMs;
        }
    }
    
    public PerformanceMetrics GetMetrics() => _metrics;
}

public struct PerformanceMetrics
{
    public float AverageFPS;
    public float MinFPS;
    public float MaxFPS;
    public float AverageFrameTime;
    public float MaxFrameTime;
    public float CurrentMemoryMB;
    public float PeakMemoryMB;
    public float GPUFrameTime;
    public long DrawCalls;
    public long Vertices;
}

public struct FrameTimingData
{
    public float DeltaTime;
    public float FPS;
    public long MemoryUsage;
    public float Timestamp;
}

public class ProfilerData
{
    public long StartTime;
    public double TotalTime;
    public double LastTime;
    public int CallCount;
    
    public double AverageTime => CallCount > 0 ? TotalTime / CallCount : 0;
}
```

## 통합 사용 예제

모든 최적화 기법을 통합한 예제입니다.

```csharp
public class PoliceThiefPerformanceManager : MonoBehaviour
{
    [Header("Performance Components")]
    [SerializeField] private PerformanceMonitor performanceMonitor;
    [SerializeField] private BatteryOptimizer batteryOptimizer;
    [SerializeField] private MobileMemoryManager memoryManager;
    [SerializeField] private GarbageCollectionManager gcManager;
    
    void Awake()
    {
        InitializePerformanceSystem();
    }
    
    void Start()
    {
        ConfigureForPlatform();
        StartPerformanceMonitoring();
    }
    
    private void InitializePerformanceSystem()
    {
        // 플랫폼 감지 및 최적화 설정
        if (Application.isMobilePlatform)
        {
            InitializeMobileOptimizations();
        }
        else
        {
            InitializeDesktopOptimizations();
        }
        
        // 오브젝트 풀 초기화
        InitializeObjectPools();
        
        // 네트워크 최적화 초기화
        InitializeNetworkOptimizations();
    }
    
    private void InitializeMobileOptimizations()
    {
        Application.targetFrameRate = 30;
        QualitySettings.vSyncCount = 0;
        
        // 모바일 특화 설정
        if (batteryOptimizer == null)
        {
            batteryOptimizer = gameObject.AddComponent<BatteryOptimizer>();
        }
        
        if (memoryManager == null)
        {
            memoryManager = gameObject.AddComponent<MobileMemoryManager>();
        }
    }
    
    private void InitializeDesktopOptimizations()
    {
        Application.targetFrameRate = 60;
        QualitySettings.vSyncCount = 1;
        
        // 데스크톱 특화 설정
        QualitySettings.antiAliasing = 4;
        QualitySettings.anisotropicFiltering = AnisotropicFiltering.ForceEnable;
    }
    
    private void StartPerformanceMonitoring()
    {
        if (performanceMonitor == null)
        {
            performanceMonitor = gameObject.AddComponent<PerformanceMonitor>();
        }
        
        // 성능 경고 이벤트 구독
        InvokeRepeating(nameof(CheckPerformanceWarnings), 5f, 5f);
    }
    
    private void CheckPerformanceWarnings()
    {
        var metrics = performanceMonitor.GetMetrics();
        
        if (metrics.AverageFPS < 20f)
        {
            Log.Warning($"Low FPS detected: {metrics.AverageFPS:F1}", "Performance");
            HandleLowFPS();
        }
        
        if (metrics.CurrentMemoryMB > 200f)
        {
            Log.Warning($"High memory usage: {metrics.CurrentMemoryMB:F1}MB", "Performance");
            HandleHighMemoryUsage();
        }
    }
    
    private void HandleLowFPS()
    {
        // FPS 저하 시 대응
        QualitySettings.DecreaseLevel();
        Application.targetFrameRate = Mathf.Max(15, Application.targetFrameRate - 5);
    }
    
    private void HandleHighMemoryUsage()
    {
        // 메모리 사용량 증가 시 대응
        Resources.UnloadUnusedAssets();
        GC.Collect();
    }
}
```

## 다음 단계

Performance Optimization 가이드를 마스터했다면, 다음 문서를 참조하세요:

1. [Extension Guide](./06_Extension_Guide.md) - 확장 방안