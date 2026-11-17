import { describe, expect, it } from "vitest";
import { catalogues, pickLocale } from ".";

describe("string catalogues", () => {
  it("have the same keys in every interface language", () => {
    const keys = Object.keys(catalogues.en).sort();
    expect(Object.keys(catalogues["zh-CN"]).sort()).toEqual(keys);
    expect(Object.keys(catalogues.ja).sort()).toEqual(keys);
  });

  it("give every string in every interface language, of the same kind as the English one", () => {
    for (const [locale, catalogue] of Object.entries(catalogues)) {
      for (const [key, english] of Object.entries(catalogues.en)) {
        const value = (catalogue as Record<string, unknown>)[key];
        const where = `${locale}.${key}`;
        expect(typeof value, where).toBe(typeof english);
        if (typeof value === "string") expect(value.trim(), where).not.toBe("");
        if (typeof value === "function" && typeof english === "function") {
          expect(value.length, where).toBe(english.length);
          const sample = (value as (...args: unknown[]) => unknown)(...Array.from({ length: value.length }, () => 2));
          expect(typeof sample, where).toBe("string");
          expect(String(sample).trim(), where).not.toBe("");
        }
      }
    }
  });

  it("follow the system language, falling back to English", () => {
    expect(pickLocale(["zh-Hans-CN", "en"])).toBe("zh-CN");
    expect(pickLocale(["ja-JP"])).toBe("ja");
    expect(pickLocale(["de-DE", "en-GB"])).toBe("en");
    expect(pickLocale(["fr"])).toBe("en");
  });
});
