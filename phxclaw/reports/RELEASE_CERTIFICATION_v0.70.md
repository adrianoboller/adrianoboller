# Certificacao de release PhxClaw 0.70.0 -- NOT_CERTIFIED

Gerado por `tools/release_certification.py` em 2026-09-30T11:47:13+00:00 (Linux 6.18.44-fc-v50 x86_64). Nao se edita.

Obrigatorios: **7/12** passaram.

| Gate | Obrigatorio | Estado | Medido / motivo |
|---|---|---|---|
| `cargo_fmt` | sim | PASSED |  |
| `cargo_clippy_deny_warnings` | sim | PASSED |  |
| `cargo_test_workspace` | sim | FAILED | {"passed": 113, "failed": 1, "ignored": 9} |
| `cargo_audit` | sim | PASSED | {"vulnerabilities": 0} |
| `postgresql_e2e` | sim | PASSED | "16 ok, 0 falha(s)" |
| `postgresql_chaos_sigkill` | sim | PASSED | "caos: 30 s, 5292 ok, 6 erros, 6 reconexoes, 1766 hipoteses no banco, 0 perdidas, 0 divergentes" |
| `provider_local_ollama_e2e` | nao | PASSED |  |
| `channel_provider_credentialed_e2e` | sim | BLOCKED | exige credenciais reais de Telegram/Discord/Slack/WhatsApp/Teams (decisao do dono) |
| `desktop_os_automation_e2e` | sim | BLOCKED | exige desktop FISICO (teclado, mouse, captura reais); aqui so ha Xvfb |
| `device_pairing_wss_keyring_multiplatform_e2e` | sim | BLOCKED | exige hardware multiplataforma; e o servidor WSS de dispositivos NAO existe no fonte (DeviceEnvelope sem consumidor fora do device-transport) |
| `native_tauri_e2e` | sim | PASSED | "placar: 12/12" |
| `real_stt_model_e2e` | sim | PASSED | "test result: ok. 2 passed; 0 failed;" |
| `builtin_plugin_signatures` | sim | BLOCKED | 6 manifestos builtin com digest/assinatura quebrados desde o rename da v0.41; reassinar exige a chave privada phxclaw-dev-root-2026-v05r2 (externa) |
