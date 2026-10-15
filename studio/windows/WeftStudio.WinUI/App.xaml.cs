// WeftStudio.exe [screen.weft] [--pick <path>]
// The screen opens at launch; --pick answers every file dialog with <path> (the smoke test's fake
// picker, as the macOS shell takes a launch argument).

using System;
using System.Reflection;
using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;
using SlintBindings.WinUI;

namespace WeftStudio.WinUI;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        // slint-bindings' control imports `slint_bindings_ffi`, but its sb_* calls ship inside
        // weft_studio_ffi.dll with the editor: a second copy of the library would be a second Slint,
        // and the surface ws_surface_new made would not belong to it.
        NativeLibrary.SetDllImportResolver(typeof(SlintPanel).Assembly, ResolveSlint);
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        string? screen = null;
        string? pick = null;
        var argv = Environment.GetCommandLineArgs();
        for (var i = 1; i < argv.Length; i++)
        {
            if (argv[i] == "--pick" && i + 1 < argv.Length) pick = argv[++i];
            else screen ??= argv[i];
        }
        _window = new MainWindow(screen, pick);
        _window.Activate();
    }

    private static IntPtr ResolveSlint(string name, Assembly assembly, DllImportSearchPath? path) =>
        name == "slint_bindings_ffi" ? NativeLibrary.Load("weft_studio_ffi", assembly, path) : IntPtr.Zero;
}
