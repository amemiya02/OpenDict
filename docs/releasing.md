# Releasing OpenDict

Pushing a tag `vX.Y.Z` runs `.github/workflows/release.yml`. It builds installers for Windows, macOS and Linux, signs the update packages, and publishes a GitHub Release with the updater's `latest.json`. Installed copies of OpenDict check that file when they start, offer the new version in the status line, and install it when the reader clicks **Install and restart**.

Nothing is code-signed with a paid certificate (founding decision, see `docs/roadmap.md`). The updater has its own signature, described below, which is free and is what stops anyone but the maintainer from shipping an update.

## One-time setup

Do this once, before the first release. The release workflow refuses to run until it is done.

1. **Generate the updater key** on your own machine, from `apps/desktop`:

   ```bash
   npx tauri signer generate -w ~/.tauri/opendict.key
   ```

   Choose a password when asked. This writes the private key `~/.tauri/opendict.key` and the public key `~/.tauri/opendict.key.pub`.

2. **Back up both files and the password** somewhere safe outside the repository (a password manager is ideal). If the private key or its password is lost, installed copies can never update again; every reader would have to reinstall by hand.

3. **Put the public key in the app.** Copy the whole contents of `opendict.key.pub` (one base64 line) into `plugins.updater.pubkey` in `apps/desktop/src-tauri/tauri.conf.json`, replacing the empty string, and commit it. The public key is not secret.

4. **Add the private key and password as repository secrets**, from the repository root:

   ```bash
   gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/opendict.key
   gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
   ```

   In PowerShell, `<` does not work and piping the file (`Get-Content … | gh secret set …`) adds a byte-order mark that breaks signing ("Invalid symbol 239, offset 0"). Let cmd redirect the file instead:

   ```powershell
   cmd /c "gh secret set TAURI_SIGNING_PRIVATE_KEY < %USERPROFILE%\.tauri\opendict.key"
   ```

   (or in the GitHub web interface: Settings → Secrets and variables → Actions). The secret holds the contents of the key file, not its path.

5. **Let workflows write releases.** The workflow asks for `contents: write`. If the repository's Settings → Actions → General → Workflow permissions is set to read-only and the release job fails with a 403, switch it to "Read and write permissions".

Never commit the private key, and never paste it into an issue, a log or a chat.

## Making a release

1. Set the new version in `apps/desktop/src-tauri/tauri.conf.json` (`version`; this is the one the app and the updater use). Keep `apps/desktop/package.json` and `apps/desktop/src-tauri/Cargo.toml` in step.
2. Commit, then tag and push the tag:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. The workflow then:
   - **preflight**: checks the tag is `v` + the version in `tauri.conf.json`, the public key is set and the private key secret exists; creates a draft release for the tag;
   - **build**: builds on four runners (Windows x64 NSIS installer; macOS Apple silicon and Intel `.dmg` plus the update `.app.tar.gz`; Linux x64 AppImage and `.deb`), signs the update packages, uploads everything and merges each platform into `latest.json`;
   - **publish**: checks `latest.json` has a signed update for all four platforms, then publishes the release and marks it "latest".

   If a build fails, the release stays a draft and no installed copy sees it. Re-run the failed jobs from the Actions page; the draft is reused.

### Pre-release tags

A tag with a pre-release suffix (`v0.2.0-rc.1`) goes through the same path and is **published as an ordinary release**, not marked as a GitHub pre-release. That is deliberate: the updater reads `releases/latest/download/latest.json`, and GitHub's "latest" skips releases marked as pre-releases, so a marked one could never be offered as an update. Semantic versioning orders `0.2.0-rc.1 < 0.2.0-rc.2 < 0.2.0`, so testers move on to the final release automatically.

### Testing the whole path with a pre-release

To prove the release and update path end to end (the last acceptance check of the release ticket):

1. Set the version to `0.1.0-rc.1`, commit, tag `v0.1.0-rc.1`, push. Wait for the workflow to publish.
2. Install `0.1.0-rc.1` from the release on each platform you can (expect the first-launch warnings described in the README).
3. Set the version to `0.1.0-rc.2`, commit, tag `v0.1.0-rc.2`, push. Wait for the workflow to publish.
4. Start the installed `0.1.0-rc.1`. The status line should say "OpenDict 0.1.0-rc.2 is available." Click **Install and restart**. OpenDict should download, install and come back as `0.1.0-rc.2`.

## Building locally

`bundle.createUpdaterArtifacts` is on, so a full `npx tauri build` signs update packages and needs the private key in the environment:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/opendict.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD='…'
npx tauri build
```

Without the key, build without bundling (`npx tauri build --no-bundle`, what CI's smoke test does) or switch the updater artifacts off for that build: `npx tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'`.

## How the pieces fit

| Piece | Where |
|---|---|
| Updater endpoint, public key, `requireSignedVersion` | `plugins.updater` in `apps/desktop/src-tauri/tauri.conf.json` |
| Permission for the window to check, download, install and restart | `apps/desktop/src-tauri/capabilities/default.json` (`updater:default`, `process:allow-restart`) |
| The check on start and the status-line notice | `apps/desktop/src/components/UpdateNotice.tsx` |
| The release workflow | `.github/workflows/release.yml` |
| Tag, key and `latest.json` checks the workflow runs | `apps/desktop/scripts/release-check.ts` |

`requireSignedVersion` makes the app reject an update whose signature was made for a different version than the one `latest.json` announces, which stops a tampered `latest.json` from downgrading readers to an old, validly signed release. Every release from the first one is signed with the version recorded, so it has been on from the start.
