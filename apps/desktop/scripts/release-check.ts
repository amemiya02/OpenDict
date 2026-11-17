// Checks the release workflow (.github/workflows/release.yml) runs before building and before publishing.
//
//   node scripts/release-check.ts preflight <tag>                  the tag names the app version; the updater key is set
//   node scripts/release-check.ts manifest <latest.json> <version> every platform has a signed update
//
// Exits 1 and prints each problem when a check fails.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The platforms OpenDict ships, as keys in the updater's latest.json. */
export const PLATFORMS = ["windows-x86_64", "darwin-aarch64", "darwin-x86_64", "linux-x86_64"] as const;

interface ReleaseConfig {
  version: string;
  plugins: { updater: { pubkey: string } };
}

/** Problems that must stop a release from being built for `tag`, given the app's tauri.conf.json. */
export function preflightProblems(tag: string, config: ReleaseConfig): string[] {
  const problems: string[] = [];
  if (tag !== `v${config.version}`) {
    problems.push(
      `Tag ${tag} does not match the app version ${config.version} in tauri.conf.json; expected tag v${config.version}.`,
    );
  }
  if (!config.plugins.updater.pubkey.trim()) {
    problems.push("plugins.updater.pubkey in tauri.conf.json is empty: set up the updater key first (docs/releasing.md).");
  }
  return problems;
}

/** Problems that must stop a release from being published, given the latest.json its builds uploaded. */
export function manifestProblems(manifest: unknown, version: string): string[] {
  if (typeof manifest !== "object" || manifest === null || !("platforms" in manifest)) {
    return ["latest.json is not an updater manifest."];
  }
  const { version: announced, platforms } = manifest as { version?: unknown; platforms: Record<string, unknown> };
  const problems: string[] = [];
  if (announced !== version) problems.push(`latest.json announces version ${String(announced)}, expected ${version}.`);
  for (const platform of PLATFORMS) {
    const entry = platforms[platform] as { url?: unknown; signature?: unknown } | undefined;
    if (!entry || typeof entry.url !== "string" || !entry.url || typeof entry.signature !== "string" || !entry.signature) {
      problems.push(`latest.json has no signed update for ${platform}.`);
    }
  }
  return problems;
}

function readJson(path: string): unknown {
  return JSON.parse(readFileSync(path, "utf8"));
}

function main([command, ...args]: string[]): number {
  let problems: string[];
  if (command === "preflight" && args.length === 1) {
    const config = readJson(fileURLToPath(new URL("../src-tauri/tauri.conf.json", import.meta.url)));
    problems = preflightProblems(args[0]!, config as ReleaseConfig);
  } else if (command === "manifest" && args.length === 2) {
    let manifest: unknown;
    try {
      manifest = readJson(args[0]!);
    } catch {
      manifest = null;
    }
    problems = manifestProblems(manifest, args[1]!);
  } else {
    console.error("usage: release-check.ts preflight <tag> | manifest <latest.json> <version>");
    return 2;
  }
  for (const problem of problems) console.error(`::error::${problem}`);
  return problems.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
