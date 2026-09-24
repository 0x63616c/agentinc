#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
# Debug is sufficient for a local, inspectable first increment. Pass release for optimization.
profile=${1:-debug}
case "$profile" in
  debug) cargo build --locked -p agentinc-os -p ainc-daemon ;;
  release) cargo build --locked -p agentinc-os -p ainc-daemon --release ;;
  automation)
    cargo build --locked -p agentinc-os -p ainc-daemon -p gpui-pilot-cli --features agentinc-os/automation
    profile=debug
    ;;
  *) echo 'usage: crates/ainc-mac/scripts/bundle.sh [debug|release|automation]' >&2; exit 2 ;;
esac
bundle='crates/ainc-mac/dist/AgentInc.app'
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "target/$profile/agentinc-os" "$bundle/Contents/MacOS/agentinc-os"
cp "target/$profile/aincd" "$bundle/Contents/MacOS/aincd"
codesign --force --sign - "$bundle/Contents/MacOS/aincd"
cp crates/ainc-mac/assets/AppIcon.icns "$bundle/Contents/Resources/AppIcon.icns"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>AgentInc</string>
<key>CFBundleDisplayName</key><string>AgentInc</string>
<key>CFBundleIdentifier</key><string>co.worldwidewebb.agentinc</string>
<key>CFBundleExecutable</key><string>agentinc-os</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
codesign --force --deep --sign - "$bundle"
printf 'Built %s/%s\n' "$PWD" "$bundle"
