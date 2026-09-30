# PhxClaw v0.14 — Audit of uploaded Phoenix packages

## phoenix-dashboards-completo
- Source crate: `phoenix-dashboards` 1.0.0, MIT.
- 62 chart types, multiple targets, offline ECharts assets, optional DB/GUI features.
- Third-party notice in upload: qrcode-generator MIT; ECharts/ECharts-GL Apache-2.0.
- Plugin strategy: private process plugin. Bundles the 3 MB Rust crate + catalog/docs, not the 110 MB HTML demo corpus.
- Runtime-now capabilities: health, catalog list/get, source status, preview list, and basic self-contained ECharts HTML for line/area/bar/column/pie/donut/scatter.
- Full advanced catalog stays behind the bundled Rust backend and needs Cargo compilation.

## phoenix-web-absorber-fxsdk
- Cargo declares Apache-2.0; Rust std-only core.
- Includes typed design/token/UI catalogs plus ES5 FX runtime.
- The documentation cites Open Design, DevExpress, Telerik, Syncfusion and a licensed admin kit as sources/references.
- Because redistribution rights for every referenced commercial asset are not established by the upload, the PhxClaw plugin is **private-only** and excludes the 58 MB preview corpus.
- Runtime-now capabilities operate only on bundled Phoenix data: catalog summary, themes, token validation, CSS emission, UI catalog search and source status.
- Do not publish externally until a provenance/license review of the complete source corpus is signed off.

## F20 Team Runtime
- Rust source added with lease, heartbeat, fencing token, dependency gates, cancellation and expired-lease recovery.
- Mission Runtime gained a `TeamExecutor` injection point and `team_dispatch` action.
- Operational Python bootstrap proves the scheduler semantics in this environment.
- Rust cargo compilation remains a release gate.
