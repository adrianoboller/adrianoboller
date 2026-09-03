# WX Claude Code

![WX Claude Code](marca-wx-claude-code.png)

Plugin para o Claude Code que converte um projeto **WINDEV, WEBDEV ou WINDEV
Mobile** para outra linguagem sem inventar o que o projeto faz. Começa por um
questionário (A–J), passa por gates com aprovação humana e termina com cada
regra ligada a evidência, código e teste.

## Instalar

```bash
claude plugin marketplace add adrianoboller/adrianoboller
claude plugin install wx-claude-code@wx-claude-code
```

Ou, para testar direto da pasta: `claude --plugin-dir ./wx-claude-code`.

Requisitos: Python 3.10+ (scripts de intake e do corpus) e Node 22+ (Impeccable).

## Comandos

| Comando | O que faz |
| --- | --- |
| `/wx-claude-code:questionario` | Perguntas **A–J** abaixo. Gera `.wx-migration/` com manifesto, configuração, `CLAUDE.md` e esboço de `DESIGN.md`. |
| `/wx-claude-code:converter` | Conversão por gates G0–G7 (pré-flight, inventário, especificação, arquitetura, piloto vertical, ondas, endurecimento, cutover). |
| `/wx-claude-code:estilo-telas` | Paleta, tema, tipografia e densidade viram `PRODUCT.md` e `DESIGN.md` pelo Impeccable; cada tela convertida nasce nesse sistema. |
| `/wx-claude-code:laudo-tokens` | Laudo de uso de tokens em 3 fases. Somente leitura; nada muda sem aprovação. |
| `/impeccable <comando> <alvo>` | Os 23 comandos do Impeccable (`polish`, `audit`, `critique`, `shape`, `harden`…). |

## O questionário

| Letra | Pergunta | Onde cai |
| --- | --- | --- |
| A | O `.SQL` do projeto | `sql_scripts` do manifesto |
| B | PDF só dos códigos | `code_documents` |
| C | PDF só das interfaces | `ui_documents` |
| D | PDF só das queries SQL | `query_documents` |
| E | PDF completo | reserva de B, C, D e fonte de `business_rule_documents` |
| F | Estilo das telas com o Impeccable (paleta, tema, tipografia, preservar ou redesenhar) | `fidelity.ui` e `DESIGN.md` |
| G | Usar o Help completo do WX em JSON (corpus 12k) e override da versão | `wlanguage_help_json` |
| H | Linguagens para o backend | `target` da configuração |
| I | Linguagens para o frontend | `target.platforms`, navegadores e dispositivos |
| J | Ativar economia de tokens | estilo de resposta no `CLAUDE.md` e `/laudo-tokens` |

As respostas ficam em `.wx-migration/questionario.json` (modelo em
`skills/conversao-wx/templates/questionario.json`) e o script
`aplicar_questionario.py` as transforma nos arquivos que o pré-flight lê. Ele
nunca sobrescreve o que já existe.

## Skills

- `conversao-wx`: intake, gates, orquestração, rastreabilidade, scripts de
  pré-flight e o corpus WLanguage 12k (`resources/Help_WL_12k_Json.zip`,
  identidade fixada por SHA-256, estado `DEGRADED/CONDITIONAL`).
- `impeccable`: vendorizada de [pbakaus/impeccable](https://github.com/pbakaus/impeccable)
  4.1.3, Apache 2.0 (`skills/impeccable/LICENSE` e `NOTICE.md`). Inclui os
  quatro agentes e o hook de revisão de design.
- `laudo-uso-tokens`: a *SKILL Laudo_Uso_Tokens*, com o prompt-mestre curto
  (o que se cola) e o detalhado (o contrato das três fases).

## Agentes

22 agentes da conversão (orquestrador, curadoria de evidências, WLanguage,
regras, telas, dados, arquitetura, implementação, testes, segurança,
integrações, relatórios, desempenho, cutover, auditoria de qualidade, design,
grids e planilhas) mais os 4 do Impeccable. A topologia e os modelos estão em
`skills/conversao-wx/references/agent-orchestration.md`.

## O que o plugin não faz

Não lê o formato binário do WX, não faz OCR sozinho, não certifica LGPD e não
afirma equivalência sem baseline executável. O corpus do Help é derivado da
documentação PC SOFT e não inclui licença de redistribuição: uso privado.

## Prints de uso

Capturas de sessões reais do Claude Code com o plugin carregado (`docs/prints/`,
geradas por `docs/prints/gerar.md`).

| | |
| --- | --- |
| ![validar](docs/prints/01-instalar-e-validar.png) `claude plugin validate` | ![skills](docs/prints/02-skills-e-agentes.png) skills e agentes numa sessão nova |
| ![questionario](docs/prints/03-questionario-a-j.png) `/wx-claude-code:questionario` | ![preflight](docs/prints/04-aplicar-e-preflight.png) respostas viram manifesto e Gate G0 |
| ![design](docs/prints/05-design-md-impeccable.png) `DESIGN.md` da letra F | ![help](docs/prints/06-corpus-help-wlanguage.png) corpus WLanguage 12k |
| ![laudo](docs/prints/07-laudo-tokens-fase-1.png) `/wx-claude-code:laudo-tokens` | |

## Pacotes

Os zips não ficam no repositório (`dist/` está no `.gitignore`); gere com:

```bash
zip -r dist/wx-claude-code-plugin.zip wx-claude-code .claude-plugin -x 'wx-claude-code/docs/prints/*'
(cd wx-claude-code/skills && zip -r ../../dist/skill-conversao-wx-cowork.zip conversao-wx)
```

## Validar o pacote

```bash
python3 wx-claude-code/skills/conversao-wx/scripts/validate_plugin_bundle.py wx-claude-code
python3 wx-claude-code/skills/conversao-wx/scripts/query_wlanguage_help.py --verify
```
