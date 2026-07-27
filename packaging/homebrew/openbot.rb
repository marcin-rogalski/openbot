# Homebrew Cask for openbot.
#
# This is the canonical copy. It is published by copying it into the tap repo
# `marcin-rogalski/homebrew-openbot` at `Casks/openbot.rb` and updating
# `version` + `sha256` for each release (the Release workflow prints the DMG's
# SHA256 to its job summary).
#
# NOTE: openbot is currently distributed UNSIGNED. The `caveats` below tell the
# user how to clear the Gatekeeper quarantine on first launch.
cask "openbot" do
  version "0.1.0"
  sha256 "REPLACE_WITH_DMG_SHA256"

  # Verify this filename against the actual Release asset — the universal DMG
  # produced by `--target universal-apple-darwin`.
  url "https://github.com/marcin-rogalski/openbot/releases/download/v#{version}/openbot_#{version}_universal.dmg"
  name "openbot"
  desc "Local-LLM-powered Discord bots"
  homepage "https://github.com/marcin-rogalski/openbot"

  app "openbot.app"

  uninstall quit: "pl.marcinrogalski.openbot"

  caveats <<~EOS
    openbot is not notarized yet. If macOS blocks it on first launch, run:

      xattr -dr com.apple.quarantine "#{appdir}/openbot.app"

    or right-click the app in Finder and choose Open.
  EOS
end
