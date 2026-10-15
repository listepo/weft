using System;
using System.Runtime.InteropServices;
using System.Text;

namespace WeftStudio.Tests;

/// <summary>
/// Which window holds the foreground and the keyboard focus. UI Automation cannot say: the
/// Slint panel is invisible to it, and simulated keys go to the focus window, not to an element.
/// </summary>
internal static partial class Native
{
    [StructLayout(LayoutKind.Sequential)]
    private struct Rect { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    private struct GuiThreadInfo
    {
        public int Size;
        public int Flags;
        public IntPtr Active, Focus, Capture, MenuOwner, MoveSize, Caret;
        public Rect CaretRect;
    }

    [LibraryImport("user32.dll")]
    private static partial IntPtr GetForegroundWindow();

    [LibraryImport("user32.dll")]
    private static partial uint GetWindowThreadProcessId(IntPtr window, out uint process);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool GetGUIThreadInfo(uint thread, ref GuiThreadInfo info);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool IsChild(IntPtr parent, IntPtr window);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr window, StringBuilder name, int capacity);

    public static IntPtr Foreground() => GetForegroundWindow();

    /// <summary>The window that receives keystrokes on <paramref name="window"/>'s thread.</summary>
    public static IntPtr Focus(IntPtr window)
    {
        var info = new GuiThreadInfo { Size = Marshal.SizeOf<GuiThreadInfo>() };
        return GetGUIThreadInfo(GetWindowThreadProcessId(window, out _), ref info) ? info.Focus : IntPtr.Zero;
    }

    /// <summary>Keyboard focus sits in a child of the window: the XAML content, not the frame.</summary>
    public static bool FocusInside(IntPtr window) => IsChild(window, Focus(window));

    public static string Describe(IntPtr window)
    {
        if (window == IntPtr.Zero) return "none";
        var name = new StringBuilder(256);
        GetClassName(window, name, name.Capacity);
        return $"0x{window:x} {name}";
    }
}
