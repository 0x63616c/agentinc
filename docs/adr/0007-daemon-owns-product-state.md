# The daemon owns product state and execution

The headless Rust daemon owns the HTTP API, authorization and Postgres product records. Temporal owns durable agent execution, schedules, retries and workflow history; its databases stay separate from product tables. The GPUI app and generated CLI are clients of the same API, and the daemon can run locally or on another machine over Tailscale. Isolated worker Environments are later work.
