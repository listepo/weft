// The Windows App SDK pickers (1.8 and later): constructed with the window's AppWindow.Id, they
// return a path rather than a StorageFile, so the editor reads and writes the file itself.

// System holds the GetAwaiter extension that lets C# await a WinRT IAsyncOperation.
using System;
using System.Threading.Tasks;
using Microsoft.UI;
using Microsoft.Windows.Storage.Pickers;
using WeftStudio.Model;

namespace WeftStudio.WinUI;

public sealed class WinUiFilePicker(WindowId window) : IFilePicker
{
    private const string Extension = ".weft";

    public async Task<string?> PickOpenAsync()
    {
        var picker = new FileOpenPicker(window) { SuggestedStartLocation = PickerLocationId.DocumentsLibrary };
        picker.FileTypeFilter.Add(Extension);
        var result = await picker.PickSingleFileAsync();
        return result?.Path;
    }

    public async Task<string?> PickSaveAsync(string suggestedName)
    {
        var picker = new FileSavePicker(window)
        {
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
            SuggestedFileName = System.IO.Path.GetFileNameWithoutExtension(suggestedName),
            DefaultFileExtension = Extension,
        };
        picker.FileTypeChoices.Add("Weft screen", [Extension]);
        var result = await picker.PickSaveFileAsync();
        return result?.Path;
    }
}
