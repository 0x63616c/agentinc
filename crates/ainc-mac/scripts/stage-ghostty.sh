#!/bin/sh
set -eu

profile=$1
bundle=$2
package=$(CDPATH= cd -- "$(dirname "$0")/../ghostty-bridge" && pwd)
swift build --package-path "$package" --force-resolved-versions -c "$profile" --product AgentIncGhosttyBridge
products=$(swift build --package-path "$package" -c "$profile" --show-bin-path)
frameworks="$bundle/Contents/Frameworks"
resources="$bundle/Contents/Resources"
mkdir -p "$frameworks" "$resources"
cp "$products/libAgentIncGhosttyBridge.dylib" "$frameworks/"
ditto "$products/GhosttyKit_GhosttyTerminal.bundle" "$resources/GhosttyKit_GhosttyTerminal.bundle"
ghostty_resources=$(find "$resources/GhosttyKit_GhosttyTerminal.bundle" -type d -name Ghostty -print -quit)
test -n "$ghostty_resources" || { echo 'GhosttyKit resource bundle has no Ghostty directory' >&2; exit 1; }
tar -xzf "$package/ghostty-themes-1.3.1.tar.gz" -C "$ghostty_resources"
cp "$package/licenses/ghostty.txt" "$resources/GHOSTTY-LICENSE.txt"
cp "$package/licenses/ghosttykit.txt" "$resources/GHOSTTYKIT-LICENSE.txt"
