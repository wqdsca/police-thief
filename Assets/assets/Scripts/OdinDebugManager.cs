using System;
using System.Collections.Generic;
using UnityEngine;
using Sirenix.OdinInspector;

namespace PoliceThief.Debugging
{
    /// <summary>
    /// Odin Inspector 디버깅 전용 매니저
    /// 게임 로직 없이 순수 디버깅 기능만 제공
    /// </summary>
    public class OdinDebugManager : MonoBehaviour
    {
        [Title("Debug Settings")]
        [BoxGroup("General")]
        [LabelText("Enable Debug Mode")]
        [ToggleLeft]
        public bool debugMode = true;

        [BoxGroup("General")]
        [ShowIf("debugMode")]
        [Range(0, 10)]
        [LabelText("Debug Level")]
        public int debugLevel = 5;

        [Title("Test Values")]
        [TabGroup("Values", "Primitives")]
        [LabelText("Test String")]
        public string testString = "Hello Odin";

        [TabGroup("Values", "Primitives")]
        [MinMaxSlider(0, 100)]
        public Vector2 rangeValues = new Vector2(20, 80);

        [TabGroup("Values", "Primitives")]
        [ProgressBar(0, 100, ColorGetter = "GetProgressColor")]
        public float progress = 50;

        [TabGroup("Values", "Collections")]
        [ListDrawerSettings(ShowIndexLabels = true)]
        public List<string> testList = new List<string> { "Item 1", "Item 2", "Item 3" };

        [TabGroup("Values", "Collections")]
        [DictionaryDrawerSettings(KeyLabel = "Key", ValueLabel = "Value")]
        public Dictionary<string, int> testDictionary = new Dictionary<string, int>
        {
            { "First", 1 },
            { "Second", 2 },
            { "Third", 3 }
        };

        [Title("Debug Actions")]
        [FoldoutGroup("Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.4f, 0.8f, 0.4f)]
        public void TestSuccess()
        {
            UnityEngine.Debug.Log($"[OdinDebugManager] Success Test - Debug Level: {debugLevel}");
            progress = Mathf.Min(100, progress + 10);
        }

        [FoldoutGroup("Actions")]
        [Button(ButtonSizes.Large)]
        [GUIColor(0.8f, 0.4f, 0.4f)]
        public void TestError()
        {
            UnityEngine.Debug.LogError("[OdinDebugManager] Error Test - This is a test error");
            progress = Mathf.Max(0, progress - 10);
        }

        [FoldoutGroup("Actions")]
        [Button(ButtonSizes.Medium)]
        [GUIColor(0.8f, 0.8f, 0.4f)]
        public void TestWarning()
        {
            UnityEngine.Debug.LogWarning("[OdinDebugManager] Warning Test - This is a test warning");
        }

        [Title("Inspector Info")]
        [InfoBox("This is a debug-only manager using Odin Inspector for testing and debugging purposes.", InfoMessageType.Info)]
        [PropertySpace(20)]
        [DisplayAsString]
        [LabelText("Current Time")]
        public string CurrentTime => DateTime.Now.ToString("HH:mm:ss");

        [DisplayAsString]
        [LabelText("Frame Count")]
        public int FrameCount => Time.frameCount;

        [DisplayAsString]
        [LabelText("FPS")]
        public float FPS => 1.0f / Time.deltaTime;

        [Title("Debug Visualization")]
        [HorizontalGroup("Split", 0.5f)]
        [VerticalGroup("Split/Left")]
        [PreviewField(100)]
        [HideLabel]
        public Texture2D testTexture;

        [VerticalGroup("Split/Right")]
        [Range(0, 360)]
        [OnValueChanged("UpdateRotation")]
        public float rotationAngle = 0;

        [VerticalGroup("Split/Right")]
        [ColorPalette]
        public Color debugColor = Color.cyan;

        [Title("Advanced Debug Features")]
        [Searchable]
        [ListDrawerSettings(ShowIndexLabels = true, ShowPaging = true, NumberOfItemsPerPage = 5)]
        public List<DebugEntry> debugEntries = new List<DebugEntry>();

        [Serializable]
        public class DebugEntry
        {
            [TableColumnWidth(60, Resizable = false)]
            [DisplayAsString]
            public int id;

            [TableColumnWidth(120)]
            public string name;

            [ProgressBar(0, 100)]
            [TableColumnWidth(100)]
            public float value;

            [TableColumnWidth(80)]
            public bool enabled;

            public DebugEntry(int id, string name, float value, bool enabled)
            {
                this.id = id;
                this.name = name;
                this.value = value;
                this.enabled = enabled;
            }
        }

        private void Awake()
        {
            UnityEngine.Debug.Log("[OdinDebugManager] Debug Manager Initialized");
            InitializeDebugEntries();
        }

        private void InitializeDebugEntries()
        {
            if (debugEntries.Count == 0)
            {
                for (int i = 0; i < 10; i++)
                {
                    debugEntries.Add(new DebugEntry(
                        i,
                        $"Debug Entry {i}",
                        UnityEngine.Random.Range(0f, 100f),
                        UnityEngine.Random.Range(0, 2) == 1
                    ));
                }
            }
        }

        private Color GetProgressColor()
        {
            if (progress < 30) return Color.red;
            if (progress < 70) return Color.yellow;
            return Color.green;
        }

        private void UpdateRotation()
        {
            transform.rotation = Quaternion.Euler(0, rotationAngle, 0);
        }

        [Title("Debug Console")]
        [TextArea(5, 10)]
        [ReadOnly]
        public string debugConsole = "Debug console output will appear here...";

        [ButtonGroup("Console")]
        public void ClearConsole()
        {
            debugConsole = "";
            UnityEngine.Debug.Log("[OdinDebugManager] Console Cleared");
        }

        [ButtonGroup("Console")]
        public void AddLog()
        {
            string log = $"[{DateTime.Now:HH:mm:ss}] Debug log entry\n";
            debugConsole += log;
            UnityEngine.Debug.Log("[OdinDebugManager] Added log entry");
        }

        [Title("Performance Monitoring")]
        [ShowInInspector]
        [ProgressBar(0, 60, Height = 20)]
        [LabelText("Target FPS")]
        private float TargetFPS => Application.targetFrameRate > 0 ? Application.targetFrameRate : 60;

        [ShowInInspector]
        [ReadOnly]
        [LabelText("Memory Usage (MB)")]
        private float MemoryUsage => System.GC.GetTotalMemory(false) / (1024f * 1024f);

        [ShowInInspector]
        [Button("Force Garbage Collection")]
        private void ForceGC()
        {
            System.GC.Collect();
            System.GC.WaitForPendingFinalizers();
            System.GC.Collect();
            UnityEngine.Debug.Log($"[OdinDebugManager] GC Forced. Memory: {MemoryUsage:F2} MB");
        }

        private void OnValidate()
        {
            if (Application.isPlaying)
            {
                UnityEngine.Debug.Log($"[OdinDebugManager] Values changed in Inspector - Debug Mode: {debugMode}, Level: {debugLevel}");
            }
        }
    }
}