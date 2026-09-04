# Pré-requisitos

Medidos no pacote, não estimados. O instalador confere os obrigatórios e para
no primeiro que faltar:

```bash
./instalar.sh --conferir      # Linux e macOS
.\instalar.ps1 -Conferir      # Windows
```

## Obrigatórios

| item | versão | por quê | se faltar |
| --- | --- | --- | --- |
| **Python** | 3.11 ou mais novo | todos os 26 scripts do plugin | nada roda; o instalador para no passo 1 |
| **Claude Code** | CLI `claude` no PATH | é onde o plugin vive | dá para validar o pacote, mas não usar |
| **Sistema** | Linux, macOS ou Windows | — | — |
| **Disco** | ~50 MB para o plugin | 26 MB são o corpus do Help | o corpus é o que ocupa; sem ele são ~17 MB |
| **Memória** | o que o Claude Code já pede | o plugin não sobe serviço nenhum | — |

**Nenhuma dependência externa de Python é obrigatória.** Os scripts usam só a
biblioteca padrão. Foi medido varrendo os `import` de todos eles: os dois únicos
nomes de fora são `pypdf` e `pdfminer`, e ambos entram em `try/except`.

## Opcionais, e o que você perde sem eles

| item | para quê | sem ele |
| --- | --- | --- |
| **pypdf** ou **pdfminer.six** | ler PDF: `extrair_pdf.py`, `pdf_para_markdown.py`, contagem de páginas no G0 | o script diz que falta e sai com código 3, **sem inventar texto**; o G0 conta o PDF pelo tamanho em bytes |
| **git** | instalar pelo marketplace do GitHub | instale pela pasta local, que o instalador já faz |
| **Corpus do Help 12k** | semântica WLanguage por tema | o G0 fica `DEGRADED` e os agentes ficam sem a fonte técnica |

Instalar os de PDF, quando quiser:

```bash
python3 -m pip install pypdf          # ou: pdfminer.six
```

## O que o plugin **não** exige

- **Não** precisa de WINDEV, WEBDEV nem WINDEV Mobile instalados: o plugin lê o
  que você exporta do IDE (PDF, SQL), não o IDE.
- **Não** precisa de banco de dados para funcionar. O banco só aparece se você
  pedir na letra K, e aí o instalador de ambiente é gerado, não executado sozinho.
- **Não** precisa de Node, Docker ou rede. Docker só se a letra L3 pedir imagem;
  rede só para instalar pelo GitHub.
- **Não** sobe serviço, não abre porta, não escreve fora de `~/.claude`,
  `~/.wx-claude-code` e da pasta do projeto.

## Para o cliente usar

Além dos obrigatórios acima, um **serial** de licença: sem ele os hooks recusam
os scripts do plugin. Como emitir está em `licenca/ATIVACAO.md`.

## Para desenvolver o plugin

O que a bateria e os geradores usam, além dos obrigatórios:

| item | para quê |
| --- | --- |
| **bash** | `instalar.sh` e o teste que o roda |
| **PowerShell 5.1+** | provar o `instalar.ps1` numa máquina Windows — **ainda não foi provado**, e o teste só confere a estrutura |
| **Node com Playwright** | gerar os PNG e PDF da documentação (não faz parte do plugin) |

Conferir tudo de uma vez:

```bash
python3 skills/conversao-wx/scripts/validate_plugin_bundle.py . --strict
```

Esperado: `valid: true`, `tests: OK`, zero erros e zero avisos.
