# GPUI source provenance

`gpui/` contains only the core crate source/resources from Zed revision
`4c902c9db22a82f5f3a14c02442e7f60ec40d9c8`, plus `gpui-pilot.patch`.
It remains Apache-2.0 licensed (`gpui/LICENSE-APACHE`). Examples and dependency
crate test runners are omitted; application tests exercise the seam.

`scripts/vendor-pilot-gpui.py` reproduces this directory from that exact upstream
checkout and expands workspace dependency declarations into a standalone
manifest. Sibling Zed dependencies still resolve to that same revision.
No Cargo registry/git cache source is modified.

The source patch changes only three files; its automation seam is gated by the
nondefault `pilot` feature, and the layout bounds probe by `test-support`.
The transport/CLI/app logic lives outside this directory. Review the patch
alongside the upstream source; do not edit generated vendor files without
also updating the patch and testing regeneration.
