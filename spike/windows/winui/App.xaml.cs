using Microsoft.UI.Xaml;

namespace WinUiSpike;

public partial class App : Application {
    Window? window;
    public App() {
        AppDomain.CurrentDomain.FirstChanceException += (_, e) => Log.Say("first_chance", e.Exception.GetType().Name + ": " + e.Exception.Message + " hr=0x" + e.Exception.HResult.ToString("x8"));
        AppDomain.CurrentDomain.UnhandledException += (_, e) => Log.Say("domain_unhandled", e.ExceptionObject.ToString() ?? "");
        InitializeComponent();
    }
    protected override void OnLaunched(LaunchActivatedEventArgs args) {
        Log.Say("app", "OnLaunched");
        UnhandledException += (_, e) => { Log.Say("unhandled", e.Exception?.ToString() ?? e.Message); };
        try {
            Log.Say("step", "before bare Window");
            var bare = new Microsoft.UI.Xaml.Window();
            Log.Say("step", "bare Window constructed");
            window = new MainWindow();
            Log.Say("app", "MainWindow constructed");
            window.Activate();
            Log.Say("app", "window activated");
        } catch (Exception e) {
            Log.Say("app_exception", e.ToString());
            Environment.Exit(5);
        }
    }
}

static class Log {
    public static void Say(string k, string v) { Console.WriteLine($"{k}={v}"); Console.Out.Flush(); }
}
