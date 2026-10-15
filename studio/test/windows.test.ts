// The Windows shell (T69.1e): `dotnet build` of the WinUI app, which builds weft_studio_ffi.dll
// with Cargo, then `dotnet test`: the document model with a fake picker and the FlaUI smoke run
// of the built app. Windows with the .NET SDK only; elsewhere it skips and says why.
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { repoRoot, writeMsbuildProps } from "../scripts/slint-bindings.ts";

const windows = join(repoRoot, "studio/windows");
const arm = process.arch === "arm64";

const run = (command: string, args: string[]) => {
  const result = spawnSync(command, args, {
    cwd: windows,
    encoding: "utf8",
    maxBuffer: 1 << 28,
    env: { ...process.env, DOTNET_CLI_TELEMETRY_OPTOUT: "1", DOTNET_NOLOGO: "1" },
  });
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
};

const works = (command: string, args: string[]): boolean => {
  try {
    return spawnSync(command, args, { stdio: "ignore" }).status === 0;
  } catch {
    return false;
  }
};

const skipReason = (() => {
  if (process.platform !== "win32") return "not Windows";
  if (!works("dotnet", ["--version"])) return "dotnet is not available (no .NET SDK)";
  if (!works("cargo", ["--version"])) return "cargo is not available";
  return undefined;
})();
// CI sets it, so a runner that lost its toolchain fails instead of passing on a skip.
if (skipReason && process.env.WEFT_STUDIO_WINDOWS === "required") {
  throw new Error(`the Windows shell tests cannot run: ${skipReason}`);
}
if (skipReason) {
  console.warn(`skipped: the Windows shell tests need Windows with the .NET SDK: ${skipReason}`);
}

describe.skipIf(skipReason)("studio/windows", () => {
  test("dotnet build of the WinUI app", { timeout: 3_600_000 }, () => {
    writeMsbuildProps();
    const built = run("dotnet", [
      "build",
      "WeftStudio.WinUI",
      `-p:Platform=${arm ? "ARM64" : "x64"}`,
      `-p:RuntimeIdentifier=${arm ? "win-arm64" : "win-x64"}`,
    ]);
    expect(built.status, built.output.slice(-8000)).toBe(0);
  });

  test("dotnet test: the document model and the smoke run", { timeout: 1_800_000 }, () => {
    const tested = run("dotnet", ["test", "--project", "WeftStudio.Tests"]);
    expect(tested.status, tested.output.slice(-8000)).toBe(0);
  });
});
