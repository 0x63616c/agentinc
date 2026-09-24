# macOS release runner

`homelab-mini` is a repository-scoped GitHub Actions runner v2.337.0 on Calum's Mac mini
(`ssh calum@homelab`). The runner application is in `~/actions-runner`; its job
workspace is `~/actions-runner/_work`. It has the labels `self-hosted`, `macOS`,
`ARM64`, and `agentinc-release`. The mini has eight CPU cores and 8 GiB of RAM
and also runs Home Assistant and other services. It runs macOS 15.6 with
Xcode 26.3 at `/Applications/Xcode-26.3.0.app`.

## Release path

To bump the product version, edit the single `version` in the root
`Cargo.toml` under `[workspace.package]`, run `cargo xtask generate`, then
commit and merge to `main`. CI checks the generated API and client files.
The SDK crates keep independent versions. A product version change starts
Distribution after merge; other pushes do not.

[`Distribution`](../.github/workflows/release.yml) starts on a product version
change pushed to `main`. Its Ubuntu prepare job checks the version, then the
Mac runs `cargo xtask release` at that exact commit. The unsigned archive moves
to the Ubuntu job through a one-day Actions artifact. Only Ubuntu receives the
Apple and update-signing secrets; it signs, notarizes, staples, and publishes.
The Mac job has read-only repository permission and checkout does not persist
its token. No pull-request event invokes the Mac job. An owner
`workflow_dispatch` may run the path in `test=true` mode, which leaves a draft
and uses a throwaway update key. Use the branch as `--ref` for validation so
the draft guard applies even if an input is mistaken.

The older handoff is still available: run `cargo xtask release --upload` on a
Mac at the desired commit, or manually upload `unsigned.tar.gz` to a draft
`build-COMMIT` release. Dispatch Distribution with that `commit` and
`build=false`. `--upload` itself dispatches this fallback for compatibility.

The Mac job caps Cargo at four build jobs, has a three-hour timeout, and keeps
one Cargo target directory and its toolchain under the runner workspace for
reuse. It downloads checksum-pinned Temporal 1.9.1 and Codex 0.155.1 into the
same workspace. It removes per-release bundles and archives after artifact
upload. The Linux job waits for the exact commit's Rust CI check, then verifies
the handoff's commit, version, and file hashes before signing. Check free space
in `~/actions-runner/_work`; the persistent
`.cargo-target` may be removed **only while the runner is idle** if space is
needed, at the cost of a full rebuild.

The native packaging step also runs `crates/ainc-mac/scripts/stage-ghostty.sh`.
It uses the runner's Xcode Swift 6 toolchain and network access to resolve the
exact Swift package revisions in `crates/ainc-mac/ghostty-bridge/Package.resolved`;
it does not build Ghostty from source or require Zig. It stages GhosttyKit's
prebuilt libghostty, its runtime resources, the checked-in built-in themes and
license notices before the unsigned handoff is inventoried. See
`crates/ainc-mac/ghostty-bridge/README.md` for provenance and checksum. The
signed app's minimum macOS version remains 15.0.

## Connection, credentials, and restart

The listener makes outbound HTTPS long-poll connections to GitHub for assignments and
downloads. It needs no inbound port or router forwarding. Initial registration
used a short-lived repository registration token, exchanged for the runner's
own credentials in `~/actions-runner/.credentials*`. Those files are private
machine credentials: do not copy or publish them. Workflow jobs do not use the
registration token. [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)
describes the connection and automatic runner updates.

The user LaunchAgent is
`~/Library/LaunchAgents/actions.runner.0x63616c-agentinc.homelab-mini.plist`.
It runs at login with `RunAtLoad` and `KeepAlive`, `Nice=10`, and low-priority
I/O. The mini auto-logs in as `calum`, has sleep disabled, and restarts after
power loss. Together these restart the listener after a crash or machine
reboot, after login. A failed or interrupted build must be rerun; the release
script and Linux distribution are designed for an exact-commit retry.

## Status and resource use

```sh
gh api repos/0x63616c/agentinc/actions/runners --jq '.runners[] | {name,status,busy,labels}'
ssh calum@homelab 'cd ~/actions-runner && ./svc.sh status'
ssh calum@homelab 'ps -axo pid,rss,%cpu,comm | grep Runner.Listener | grep -v grep'
ssh calum@homelab 'df -h ~/actions-runner/_work; du -sh ~/actions-runner/_work/* 2>/dev/null'
```

The [2026-09-24 draft validation run](https://github.com/0x63616c/agentinc/actions/runs/36006823656)
measured the following on this mini:

| State | CPU | RSS | Elapsed |
| --- | ---: | ---: | ---: |
| Idle listener (`ps` sample) | 0.0% | 71,184 KiB (about 70 MiB) | — |
| Native release build (sampled peak of build process tree) | 400% (four cores) | 1,953 MiB | 341 seconds |
| Warm rebuild in the [final-code draft run](https://github.com/0x63616c/agentinc/actions/runs/36008543796) | 400% (four cores) | 1,345 MiB | 86 seconds |

The build script samples every two seconds. A short peak may be missed, and
summed process RSS can double-count shared pages. After the build, the
persistent target was 3.4 GiB, Cargo cache 1.0 GiB, Rust toolchain 466 MiB,
and pinned tools 372 MiB; the Mac had 50 GiB free. The job removed its
per-release bundle and archive. The Ubuntu job passed and left five signed
assets in a draft; it did not publish a release or create a public tag. The
final-code draft run also passed on both machines and produced five draft
assets without creating a tag.

## Maintenance and removal

From the mini, run `cd ~/actions-runner && ./svc.sh status` (the script expects
that working directory). The LaunchAgent logs are under
`~/Library/Logs/actions.runner.0x63616c-agentinc.homelab-mini/`. Runner
software updates itself by default; check its version and GitHub's runner
status after an update. macOS, Xcode, Rust, and pinned bundle tools are separate
from runner self-updates. The build job fetches the pinned Rust 1.98.1
toolchain, Protobuf compiler, and bundle tools into its own workspace. See
[GitHub's update policy](https://docs.github.com/en/actions/reference/runners/self-hosted-runners#runner-software-updates-on-self-hosted-runners).

For a re-registration or uninstall, first stop the service from the runner
directory, then remove the runner registration using a fresh one-hour removal
token. An admin can get registration and removal tokens in repository Settings
→ Actions → Runners or with the [repository runner API](https://docs.github.com/en/rest/actions/self-hosted-runners).
The following is a maintenance procedure, **not part of a release job**:

```sh
cd ~/actions-runner
./svc.sh stop
./svc.sh uninstall
remove_token=$(gh api -X POST repos/0x63616c/agentinc/actions/runners/remove-token --jq .token)
./config.sh remove --token "$remove_token"
unset remove_token

# Only when re-registering:
registration_token=$(gh api -X POST repos/0x63616c/agentinc/actions/runners/registration-token --jq .token)
./config.sh --url https://github.com/0x63616c/agentinc --token "$registration_token" --name homelab-mini --labels agentinc-release --work _work
unset registration_token
./svc.sh install
./svc.sh start
./svc.sh status
```

For a full uninstall, omit the re-registration block and remove the runner
directory only after confirming the runner is absent from the API. Review any
cached release inputs or targets before deleting them.

## Security boundary

This is a public repository, so untrusted pull-request code must never be
scheduled on the personal Mac. The workflow has only `push` to `main` and
owner-only `workflow_dispatch` triggers; no other workflow targets these
labels. Outside-contributor workflow runs require repository approval. The
Mac job gets no signing, notarization, or update-signing secrets. Avoid
adding `pull_request`/`pull_request_target`, broader labels, or a repository
secret to it. Review any workflow change before merging, because a trusted
`main` workflow can execute code on the mini.
