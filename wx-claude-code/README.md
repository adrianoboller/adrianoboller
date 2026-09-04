# WX Claude Code

![WX Claude Code](marca-wx-claude-code.png)

Manual de uso passo a passo em [`MANUAL.md`](MANUAL.md). Descrição para investidores em `docs/investidor/`.

Plugin para o Claude Code que converte um projeto **WINDEV, WEBDEV ou WINDEV
Mobile** para outra linguagem sem inventar o que o projeto faz. Começa por um
questionário (bloco 0 e letras A–L), passa por gates com aprovação humana e termina com cada
regra ligada a evidência, código e teste.

## O principal, que não muda

Este plugin converte **WLanguage** — WINDEV, WEBDEV e WINDEV Mobile — para outra linguagem. Isso é pétreo: tudo o mais existe para servir a esse caso.

Duas aberturas, que não substituem o principal:

- **Legado E/OU.** O projeto pode ser WX, PHP, outra linguagem, ou uma combinação. `projeto.produtos` aceita um ou mais de `windev`, `webdev`, `windev-mobile`, `php`, `outra`, e `projeto.principal` diz qual manda quando a evidência conflita.
- **Destino em qualquer linguagem.** Os perfis (Rust, Python, C# + WL_C#, Go, Java, Node, PHP) são sugestão. Com `perfil: outra`, a linguagem e o framework são livres e o processo de conversão sai genérico.

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
| `/wx-claude-code:questionario` | Bloco **0** (softhouse, diretores, endereço, logotipos, finalidade, objetivos, organograma, fluxograma, cronograma e prazo final, orçamento, riscos, pessoal, GitHub de destino sem senha, aprovador) e letras **A–L** abaixo. Gera `.wx-migration/` (manifesto, configuração, respostas legíveis, empresa, processo, ambiente, prompts, semente do PMO) e o contexto do projeto (`CLAUDE.md`, `INDEX_FILES.md`, `DESIGN.md`, `PRODUCT.md`, `.claude/`, `.mcp.json`, `Dockerfile`); a lista inteira está em «O que sai» no MANUAL. |
| `/wx-claude-code:converter` | Conversão por gates G0–G7 (pré-flight, inventário, especificação, arquitetura, piloto vertical, ondas, endurecimento, cutover). |
| `/wx-claude-code:estilo-telas` | Paleta, tema, tipografia e densidade viram `PRODUCT.md` e `DESIGN.md` pelo Impeccable; cada tela convertida nasce nesse sistema. |
| `/wx-claude-code:laudo-tokens` | Laudo de uso de tokens em 3 fases. Somente leitura; nada muda sem aprovação. |
| `/wx-claude-code:pmo` | Gerente de projetos: plano por gates, orçamento de tokens por modelo, RAID, resumo de sprint e o relatório de onze seções com painel HTML, gerado sozinho ao fechar sprint e na entrega; `exportar` salva o projeto organizado na pasta do usuário e `limpar` é o zelador dos temporários. Equipe prioritária de dez agentes com gatilho próprio (zelador por sinal, pesquisador por infrutífero, documentador, supervisor de qualidade, gestor de tarefas que pesa e escolhe o modelo, GP, testes, status por agente, base de conhecimento, tradutor a pedido). Blocos `Bloco0001` e sprints `SP00001`, com a identificação `BlocoNNNN-SPNNNNN-Título · data` abrindo toda resposta e cada sprint fechada em `.md` e `.zip`. |
| `/impeccable <comando> <alvo>` | Os comandos do Impeccable (`polish`, `audit`, `critique`, `shape`, `harden`…; 23 segundo o SKILL.md de origem). |

## O questionário

| Letra | Pergunta | Onde cai |
| --- | --- | --- |
| 0 | Empresa e projeto: dezesseis itens, um por mensagem, antes de A, incluindo quem aprova | `respostas_questionario.md`, `empresa.md`, `entrega.json`, `pmo/` |
| A | O `.SQL` do projeto | `sql_scripts` do manifesto |
| B | PDF só dos códigos | `code_documents` |
| C | PDF só das interfaces | `ui_documents` |
| D | PDF só das queries SQL | `query_documents` |
| E | PDF completo | reserva de B, C, D e fonte de `business_rule_documents` |
| F | Qualidade das telas com o Impeccable: a tela principal como modelo (F0), treze subperguntas de ERP (F1–F13: operação, teclado, grids, formulários, formatos, impressão, estados, acessibilidade, botões, posição, ícones, cores, fundo) e depois paleta, tema, tipografia | `PRODUCT.md`, `DESIGN.md` por seção, `fidelity.ui` |
| G | Usar o Help completo do WX em JSON (corpus 12k) e override da versão | `wlanguage_help_json` |
| H | Para qual linguagem converter o backend: o plugin orienta (Rust, Python, C# + WL_C#, e mais), mostra o processo de conversão de cada opção e pergunta a estratégia | `target`, `DEC-0001`, `processo-de-conversao.md` |
| I | Linguagens para o frontend, com o processo e o ritmo (tela a tela, módulo a módulo) | `target.platforms`, navegadores e dispositivos |
| J | Ativar economia de tokens | estilo de resposta no `CLAUDE.md` e `/laudo-tokens` |
| K | Ambiente: privilégios (sudo ou root), Rust/Cargo, PostgreSQL, MySQL, MariaDB, Supabase (marcáveis, com login e papéis por nível; senha só por nome de variável), ligação com o GitHub e n8n integrado ao projeto (sim ou não, com webhooks e fluxos) | `ambiente.md`, `ambiente/`, `ambiente/n8n/` |
| L | Contexto do Claude Code e implantação: requisitos da v1, prototipação, deploy (Dockerfile, compose), hooks do projeto, MCP e skills | `prompts/kickoff.md`, `INDEX_FILES.md`, `.claude/`, `.mcp.json`, `Dockerfile` |

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
- oito skills de ERP (`erp-accounting`, `erp-inventory`, `erp-brazil-fiscal`,
  `erp-multi-company`, `erp-approval-workflows`, `erp-lgpd`,
  `erp-integration-reliability`, `windev-wlanguage-erp`), do pacote pesquisado
  no skills.sh em 4 de setembro de 2026 (`skills/LEIA-ME-erp.md`). Descrições
  encurtadas a 150 caracteres para não sumirem da listagem; o item L6 do
  questionário gera o esqueleto de ERP e liga cada módulo à skill dele.
  Skills externas do catálogo skills.sh não entram no plugin: o projeto
  gerado recebe `docs/skills-recomendadas.md` com as que cabem nas respostas
  (`references/skills-sh.md`).

- `php-legado-e-destino`: PHP nos dois sentidos — ler um sistema PHP legado
  (procedural ou OOP, com as armadilhas que mudam o resultado convertido) e
  gerar PHP 8.3 como destino, com a tabela WLanguage → PHP.

## Equipe WLanguage, balanceamento e PMO

- **Sete especialistas WLanguage por tema do Help** (`wl-hfsql`, `wl-ui-controls`,
  `wl-communication`, `wl-standard-functions`, `wl-mobile`, `wl-web`,
  `wl-errors`). Cada um consulta só a sua fatia do corpus com `--group`, o
  que reduziu uma busca de 5,4 s para 0,5 s. Divisão em
  `skills/conversao-wx/references/equipe-wlanguage.md`.
- **Balanceamento de modelos**: `rotear_modelo.py` escolhe `haiku`, `sonnet`
  ou `opus` e o effort pela classe da tarefa, pelos sinais de risco e pelo
  orçamento do gate; regra em `references/balanceamento-de-modelos.md`.
- **PMO** com as três técnicas em código: **Scrum** (sprint por gate, backlog,
  definição de pronto, resumo da sprint em doze seções; o relatório do PMO, outro documento, tem onze), **Kanban** (quadro gerado da
  matriz com limite de WIP) e **PDCA** (ciclo com critério numérico cujo
  fechamento, frutífero ou infrutífero, grava em `base_de_conhecimento.md`).
  Painel em `.wx-migration/pmo/status.md`, tudo medido. Regra em
  `references/pmo.md`; manual em `MANUAL.md`.

## Para qual linguagem converter

A letra H não pergunta «qual linguagem». Quando o usuário não sabe, o plugin
levanta quatro sinais (quem mantém, desktop/web/mobile, desempenho ou prazo,
linguagem já em uso) e mostra três opções com o porquê, a recomendada
primeiro: **Rust** (Axum + PostgreSQL), **Python** (FastAPI) e **C# (.NET 8) +
WL_C#**, com Go, Java e Node quando os sinais apontarem. A matriz completa
está em `skills/conversao-wx/references/perfis-de-destino.md`.

**WL_C#** (https://bernardsobra.github.io/WL-web/) é a biblioteca de Bernard
Sobra que porta mais de 480 funções do WLanguage para C# com o mesmo nome.
O plugin embute um índice de 261 nomes lidos do `WL.dll` 1.0 e o perfil em
`references/perfil-csharp-wl.md`; o `WL.dll` é baixado da release oficial e
conferido por hash, não redistribuído. Provado em sessão real: com os sinais
«equipe WINDEV, desktop Windows, prazo manda», a recomendação foi C# + WL_C#.

## Projeto de exemplo

`exemplos/estoque-wx/` é um sistema WINDEV 2025 sintético e completo: `.SQL`
com sete tabelas, os quatro PDFs no formato da documentação técnica (texto de
verdade, gerados de `fontes/`), quatro screenshots com estado declarado,
dados de amostra e dez casos de golden master. O G0 sobre ele dá
`CONDITIONAL` com zero erros. Há uma divergência plantada para o G2 achar.
Veja `exemplos/estoque-wx/LEIA-ME.md`.

## Provas em código

- `extrair_pdf.py`: texto por página com `arquivo#page=N` e hash; pouco texto vira `OCR_REQUIRED`.
- `golden.py`: captura resultados do legado e compara com o novo, com tolerância; devolve `n/total`.
- `uso_de_tokens.py`: lê o `usage` das sessões do Claude Code (MEDIDO) e lança no orçamento do gate.
- `hooks/portao_g0.py`: nega `Write`/`Edit` fora de `.wx-migration/` enquanto o G0 estiver `BLOCKED`.
- `pmo.py painel`: o painel do PMO em HTML, gerado do mesmo código do `status`.
- `tests/testes.py`: 27 testes de regressão; o validador em modo estrito os executa.

## Limitações conhecidas

- A listagem de skills que o modelo devolve numa sessão nova oscila entre
  6 e 7 itens; confira por nome, não por contagem.
- O questionário pergunta uma letra por vez, mas não impede o usuário de
  responder várias de uma vez; nesse caso ele confirma cada uma e segue.
- O corpus do Help fica `DEGRADED/CONDITIONAL` por três defeitos medidos e
  deliberadamente não saneados; o porquê e a pendência de licença estão em
  `skills/conversao-wx/references/corpus-saneamento.md`.
- `extrair_pdf.py` precisa de `pypdf` ou `pdfminer.six`; sem eles diz isso e
  para, em vez de inventar texto.

## Equipe de grande porte: dez papéis com PDCA

Para projetos grandes, dez papéis com dono (A orquestrador, B engenheiro,
C DBA, D zelador, E designer, F prova real, G QA, H documentação,
I versionador, J pesquisador), cada um com quatro subagentes Plan, Do,
Check e Act. Um papel só trabalha em item do backlog com a sua letra, e o
backlog é do PMO. No fechamento, `pmo.py entregar` zipa para o stakeholder
o resumo da sprint, as técnicas aplicadas com números, a base de
conhecimento, o que cada ferramenta faz (lido dos scripts), decisões,
lacunas e o Kanban. Regra em `references/papeis-e-pdca.md`.

## Agentes

30 agentes da conversão, mais 50 da camada de papéis (10 papéis × [papel + 4 fases PDCA]) (orquestrador, curadoria de evidências, WLanguage,
regras, telas, dados, arquitetura, implementação, testes, segurança,
integrações, relatórios, desempenho, cutover, auditoria de qualidade, design,
grids e planilhas) mais os 4 do Impeccable. A topologia e os modelos estão em
`skills/conversao-wx/references/agent-orchestration.md`.

## O que o plugin não faz

Não lê o formato binário do WX, não faz OCR sozinho, não certifica LGPD, não
aprova gate no lugar do humano e não afirma equivalência sem baseline
executável. O corpus do Help é derivado da documentação PC SOFT e não inclui
licença de redistribuição: uso privado. A lista completa está no fim do MANUAL.

## Prints de uso

Capturas de sessões reais do Claude Code com o plugin carregado (`docs/prints/`,
geradas por `docs/prints/gerar.md`).

| | |
| --- | --- |
| ![validar](docs/prints/01-instalar-e-validar.png) `claude plugin validate` | ![skills](docs/prints/02-skills-e-agentes.png) skills e agentes numa sessão nova |
| ![questionario](docs/prints/03-questionario-a-j.png) `/wx-claude-code:questionario` | ![preflight](docs/prints/04-aplicar-e-preflight.png) respostas viram manifesto e Gate G0 |
| ![design](docs/prints/05-design-md-impeccable.png) `DESIGN.md` da letra F | ![help](docs/prints/06-corpus-help-wlanguage.png) corpus WLanguage 12k |
| ![laudo](docs/prints/07-laudo-tokens-fase-1.png) `/wx-claude-code:laudo-tokens` | ![pmo](docs/prints/08-pmo-orcamento-e-roteamento.png) PMO: orçamento, roteamento e painel |
| ![equipe](docs/prints/09-equipe-wlanguage-por-tema.png) delegação real aos `wl-*-specialist` | ![scrum](docs/prints/10-pmo-scrum-kanban-pdca.png) Scrum, Kanban e PDCA com a base de conhecimento |
| ![pmo-sessao](docs/prints/11-pmo-sessao-real.png) `/wx-claude-code:pmo status` numa sessão real | ![painel](docs/prints/12-painel-pmo-html-tema-claro.png) `pmo.py painel`, tema claro |
| ![letra-h](docs/prints/13-questionario-h-orientacao-de-linguagem.png) letra H: sinais e três opções, a recomendada primeiro | ![exemplo](docs/prints/14-exemplo-estoque-g0-extracao-golden.png) o exemplo ESTOQUE no G0, extração e golden |
| ![papeis](docs/prints/15-papeis-backlog-e-entrega-zipada.png) backlog com papel dono, Kanban por papel e a entrega zipada ao stakeholder | ![letra-f](docs/prints/16-letra-f-erp-botoes-e-design-md.png) letra F para ERP: a tabela de botões, posição e fundo no `DESIGN.md` |
| ![bloco-0](docs/prints/17-bloco-0-empresa-e-projeto.png) bloco 0: softhouse, diretores, endereço, um item por mensagem | ![senha](docs/prints/18-senha-colada-nao-e-gravada.png) senha colada na conversa: não gravada, não repetida, revogar e usar `credencial_ref` |
| ![processo](docs/prints/19-letra-h-processo-de-conversao.png) letra H: sinais, três opções e o processo de conversão para a escolhida, peça por peça | ![tela-modelo](docs/prints/20-letra-f0-tela-modelo.png) F0: a tela principal do legado como modelo, aberta antes de registrar, com o que preservar e o que mudar |
| ![licenca](docs/prints/21-licenca-serial-de-ativacao.png) serial de ativação: sem ele o PMO recusa; instalado, a mesma sessão roda | ![respostas](docs/prints/22-respostas-do-questionario.png) sessão nova acha o aprovador e o prazo em `respostas_questionario.md`, sem perguntar |
| ![ambiente](docs/prints/23-letra-k-ambiente-sem-senha.png) letra K: PostgreSQL, papéis por nível e a senha do root que não é gravada nem repetida | ![n8n](docs/prints/24-letra-k7-n8n-integrado.png) K7: n8n sim ou não, e cada item da integração um por mensagem |
| ![kickoff](docs/prints/25-primeira-sessao-index-e-kickoff.png) primeira sessão: lê `INDEX_FILES.md` e o kickoff, sabe o escopo da v1 e recusa código sem G0 | ![exportar](docs/prints/26-pmo-exportar-projeto-organizado.png) `pmo exportar`: o projeto organizado na pasta do usuário, sem segredo, com hashes |
| ![zelador](docs/prints/27-zelador-limpeza-diaria.png) o zelador limpa temporários ao abrir a sessão e deixa o registro medido | ![identificacao](docs/prints/28-identificacao-bloco-sprint.png) toda resposta abre com `BlocoNNNN-SPNNNNN-Título · data`, injetado pelo hook |
| ![equipe](docs/prints/29-equipe-prioritaria-pesquisador-e-status.png) equipe prioritária: infrutífero aciona o Pesquisador; status por agente sem inventar | ![rag](docs/prints/30-rag-e-guarda-de-anexos.png) RAG do projeto cita `arquivo#linha`; anexo somente leitura recusado |
| ![corpus](docs/prints/31-corpus-no-claude-md-e-no-rag.png) o Help consultado por tema, com id e hash, a partir do `CLAUDE.md` gerado e do RAG | ![skills erp](docs/prints/32-skills-erp-listadas.png) as onze skills do plugin listadas numa sessão nova, oito delas de ERP |
| ![esqueleto](docs/prints/33-esqueleto-erp-e-skill-por-modulo.png) esqueleto de ERP gerado por L6: módulo → skill, ADR lida, `erp-inventory` carregada e citada | ![skills.sh](docs/prints/34-skills-recomendadas-e-regras-absorvidas.png) skills do skills.sh que cabem nas respostas, tipos de dinheiro e data, STRIDE → SEC-* → teste |
| ![artefatos](docs/prints/35-artefatos-e-skill-de-php.png) artefatos: arquivado ≠ declarado, o hash como prova, e a skill de PHP nas armadilhas do legado | ![comandos](docs/prints/36-comandos-e-legado-e-ou.png) os dezessete comandos, as 59 perguntas por id, e legado só PHP com destino Elixir aceito |

## Hooks e RAG

Os hooks do plugin fazem valer as regras: licença, portão G0, anexos somente leitura, nenhum segredo em arquivo, Kanban sincronizado com a matriz, identificação em toda interação, zelador diário. O RAG local (`rag.py`, BM25 sem dependência) indexa os documentos do projeto e injeta a cada pergunta os trechos mais próximos com `arquivo#linha`. Detalhe em `references/hooks-e-rag.md`.

## Licença e serial

O plugin só roda com serial válido em `~/.wx-claude-code/licenca`, assinado com RSA-2048 pela chave privada de quem distribui; o plugin traz só a pública. Sem serial, os comandos param e o hook nega os scripts e a escrita em `.wx-migration/`. O que isso protege e o que não protege, e os comandos de quem distribui, em `licenca/LEIA-ME.md`; capítulo 8 do manual para o cliente.

## Vídeo de uso

`docs/video/wx-claude-code-video-de-uso.mp4` (e `.webm`), 3 min 07 s: vinte e três cenas com as
mesmas saídas reais dos prints, reproduzidas num terminal animado.

## Pacotes

O plugin completo passa de 30 MB por causa do corpus do Help. Para distribuir
em dois arquivos: o plugin sem o corpus e o `Help_WL_12k_Json.zip` à parte,
que vai em `skills/conversao-wx/resources/` (o `--verify` confere o hash).

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
