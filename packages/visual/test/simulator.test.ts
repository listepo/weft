// The simulator-sharing helpers of the SwiftUI suite: the mode setting, the own-mode device name,
// device selection and the machine-wide lock. None of it needs Xcode.
import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, test } from "vitest";
import {
  OWN_PREFIX,
  acquireLock,
  deviceName,
  findDevice,
  simulatorMode,
  type SimulatorDevice,
} from "../src/simulator.ts";

describe("simulatorMode", () => {
  test("is shared unless the setting says otherwise", () => {
    assert.equal(simulatorMode({}), "shared");
    assert.equal(simulatorMode({ WEFT_SIMULATOR: "shared" }), "shared");
    assert.equal(simulatorMode({ WEFT_SIMULATOR: "own" }), "own");
  });

  test("refuses an unknown value instead of guessing", () => {
    assert.throws(() => simulatorMode({ WEFT_SIMULATOR: "private" }), /"shared" or "own"/);
    assert.throws(() => simulatorMode({ WEFT_SIMULATOR: "" }), /"shared" or "own"/);
  });
});

describe("deviceName", () => {
  test("is the worktree folder behind the prefix", () => {
    assert.equal(deviceName("weft-t46"), "weft-visual-weft-t46");
  });

  test("keeps the name typeable for simctl", () => {
    assert.equal(deviceName("my work tree/v2"), "weft-visual-my-work-tree-v2");
  });

  test("differs between worktrees", () => {
    assert.notEqual(deviceName("weft-t46"), deviceName("weft-t47"));
  });
});

describe("findDevice", () => {
  const RUNTIME = "runtime.iOS-27-0";
  const TYPE = "type.iPhone-17";
  const device = (name: string, udid: string, type = TYPE): SimulatorDevice => ({
    name,
    udid,
    deviceTypeIdentifier: type,
  });
  const devices = {
    [RUNTIME]: [
      device(deviceName("weft-t46"), "OWN-46"),
      device("iPhone 17", "SHARED"),
      device("iPhone 17 Pro", "OTHER-TYPE", "type.iPhone-17-Pro"),
    ],
    "runtime.iOS-26-0": [device("iPhone 17", "OTHER-RUNTIME")],
  };

  test("shared mode skips the devices the suite created", () => {
    assert.equal(findDevice(devices, RUNTIME, TYPE)?.udid, "SHARED");
    assert.equal(
      findDevice({ [RUNTIME]: [device(OWN_PREFIX + "a", "A")] }, RUNTIME, TYPE),
      undefined,
    );
  });

  test("own mode takes the device with the worktree's name", () => {
    assert.equal(findDevice(devices, RUNTIME, TYPE, deviceName("weft-t46"))?.udid, "OWN-46");
    assert.equal(findDevice(devices, RUNTIME, TYPE, deviceName("weft-t47")), undefined);
  });

  test("matches the pinned type and runtime only", () => {
    assert.equal(findDevice(devices, "runtime.none", TYPE), undefined);
    assert.equal(findDevice(devices, RUNTIME, "type.none"), undefined);
  });
});

describe("acquireLock", () => {
  let dir = "";
  let lock = "";
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "weft-lock-"));
    lock = join(dir, "sub", "simulator.lock");
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  const fast = { pollMs: 10 };

  test("creates the lock with this process's pid and removes it on release", async () => {
    const release = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    assert.equal(readFileSync(lock, "utf8").split("\n")[0], String(process.pid));
    release();
    assert.equal(existsSync(lock), false);
    release();
  });

  test("leaves nothing but the lock behind", async () => {
    const release = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    assert.deepEqual(readdirSync(join(dir, "sub")), ["simulator.lock"]);
    release();
    assert.deepEqual(readdirSync(join(dir, "sub")), []);
  });

  test("a second run waits until the first one releases", async () => {
    const first = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    const order: string[] = [];
    const waiting: number[] = [];
    const second = acquireLock(lock, {
      ...fast,
      timeoutMs: 5000,
      onWait: (pid) => waiting.push(pid),
    }).then((release) => {
      order.push("second");
      return release;
    });
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.equal(order.length, 0);
    order.push("first");
    first();
    const release = await second;
    assert.deepEqual(order, ["first", "second"]);
    assert.deepEqual(waiting, [process.pid]);
    release();
  });

  test("gives up after the timeout and names the holder and the lock file", async () => {
    const release = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    await assert.rejects(
      acquireLock(lock, { ...fast, timeoutMs: 50 }),
      (error: Error) =>
        error.message.includes(`process ${process.pid}`) && error.message.includes(lock),
    );
    // The waiter that gave up leaves the holder's lock alone.
    assert.equal(existsSync(lock), true);
    release();
  });

  test("takes over the lock of a process that is gone", async () => {
    const holder = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    const dead = process.pid;
    const release = await acquireLock(lock, {
      ...fast,
      timeoutMs: 1000,
      isAlive: (pid) => pid !== dead,
    });
    assert.equal(readFileSync(lock, "utf8").split("\n")[0], String(process.pid));
    // The first holder's release must not remove the lock that replaced its own.
    holder();
    assert.equal(existsSync(lock), true);
    release();
    assert.equal(existsSync(lock), false);
  });

  test("takes over a lock left without a readable pid", async () => {
    await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    writeFileSync(lock, "not a pid");
    const release = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    assert.equal(readFileSync(lock, "utf8").split("\n")[0], String(process.pid));
    release();
  });

  test("does not take over the lock of a live process", async () => {
    const release = await acquireLock(lock, { ...fast, timeoutMs: 1000 });
    await assert.rejects(
      acquireLock(lock, { ...fast, timeoutMs: 50, isAlive: () => true }),
      /still held/,
    );
    release();
  });

  test("waiters that find a stale lock together end up with one holder", async () => {
    // A real dead pid, not a stubbed one: the default liveness check is the one that runs.
    const dead = 2 ** 22 - 1;
    mkdirSync(join(lock, ".."));
    writeFileSync(lock, `${dead}\nstale\n`);
    const holders: string[] = [];
    const contenders = Array.from({ length: 6 }, (_, i) =>
      acquireLock(lock, { ...fast, timeoutMs: 10_000 }).then(async (release) => {
        holders.push(`in-${i}`);
        await new Promise((resolve) => setTimeout(resolve, 30));
        holders.push(`out-${i}`);
        release();
      }),
    );
    await Promise.all(contenders);
    // Each turn is entered and left before the next begins.
    for (let i = 0; i < holders.length; i += 2) {
      assert.equal(holders[i]?.replace("in-", ""), holders[i + 1]?.replace("out-", ""));
    }
    assert.equal(existsSync(lock), false);
  });
});
