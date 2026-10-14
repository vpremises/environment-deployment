# Environment deployment

Typed environment configuration, reconciliation planning and provider rendering for vPremises.

| Location | Responsibility |
| --- | --- |
| `crates/environment-deployment` | Closed configuration types and schema, profile resolution, desired-state digest, plan/receipt ledger and maintenance-window calculation |
| `crates/incus-renderer` | Incus-specific validation and OpenTofu JSON generation using the same resolved environment type |
| `crates/environment-deployment/examples/reconciliation.rs` | Synthetic in-memory plan, receipt, replay rejection and rollback planning |

Contracts and implementation live together by feature. The renderer consumes the core's closed types directly; there is no parallel protocol or compatibility adapter. The receipt schema is next to its owning implementation.

```sh
cargo +1.97 test --locked --workspace --all-targets
cargo +1.97 run --locked -p vpremises-environment-deployment --example reconciliation
```

This source-only extraction contains synthetic fixtures and no private repository history, operational profiles, registrations or secrets. HAT Inc. has authorized publication of the selected common code under Apache-2.0; private configuration remains separately maintained.

Operator commands belong in `vpremises/environment-operations`. Actual profiles and operation data belong in private `vpremises/environment-configurations`. This library calculates plans and renders configuration; it does not connect to providers, apply plans, sign an operational catalog or enable a timer. Live execution requires a separately established authorization and deployment path.

Workspace crates currently use `publish = false`; public Git source is the distribution path until a separate crates.io release has been reviewed. Configuration and receipt URIs remain identifiers for the current schema, rather than a legacy compatibility layer.

[OSS responsibility and source completion](docs/oss-source-contract.md) defines the supported package boundary and required acceptance. It does not declare unexecuted source, distribution or production checks complete.

Dependency updates: [repository policy](docs/dependency-updates.md).
