using System.Runtime.InteropServices;
using System.Diagnostics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace WinUiSpike;

public sealed partial class MainWindow : Window {
    [DllImport("rigdll.dll")] static extern IntPtr rig_create(int warp);
    [DllImport("rigdll.dll")] static extern IntPtr rig_swapchain(IntPtr h);
    [DllImport("rigdll.dll")] static extern long rig_present(IntPtr h, int verify);

    static readonly Guid IID_ISwapChainPanelNative = new("63aad0b8-7c24-40ff-85a8-640d944cc325");
    IntPtr rig;
    int frames; readonly List<double> ms = new();

    public MainWindow() {
        InitializeComponent();
        Display.Loaded += (_, _) => Start();
        // Watchdog: say where it stuck instead of hanging the runner.
        new Thread(() => { Thread.Sleep(45000); Log.Say("watchdog", "45 s without finishing"); Environment.Exit(4); }) { IsBackground = true }.Start();
    }

    unsafe void Start() {
        Log.Say("panel_loaded", $"{Display.ActualWidth}x{Display.ActualHeight}");
        rig = rig_create(1);
        if (rig == IntPtr.Zero) { Environment.Exit(2); }
        // ISwapChainPanelNative::SetSwapChain, vtable slot 3 (after IUnknown).
        var unk = Marshal.GetIUnknownForObject(Display);
        var hr = Marshal.QueryInterface(unk, in IID_ISwapChainPanelNative, out var native);
        Log.Say("query_ISwapChainPanelNative", $"hr=0x{hr:x8}");
        if (hr != 0) Environment.Exit(3);
        var sc = rig_swapchain(rig);
        var vtbl = *(IntPtr**)native;
        var setSwapChain = (delegate* unmanaged[Stdcall]<IntPtr, IntPtr, int>)vtbl[3];
        hr = setSwapChain(native, sc);
        Log.Say("SetSwapChain", $"hr=0x{hr:x8}");
        Log.Say("first_frame_pixel_mismatches", rig_present(rig, 1).ToString());
        CompositionTarget.Rendering += (_, _) => {
            var t = Stopwatch.StartNew();
            rig_present(rig, 0);
            ms.Add(t.Elapsed.TotalMilliseconds);
            if (++frames == 120) {
                ms.Sort();
                Log.Say("frame_ms_median", ms[ms.Count / 2].ToString("F3"));
                Log.Say("frames", frames.ToString());
                var hold = int.TryParse(Environment.GetEnvironmentVariable("HOLD_MS"), out var h) ? h : 0;
                if (hold == 0) Environment.Exit(0);
                // Keep presenting so a screenshot can see the panel.
                new Thread(() => { Thread.Sleep(hold); Environment.Exit(0); }) { IsBackground = true }.Start();
            }
        };
    }
}
