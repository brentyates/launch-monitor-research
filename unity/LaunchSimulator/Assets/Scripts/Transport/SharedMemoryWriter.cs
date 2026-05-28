using UnityEngine;
using System;
using System.IO;
using System.IO.MemoryMappedFiles;
using System.Runtime.InteropServices;
using System.Threading;
using LaunchMonitor.Core;
using LaunchMonitor.Camera;

namespace LaunchMonitor.Transport
{
    public enum SharedMemoryState
    {
        Idle = 0,
        Ready = 1,
        Streaming = 2,
        Complete = 3
    }

    public enum RustCommand
    {
        None = 0,
        Launch = 1,
        Reset = 2,
        Calibrate = 3
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct GroundTruthData
    {
        public float speedMph;
        public float vlaDeg;
        public float hlaDeg;
        public float spinRpm;
        public float spinAxisDeg;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct LaunchCommand
    {
        public int command;
        public float speedMph;
        public float vlaDeg;
        public float hlaDeg;
        public float spinRpm;
        public float spinAxisDeg;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct SharedHeader
    {
        public uint magic;
        public int state;
        public int writeHead;
        public int frameCount;
        public int width;
        public int height;
        public float fps;
        public GroundTruthData groundTruth;
        public LaunchCommand rustCommand;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct FrameHeader
    {
        public int frameIndex;
        public double timestamp;
        public float ballPosX;
        public float ballPosY;
        public float ballPosZ;
        public float ballVelX;
        public float ballVelY;
        public float ballVelZ;
    }

    public class SharedMemoryWriter : MonoBehaviour
    {
        public static SharedMemoryWriter Instance { get; private set; }

        private static string _sharedMemoryPath;
        private static string SHARED_MEMORY_PATH
        {
            get
            {
                if (_sharedMemoryPath == null)
                    _sharedMemoryPath = GetSharedMemoryPath();
                return _sharedMemoryPath;
            }
        }
        private const uint MAGIC = 0x474F4C46;
        private const int RING_BUFFER_SIZE = 60;
        private const int HEADER_SIZE = 104;

        [Header("State")]
        public bool IsConnected { get; private set; }
        public SharedMemoryState CurrentState { get; private set; }

        private FileStream fileStream;
        private MemoryMappedFile mmf;
        private MemoryMappedViewAccessor accessor;
        private int frameSize;
        private int totalSize;

        public event Action OnConnected;
        public event Action OnDisconnected;
        public event Action<string> OnError;
        public event Action<LaunchCommand> OnLaunchCommand;

        void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }
            Instance = this;
        }

        void OnDestroy()
        {
            Disconnect();
        }

        private int pollCount;
        private bool loggedConnectionStatus;
        private int updateCount;

        void Update()
        {
            updateCount++;
            if (updateCount == 1 || updateCount == 60 || updateCount == 120)
            {
                Debug.Log($"SharedMemoryWriter.Update frame #{updateCount}, IsConnected={IsConnected}");
            }

            try
            {
                if (!loggedConnectionStatus && IsConnected)
                {
                    Debug.Log($"SharedMemoryWriter Update running, IsConnected={IsConnected}");
                    loggedConnectionStatus = true;
                }

                PollCommands();
            }
            catch (System.Exception e)
            {
                Debug.LogError($"SharedMemoryWriter.Update exception: {e}");
            }
        }

        private bool loggedFirstPoll;
        private int pollDebugCount;

        private const int OFFSET_MAGIC = 0;
        private const int OFFSET_STATE = 4;
        private const int OFFSET_WRITE_HEAD = 8;
        private const int OFFSET_FRAME_COUNT = 12;
        private const int OFFSET_WIDTH = 16;
        private const int OFFSET_HEIGHT = 20;
        private const int OFFSET_FPS = 24;
        private const int OFFSET_GROUND_TRUTH = 28;
        private const int OFFSET_RUST_COMMAND = 48;

        private void PollCommands()
        {
            if (!IsConnected) return;

            if (!loggedFirstPoll)
            {
                Debug.Log("PollCommands running for first time");
                loggedFirstPoll = true;
            }

            // Read just the command part first to check
            LaunchCommand cmd;
            accessor.Read(OFFSET_RUST_COMMAND, out cmd);

            pollDebugCount++;
            if (pollDebugCount <= 300)
            {
                Debug.Log($"Poll #{pollDebugCount}: rustCmd.command={cmd.command}, rustCmd.speed={cmd.speedMph}");
            }

            if (cmd.command != (int)RustCommand.None)
            {
                Debug.Log($"Received command from Rust: cmd={cmd.command}, speed={cmd.speedMph}");

                if (OnLaunchCommand != null)
                {
                    // Clear the command in shared memory immediately
                    var clearCmd = cmd; // Copy to keep data but clear command ID
                    clearCmd.command = (int)RustCommand.None;
                    
                    // Write ONLY the command struct back
                    accessor.Write(OFFSET_RUST_COMMAND, ref clearCmd);
                    accessor.Flush();

                    Debug.Log("Invoking OnLaunchCommand event");
                    OnLaunchCommand.Invoke(cmd);
                }
                else
                {
                    Debug.LogWarning("OnLaunchCommand event has no subscribers, keeping command in buffer");
                }
            }
        }

        public bool Connect(int width, int height)
        {
            try
            {
                int pixelSize = width * height * 4;
                int frameHeaderSize = Marshal.SizeOf<FrameHeader>();
                frameSize = frameHeaderSize + (pixelSize * 2);
                totalSize = HEADER_SIZE + (frameSize * RING_BUFFER_SIZE);

                fileStream = new FileStream(
                    SHARED_MEMORY_PATH,
                    FileMode.Create,
                    FileAccess.ReadWrite,
                    FileShare.ReadWrite);
                fileStream.SetLength(totalSize);

                mmf = MemoryMappedFile.CreateFromFile(
                    fileStream,
                    null,
                    totalSize,
                    MemoryMappedFileAccess.ReadWrite,
                    HandleInheritability.None,
                    false);
                accessor = mmf.CreateViewAccessor();

                // Initial full write is okay since we just created the file
                WriteHeader(new SharedHeader
                {
                    magic = MAGIC,
                    state = (int)SharedMemoryState.Streaming,
                    writeHead = 0,
                    frameCount = 0,
                    width = width,
                    height = height,
                    fps = TimeController.Instance?.TargetFps ?? 240f,
                    groundTruth = new GroundTruthData()
                });

                IsConnected = true;
                CurrentState = SharedMemoryState.Streaming;
                OnConnected?.Invoke();
                return true;
            }
            catch (Exception e)
            {
                OnError?.Invoke($"Failed to create shared memory: {e.Message}");
                return false;
            }
        }

        public void Disconnect()
        {
            if (accessor != null)
            {
                accessor.Dispose();
                accessor = null;
            }

            if (mmf != null)
            {
                mmf.Dispose();
                mmf = null;
            }

            if (fileStream != null)
            {
                fileStream.Dispose();
                fileStream = null;
            }

            IsConnected = false;
            OnDisconnected?.Invoke();
        }

        public void SetReady(LaunchParameters launchParams, int expectedFrameCount)
        {
            if (!IsConnected) return;

            int state = (int)SharedMemoryState.Ready;
            int writeHead = 0;
            float fps = TimeController.Instance?.TargetFps ?? 240f;
            var gt = new GroundTruthData
            {
                speedMph = launchParams.speedMph,
                vlaDeg = launchParams.vlaDeg,
                hlaDeg = launchParams.hlaDeg,
                spinRpm = launchParams.spinRpm,
                spinAxisDeg = launchParams.spinAxisDeg
            };

            // Write specific fields
            accessor.Write(OFFSET_STATE, ref state);
            accessor.Write(OFFSET_WRITE_HEAD, ref writeHead);
            accessor.Write(OFFSET_FRAME_COUNT, ref expectedFrameCount);
            accessor.Write(OFFSET_FPS, ref fps);
            accessor.Write(OFFSET_GROUND_TRUTH, ref gt);
            accessor.Flush();
            
            CurrentState = SharedMemoryState.Ready;
        }

        public void SetStreaming()
        {
            if (!IsConnected) return;

            int state = (int)SharedMemoryState.Streaming;
            accessor.Write(OFFSET_STATE, ref state);
            accessor.Flush();
            
            CurrentState = SharedMemoryState.Streaming;
        }

        public void SetComplete(int finalFrameCount)
        {
            if (!IsConnected) return;

            int state = (int)SharedMemoryState.Complete;
            accessor.Write(OFFSET_STATE, ref state);
            accessor.Write(OFFSET_FRAME_COUNT, ref finalFrameCount);
            accessor.Flush();
            
            CurrentState = SharedMemoryState.Complete;
        }

        public void SetIdle()
        {
            if (!IsConnected) return;

            int state = (int)SharedMemoryState.Idle;
            int writeHead = 0;
            accessor.Write(OFFSET_STATE, ref state);
            accessor.Write(OFFSET_WRITE_HEAD, ref writeHead);
            accessor.Flush();
            
            CurrentState = SharedMemoryState.Idle;
        }

        public void UpdateGroundTruth(LaunchParameters launchParams)
        {
            if (!IsConnected) return;

            var gt = new GroundTruthData
            {
                speedMph = launchParams.speedMph,
                vlaDeg = launchParams.vlaDeg,
                hlaDeg = launchParams.hlaDeg,
                spinRpm = launchParams.spinRpm,
                spinAxisDeg = launchParams.spinAxisDeg
            };
            accessor.Write(OFFSET_GROUND_TRUTH, ref gt);
            accessor.Flush();
        }

        public void WriteFrame(CapturedFrame frame)
        {
            if (!IsConnected) return;

            int slotIndex = frame.frameIndex % RING_BUFFER_SIZE;
            long offset = HEADER_SIZE + (slotIndex * frameSize);

            var frameHeader = new FrameHeader
            {
                frameIndex = frame.frameIndex,
                timestamp = frame.timestamp,
                ballPosX = frame.ballPosition.x,
                ballPosY = frame.ballPosition.y,
                ballPosZ = frame.ballPosition.z,
                ballVelX = frame.ballVelocity.x,
                ballVelY = frame.ballVelocity.y,
                ballVelZ = frame.ballVelocity.z
            };

            accessor.Write(offset, ref frameHeader);

            int headerSize = Marshal.SizeOf<FrameHeader>();
            int pixelSize = frame.cam0Pixels.Length;

            accessor.WriteArray(offset + headerSize, frame.cam0Pixels, 0, pixelSize);
            accessor.WriteArray(offset + headerSize + pixelSize, frame.cam1Pixels, 0, pixelSize);

            Thread.MemoryBarrier();

            // Only update write head
            int newHead = frame.frameIndex + 1;
            accessor.Write(OFFSET_WRITE_HEAD, ref newHead);
            accessor.Flush();
        }

        private SharedHeader ReadHeader()
        {
            SharedHeader header;
            accessor.Read(0, out header);
            return header;
        }

        private void WriteHeader(SharedHeader header)
        {
            accessor.Write(0, ref header);
            accessor.Flush();
        }

        public int GetRustReadHead()
        {
            if (!IsConnected) return -1;
            return 0;
        }

        private static string GetSharedMemoryPath()
        {
            string dataPath = Application.dataPath;
            string projectRoot = Path.GetFullPath(Path.Combine(dataPath, "..", "..", "..", ".."));
            string path = Path.Combine(projectRoot, "LaunchMonitorSharedMemory");
            Debug.Log($"SharedMemory path: {path}");
            return path;
        }
    }
}
