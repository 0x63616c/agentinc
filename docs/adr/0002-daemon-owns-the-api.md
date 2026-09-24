# A daemon owns the API; the Mac app is one client

AgentInc runs as a headless Rust daemon that serves an HTTP API described by an OpenAPI spec and hosts the durable agent work. The native Mac app is a client of that API, and agents use the same API as their tools. We chose this over a single app binary because "anything a human can do in the UI, an agent can do through the same tools" makes the API the product, work keeps running when the window closes, and other machines can join later.
