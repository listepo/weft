// The ws_* editor of weft_studio_ffi.dll as a C# object. No WinUI type here, so the tests compile
// this file in and run without the Windows App SDK runtime.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using WeftStudio.Native;

namespace WeftStudio.Model;

public sealed class EditorException(string message) : Exception(message);

/// <summary>The state JSON of the open document (`ws_state_json`), the fields the shell reads.</summary>
public sealed record EditorState(string Path, string Title, string Screen, bool Dirty);

public sealed unsafe class Editor : IDisposable
{
    private WsEditor* _handle;
    private GCHandle _self;

    /// <summary>After an open or a save; read <see cref="State"/> for the title.</summary>
    public event Action? Changed;

    public Editor()
    {
        // The JSON shape is versioned with the API: a library of another version is refused here.
        var version = NativeMethods.ws_api_version();
        if (version != NativeMethods.WS_API_VERSION)
            throw new EditorException($"weft_studio_ffi speaks API {version}, this shell {NativeMethods.WS_API_VERSION}");
        // Rust holds this handle until ws_editor_free; it keeps the object alive while events can arrive.
        _self = GCHandle.Alloc(this);
        _handle = NativeMethods.ws_editor_new(&OnEvent, (void*)GCHandle.ToIntPtr(_self));
        if (_handle == null)
        {
            _self.Free();
            throw new EditorException(LastError());
        }
    }

    public EditorState? State
    {
        get
        {
            var text = NativeMethods.ws_state_json(Live());
            if (text == null) return null;
            try
            {
                using var json = JsonDocument.Parse(Marshal.PtrToStringUTF8((IntPtr)text) ?? "{}");
                var root = json.RootElement;
                return new EditorState(
                    root.GetProperty("path").GetString() ?? "",
                    root.GetProperty("title").GetString() ?? "",
                    root.GetProperty("screen").GetString() ?? "",
                    root.GetProperty("dirty").GetBoolean());
            }
            finally
            {
                NativeMethods.ws_string_free(text);
            }
        }
    }

    /// <summary>False, with <see cref="LastError"/>, when the file cannot be opened; the open document stays.</summary>
    public bool Open(string path)
    {
        fixed (byte* p = Utf8(path)) return NativeMethods.ws_open(Live(), p);
    }

    /// <summary>Saves to <paramref name="path"/>, or to the current path when it is null.</summary>
    public bool Save(string? path)
    {
        if (path is null) return NativeMethods.ws_save(Live(), null);
        fixed (byte* p = Utf8(path)) return NativeMethods.ws_save(Live(), p);
    }

    /// <summary>A new <c>SbHost*</c> showing the open screen; the caller owns it (<c>SlintHost.Adopt</c>).</summary>
    public IntPtr NewSurface(int pixelWidth, int pixelHeight, float scale)
    {
        var host = NativeMethods.ws_surface_new(Live(), (uint)Math.Max(pixelWidth, 1), (uint)Math.Max(pixelHeight, 1), scale);
        if (host == null) throw new EditorException(LastError());
        return (IntPtr)host;
    }

    public static string LastError() =>
        Marshal.PtrToStringUTF8((IntPtr)NativeMethods.ws_last_error()) ?? "unknown error";

    public void Dispose()
    {
        if (_handle == null) return;
        NativeMethods.ws_editor_free(_handle);
        _handle = null;
        _self.Free();
    }

    private WsEditor* Live() =>
        _handle != null ? _handle : throw new ObjectDisposedException(nameof(Editor));

    private static byte[] Utf8(string text) => Encoding.UTF8.GetBytes(text + "\0");

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnEvent(void* userData, byte* eventJson)
    {
        // An exception must not unwind into Rust; a malformed event is dropped.
        try
        {
            if (GCHandle.FromIntPtr((IntPtr)userData).Target is not Editor editor) return;
            using var json = JsonDocument.Parse(Marshal.PtrToStringUTF8((IntPtr)eventJson) ?? "{}");
            // `action` events (the screen's `perform` calls) have no use in M1's live preview.
            if (json.RootElement.TryGetProperty("type", out var type) && type.GetString() == "changed")
                editor.Changed?.Invoke();
        }
        catch (Exception)
        {
        }
    }
}
