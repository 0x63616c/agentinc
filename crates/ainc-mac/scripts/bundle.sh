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
codesign --force --sign - "$bundle/Contents/MacOS/aincd"
cp crates/ainc-mac/assets/AppIconDev.icns "$bundle/Contents/Resources/AppIcon.icns"
version=$(cargo metadata --no-deps --format-version=1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "ainc-release"))')
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
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
codesign --force --deep --sign - "$bundle"
printf 'Built %s/%s\n' "$PWD" "$bundle"
