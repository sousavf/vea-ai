# vea-policy

`vea-policy` is Vea's deterministic authorization boundary for model-proposed actions.

It provides:

- strict typed action, capability-scope, destination, policy, and approval wire forms;
- independently issued host capability grants—an action can never grant itself authority;
- portable path, identifier, collection, digest, size, and lifetime validation;
- deterministic integer-only canonical JSON with domain-separated SHA-256 digests;
- complete effective-policy and broker-state binding;
- approval verification against fresh action, policy, project, resource, and destination state;
- explicit unused/consumed/revoked approval status for later atomic store enforcement.

The crate is intentionally independent of Tauri, SQLite, filesystem, Git, URL resolution, credentials, and wall-clock APIs. Owning Rust brokers must supply fresh opaque state digests immediately before execution. The durable store must atomically consume an approval when recording an authorized side effect.
