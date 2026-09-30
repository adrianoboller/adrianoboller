# PhxClaw v0.33 — Continuous Model Arena

## Authority chain

```text
v0.31 safety/cost/privacy eligibility
              ↓
v0.32 promoted performance profiles
              ↓
v0.33 champion/challenger arena
              ↓
promotion recommendation
              ↓
v0.32 Promotion Gate
```

The Arena never broadens the candidate set selected by the v0.31 safety router. In production, champion and challenger must also have matching promoted v0.32 profiles; fixture arenas are explicitly segregated. A model excluded for privacy, capability, health or budget cannot be reintroduced by an A/B experiment.

## Modes

- `shadow`: champion serves the user; challenger is evaluated out-of-band when policy permits the duplicate execution.
- `canary`: a deterministic percentage receives the challenger.
- `paired`: champion serves while a paired challenger result is collected for comparison.

The signed Arena spec and challenger membership are immutable after registration; lifecycle changes are separate append-only state events.

Assignment uses arena UUID + request UUID + signed salt. The same logical request therefore stays in the same cohort.

## Evidence boundary

Fixtures prove Arena mechanics only. Production promotion recommendations require `EvidenceClass::Production`. Raw prompts and raw outputs are not stored by this subsystem by default; hashes and measurements are persisted.

## Promotion boundary

`ChallengerWins` does not directly replace the champion. After the configured number of consecutive winning windows, v0.33 may create a `PromotionRecommendation`. The recommendation explicitly requires the v0.32 Promotion Gate.

## Drift

The champion is continuously compared with its promoted v0.32 baseline. Quality/success regressions or safety violations can force fallback to the v0.31 base router. Latency/cost drift can pause the Arena without silently changing privacy or capability rules.

## No global leaderboard

A model may win one task-family/complexity arena and lose another. PhxClaw does not convert Arena data into a permanent global ranking.
