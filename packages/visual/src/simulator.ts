// How the SwiftUI screenshot suite shares the iOS Simulator between runs. The suite installs one
// app id and screenshots whatever the device shows, so two runs on one device overwrite each other
// and fail with false pixel diffs. `WEFT_SIMULATOR` picks the remedy: `shared` (default) is one
// device for everyone with a machine-wide lock so runs take turns, `own` is a device per worktree.
import { randomUUID } from "node:crypto";
import { linkSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export type SimulatorMode = "shared" | "own";

/** Marks the devices the suite creates, so shared mode skips them and the README can list them. */
export const OWN_PREFIX = "weft-visual-";

export function simulatorMode(
  env: Record<string, string | undefined> = process.env,
): SimulatorMode {
  const value = env.WEFT_SIMULATOR ?? "shared";
  if (value === "shared" || value === "own") return value;
  throw new Error(`WEFT_SIMULATOR must be "shared" or "own", not "${value}"`);
}

/** The own-mode device of a worktree, named after its folder. */
export function deviceName(worktree: string): string {
  // simctl takes the name as one argument, but a name with spaces or slashes is awkward to type
  // back when deleting the device by hand.
  return OWN_PREFIX + worktree.replace(/[^A-Za-z0-9._-]+/g, "-");
}

export interface SimulatorDevice {
  udid: string;
  name: string;
  deviceTypeIdentifier: string;
}

/**
 * The device of `type` on `runtime` from `simctl list devices -j`: the one called `name` in own
 * mode, otherwise the first one the suite did not create itself.
 */
export function findDevice(
  devices: Record<string, SimulatorDevice[]>,
  runtime: string,
  type: string,
  name?: string,
): SimulatorDevice | undefined {
  return devices[runtime]?.find(
    (d) =>
      d.deviceTypeIdentifier === type &&
      (name === undefined ? !d.name.startsWith(OWN_PREFIX) : d.name === name),
  );
}

/**
 * Locks live in the user's cache folder, not in `os.tmpdir()` or the worktree: simulators belong
 * to the user, while `TMPDIR` differs between sessions and sandboxes, which would give two runs
 * two locks.
 */
export const LOCK_DIR = join(homedir(), "Library", "Caches", "weft-visual");

export interface LockOptions {
  /** How long to wait for a live holder before giving up. */
  timeoutMs: number;
  pollMs?: number;
  /** Tells whether the process that holds the lock still runs. */
  isAlive?: (pid: number) => boolean;
  /** Called once, when the lock turns out to be held. */
  onWait?: (holderPid: number) => void;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function processIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    // EPERM means the process exists but belongs to someone else.
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

const holderPid = (content: string): number => Number.parseInt(content.split("\n")[0] ?? "", 10);

function read(path: string): string | undefined {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}

/**
 * Moves a dead holder's lock out of the way. The lock is renamed first and checked afterwards,
 * so a waiter that was slower than another one cannot delete the lock the faster one just took:
 * it puts back what it grabbed instead. A third party can still slip into that window; the
 * loser notices at release, which only removes a lock whose content is its own.
 */
function takeOver(path: string, seen: string): void {
  const grave = `${path}.stale.${randomUUID()}`;
  try {
    renameSync(path, grave);
  } catch (error) {
    // Gone already: another waiter took it over first. Any other failure must not spin forever.
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  if (read(grave) !== seen) {
    try {
      linkSync(grave, path);
    } catch {
      // Someone else holds the lock already; nothing to put back.
    }
  }
  rmSync(grave, { force: true });
}

/**
 * Takes the lock at `path`, waiting for a live holder. The holder's pid is written to a temp file
 * that is hard-linked to `path`, which is atomic, fails when the lock is held, and never shows a
 * half-written lock to a waiter. A lock whose process died is taken over. Resolves to the release
 * function, which is safe to call twice.
 */
export async function acquireLock(path: string, options: LockOptions): Promise<() => void> {
  const { timeoutMs, pollMs = 1000, isAlive = processIsAlive, onWait } = options;
  mkdirSync(join(path, ".."), { recursive: true });
  const content = `${process.pid}\n${randomUUID()}\n`;
  const pending = `${path}.${randomUUID()}.tmp`;
  writeFileSync(pending, content);
  const deadline = Date.now() + timeoutMs;
  let warned = false;
  try {
    for (;;) {
      try {
        linkSync(pending, path);
        return () => {
          if (read(path) === content) rmSync(path, { force: true });
        };
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      }
      const held = read(path);
      if (held === undefined) continue;
      const pid = holderPid(held);
      // A lock without a valid pid cannot be told from a dead one, and the atomic create means no
      // live holder leaves one behind.
      if (!Number.isInteger(pid) || pid <= 0 || !isAlive(pid)) {
        takeOver(path, held);
        continue;
      }
      if (Date.now() >= deadline) {
        throw new Error(
          `the iOS Simulator lock is still held by process ${pid} after ${Math.round(timeoutMs / 1000)} s (${path}). ` +
            `If that process is hung, kill it or delete the lock file; WEFT_SIMULATOR=own gives this worktree a simulator of its own.`,
        );
      }
      if (!warned) {
        warned = true;
        onWait?.(pid);
      }
      await sleep(pollMs);
    }
  } finally {
    rmSync(pending, { force: true });
  }
}
