// The document model over the real weft_studio_ffi.dll, with the fake picker in place of the
// dialogs. Every test makes its editor on its own thread and stays on it, as the ws_* calls require.

using System.IO;
using System.Threading.Tasks;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using WeftStudio.Model;

namespace WeftStudio.Tests;

[TestClass]
public sealed class DocumentTests
{
    private static Document NewDocument(FixedFilePicker picker) => new(new Editor(), picker);

    [TestMethod]
    public void Without_a_document_the_title_is_the_app()
    {
        using var document = NewDocument(new FixedFilePicker(null));
        Assert.IsFalse(document.IsOpen);
        Assert.AreEqual("WeftStudio", document.Title);
    }

    [TestMethod]
    public async Task Open_through_the_picker_shows_the_file_name()
    {
        var picker = new FixedFilePicker(Paths.Corpus("login"));
        using var document = NewDocument(picker);
        var changed = 0;
        document.Changed += () => changed++;
        Assert.IsTrue(await document.OpenAsync(), document.Error);
        Assert.AreEqual(1, picker.Asked);
        Assert.AreEqual(1, changed);
        Assert.AreEqual("screen.weft", document.Title);
        Assert.AreEqual("login", document.State?.Screen);
        Assert.IsFalse(document.State?.Dirty);
    }

    [TestMethod]
    public async Task A_cancelled_open_changes_nothing()
    {
        using var document = NewDocument(new FixedFilePicker(null));
        Assert.IsFalse(await document.OpenAsync());
        Assert.IsNull(document.Error);
        Assert.IsFalse(document.IsOpen);
    }

    [TestMethod]
    public async Task Save_as_writes_the_same_bytes_and_takes_the_new_name()
    {
        using var scratch = new Scratch();
        var copy = scratch.File("copy.weft");
        var picker = new FixedFilePicker(copy);
        using var document = NewDocument(picker);
        Assert.IsTrue(document.Open(Paths.Corpus("login")), document.Error);
        Assert.IsTrue(await document.SaveAsAsync(), document.Error);
        Assert.AreEqual("screen.weft", picker.SuggestedName);
        CollectionAssert.AreEqual(File.ReadAllBytes(Paths.Corpus("login")), File.ReadAllBytes(copy));
        Assert.AreEqual("copy.weft", document.Title);
        Assert.AreEqual(copy, document.State?.Path);
    }

    [TestMethod]
    public async Task Save_writes_to_the_open_file_without_asking()
    {
        using var scratch = new Scratch();
        var original = File.ReadAllBytes(Paths.Corpus("login"));
        var screen = scratch.Write("screen.weft", original);
        var picker = new FixedFilePicker(null);
        using var document = NewDocument(picker);
        Assert.IsTrue(document.Open(screen), document.Error);
        File.Delete(screen);
        Assert.IsTrue(await document.SaveAsync(), document.Error);
        Assert.AreEqual(0, picker.Asked);
        CollectionAssert.AreEqual(original, File.ReadAllBytes(screen));
    }

    [TestMethod]
    public async Task A_cancelled_save_as_writes_nothing()
    {
        using var document = NewDocument(new FixedFilePicker(null));
        Assert.IsTrue(document.Open(Paths.Corpus("login")), document.Error);
        Assert.IsFalse(await document.SaveAsAsync());
        Assert.IsNull(document.Error);
        Assert.AreEqual(Paths.Corpus("login"), document.State?.Path);
    }

    [TestMethod]
    public async Task Saving_without_a_document_fails_without_asking()
    {
        var picker = new FixedFilePicker("ignored.weft");
        using var document = NewDocument(picker);
        Assert.IsFalse(await document.SaveAsync());
        Assert.AreEqual("Open a screen first.", document.Error);
        Assert.AreEqual(0, picker.Asked);
    }

    [TestMethod]
    public void A_refused_open_reports_why_and_keeps_the_document()
    {
        using var scratch = new Scratch();
        var broken = scratch.Write("broken.weft", "<screen id=\"s\" weft=\"0.1\"><heading"u8.ToArray());
        using var document = NewDocument(new FixedFilePicker(null));
        Assert.IsTrue(document.Open(Paths.Corpus("login")), document.Error);
        Assert.IsFalse(document.Open(broken));
        StringAssert.Contains(document.Error, "syntax errors\n");
        Assert.AreEqual("screen.weft", document.Title);
        Assert.IsFalse(document.Open(scratch.File("missing.weft")));
        StringAssert.Contains(document.Error, "cannot read");
        Assert.AreEqual("screen.weft", document.Title);
    }

    [TestMethod]
    public void A_refused_screen_opens_but_its_surface_says_why()
    {
        using var scratch = new Scratch();
        var refused = scratch.Write("refused.weft",
            "<screen id=\"s\" weft=\"0.1\"><heading id=\"t\" level=\"1\" shade=\"dark\">Hi</heading></screen>"u8.ToArray());
        using var document = NewDocument(new FixedFilePicker(null));
        StringAssert.Contains(Assert.ThrowsExactly<EditorException>(() => document.NewSurface(1, 1, 1)).Message, "no document");
        Assert.IsTrue(document.Open(refused), document.Error);
        StringAssert.Contains(Assert.ThrowsExactly<EditorException>(() => document.NewSurface(480, 800, 1)).Message, "shade");
    }

    // The only test that draws: Slint takes the first thread that does as its UI thread.
    [TestMethod]
    public void A_surface_of_the_open_screen_is_a_host()
    {
        using var document = NewDocument(new FixedFilePicker(null));
        Assert.IsTrue(document.Open(Paths.Corpus("login")), document.Error);
        var host = document.NewSurface(480, 800, 1);
        Assert.AreNotEqual(System.IntPtr.Zero, host);
        Native.sb_host_free(host);
    }
}

internal static partial class Native
{
    // The test frees the host itself; in the app SlintHost.Adopt owns it.
    [System.Runtime.InteropServices.LibraryImport("weft_studio_ffi")]
    internal static partial void sb_host_free(System.IntPtr host);
}
