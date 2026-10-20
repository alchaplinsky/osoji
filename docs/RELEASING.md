# Releasing Osoji

Osoji ships as a signed + notarized **universal** macOS app and as a Linux
AppImage and `.deb`, distributed via public GitHub Releases, with an in-app
auto-updater. This doc covers the one-time setup and the per-release procedure.

## Overview

```
bump version → run Release workflow (manual) → macOS job builds/signs/notarizes
            → PUBLISHED GitHub release (.dmg + .app.tar.gz + .sig + latest.json)
            → Linux job adds .AppImage + .sig + .deb, merges them into latest.json
            → installed apps fetch latest.json on launch → toast → restart
```

- **Build:** `universal-apple-darwin` (single binary for Apple Silicon + Intel)
  on macOS; `x86_64` AppImage and `.deb` on Linux (see [Linux](#linux) below).
- **Sign:** Developer ID Application cert.
- **Notarize + staple:** handled automatically by `tauri-apps/tauri-action`.
- **Update artifacts:** `createUpdaterArtifacts: true` emits `<app>.app.tar.gz`
  and a `.sig` signed with the Osoji minisign key; on Linux the AppImage itself
  is signed.
- **Endpoint:** the repo is public, so the app reads the manifest straight from
  GitHub — `https://github.com/alchaplinsky/osoji/releases/latest/download/latest.json`
  (`plugins.updater.endpoints` in `src-tauri/tauri.conf.json`). No proxy needed.

## One-time setup

### 1. Updater signing keypair

A minisign keypair signs every update artifact; the app verifies it with the
public key embedded in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

A keypair was generated for Osoji and the **public** key is already committed.
The **private** key lives outside the repo at `~/.tauri/osoji.key`. To
regenerate (e.g. to set a password), run:

```sh
bun run tauri signer generate -w ~/.tauri/osoji.key
```

Then update `plugins.updater.pubkey` in `tauri.conf.json` with the new
`~/.tauri/osoji.key.pub` contents. **If you lose the private key, existing
installs can no longer auto-update** — they'd need a fresh manual install.

### 2. GitHub repository secrets

Set these under **Settings → Secrets and variables → Actions**:

| Secret | How to produce it |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of `~/.tauri/osoji.key`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for that key (empty if generated without one). |
| `APPLE_CERTIFICATE` | Export your "Developer ID Application" cert + key from Keychain as a `.p12`, then `base64 -i cert.p12 \| pbcopy`. |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting the `.p12`. |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: <Name> (<TEAM_ID>)` (from `security find-identity -v -p codesigning`). |
| `APPLE_ID` | Apple ID email of the developer account. |
| `APPLE_PASSWORD` | An **app-specific password** (appleid.apple.com → Sign-In and Security → App-Specific Passwords). Not your account password. |
| `APPLE_TEAM_ID` | Your Apple Developer Team ID. |

The Apple secrets are the same ones Fletch uses; if both apps are released from
the same developer account, copy them over.

## Per-release procedure

1. **Bump the version** in three files so they agree. `scripts/check-version.sh`
   asserts it, in CI on every PR and again as the first step of the Release
   workflow, so a mismatch fails before anything is built:
   - `src-tauri/tauri.conf.json` → `version` — the authority: the release
     tag/name (`v__VERSION__`) comes from it, and it is what the updater
     compares against
   - `src-tauri/Cargo.toml` → `[workspace.package] version` — every crate
     inherits it, so it is what the binary reports
   - `package.json` → `version`

   Run `./scripts/check-version.sh` before opening the PR: it prints the agreed
   version, or names the file that disagrees.
2. Merge that to `main`.
3. Run the **Release** workflow: GitHub → Actions → *Release* → *Run workflow*.
4. Wait for it to finish. The macOS job creates a **published** release
   `Osoji v<version>` with the `.dmg`, `.app.tar.gz`, `.app.tar.gz.sig`, and
   `latest.json`; the Linux job then adds the `.AppImage`, its `.sig` and the
   `.deb`; a last job fills the body with auto-generated release notes (shown
   in the update toast). The release is public from the moment the macOS job
   publishes it, so for a few minutes it lists macOS assets only.

Installed apps pick the new version up on their next launch (or via
*Osoji → Check for Updates…*).

## Linux

The `release-linux` job runs on `ubuntu-22.04` (an AppImage runs on distros
with a glibc at least as new as the one it was built against, so the older
runner widens where it runs), installs the Tauri system packages, and runs
`tauri-apps/tauri-action` with `--bundles appimage,deb`. It uses the same
`TAURI_SIGNING_PRIVATE_KEY` secrets as macOS and no Apple ones.

It waits for the macOS job (`needs: release`): tauri-action writes
`latest.json` by downloading the one already on the release, adding its own
platform entries and uploading the result, with nothing to stop two jobs doing
that at once and one overwriting the other's entries. The merged manifest ends
up with `darwin-aarch64` / `darwin-x86_64` pointing at the `.app.tar.gz` and
`linux-x86_64` pointing at the AppImage (the `.deb` gets a `linux-x86_64-deb`
entry nobody reads).

**The updater serves AppImage installs only.** An AppImage is one file the
updater can swap in place. A `.deb` would have it run `dpkg` through `pkexec`
and ask for the root password, which a disk cleaner should not do, so the app
does not register the updater plugin unless it runs as an AppImage (the
`APPIMAGE` environment variable, `can_self_update` in `src-tauri/src/lib.rs`),
and the startup check asks `updater_available` before doing anything. `.deb`
users update by downloading and installing the next `.deb`. No `.rpm` is built
until something tests it.

## Local signed build

To produce a signed + notarized universal build on your Mac (no CI):

```sh
cp .env.example .env   # then fill in the values
./scripts/release-local.sh
```

Artifacts land in `src-tauri/target/universal-apple-darwin/release/bundle/`.

Note: with `createUpdaterArtifacts` on, a plain `bun tauri build` also needs
`TAURI_SIGNING_PRIVATE_KEY` in the environment or it stops with "A public key
has been found, but no private key". Dev builds (`bun tauri dev`) are unaffected.

## How the in-app updater behaves

`src/lib/autoUpdate.ts`, driven by `src/hooks/useAutoUpdate.ts`:

- **Startup** — silent check once at launch. No-op in dev builds and in
  installs that cannot update themselves (a Linux `.deb`, see above). If a newer
  release exists, it is downloaded and staged in the background; an
  "Update ready" toast (bottom-right) then offers *Restart now* / *Skip for now*.
  A skipped update still applies on the next launch. Any failure (offline,
  GitHub down) is logged and swallowed so it never blocks startup.
- **On demand** — *Osoji → Check for Updates…* in the app menu. Reports every
  outcome as a toast: checking → up to date / download progress → update ready /
  check failed.

Capabilities required (`src-tauri/capabilities/default.json`):
`updater:default`, `process:allow-restart`.
