using System;
using System.IO;
using System.Linq;
using System.Reflection;

namespace WeftStudio.Tests;

internal static class Paths
{
    public static readonly string RepoRoot = typeof(Paths).Assembly
        .GetCustomAttributes<AssemblyMetadataAttribute>()
        .Single(a => a.Key == "WeftRepoRoot").Value!;

    public static string Corpus(string screen) => Path.Combine(RepoRoot, "corpus", screen, "screen.weft");

    /// <summary>The built app: <c>WEFT_STUDIO_EXE</c>, else the newest build under the app's <c>bin</c>.</summary>
    public static string App()
    {
        if (Environment.GetEnvironmentVariable("WEFT_STUDIO_EXE") is { Length: > 0 } exe) return exe;
        var bin = Path.Combine(RepoRoot, "studio", "windows", "WeftStudio.WinUI", "bin");
        var built = Directory.Exists(bin)
            ? new DirectoryInfo(bin).EnumerateFiles("WeftStudio.exe", SearchOption.AllDirectories)
                .OrderByDescending(f => f.LastWriteTimeUtc).FirstOrDefault()
            : null;
        return built?.FullName ?? throw new FileNotFoundException(
            $"No WeftStudio.exe under {bin}: dotnet build studio/windows/WeftStudio.WinUI -p:Platform=x64 first");
    }
}

/// <summary>A directory of its own per test, removed afterwards.</summary>
internal sealed class Scratch : IDisposable
{
    public string Dir { get; } = Directory.CreateTempSubdirectory("weft-studio-").FullName;

    public string File(string name) => Path.Combine(Dir, name);

    public string Write(string name, byte[] content)
    {
        var path = File(name);
        System.IO.File.WriteAllBytes(path, content);
        return path;
    }

    public void Dispose()
    {
        try { Directory.Delete(Dir, recursive: true); }
        catch (IOException) { }
    }
}
