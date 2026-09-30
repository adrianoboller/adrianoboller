# F23 hardening — production key provider

`phxclaw-key-provider` introduz uma fronteira explícita de master-key.

- `OsKeyringProvider` (feature `os-keyring`) usa o credential/keyring nativo do sistema.
- `DevEnvKeyProvider` só existe com feature `dev-env` e declara `is_release_safe=false`.
- `validate_provider_for_release()` falha se provider inseguro for selecionado em release.
- `KeyMaterial` não implementa Display, mostra apenas `[REDACTED]` em Debug e zeroiza bytes no Drop.
- PostgreSQL guarda apenas metadata/identificador do provider; nunca a master key.

Gate ainda obrigatório: compilar e testar o backend `keyring` real em Windows/Linux/macOS e integrar o `Secret Broker` v0.18/v0.20 ao trait durante o merge completo.
