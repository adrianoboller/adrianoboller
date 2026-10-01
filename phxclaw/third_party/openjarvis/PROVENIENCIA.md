# OpenJarvis: fonte de terceiro guardada para estudo

| campo | valor |
|---|---|
| origem | https://github.com/open-jarvis/openjarvis |
| commit | `f0ecea0dc4c3144e91352449aae0dd3ef88d0280` (2026-09-30) |
| trazido em | 01/10/2026, por pedido do dono («adicione no projeto») |
| licença | Apache-2.0 (`LICENSE` nesta pasta, intacta) |
| autores | Hazy Research / Stanford (ver `README.md` e `CITATION`) |

## O que é e o que não é

É uma cópia de referência, fixada no commit acima, para a absorção das capacidades do
OpenJarvis (`docs/absorcao/`). **Não é código do PhxClaw**: nada daqui é compilado,
linkado ou importado pelo produto. O workspace Cargo do PhxClaw a exclui
(`exclude = ["third_party"]`). Quando uma ideia daqui virar código nosso, ela é
reescrita contra as nossas restrições, e a divergência fica registrada.

## O que ficou de fora, e por quê

- o histórico `.git`;
- `desktop/src-tauri/binaries/` (74 MB): o binário do Ollama para macOS (aarch64). É
  binário de terceiro, não fonte, e não roda no alvo Linux;
- `assets/openjarvis_demo_reel.webp` (4,6 MB): vídeo de demonstração.

## Conferido antes de entrar

- licença Apache-2.0, compatível com a do PhxClaw;
- as «chaves» que uma varredura acha aqui são exemplos do próprio módulo de segurança
  deles e de testes (`AKIAIOSFODNN7EXAMPLE`, sequências do alfabeto, cabeçalhos PEM sem
  corpo válido), não credenciais.
