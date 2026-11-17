import { describe, expect, it } from "vitest";
import { linkedHeadword, soundUrl } from "./entryLinks";

const src = "http://odentry.localhost/73616d706c65/?headword=apple";
const base = "http://odentry.localhost/73616d706c65/";

describe("entry:// links", () => {
  it("name the headword they link to", () => {
    expect(linkedHeadword("entry://serendipity")).toBe("serendipity");
    expect(linkedHeadword("ENTRY://pear")).toBe("pear");
    expect(linkedHeadword("entry://食べる")).toBe("食べる");
    expect(linkedHeadword("entry://ice%20cream")).toBe("ice cream");
    expect(linkedHeadword("entry://look up#sense2")).toBe("look up");
    expect(linkedHeadword("entry:// spaced ")).toBe("spaced");
  });

  it("keep a headword that only looks percent-encoded", () => {
    expect(linkedHeadword("entry://100%")).toBe("100%");
  });

  it("name nothing for other links, anchors in the same Entry, or nonsense", () => {
    expect(linkedHeadword("sound://a.mp3")).toBeNull();
    expect(linkedHeadword("https://example.com/")).toBeNull();
    expect(linkedHeadword("entry://#sense2")).toBeNull();
    expect(linkedHeadword("entry://")).toBeNull();
    expect(linkedHeadword("entry://" + "a".repeat(2000))).toBeNull();
  });
});

describe("sound:// links", () => {
  it("play a file from beneath their own Dictionary", () => {
    expect(soundUrl(src, "sound://uk/apple.mp3")).toBe(`${base}uk/apple.mp3`);
    expect(soundUrl(src, "SOUND://\\us\\apple.mp3")).toBe(`${base}us/apple.mp3`);
    expect(soundUrl(src, "sound://a b#1?.mp3")).toBe(`${base}a%20b%231%3F.mp3`);
    expect(soundUrl(src, "sound://%E3%81%82.mp3")).toBe(`${base}%E3%81%82.mp3`);
  });

  it("accept a bare path, as the Pronounce action has it", () => {
    expect(soundUrl(src, "uk/apple.mp3")).toBe(`${base}uk/apple.mp3`);
  });

  it("never reach outside their Dictionary", () => {
    expect(soundUrl(src, "sound://../6f74686572/a.mp3")).toBeNull();
    expect(soundUrl(src, "sound://%2e%2e/6f74686572/a.mp3")).toBeNull();
    expect(soundUrl(src, "sound://a/./b.mp3")).toBeNull();
    expect(soundUrl(src, "sound://")).toBeNull();
    expect(soundUrl(src, "sound://a//b.mp3")).toBeNull();
    expect(soundUrl(src, "entry://a.mp3")).toBeNull();
    expect(soundUrl(src, "https://example.com/a.mp3")).toBeNull();
    expect(soundUrl(src, "sound://" + "a".repeat(2000))).toBeNull();
  });
});
