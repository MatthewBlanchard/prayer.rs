# prayer-scheduler

`prayer-scheduler` is Prayer's pure, claimable per-session action scheduler. It
orders durable `prayer-actions` envelopes but performs no I/O and executes no
actions itself.

A normal queue is protected by a generation-stamped `QueueClaim`, preventing
stale owners from appending, cancelling, or releasing work. A higher-precedence
interrupt lane can pause a running normal action until interrupt work
completes. The scheduler also records outcomes, exposes transient
`SchedulerEvent` values, and supports an intentional, versioned
`SchedulerCheckpoint` persistence boundary.

```rust
use prayer_actions::{Action, ActionEnvelope, ActionOrigin};
use prayer_scheduler::{QueueOwner, Scheduler};

let mut scheduler = Scheduler::new();
let claim = scheduler.claim(QueueOwner::Manual {
    run_id: "run-1".into(),
})?;

scheduler.append(&claim, [ActionEnvelope::new(
    "action-1",
    Action::Dock,
    ActionOrigin::Manual {
        run_id: "run-1".into(),
    },
)])?;

let next = scheduler.start_next()?;
assert!(next.is_some());
# Ok::<(), prayer_scheduler::SchedulerError>(())
```

Restore persisted state with `Scheduler::from_checkpoint`; it validates both
the scheduler and queued action schema versions.

From the repository root:

```console
cargo check -p prayer-scheduler
cargo test -p prayer-scheduler
```
