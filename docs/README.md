# readsb-rs: Mode-S/ADSB/TIS Message Decoder — Rust Rewrite

## Plan Index

This document indexes the 3 sub-plans for rewriting readsb (45K LOC C ADS-B receiver) in Rust.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Sub-Plan 1: Core Computation                 │
│  Phases 1-3: CRC, CPR, Mode S parsing                              │
│  Pure Rust, zero I/O, zero FFI                                      │
│  docs/plans/01-core-computation.md                                  │
└────────────────────────────────┬────────────────────────────────────┘
                                 │ depends on
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Sub-Plan 2: State Tracking + DSP               │
│  Phases 4-5: Aircraft tracking, demodulation, SDR abstraction       │
│  Pure Rust + FFI boundary at librtlsdr                              │
│  docs/plans/02-tracking-demod.md                                    │
└────────────────────────────────┬────────────────────────────────────┘
                                 │ depends on
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Sub-Plan 3: Application Layer                  │
│  Phases 6-8: Networking, output, main loop, Docker/CI               │
│  Rust + tokio + serde + clap                                        │
│  docs/plans/03-application-layer.md                                 │
└─────────────────────────────────────────────────────────────────────┘
```

## Quick Reference

| Sub-Plan | Phases | Files Created | Lines of Code | Key Crates | Testable Independently? |
|----------|--------|---------------|---------------|------------|------------------------|
| 1: Core | 1-3 | 20 | ~1,500 | bitflags | Yes — `cargo test` |
| 2: Tracking + DSP | 4-5 | 12 | ~1,300 | async-trait | Yes — `cargo test` (depends on Sub-Plan 1 types) |
| 3: Application | 6-8 | 16 | ~800 | tokio, serde, clap | Partial (parser unit tests yes, full integration no) |

## Original C Reference

The C codebase lives at `/CODE/readsb/` — 45,767 lines, GPL v3. Cross-validation commands:
```bash
cd /CODE/readsb
make crctests && ./crctests   # CRC reference
make cprtests && ./cprtests   # CPR reference
```

## Execution Options

Each sub-plan is designed for agentic execution. Two approaches per sub-plan:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks
2. **Inline Execution** — batch execution with checkpoints

Start with Sub-Plan 1 (`docs/plans/01-core-computation.md`) — the other two depend on it.
