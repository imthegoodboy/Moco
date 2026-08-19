import { describe, expect, it } from "vitest";
import { formatBytes, initials } from "./format";

describe("format helpers", () => {
  it("formats model sizes for the interface", () => {
    expect(formatBytes(153_406_304)).toBe("146 MB");
    expect(formatBytes(0)).toBe("0 B");
  });

  it("builds compact model initials", () => {
    expect(initials("Liquid AI")).toBe("LA");
    expect(initials("Moco")).toBe("M");
  });
});
