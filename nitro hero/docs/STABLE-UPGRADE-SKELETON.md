# Replacing the installed Tauri Nitro over the air

Replaces Codex's design skeleton, which was sound but left every step unbuilt. The parts
marked built exist in this repo. Nothing has been published, and `updates/latest.json` is unchanged.

## How it reaches people

The installed v0.1.72 app already polls
`https://github.com/nostalg1cc/Chatterbox/releases/latest/download/latest.json`. When it sees a
newer version, it downloads the listed installer, checks its signature against the key built into
it, runs it with `/P /R /UPDATE`, and exits. It doesn't care what the installer contains. So Hero
ships as an ordinary release to that same manifest, and no one has to reinstall anything.

Facts checked on this machine: the app lives in `%LOCALAPPDATA%\Nitro` as `nitro.exe` and
`uninstall.exe`, with the uninstall entry at `HKCU\...\Uninstall\Nitro`. Its data is separate, in
`%LOCALAPPDATA%\io.github.nostalg1cc.dislight\EBWebView`, so replacing the program leaves it alone.

## What's built

| Piece | Where | Checked |
| --- | --- | --- |
| Installer: same folder, same uninstall entry, shortcut, `/R` relaunch, waits for the old app to exit | `release/nitro.nsi` | Compiles. Not run |
| Build, with optional signing from the environment | `scripts/build-release.ps1` | See below |
| Hero's own updater: same manifest and key, signature checked before anything is saved or run | `src/updater.rs`, Settings, About | Unit tests, including tampering and the real v0.1.72 signature format |
| Version above 0.1.72 (0.2.0), built-in config and icon, no dependence on the repo | `Cargo.toml`, `build.rs` | Builds |
| Settings and sign-in carried over once, read-only | `src/migrate.rs` | Unit tests, and the format was checked against the real files |

## Carry-over

On first run Hero reads the old app's `dislight-preferences-v1` and its Supabase refresh token from
WebView storage. The settings map across, except device ids, because browser ids mean nothing to
the native engine, so devices go back to default. The token is exchanged once for a new session,
which is then sealed with DPAPI like any sign-in. If anything isn't readable, for example it's in a
compressed block, that's just defaults or a normal sign-in. The old data isn't changed or deleted.

## Status

v0.2.1 is the latest release. It fixes v0.2.0, where the sign-in carry-over never ran on first launch because startup returned early without a saved session. Hero updating itself from v0.2.0 to v0.2.1 is the first real test of its updater.

### v0.2.0

Rolled out: published as the latest GitHub release at the user's request, so installed v0.1.72 clients are offered it. To pause, mark it a pre-release again with `gh release edit v0.2.0 --repo nostalg1cc/Chatterbox --prerelease --latest=false` and make v0.1.72 latest. That doesn't undo updates already installed. The old client runs the installer with `/S /R /UPDATE /ARGS` and exits (installMode quiet, tauri-plugin-updater 2.10.1). Checked: signature against the old client's key, tampering refused, the uploaded files match the build, carry-over reads the real old-app data, and the installer against a fake old install via `/DAPP_ID=`, including a held exe, a custom install location, relaunch and uninstall. Not checked: the installer over a real v0.1.72, the old client's updater end to end, Hero updating itself. The tag points at main's HEAD, and the source of this build wasn't committed when it was published.

## Release checklist

Only when explicitly asked to release.

1. Two-person test on the release build: call audio and no static, share and watch, soundboard both
   ways. These still haven't been confirmed with a real second person.
2. Run `scripts/build-release.ps1` with the signing key in the environment.
3. **Test the installer on a machine that has v0.1.72, or a Windows VM.** It hasn't been run, because
   running it here would replace the user's own install. Confirm the old app updates through its
   real updater, one Nitro in Apps, a working shortcut and uninstall, settings and sign-in carried
   over, and that a tampered installer is refused.
4. From the installed result, confirm Hero can then update itself to a later test version. This is
   the step that can't be fixed after shipping: a broken updater strands everyone on that version.
5. Create the GitHub release with the installer, then publish `latest.json` with its version, URL and
   `.sig` contents last. Publishing it is what starts everyone updating.

## Rollback

The old app can't be remotely restored once replaced. The only way back is a higher version from
Hero's own updater, which is why step 4 matters. Pausing means not publishing, or republishing,
`latest.json`. The old app's data stays on disk, so reinstalling v0.1.72 by hand also works.

## Known gaps before release

- Installed main Nitro clients stay on v0.1.72 until they update, so Hero and Tauri users mix for a
  while. They interoperate on calls, but Tauri's shares bypass the usage budget.
- Hero has no Realtime for messages yet, so new messages arrive by 3-second polling.
- Sending and playing video in chat isn't built, and main Nitro has it. Decide whether that blocks
  replacing the app, because people would lose it.
