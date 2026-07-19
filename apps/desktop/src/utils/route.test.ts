import { describe, expect, it } from "vitest";
import { getDefaultRoute } from "./route";

describe("default route", () => {
  it("opens the App console inside Tauri", () => {
    expect(getDefaultRoute(true)).toBe("/desktop");
  });

  it("opens the Web client in a browser", () => {
    expect(getDefaultRoute(false)).toBe("/web");
  });
});
