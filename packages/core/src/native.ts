// The native addon (crates/weft-node), the faster of the two engines behind the core. `./wasm.ts`
// and `./web.ts` ask for it at load time and fall back to the WebAssembly module, so a missing,
// unsupported or broken addon never stops the core from loading. Browsers and Figma have no
// `process`; Deno has no addon support worth the permission prompts. Both stay on WebAssembly
// without a message.

/** What `WEFT_ENGINE` says: `auto` tries the addon, `native` insists on it, `wasm` skips it. */
export type EngineChoice = "auto" | "native" | "wasm";
export type Engine = "native" | "wasm";

/**
 * The choice is an environment variable, not a `weft.json` key: weft.json is read through the
 * engine, so the engine has to be picked before any project file is.
 */
export const ENGINE_ENV = "WEFT_ENGINE";

/** `process.platform-process.arch` → the suffix `napi build --platform` gives the file. */
const BUILDS: Readonly<Record<string, string>> = {
  "darwin-arm64": "darwin-arm64",
  "linux-x64": "linux-x64-gnu",
  "linux-arm64": "linux-arm64-gnu",
  "win32-x64": "win32-x64-msvc",
};

export type NativeOptions = {
  /** Overrides `WEFT_ENGINE`. */
  choice?: string | undefined;
  /** The folder that holds the `.node` files; the package's `native/` folder by default. */
  dir?: URL | undefined;
  warn?: ((message: string) => void) | undefined;
};

type NodeRequire = (path: string) => unknown;

/** The file name suffix of this platform's prebuilt addon, or `undefined` when there is none. */
export function buildName(): string | undefined {
  const { platform, arch } = globalThis.process ?? {};
  return BUILDS[`${platform}-${arch}`];
}

/** The addon is glibc-only; `process.report` is how Node says which libc it runs on. */
function onMusl(): boolean {
  if (process.platform !== "linux") return false;
  const header = (process.report?.getReport?.() as { header?: { glibcVersionRuntime?: string } })
    ?.header;
  return header !== undefined && header.glibcVersionRuntime === undefined;
}

/**
 * The addon's exports, or `undefined` when the core should use WebAssembly. A missing file is
 * silent under `auto` (a bundle, a browser-less sandbox or an unsupported platform never has one)
 * and a warning under `native`; a file that cannot be loaded is a warning either way.
 */
export function loadNative(options: NativeOptions = {}): object | undefined {
  const proc = globalThis.process;
  const modules = proc?.getBuiltinModule;
  if (proc === undefined || modules === undefined || proc.versions?.deno !== undefined) {
    return undefined;
  }
  const warn = options.warn ?? ((message: string) => console.warn(`weft: ${message}`));
  let choice = options.choice ?? proc.env?.[ENGINE_ENV] ?? "auto";
  if (choice !== "auto" && choice !== "native" && choice !== "wasm") {
    warn(`${ENGINE_ENV}=${choice} is not auto, native or wasm; using auto`);
    choice = "auto";
  }
  if (choice === "wasm") return undefined;

  const build = buildName();
  const dir = options.dir ?? new URL("../native/", import.meta.url);
  const { existsSync } = modules("node:fs") as typeof import("node:fs");
  const { fileURLToPath } = modules("node:url") as typeof import("node:url");
  const file = build === undefined ? undefined : fileURLToPath(new URL(`weft.${build}.node`, dir));
  if (file === undefined || !existsSync(file) || onMusl()) {
    if (choice === "native") {
      warn(`no native addon for ${proc.platform}-${proc.arch}; using WebAssembly`);
    }
    return undefined;
  }
  try {
    const { createRequire } = modules("node:module") as typeof import("node:module");
    const require: NodeRequire = createRequire(import.meta.url);
    return require(file) as object;
  } catch (error) {
    warn(`cannot load ${file}, using WebAssembly: ${(error as Error).message}`);
    return undefined;
  }
}
