# ADR-0067 — BPM Editor + Crash-safe Replay

F13 uses a decoupled bpmn-js modeler for BPMN 2.0 authoring and a native Rust/PostgreSQL runtime for execution. UI does not own execution state. Runtime claims tokens with `FOR UPDATE SKIP LOCKED`, increments fencing tokens, writes state mutation and event journal in one transaction, and replays strictly by event sequence.

The web editor pins bpmn-js 18.30.1; redistribution/build must preserve upstream bpmn.io license terms.
