# Kit portátil de base de conhecimento

Montado em 01/10/2026, por decisão do dono, a partir do PhxSql. É o que dá para
levar para um projeto novo **sem levar o PhxSql junto**: a lei global, as leis
de processo com a cicatriz de cada uma, os aprendizados que já trazem a prova
escrita, e os scripts que tornam essas leis executáveis.

```
kit-portatil/
├── LEIA-ME.md                  este arquivo
├── extrair.py                  triagem mecânica das cognições (refaz CANDIDATAS.md)
├── lei-global/CLAUDE.md        a lei global RECONSTRUÍDA — a conferir pelo dono
├── regras/                     8 leis de processo genéricas, cada uma com a cicatriz
│   ├── 01-portoes-unicos-antes-do-commit.md
│   ├── 02-guarda-repoe-o-defeito-e-tem-de-cair.md
│   ├── 03-catraca-so-desce-e-aposenta.md
│   ├── 04-backup-provado-por-restauracao.md
│   ├── 05-teste-de-e-s-com-teto-de-voltas.md
│   ├── 06-zelador-prova-que-ninguem-usa.md
│   ├── 07-integrador-unico-que-comita.md
│   └── 08-numero-so-de-gerador.md
├── aprendizados/
│   ├── CANDIDATAS.md           GERADO por extrair.py — não se edita
│   └── SELECIONADAS.md         a curadoria: o que serve fora, com o porquê
└── scripts/                    generalizados, sem caminho fixo do PhxSql
    ├── portoes.sh              todos os portões, um código de saída
    ├── backup.sh               bundle git provado por clone + árvore igual
    ├── backup-completo.sh      árvore + arquivos de fora, provado por SHA-256
    ├── provar-guardas.py       catálogo trecho→troca→caem/seguem, veredito
    ├── zelador.sh              apaga só o que provou que ninguém usa
    └── mesclar_catalogo.py     merge de catálogo com id: novas + só-da-frente, conflito à mesa
```

## Como copiar para um projeto novo

1. **A lei global** — depois de o dono conferir:
   `cp kit-portatil/lei-global/CLAUDE.md ~/.claude/CLAUDE.md`. Ela vale para
   todo projeto; o `CLAUDE.md` do projeto acrescenta, nunca tira.
2. **As regras** — copie `regras/` para `docs/regras/` do projeto novo (ou
   cole as que valem no `CLAUDE.md` dele). Mantenha a cicatriz: regra sem o
   porquê é a primeira a ser derrubada.
3. **Os scripts** — copie `scripts/` para a raiz ou para `ferramentas/`. Cada
   um tem o cabeçalho de uso (`--help` nos `.sh`, docstring nos `.py`).
4. **Os aprendizados** — `SELECIONADAS.md` é leitura, não cópia: no projeto
   novo, cada regra **se re-prova** antes de virar FRUTÍFERO lá. Prova de outro
   projeto não é evidência validada no seu.

## O que adaptar em cada script

| Script | O que o projeto novo fornece |
|---|---|
| `portoes.sh` | `portoes.passos` na raiz: `nome \| comando` por linha. Sem ele, cai no trio do Rust (fmt, clippy `-D warnings`, test). Ponha as catracas como passo. |
| `backup.sh` | Nada (branch atual, destino no pai do repositório). `KIT_PREFIXO` muda o nome. |
| `backup-completo.sh` | `KIT_EXCLUIR` (o que fica de fora: compilado, dependências) e `KIT_FORA` (arquivos fora do repositório — por padrão a lei global). Precisa do `backup.sh` ao lado. |
| `provar-guardas.py` | Um `catalogo.py` com `COPIAR`, `COMANDO` (com `{alvo}`), `LINHA` (regex nome+veredito da saída do seu executor de testes) e `GUARDAS`. |
| `zelador.sh` | `zelador.alvos`: `dir \| caminho`, `trava \| caminho \| arquivo-de-trava`, `tmp \| glob \| minutos`. Opcional `KIT_ESTA_MEDINDO`. |
| `mesclar_catalogo.py` | O caminho do catálogo; `--bloco`/`--fim` se a entrada não tiver o formato `{"id": ...}` com 4 espaços. |

**Exercitado nesta montagem (01/10/2026):** `portoes.sh` com passos de
arquivo (um vermelho no meio → VERMELHO, saída 1); `provar-guardas.py` contra
um projeto de brinquedo (PROVADA, NÃO PEGOU, QUEBRADA por forma; o original
intacto depois) e `--autoteste` (7 casos); `zelador.sh` (apagou o livre,
recusou o diretório com `cwd` de um processo vivo, nomeando o PID).
**Não exercitados:** `backup.sh`, `backup-completo.sh` e `mesclar_catalogo.py`
— o disco estava apertado e o ambiente isola git fora da cópia de trabalho.
Os três seguem a lógica dos originais, que estão provados no PhxSql; a
primeira corrida num projeto novo é a prova deles.

## Refazer a triagem dos aprendizados

```
python3 kit-portatil/extrair.py              # regrava aprendizados/CANDIDATAS.md
python3 kit-portatil/extrair.py --stdout
python3 kit-portatil/extrair.py --cognicao OUTRA/PASTA --codigo OUTRO/CODIGO --git REPO
```

Só lê as cognições; nunca escreve nelas. Confere cada referência (teste
existe como `#[test] fn`, commit existe no git, arquivo existe no disco) e
procura a frase da prova nos dois sentidos. A curadoria (`SELECIONADAS.md`) é
humana e cita a data.

## A `base-de-conhecimento/` da raiz está parada desde 01/09/2026

O `base-de-conhecimento/00-INDICE.md` diz: gerada em **01/09/2026 18:50** de
um transcrito de **110 MB / 36.014 linhas**; o último commit na pasta é de
01/09/2026. Tudo o que aconteceu depois — modo honesto, pesquisador decide,
cognição com estado, a reconstrução da lei global — não está lá.

**Como se refaz:** `python3 base-de-conhecimento/extrair.py CAMINHO/DO/transcrito.jsonl`.

**Não rode sem argumento neste contêiner.** Sem argumento, o extrator pega o
**maior** `.jsonl` de `/root/.claude/projects/-home-user-adrianoboller/`, e
hoje (01/10) só existe lá o transcrito desta sessão — **30 MB, desde 29/09**.
O de 110 MB que gerou a base não existe mais neste contêiner. Rodar assim
**sobrescreveria** `01-PEDIDOS.md`, `02-BRIEFINGS-DE-AGENTE.md`,
`03-RECEITAS-DE-SHELL.md` e `scripts/00-INDICE.md` com uma base menor, e
deixaria órfãos os scripts numerados acima da nova contagem. O conteúdo de
01/09 só voltaria pelo git.

O caminho certo, a decidir: gerar a base desta sessão numa **pasta nova**
(o extrator grava no diretório dele, então copie-o para
`base-de-conhecimento/sessao-AAAAMMDD/` e rode lá), sem tocar a de 01/09.
Foi justamente essa base parada que permitiu reconstruir a lei global: a
receita que criou o `~/.claude/CLAUDE.md` em 30/08 está, palavra por palavra,
em `03-RECEITAS-DE-SHELL.md` — script que não morreu com a sessão.
