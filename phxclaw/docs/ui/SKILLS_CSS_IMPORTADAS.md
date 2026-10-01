# Skills de CSS, Grid, Flexbox, responsividade e Bootstrap — importação

Pedido do dono, 01/10/2026. Seis skills públicas avaliadas; só entra a que tem **arquivo de
licença** compatível com Apache-2.0 e passou pela leitura contra injeção.

**Onde ficam:** `config/skills/ui-css/<nome>/` — `SKILL.md` (como o importador grava),
`ORIGEM.json` (SHA-256 do original) e o arquivo de licença do autor ao lado (MIT e Apache-2.0
pedem o aviso junto da cópia). O agente as carrega com
`PHXCLAW_SKILLS_DIR=config/skills/ui-css` (ou `agente.skills_dir` no `config.json`).

**Como entraram:** baixados só `SKILL.md` e licença por `raw.githubusercontent.com/<repo>/HEAD/…`
em 01/10/2026 (a rede respondeu 200; a API do GitHub e as páginas `github.com` responderam 403
pelo proxy, então a listagem do repositório não foi consultada), e importados pelo comando que
já existe: `PHXCLAW_SKILLS_DIR=config/skills/ui-css phxclaw skills importar DIR` — scripts
desligados (nenhuma das quatro trazia `scripts/`), nenhum nome de ferramenta a traduzir.

| Repositório / skill | Licença conferida | SHA-256 do `SKILL.md` | Resultado |
|---|---|---|---|
| `wshobson/agents` · `plugins/ui-design/skills/responsive-design` | MIT, `LICENSE` na raiz (© 2024 Seth Hobson) | `c8bd56f8fc0f122c4ff392b02617f0e6bb4a353b2d0189235c6ee77cdcfe8690` | **importada** |
| `librefang/librefang-registry` · `skills/css-expert` | MIT, `LICENSE` na raiz (© 2026 LibreFang Contributors) | `e86b2abc97b821cff7997c4c3db9fdbda7d483d0603f3e79248db30219f1790f` | **importada** |
| `OneWave-AI/claude-skills` · `responsive-layout-builder` | MIT, `LICENSE` na raiz (© 2025 OneWave AI) | `85fc22a00781c49e8b1ffaa5f25758567c1a1681a9dbe03b56351912105513f1` | **importada** |
| `anthropics/skills` · `skills/frontend-design` | Apache-2.0, `LICENSE.txt` na pasta da skill (a raiz não tem `LICENSE`) | `d91970639e9f5c37682ac7ab60094d35f1c7c1f38d731bd56396563aee10c1d3` | **importada** |
| `lindoelio/agent-skills` · `bootstrap-5` | **sem arquivo de licença**: 404 em `LICENSE`, `LICENSE.md`, `LICENSE.txt`, `license`, `COPYING` na raiz e na pasta; o cabeçalho diz `license: MIT` sem titular nem texto | `3acb1b112f9f45455d918092bddc21020d35374caf7e093b1c0b6507160ec68a` | **recusada** — declaração sem texto de licença não é licença que se possa cumprir (o MIT exige reproduzir o aviso) |
| `Ortus-Solutions/skills` · `bootstrap-expert` | **sem arquivo de licença**: os mesmos 404; cabeçalho `license: MIT` sem texto | `878674484242992357b9fa9afdf3889cdd4c313341569f9c8380aafa0411fdca` | **recusada** — mesmo motivo |

SHA-256 das licenças guardadas: wshobson `f89abb55…c746`, librefang `a8d02861…fdca`,
OneWave `197dce10…d460`, anthropics `0d542e0c…4594`.

## Leitura contra injeção

Cada `SKILL.md` foi lido inteiro antes de importar, procurando ordem de executar comando,
baixar coisa ou mexer em credencial.

- `responsive-design`, `css-expert`, `responsive-layout-builder`: só técnica de CSS. Nada.
- `frontend-design`: só orientação de desenho. Pede «screenshots to review if your environment
  supports it» e «confirm with the client» — conduta, não comando; aceito.
- `bootstrap-5` (recusada por licença): traz `<link>`/`<script>` de CDN jsDelivr nos exemplos.
  Não é injeção, mas contraria a regra deste repositório de não usar CDN — mais um motivo para
  não entrar como está.

## O que ficou de fora

- `responsive-design` remete a `references/details.md` (baixado e lido: só padrões de CSS).
  O importador copia só o `SKILL.md`, então a referência **não** está na pasta — a skill diz
  para lê-la e ela não existe aqui. Copiar à mão fugiria do comando pedido; fica registrado.
- O importador não guarda a URL de origem no `ORIGEM.json` (guarda o caminho local); a URL está
  nesta tabela.
