# Third-Party Notices

## Claw Code

Project: `ultraworkers/claw-code`
Repository: https://github.com/ultraworkers/claw-code
License: MIT
Copyright: 2026 UltraWorkers and Claw Code contributors

PhxClaw v0.16 integrates compatibility concepts and provides an external CLI bridge. A vendored upstream checkout, when requested on a private build host, is kept under `private/vendor/claw-code/upstream` with its original license and exact commit lock.

## openclaw-rs (user-supplied source snapshot)

Project: `neul-labs/openclaw-rs`
Repository declared by upstream: https://github.com/neul-labs/openclaw-rs
License: MIT
Vendored snapshot: `private/vendor/openclaw-rs/upstream`

PhxClaw v0.17 preserves the upstream LICENSE and provenance lock. Selected architectural concepts are adapted into the Channel Gateway and Plugin SDK. Native FFI loading is not imported into the PhxClaw core.
### v0.18 F23 design reuse

The F23 Secret & Credential Broker and secret-backed channel providers adapt MIT-licensed patterns observed in the supplied `openclaw-rs` snapshot, including AES-256-GCM at-rest encryption, redacted secret wrappers, credential scoping, and Telegram/Discord REST integration. PhxClaw adds UUIDv7 leases, Event Bus/Evidence Ledger integration, origin allowlists, rotation/revocation semantics, and fail-closed capability policies.


## RustClaw (user-supplied source snapshot)

Repository claimed by supplied README: https://github.com/Adaimade/RustClaw
README license claim: MIT
License file in supplied archive: **missing**
State: quarantined reference only

The source is stored under `private/vendor-quarantine/rustclaw/upstream` and is not compiled, linked, or copied into the PhxClaw core until the license text is independently verified and preserved.

## RustClaw 0.5.0 — MIT

PhxClaw v0.19 vendors the user-supplied RustClaw 0.5.0 snapshot under
`private/vendor/rustclaw/upstream` and adapts selected gateway, session/memory,
cron, and MCP compatibility concepts in `phxclaw-rustclaw-native`.

Copyright notice from the supplied license: `Copyright (c) 2024-2026 Openclaw-Rust`.
The complete MIT license is preserved at `private/vendor/rustclaw/upstream/LICENSE.md`.

Reuse is permitted under the MIT terms. PhxClaw remains private-only; MIT
notices are retained for copied/adapted substantial portions.


## Tree-sitter grammars — v0.63

Tree-sitter Rust bindings and language grammars for Rust, Python, JavaScript, TypeScript, Go, C, C++ and Java are consumed as crates.io dependencies. These upstream projects publish permissive licenses; exact crate versions are pinned in the workspace and must be preserved in the generated SBOM/lockfile on the native build host.

## alacritty_terminal 0.26 — Apache-2.0

Crate: https://crates.io/crates/alacritty_terminal (projeto Alacritty)
Uso: dependencia do `crates/phxclaw-terminal` (PTY via rustix-openpty, laco de leitura e
emulador VT). Usada como biblioteca, sem copia de fonte.

## Helix 25.07.1 — MPL-2.0

Repositorio: https://github.com/helix-editor/helix (tag 25.07.1, commit
ac94841019910ff405f31a8668389a06a169e0e5)
Uso: o editor do IDE roda como PROCESSO SEPARADO, sem modificacao, dentro do motor de
terminal. Nao e vendorizado nem ligado ao PhxClaw; `tools/instalar_helix.sh` o compila do
fonte e o instala em /opt/helix com o LICENSE dele ao lado.

## Exo 2 — SIL Open Font License 1.1

Fonte da marca, em `apps/phxclaw-ui/assets/fonte/exo2-latin.woff2`, com a licenca em
`apps/phxclaw-ui/assets/fonte/OFL.txt` (a mesma copia que o PhxSql ja distribui).

## IBM Plex Mono — SIL Open Font License 1.1

Copyright 2017 IBM Corp., Reserved Font Name "Plex". Fonte dos DADOS na interface (o
«Style Phoenix Padrao», `docs/ui/STYLE_PHOENIX_PADRAO.md`: dado em mono, rotulo em Exo 2), em
`apps/phxclaw-ui/assets/fonte/ibmplexmono-400.woff2` e `ibmplexmono-700.woff2`, com a licenca
em `apps/phxclaw-ui/assets/fonte/OFL-IBMPlexMono.txt`. LOCAL, nunca de CDN: a captura do PhxSql
mostrou a fonte de CDN caindo em Arial sem rede.
Os dois WOFF2 sao os PUBLICADOS PELA IBM, sem alteracao: pacote npm `@ibm/plex-mono` 1.1.0,
`fonts/complete/woff2/IBMPlexMono-Regular.woff2` (sha256 49ce58b4...6d131) e `-Bold.woff2`
(sha256 5788454f...6a235d). Uma conversao nossa a partir do TTF foi descartada antes do commit:
o nome «Plex» e reservado pela OFL e a garantia explicita do FAQ sobre conversao de formato
nao cobre o WOFF2 -- usar o arquivo da propria IBM tira a duvida em vez de interpreta-la.

## Wycheproof test vectors — Apache-2.0

`crates/phxclaw-agent/tests/dados/rsa/wycheproof_rsa_2048_sha256.txt` is extracted from
`testvectors_v1/rsa_signature_2048_sha256_test.json` of https://github.com/C2SP/wycheproof
(Apache License 2.0). The file header records the upstream commit and the SHA-256 of the original.
Used only as test data for the RSA PKCS#1 v1.5 verifier; no code is copied.

## NIST CAVP SigVer15 test vectors — public domain

`crates/phxclaw-agent/tests/dados/rsa/nist_sigver15_sha256.txt` is extracted from the NIST CAVP
`186-3rsatestvectors.zip` (U.S. Government work, public domain). Test data only.
