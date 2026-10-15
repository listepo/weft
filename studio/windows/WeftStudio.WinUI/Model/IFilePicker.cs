using System.Threading.Tasks;

namespace WeftStudio.Model;

/// <summary>The native file dialogs, behind an interface so the tests and the smoke run answer them.</summary>
public interface IFilePicker
{
    /// <summary>A <c>.weft</c> file to open, or null when the user cancels.</summary>
    Task<string?> PickOpenAsync();

    /// <summary>Where to save, suggesting <paramref name="suggestedName"/>; null when the user cancels.</summary>
    Task<string?> PickSaveAsync(string suggestedName);
}
