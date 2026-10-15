// The built app through UI Automation (FlaUI, UIA3): a corpus screen on the command line, the
// window title, a capture of the Slint panel, and Save As through the --pick fake picker. The
// panel is invisible to UI Automation (slint-bindings T14), so it is checked by its pixels.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
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
    public void The_app_shows_a_corpus_screen_and_saves_it_as_the_same_bytes()
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
            Mouse.Click(new System.Drawing.Point(panel.Left + 24, menu.BoundingRectangle.Bottom + 24));
            Keyboard.TypeSimultaneously(VirtualKeyShort.CONTROL, VirtualKeyShort.SHIFT, VirtualKeyShort.KEY_S);
            var ok = Retry.WhileFalse(() => window.Title == "copy.weft", TimeSpan.FromSeconds(20)).Result;
            if (!ok)
            {
                var hwnd = window.Properties.NativeWindowHandle.Value;
                var texts = string.Join(" / ", window.FindAllDescendants().Select(e => $"{e.ControlType}:{Safe(() => e.Name)}").Where(t => !t.EndsWith(':')).Take(30));
                var windows = string.Join(" / ", app.GetAllTopLevelWindows(automation).Select(w => Safe(() => w.Title)));
                using var shot = Capture.Element(window);
                shot.ToFile(Path.Combine(Paths.RepoRoot, "target", "weft-studio-windows.png"));
                Assert.Fail($"PROBE MISS title {window.Title} | fg {Native.Describe(Native.Foreground())} app {Native.Describe(hwnd)} focus {Native.Describe(Native.Focus(hwnd))} | copy exists {File.Exists(copy)} | windows: {windows} | tree: {texts}");
            }
            CollectionAssert.AreEqual(File.ReadAllBytes(screen), File.ReadAllBytes(copy));
        }
        finally
        {
            app.Close();
        }
    }

    private static string Safe(Func<string> read) { try { return read(); } catch (Exception ex) { return ex.GetType().Name; } }

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
