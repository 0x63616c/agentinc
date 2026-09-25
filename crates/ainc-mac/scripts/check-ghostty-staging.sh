#!/bin/sh
# Fail the native handoff if Ghostty's compiled Bundle.module cannot find its resources.
set -eu

bundle=$1
dylib="$bundle/Contents/Frameworks/libAgentIncGhosttyBridge.dylib"
resources="$bundle/Contents/Resources/GhosttyKit_GhosttyTerminal.bundle"
test -f "$dylib" && test -d "$resources" || {
  echo 'Ghostty bridge or resource bundle missing from staged app' >&2
  exit 1
}
# The native SwiftPM accessor searches App.app/ and a compile-time build path;
# neither is present in a signed release. SwiftBuild's accessor searches Resources.
if strings "$dylib" | grep -Fq 'could not load resource bundle: from'; then
  echo 'Ghostty bridge contains a build-tree-only resource accessor' >&2
  exit 1
fi
if ! strings "$dylib" | grep -Fq 'unable to find bundle named GhosttyKit_GhosttyTerminal'; then
  echo 'Unknown Ghostty resource accessor; verify its lookup path before release' >&2
  exit 1
fi
