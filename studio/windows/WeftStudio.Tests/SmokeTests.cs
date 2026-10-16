// The built app through UI Automation (FlaUI, UIA3): a corpus screen on the command line, the
// window title, a capture of the Slint panel, and Save As through the --pick fake picker, typed
// while the panel has focus. The panel is invisible to UI Automation (slint-bindings T14), so it
// is checked by its pixels.

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
            Retry.WhileFalse(() => (colours = PanelPixels(window, menu).Distinct().Count()) > 1, Wait);
            Assert.IsTrue(colours > 1, "the Slint panel is one colour; see the capture");

            // Click into the login screen's Email field as a user does, so the panel holds keyboard
            // focus and the accelerator has to get past it. Pointer input reaches the panel later
            // than keys sent right behind it, so the keys wait until the field shows its focus; the
            // pointer is moved there first so that a hover effect is not taken for that.
            var field = new System.Drawing.Point(window.BoundingRectangle.Left + 120, menu.BoundingRectangle.Bottom + 90);
            Mouse.MoveTo(field);
            var unfocused = PanelPixels(window, menu);
            Retry.WhileFalse(() => Same(ref unfocused, PanelPixels(window, menu)), Wait);
            Mouse.Click(field);
            Assert.IsTrue(Retry.WhileFalse(() => !PanelPixels(window, menu).SequenceEqual(unfocused), Wait).Result,
                "the panel did not change after the click into the Email field, so it may not have focus; see the capture");
            Keyboard.TypeSimultaneously(VirtualKeyShort.CONTROL, VirtualKeyShort.SHIFT, VirtualKeyShort.KEY_S);
            Assert.IsTrue(Retry.WhileFalse(() => window.Title == "copy.weft", Wait).Result,
                $"Ctrl+Shift+S with the panel focused did not save: title {window.Title}, {(File.Exists(copy) ? "the file was written" : "no file")}");
            CollectionAssert.AreEqual(File.ReadAllBytes(screen), File.ReadAllBytes(copy));
        }
        finally
        {
            app.Close();
        }
    }

    /// <summary>True when two captures in a row match; keeps the latest in <paramref name="last"/>.</summary>
    private static bool Same(ref int[] last, int[] next)
    {
        var same = last.SequenceEqual(next);
        last = next;
        return same;
    }

    /// <summary>
    /// The pixels below the menu bar, sampled every 4 pixels. The capture is kept in
    /// <c>target/weft-studio-windows.png</c> for the CI artifact.
    /// </summary>
    private static int[] PanelPixels(Window window, AutomationElement menu)
    {
        using var capture = Capture.Element(window);
        Directory.CreateDirectory(Path.Combine(Paths.RepoRoot, "target"));
        capture.ToFile(Path.Combine(Paths.RepoRoot, "target", "weft-studio-windows.png"));
        var bitmap = capture.Bitmap;
        var top = menu.BoundingRectangle.Bottom - window.BoundingRectangle.Top + 8;
        var pixels = new List<int>();
        for (var y = top; y < bitmap.Height - 8; y += 4)
            for (var x = 8; x < bitmap.Width - 8; x += 4)
                pixels.Add(bitmap.GetPixel(x, y).ToArgb());
        return [.. pixels];
    }
}
