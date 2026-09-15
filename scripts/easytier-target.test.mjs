import { test } from "node:test";
import assert from "node:assert/strict";
import { selectEasyTierTarget } from "./easytier-target.mjs";

test("Intel Mac build on an ARM runner selects Intel Core", () => {
  assert.equal(selectEasyTierTarget("darwin", "arm64", "darwin-x64").triple, "x86_64-apple-darwin");
});
test("native builds preserve their platform and executable names", () => {
  assert.equal(selectEasyTierTarget("darwin", "arm64").triple, "aarch64-apple-darwin");
  assert.deepEqual(selectEasyTierTarget("win32", "x64").executables, ["easytier-core.exe", "easytier-cli.exe"]);
});
test("invalid targets fail before downloading", () => {
  assert.throws(() => selectEasyTierTarget("darwin", "arm64", "linux-x64"));
  assert.throws(() => selectEasyTierTarget("darwin", "arm64", "win32-x64"));
});
