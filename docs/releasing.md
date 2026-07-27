# Releasing

Distribution is currently **unsigned** and targets **macOS (universal), Windows, and
Linux**. CI/CD is GitHub Actions.

## Cut a release

1. Bump the version in `src-tauri/tauri.conf.json` (keep `package.json` in sync if you
   track it there).
2. Tag and push:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. The **[Release workflow](../.github/workflows/release.yml)** runs a 3-platform matrix
   via [`tauri-action`](https://github.com/tauri-apps/tauri-action) and attaches the
   installers to a **draft** GitHub Release:
   - macOS — universal `.dmg` / `.app`
   - Windows — `.msi` / `.exe`
   - Linux — `.deb` / `.AppImage`
4. Review the draft, then publish it.
5. The macOS job prints the DMG's **SHA256** to its run summary — copy it for the cask.

## Homebrew (macOS) — custom tap

Homebrew installs GUI apps as **casks**. We host our own tap so we control releases.
The canonical cask lives at [`packaging/homebrew/openbot.rb`](../packaging/homebrew/openbot.rb).

### One-time: create the tap repo

Create a public repo named exactly `homebrew-openbot` under `marcin-rogalski`, then:

```sh
mkdir Casks
cp <this-repo>/packaging/homebrew/openbot.rb Casks/openbot.rb
git add Casks/openbot.rb && git commit -m "openbot 0.1.0" && git push
```

### Per release: bump the cask

Edit `Casks/openbot.rb` in the tap repo:

- `version` → the new version
- `sha256` → the value from the Release run summary
- confirm the DMG filename in `url` matches the actual Release asset

Commit and push. Users then get it with:

```sh
brew tap marcin-rogalski/openbot
brew install --cask openbot
brew upgrade --cask openbot   # later releases
```

Because the app is unsigned, the cask's `caveats` tell users how to clear the Gatekeeper
quarantine on first launch.

## Signing & notarization (when you're ready)

To remove the macOS Gatekeeper warning and become eligible for the official
`homebrew-cask`:

1. Get an Apple Developer ID ($99/yr).
2. Add repo secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
   `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`.
3. Uncomment the `env:` block in `.github/workflows/release.yml`.

No other pipeline changes are needed — the same workflow signs + notarizes when the
secrets are present.

## Auto-update (optional, later)

Tauri's updater plugin can point installed apps at a `latest.json` on the GitHub Release
so they self-update without going back through brew. This needs a signing keypair
(`TAURI_SIGNING_PRIVATE_KEY`) and the updater plugin wired into the app.

## Bundle identifier

The app identifier is `pl.marcinrogalski.openbot` (in `src-tauri/tauri.conf.json`). It
determines the app's data directory (bots, config, knowledge index), so changing it later
relocates that data — existing local data won't carry over automatically. The cask's
`uninstall quit:` value must match it.
