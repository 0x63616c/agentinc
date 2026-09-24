# Use isolated Tilt and Docker Compose development stacks

Each worktree gets its own Tilt + Docker Compose stack for Postgres, Temporal and the native daemon/worker, identified by a stable hash of its canonical path. Isolate volumes, networks, namespaces, queues, ports, data and credentials; order startup by readiness rather than sleeps. This lets two worktrees run without sharing state or disrupting each other.
