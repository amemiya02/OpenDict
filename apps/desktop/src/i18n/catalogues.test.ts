import { describe, expect, it } from "vitest";
import { catalogues, pickLocale } from ".";

describe("string catalogues", () => {
  it("have the same keys in every interface language", () => {
    const keys = Object.keys(catalogues.en).sort();
    expect(Object.keys(catalogues["zh-CN"]).sort()).toEqual(keys);
    expect(Object.keys(catalogues.ja).sort()).toEqual(keys);
  });

  it("follow the system language, falling back to English", () => {
    expect(pickLocale(["zh-Hans-CN", "en"])).toBe("zh-CN");
    expect(pickLocale(["ja-JP"])).toBe("ja");
    expect(pickLocale(["de-DE", "en-GB"])).toBe("en");
    expect(pickLocale(["fr"])).toBe("en");
  });
});
