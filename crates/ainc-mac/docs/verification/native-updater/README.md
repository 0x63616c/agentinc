# Native updater visual check

The baseline [software-update settings](before-settings.png) are the previously
captured GPUI controls. The old separate update dialog was not captured with a
release manifest. The new [offer](offer.png) and [download progress](progress.png)
show the AppKit windows using the synthetic manifest in
`../../../tests/fixtures/update-manifest.json`.

Run the smoke from the workspace root using the command in
`../../../README.md`. It opens each window, captures its own AppKit content view
at Retina scale, and exits without checking a feed, downloading, or installing.
This capture method needs no Screen Recording permission; it excludes the
system titlebar and WindowServer shadow. The smoke ran from an ad hoc signed
AgentInc bundle in dark appearance. The offer and progress captures were
visually inspected at 1240×900 and 940×360 pixels.
