// `node:assert/strict` for the browser leg, where Vite replaces Node's modules with stubs that throw.
// The suites keep their Node assertions; these are the ones they use, on Chai's strict equivalents
// that Vitest already bundles, so no polyfill package is added for a handful of functions.
import { assert as chai } from "vitest";

const assert = Object.assign((value: unknown, message?: string) => chai.ok(value, message), {
  ok: chai.ok,
  equal: chai.strictEqual,
  notEqual: chai.notStrictEqual,
  deepEqual: chai.deepStrictEqual,
  deepStrictEqual: chai.deepStrictEqual,
  match: chai.match,
  doesNotMatch: chai.notMatch,
  throws: chai.throws,
  doesNotThrow: chai.doesNotThrow,
});

export default assert;
