import { describe, expect, it } from "vitest";
import { formatSize } from "./format";

describe("file sizes", () => {
  it("read the way file managers show them", () => {
    expect(formatSize(971, "en")).toBe("971 B");
    expect(formatSize(1126, "en")).toBe("1.1 KB");
    expect(formatSize(38 * 1024 * 1024, "en")).toBe("38 MB");
    expect(formatSize(1.25 * 1024 ** 3, "zh-CN")).toBe("1.3 GB");
    expect(formatSize(0, "ja")).toBe("0 B");
  });
});
