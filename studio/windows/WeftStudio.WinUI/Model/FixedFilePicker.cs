using System.Threading.Tasks;

namespace WeftStudio.Model;

/// <summary>
/// Answers every dialog with one path, or cancels when it is null: the app's <c>--pick</c>
/// argument for the smoke test, and the tests' fake.
/// </summary>
public sealed class FixedFilePicker(string? path) : IFilePicker
{
    public string? Path { get; set; } = path;

    /// <summary>How many dialogs were asked for, and the name the last save dialog suggested.</summary>
    public int Asked { get; private set; }
    public string? SuggestedName { get; private set; }

    public Task<string?> PickOpenAsync()
    {
        Asked++;
        return Task.FromResult(Path);
    }

    public Task<string?> PickSaveAsync(string suggestedName)
    {
        Asked++;
        SuggestedName = suggestedName;
        return Task.FromResult(Path);
    }
}
