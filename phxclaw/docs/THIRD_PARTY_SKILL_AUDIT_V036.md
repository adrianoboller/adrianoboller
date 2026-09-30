# PhxClaw v0.36 — Third-party Skill Audit

Audit date: 2026-09-28.

Policy: adapter-first. This overlay does **not** vendor the upstream repositories. A skill becomes executable only through a PhxClaw adapter with explicit permissions, provenance, license state, sandbox/process allowlist, Evidence Ledger hooks and F24 Promotion Gate where learning is involved.

| Skill | Canonical source | License | Status | Integration |
|---|---|---|---|---|
| Agent Reach | https://github.com/Panniantong/Agent-Reach | MIT | verified_permissive | external_cli_adapter |
| Last 30 Days | https://github.com/mvanhorn/last30days-skill | MIT | verified_permissive | skill_adapter |
| Deep Research | https://github.com/199-biotechnologies/claude-deep-research-skill | MIT | verified_permissive | skill_adapter |
| User Research | https://github.com/cookiy-ai/user-research-skill | MIT | verified_permissive | skill_adapter |
| QMD Search | https://github.com/tobi/qmd | MIT | verified_permissive | external_cli_mcp_adapter |
| Graphify | https://github.com/Graphify-Labs/graphify | Apache-2.0 / MIT (repository exposes both; preserve file-scope notices) | verified_permissive_adapter_only | external_cli_adapter |
| Ponytail | https://github.com/DietrichGebert/ponytail | MIT | verified_permissive | methodology_adapter |
| Napkin | https://github.com/blader/napkin | MIT | verified_permissive | governed_memory_adapter |
| Tech Debt Audit | https://github.com/ksimback/tech-debt-skill | MIT | verified_permissive | skill_adapter |
| Understand Anything | https://github.com/Egonex-AI/Understand-Anything | MIT | verified_permissive_name_matched | knowledge_graph_adapter |
| UI/UX Pro Max | https://github.com/nextlevelbuilder/ui-ux-pro-max-skill | MIT | verified_permissive | skill_adapter |
| Frontend Slides | https://github.com/zarazhangrui/frontend-slides | MIT | verified_permissive | artifact_adapter |
| Scroll World | https://github.com/oso95/scroll-world | MIT | verified_permissive | skill_adapter |
| Visual Explainer | https://github.com/nicobailon/visual-explainer | MIT | verified_permissive | skill_adapter |
| Fireworks Tech Graph | https://github.com/yizhiyanhua-ai/fireworks-tech-graph | MIT | verified_permissive | external_cli_adapter |
| Claude SEO | https://github.com/AgriciDaniel/claude-seo | MIT | verified_permissive | skill_adapter |
| Humanizer | https://github.com/blader/humanizer | MIT | verified_permissive | text_transform_adapter |
| Auto Research in Sleep | https://github.com/wanshuiyin/Auto-claude-code-research-in-sleep | MIT | verified_permissive | task_graph_adapter |
| Video ShotCraft | https://github.com/karekin/video-shotcraft | Apache-2.0 for repository code/skill; bundled media/assets carry separate terms | verified_permissive_skill_only | skill_adapter_no_bundled_assets |
| FFmpeg Skill | https://github.com/kajisho5/ffmpeg-skill | MIT for skill; FFmpeg binary licensing depends on build/configuration | verified_permissive_skill_only | external_binary_adapter |

## Special handling

- **QMD Search:** the supplied wrapper points to `tobi/qmd`; PhxClaw targets the canonical upstream MIT skill/CLI.
- **Graphify:** repository exposes Apache-2.0 and MIT notices. PhxClaw uses an external CLI adapter and preserves file-scope provenance; no source is copied by this overlay.
- **Understand Anything:** the supplied short link could not be resolved directly during audit. The exact-name current repository `Egonex-AI/Understand-Anything` was matched and its MIT license verified. The adapter keeps this provenance note.
- **Video ShotCraft:** repository code/skill is Apache-2.0, but media/audio and Remotion carry separate terms. PhxClaw excludes bundled media/assets and treats Remotion as an optional external dependency with a separate license gate.
- **FFmpeg Skill:** skill is MIT. The installed FFmpeg binary must report its own build/license configuration before production use.

No upstream project is allowed to modify PhxClaw Constitution, policies or microkernel directly.
