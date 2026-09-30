# PhxClaw v0.60 verifier report

PASS: 42
FAIL: 0

- PASS `file:MANIFEST.json`
- PASS `file:VERSION`
- PASS `file:README.md`
- PASS `file:crates/phxclaw-provenance-core/Cargo.toml`
- PASS `file:crates/phxclaw-provenance-core/src/lib.rs`
- PASS `file:schemas/source-provenance.schema.json`
- PASS `file:policies/source-ingestion-policy.json`
- PASS `file:inventory/source_inventory_2026-09-29.json`
- PASS `file:sql/v060_source_provenance.sql`
- PASS `file:docs/ADR-0060-source-provenance-firewall.md`
- PASS `file:docs/THIRD_PARTY_SOURCES_v060.md`
- PASS `file:integration/INTEGRATION_v060.md`
- PASS `file:ui/PhxClaw_ProvenanceFirewall_UI_v060.html`
- PASS `manifest.version`
- PASS `manifest.uuidv7` 01a0efd8-8895-7703-b632-dcae038596b1
- PASS `policy.uuidv7` 01a0efd8-8895-79f8-81b6-ca21308a4f8c
- PASS `policy.fail_closed`
- PASS `policy.default_quarantine`
- PASS `inventory.uuidv7` 01a0efd8-8896-796f-8ebb-3e467cea2ff9
- PASS `inventory.count` 6
- PASS `uuidv7:architect-main.zip` 01a0efd8-8896-7e1e-8836-5fb8da910146
- PASS `sha256:architect-main.zip`
- PASS `allow-has-license:architect-main.zip`
- PASS `allow-origin-known:architect-main.zip`
- PASS `allow-not-leak:architect-main.zip`
- PASS `uuidv7:oh-my-claudecode-main.zip` 01a0efd8-8896-7cf1-92be-30490e49ba88
- PASS `sha256:oh-my-claudecode-main.zip`
- PASS `allow-has-license:oh-my-claudecode-main.zip`
- PASS `allow-origin-known:oh-my-claudecode-main.zip`
- PASS `allow-not-leak:oh-my-claudecode-main.zip`
- PASS `uuidv7:claw-code-main.zip` 01a0efd8-8896-7d1c-8721-6b8db458f0fa
- PASS `sha256:claw-code-main.zip`
- PASS `uuidv7:claude-code-main.zip` 01a0efd8-8896-74be-8045-cb8d666b10a3
- PASS `sha256:claude-code-main.zip`
- PASS `deny-has-hard-reason:claude-code-main.zip`
- PASS `uuidv7:src.zip` 01a0efd8-8896-7916-8b8b-265bbb1de32a
- PASS `sha256:src.zip`
- PASS `uuidv7:phoenix s5272 phx cobol absorber.zip` 01a0efd8-8896-7dd8-94fe-555f45b46007
- PASS `sha256:phoenix s5272 phx cobol absorber.zip`
- PASS `decision-counts` {'ALLOW': 2, 'QUARANTINE': 3, 'DENY': 1}
- PASS `no-proprietary-source-import`
- PASS `no-unsafe-rust`
