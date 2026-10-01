# Certificacao de release PhxClaw 0.70.0 -- NOT_CERTIFIED

Gerado por `tools/release_certification.py` em 2026-10-01T01:49:11+00:00 (Linux 6.18.44-fc-v50 x86_64). Nao se edita.

Alvo: **linux**. Obrigatorios: **11/13** passaram.

| Gate | Obrigatorio | Estado | Medido / motivo |
|---|---|---|---|
| `cargo_fmt` | sim | PASSED |  |
| `cargo_clippy_deny_warnings` | sim | PASSED |  |
| `cargo_test_workspace` | sim | PASSED | {"passed": 273, "failed": 0, "ignored": 12} |
| `cargo_audit` | sim | PASSED | {"vulnerabilities": 0} |
| `agente_autonomo` | sim | PASSED | {"passed": 124, "failed": 0, "ignored": 2} |
| `postgresql_e2e` | sim | PASSED | "16 ok, 0 falha(s)" |
| `postgresql_chaos_sigkill` | sim | PASSED | "caos: 30 s, 8653 ok, 5 erros, 5 reconexoes, 2887 hipoteses no banco, 0 perdidas, 0 divergentes" |
| `provider_local_ollama_e2e` | nao | PASSED |  |
| `channel_provider_credentialed_e2e` | sim | BLOCKED | PHXCLAW_TELEGRAM_BOT_TOKEN/PHXCLAW_TELEGRAM_CHAT_ID nao definidos (bot do BotFather, nas variaveis do ambiente) |
| `desktop_os_automation_e2e` | sim | BLOCKED | sem display fisico (DISPLAY ausente ou de um Xvfb): rode a certificacao numa sessao grafica Linux de verdade |
| `desktop_os_automation_xvfb` | nao | PASSED | "placar: 4/4" |
| `device_pairing_wss_keyring_multiplatform_e2e` | nao | BLOCKED | fora do alvo linux (Windows, macOS, Android, iOS): medido pelo kit do dono; ver device_pairing_wss_keyring_linux_e2e |
| `device_pairing_wss_keyring_linux_e2e` | sim | PASSED | "placar: 4/4" |
| `native_tauri_e2e` | sim | PASSED | "placar: 12/12" |
| `real_stt_model_e2e` | sim | PASSED | "test result: ok. 2 passed; 0 failed;" |
| `builtin_plugin_signatures` | sim | PASSED |  |
