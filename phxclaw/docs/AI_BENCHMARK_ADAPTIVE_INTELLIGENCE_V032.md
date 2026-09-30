# PhxClaw v0.32 — AI Benchmark & Adaptive Model Intelligence

## Principle
No provider/model receives a permanent global rank. PhxClaw measures performance by task family, complexity band, dataset/scorer/environment hash and time window.

## Evidence boundary
Production profiles are derived only from append-only observations. Fixture results validate the benchmark machinery but are marked `evidence_class=fixture` and the Rust promotion gate rejects them. Adaptive routing accepts only `PromotedProfile` records, never arbitrary aggregate profiles.
Raw prompts/outputs are not persisted by default; SHA-256 identifiers bind inputs, expected contracts and outputs.

## Pipeline
`Suite -> Run -> Observation -> Aggregate Profile -> Promotion Gate -> Adaptive Route Evidence`

Promoted profiles require minimum samples, coverage, success and quality, plus regression gates. Stale profiles cannot influence adaptive routing.

## Task families
Coding, repository analysis, tool use, structured output, reasoning, vision, embeddings and general workloads. Complexity is Low/Medium/High/Extreme.

## Router composition
The v0.31 router remains the authority for privacy, capabilities, cloud permission, data controls, current catalog/health, circuit state and budget. v0.32 can only re-rank candidates that v0.31 already declared eligible.

If no fresh promoted profile exists, policy chooses between base v0.31 routing and fail-closed `profile_required` behavior. No score is invented.
