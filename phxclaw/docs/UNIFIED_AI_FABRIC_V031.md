# Unified AI Fabric v0.31

## Pipeline

`Task -> Privacy Gate -> Capability/Complexity Gate -> Freshness Gate -> Budget Gate -> Circuit Gate -> Deterministic Ranking -> Provider -> Evidence`

### Privacy

`restricted`: local-only by default. `confidential`: cloud requires explicit per-request authorization when policy says so. Provider metadata may advertise controls such as `zdr`, region or enterprise agreement, but PhxClaw never infers them from provider name.

### Dynamic catalog

No model name, price or performance score is trusted forever. Each catalog/health observation has TTL. Expired observations make the candidate ineligible. Cost unknown + cloud budget policy = fail closed.

### Fallback

Fallback chain is selected from the same eligible candidate set. It cannot relax tenant, privacy, capability, data-control, context, latency, budget or allowlist requirements.

### Evidence

The default evidence projection stores request UUID, prompt SHA-256, routing decision SHA-256, selected provider UUID and model ID. Raw prompt content is not required for the audit trail.

### Complexity

Each task can require a minimum `reasoning_tier`; a cheaper/local model below that tier is ineligible rather than silently accepted. Tier metadata is observed/configured and must be justified by evaluation evidence in production.
