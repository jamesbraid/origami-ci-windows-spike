using System.Diagnostics;
using uniffi.ffi_core;

class Obs : Observer {
    public int Frames; public int States; public int LastThread;
    public void FrameReady(uint id, ulong seq) { Frames++; LastThread = Environment.CurrentManagedThreadId; }
    public void StateChanged(MachineInfo info) { States++; }
}

static class P {
    static async Task<int> Main() {
        var core = new Core();
        Console.WriteLine($"machines={core.Machines().Count()} first={core.Machines().First().Name}");
        var o = new Obs();
        core.SetObserver(o);
        core.Start(1);
        await Task.Delay(300);
        Console.WriteLine($"callbacks frames={o.Frames} states={o.States} on_thread_not_main={o.LastThread != Environment.CurrentManagedThreadId}");
        var sw = Stopwatch.StartNew();
        var t = core.Stop(1, 400);
        Console.WriteLine($"async_returned_before_done={!t.IsCompleted}");
        var ticks = 0; while (!t.IsCompleted) { await Task.Delay(50); ticks++; }
        Console.WriteLine($"async_result={await t} ms={sw.ElapsedMilliseconds} ui_ticks_while_waiting={ticks}");
        try { await core.Stop(0, 1); Console.WriteLine("error_mapping=FAILED no exception"); }
        catch (CoreException.NotRunning e) { Console.WriteLine($"error_mapping=ok {e.GetType().Name}"); }
        try { var c2 = new Core(); c2.Start(9); } catch (CoreException e) { Console.WriteLine($"sync_error_mapping=ok {e.GetType().Name}"); }
        return 0;
    }
}
