import { describe, it, expect } from "vitest";
import { escapeHtml, validIp, canDeploy } from "./flow";
describe("input and operation boundaries", () => {
  it("escapes remote text before HTML interpolation", () =>
    expect(escapeHtml('<img src=x onerror="alert(1)">')).toBe(
      "&lt;img src=x onerror=&quot;alert(1)&quot;&gt;",
    ));
  it("rejects shell fragments and malformed addresses", () => {
    for (const x of [
      "1.2.3.999",
      "1.2.3.4;id",
      "01.2.3.4",
      "0.0.0.0",
      "server.local",
    ])
      expect(validIp(x)).toBe(false);
    expect(validIp("203.0.113.1")).toBe(true);
  });
  it("requires session, license and explicit server consent", () => {
    expect(canDeploy("session", "license", true)).toBe(true);
    expect(canDeploy("", "license", true)).toBe(false);
    expect(canDeploy("s", "", true)).toBe(false);
    expect(canDeploy("s", "l", false)).toBe(false);
  });
});
