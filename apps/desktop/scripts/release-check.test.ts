import { describe, expect, it } from "vitest";
import { manifestProblems, preflightProblems } from "./release-check.ts";

const PUBKEY = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEFCQ0QKUldUWA==";

function config(overrides: { version?: string; pubkey?: string } = {}) {
  return {
    version: overrides.version ?? "0.2.0",
    plugins: {
      updater: {
        pubkey: overrides.pubkey ?? PUBKEY,
        endpoints: ["https://github.com/amemiya02/OpenDict/releases/latest/download/latest.json"],
      },
    },
  };
}

describe("release preflight", () => {
  it("accepts a tag that names the app's version when the updater key is set", () => {
    expect(preflightProblems("v0.2.0", config())).toEqual([]);
    expect(preflightProblems("v0.2.0-rc.1", config({ version: "0.2.0-rc.1" }))).toEqual([]);
  });

  it("rejects a tag that does not match the version in tauri.conf.json", () => {
    expect(preflightProblems("v0.3.0", config())).toEqual([
      'Tag v0.3.0 does not match the app version 0.2.0 in tauri.conf.json; expected tag v0.2.0.',
    ]);
  });

  it("rejects a release whose updater public key has not been set up", () => {
    expect(preflightProblems("v0.2.0", config({ pubkey: "" }))).toEqual([
      "plugins.updater.pubkey in tauri.conf.json is empty: set up the updater key first (docs/releasing.md).",
    ]);
  });
});

const signed = (name: string) => ({ url: `https://github.com/amemiya02/OpenDict/releases/download/v0.2.0/${name}`, signature: "c2ln" });

describe("updater manifest (latest.json)", () => {
  const complete = {
    version: "0.2.0",
    platforms: {
      "windows-x86_64": signed("OpenDict_0.2.0_x64-setup.exe"),
      "darwin-aarch64": signed("OpenDict_aarch64.app.tar.gz"),
      "darwin-x86_64": signed("OpenDict_x64.app.tar.gz"),
      "linux-x86_64": signed("OpenDict_0.2.0_amd64.AppImage"),
    },
  };

  it("accepts a manifest with a signed download for every platform", () => {
    expect(manifestProblems(complete, "0.2.0")).toEqual([]);
  });

  it("names every platform that is missing or unsigned, and a wrong version", () => {
    const partial = {
      version: "0.1.0",
      platforms: {
        "windows-x86_64": signed("OpenDict_0.2.0_x64-setup.exe"),
        "darwin-aarch64": { ...signed("OpenDict_aarch64.app.tar.gz"), signature: "" },
      },
    };
    expect(manifestProblems(partial, "0.2.0")).toEqual([
      "latest.json announces version 0.1.0, expected 0.2.0.",
      "latest.json has no signed update for darwin-aarch64.",
      "latest.json has no signed update for darwin-x86_64.",
      "latest.json has no signed update for linux-x86_64.",
    ]);
  });

  it("rejects something that is not a manifest at all", () => {
    expect(manifestProblems("Not Found", "0.2.0")).toEqual(["latest.json is not an updater manifest."]);
  });
});
