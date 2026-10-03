#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
# Debug is sufficient for a local, inspectable first increment. Pass release for optimization.
profile=${1:-debug}
case "$profile" in
  debug) cargo build --locked -p agentinc-os -p ainc-daemon -p ainc-release -p ainc-cli --bins ;;
  release) cargo build --locked -p agentinc-os -p ainc-daemon -p ainc-release -p ainc-cli --bins --release ;;
  automation)
    cargo build --locked -p agentinc-os -p ainc-daemon -p ainc-release -p ainc-cli --bins -p gpui-pilot-cli --features agentinc-os/automation
    profile=debug
    ;;
  *) echo 'usage: crates/ainc-mac/scripts/bundle.sh [debug|release|automation]' >&2; exit 2 ;;
esac
bundle='crates/ainc-mac/dist/AgentInc Dev.app'
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "target/$profile/agentinc-os" "$bundle/Contents/MacOS/AgentInc"
cp "target/$profile/aincd" "$bundle/Contents/MacOS/aincd"
cp "target/$profile/ainc-update" "$bundle/Contents/MacOS/ainc-update"
cp "target/$profile/ainc" "$bundle/Contents/MacOS/ainc"
# Stage the Ghostty bridge only when its inputs changed since the last bundle of this profile.
bridge=crates/ainc-mac/ghostty-bridge
stamp="crates/ainc-mac/dist/.ghostty-stamp-$profile"
inputs=$(find "$bridge/Package.swift" "$bridge/Package.resolved" "$bridge/Sources" "$bridge/licenses" "$bridge/ghostty-themes-1.3.1.tar.gz" crates/ainc-mac/scripts/stage-ghostty.sh crates/ainc-mac/scripts/check-ghostty-staging.sh -type f | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256 | cut -d ' ' -f 1)
if [ -f "$bundle/Contents/Frameworks/libAgentIncGhosttyBridge.dylib" ] && [ "$(cat "$stamp" 2>/dev/null)" = "$inputs" ]; then
  echo 'Ghostty bridge unchanged; keeping the staged copy'
else
  rm -f "$stamp"
  crates/ainc-mac/scripts/stage-ghostty.sh "$profile" "$bundle"
  printf '%s\n' "$inputs" > "$stamp"
fi
codesign --force --sign - "$bundle/Contents/MacOS/aincd"
cp crates/ainc-mac/assets/AppIconDev.icns "$bundle/Contents/Resources/AppIcon.icns"
# The product version is the one `[workspace.package]` line in the root Cargo.toml.
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
test -n "$version" || { echo 'no workspace version in Cargo.toml' >&2; exit 1; }
build=$(git rev-list --count HEAD)
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>AgentInc</string>
<key>CFBundleDisplayName</key><string>AgentInc</string>
<key>CFBundleIdentifier</key><string>co.worldwidewebb.agentinc.dev</string>
<key>CFBundleExecutable</key><string>AgentInc</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version-dev</string>
<key>CFBundleVersion</key><string>$build</string>
<key>NSHumanReadableCopyright</key><string>Copyright © 2026 Calum Webb</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
codesign --force --deep --sign - "$bundle"
printf 'Built %s/%s\n' "$PWD" "$bundle"
