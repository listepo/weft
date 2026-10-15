// The built app through UI Automation (FlaUI, UIA3): a corpus screen on the command line, the
// window title, a capture of the Slint panel, and Save As through the --pick fake picker. The
// panel is invisible to UI Automation (slint-bindings T14), so it is checked by its pixels.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Capturing;
using FlaUI.Core.Input;
using FlaUI.Core.Tools;
using FlaUI.Core.WindowsAPI;
using FlaUI.UIA3;
using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace WeftStudio.Tests;

[TestClass]
public sealed class SmokeTests
{
    private static readonly TimeSpan Wait = TimeSpan.FromSeconds(60);

    [TestMethod]
    public void Probe()
    {
        var lines = new List<string>();
        for (var i = 0; i < 12; i++)
        {
            try { Once(lines, i); }
            catch (Exception ex) { lines.Add($"#{i} THREW {ex.GetType().Name}: {ex.Message.Split('\n')[0]}"); }
        }
        var report = string.Join("\n", lines);
        Console.WriteLine(report);
        Assert.Fail(report);
    }

    private static void Once(List<string> lines, int i)
    {
        using var scratch = new Scratch();
        var screen = Paths.Corpus("login");
        var copy = scratch.File("copy.weft");
        var start = new ProcessStartInfo(Paths.App()) { ArgumentList = { screen, "--pick", copy } };
        using var app = FlaUI.Core.Application.Launch(start);
        using var automation = new UIA3Automation();
        try
        {
            var window = app.GetMainWindow(automation, Wait) ?? throw new AssertFailedException("no window");
            Assert.IsTrue(Retry.WhileFalse(() => window.Title == "screen.weft", Wait).Result, $"title: {window.Title}");
            var menu = window.FindFirstDescendant(cf => cf.ByAutomationId("Menu"))
                ?? throw new AssertFailedException("no menu bar");
            window.SetForeground();

            // The panel loads and draws a few frames after the window shows.
            var colours = 0;
            Retry.WhileFalse(() => (colours = PanelColours(window, menu)) > 1, Wait);
            Assert.IsTrue(colours > 1, "the Slint panel is one colour; see the capture");

            // Focus the panel as a user does, so the accelerator has to get past the surface.
            var panel = window.BoundingRectangle;
            var hwnd = window.Properties.NativeWindowHandle.Value;
            string State() => $"fg {Native.Describe(Native.Foreground())} (app {Native.Describe(hwnd)}) focus {Native.Describe(Native.Focus(hwnd))} inside {Native.FocusInside(hwnd)}";
            var preClick = State();
            Mouse.Click(new System.Drawing.Point(panel.Left + 24, menu.BoundingRectangle.Bottom + 24));
            var preKeys = State();
            Keyboard.TypeSimultaneously(VirtualKeyShort.CONTROL, VirtualKeyShort.SHIFT, VirtualKeyShort.KEY_S);
            var ok = Retry.WhileFalse(() => window.Title == "copy.weft", TimeSpan.FromSeconds(15)).Result;
            var after = State();
            lines.Add($"#{i} {(ok ? "ok" : "MISS")} | preClick: {preClick} | preKeys: {preKeys} | after: {after} | title {window.Title}");
        }
        finally
        {
            app.Close();
        }
    }

    /// <summary>
    /// Distinct colours below the menu bar, sampled every 4 pixels. The capture is kept in
    /// <c>target/weft-studio-windows.png</c> for the CI artifact.
    /// </summary>
    private static int PanelColours(Window window, AutomationElement menu)
    {
        using var capture = Capture.Element(window);
        Directory.CreateDirectory(Path.Combine(Paths.RepoRoot, "target"));
        capture.ToFile(Path.Combine(Paths.RepoRoot, "target", "weft-studio-windows.png"));
        var bitmap = capture.Bitmap;
        var top = menu.BoundingRectangle.Bottom - window.BoundingRectangle.Top + 8;
        var seen = new HashSet<int>();
        for (var y = top; y < bitmap.Height - 8; y += 4)
            for (var x = 8; x < bitmap.Width - 8; x += 4)
                seen.Add(bitmap.GetPixel(x, y).ToArgb());
        return seen.Count;
    }
}
