// One window's document: the menu commands over the editor and the pickers. A failed command
// leaves its message in Error for the window's dialog; the document stays as it was.

using System;
using System.Threading.Tasks;

namespace WeftStudio.Model;

public sealed class Document(Editor editor, IFilePicker picker) : IDisposable
{
    public const string AppName = "WeftStudio";

    /// <summary>After an open or a save.</summary>
    public event Action? Changed
    {
        add => editor.Changed += value;
        remove => editor.Changed -= value;
    }

    public EditorState? State => editor.State;

    /// <summary>The file name of the open document, or the app's name without one.</summary>
    public string Title => State?.Title is { Length: > 0 } title ? title : AppName;

    /// <summary>The message of the last failed command, with the diagnostics, one per line.</summary>
    public string? Error { get; private set; }

    public bool IsOpen => State is not null;

    public bool Open(string path)
    {
        Error = null;
        if (editor.Open(path)) return true;
        Error = Editor.LastError();
        return false;
    }

    public async Task<bool> OpenAsync()
    {
        Error = null;
        var path = await picker.PickOpenAsync();
        return path is not null && Open(path);
    }

    /// <summary>Saves to the document's path; asks for one, as Save As does, when there is none.</summary>
    public Task<bool> SaveAsync() =>
        State is { Path.Length: > 0 } ? Task.FromResult(Save(null)) : SaveAsAsync();

    public async Task<bool> SaveAsAsync()
    {
        Error = null;
        if (!IsOpen) return Fail("Open a screen first.");
        var path = await picker.PickSaveAsync(State?.Title is { Length: > 0 } title ? title : "screen.weft");
        return path is not null && Save(path);
    }

    /// <summary>A surface for the window's panel; <see cref="EditorException"/> when the screen is refused.</summary>
    public IntPtr NewSurface(int pixelWidth, int pixelHeight, float scale) =>
        editor.NewSurface(pixelWidth, pixelHeight, scale);

    public void Dispose() => editor.Dispose();

    private bool Save(string? path)
    {
        if (editor.Save(path)) return true;
        return Fail(Editor.LastError());
    }

    private bool Fail(string message)
    {
        Error = message;
        return false;
    }
}
