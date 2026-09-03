# Manual de uso — WX Claude Code

Plugin do Claude Code para converter um projeto **WINDEV, WEBDEV ou WINDEV
Mobile** para outra linguagem sem inventar o que o projeto faz. Sete
capítulos, na ordem em que você vai precisar deles.

1. Como instalar
2. Comandos `/` do plugin
3. Como funciona a gerência de projeto
4. Como funciona a economia de tokens
5. Como subir os arquivos
6. Como invocar o wizard
7. Como definir a linguagem e a plataforma de destino

---

## 1. Como instalar

**Requisitos.** Claude Code; Python 3.10 ou mais novo; Node 22 ou mais novo
(só para o Impeccable, o módulo de qualidade gráfica). Para extrair texto de
PDF, `pip install pypdf`.

**Pelo marketplace** (recomendado):

```bash
claude plugin marketplace add adrianoboller/adrianoboller
claude plugin install wx-claude-code@wx-claude-code
```

**Pelo zip.** O plugin vem em dois arquivos, porque o completo passa de
30 MB: `wx-claude-code-<versão>-plugin-sem-corpus.zip` e
`Help_WL_12k_Json-corpus-do-plugin.zip`.

1. Descompacte o primeiro numa pasta, por exemplo `~/plugins/`. Ele cria
   `wx-claude-code/` e `.claude-plugin/`.
2. Copie o `Help_WL_12k_Json.zip` do segundo para
   `wx-claude-code/skills/conversao-wx/resources/`. Não descompacte.
3. Confira o corpus:

```bash
python3 wx-claude-code/skills/conversao-wx/scripts/query_wlanguage_help.py --verify
```

   O hash tem de ser `a95ed553…` e o estado `DEGRADED/CONDITIONAL` (três
   defeitos conhecidos e documentados; não é erro de instalação).

4. Carregue sem instalar, para testar: `claude --plugin-dir ~/plugins/wx-claude-code`.
   Ou instale de vez: `claude plugin marketplace add ~/plugins` e
   `claude plugin install wx-claude-code@wx-claude-code`.

**Conferir.** Numa sessão nova, peça «liste as skills e os agentes com prefixo
`wx-claude-code:`». Devem aparecer 5 comandos, 3 skills e 84 agentes. A
listagem que o modelo devolve pode omitir um item; confira por nome, não
por contagem.

**Validar o pacote** (roda os 13 testes de regressão):

```bash
python3 wx-claude-code/skills/conversao-wx/scripts/validate_plugin_bundle.py wx-claude-code --strict
claude plugin validate wx-claude-code
```

---

## 2. Comandos `/` do plugin

| Comando | O que faz | Quando |
| --- | --- | --- |
| `/wx-claude-code:questionario <projeto>` | o wizard: dez perguntas A–J, uma por vez; gera `.wx-migration/` | sempre primeiro |
| `/wx-claude-code:converter <modo> <projeto>` | conversão por gates G0–G7; `modo` é `inventario`, `plano`, `piloto` ou `completo` | depois do wizard |
| `/wx-claude-code:pmo <ação> <projeto>` | gerência: `iniciar`, `status`, `sprint`, `kanban`, `pdca`, `orcamento`, `entregar`, `painel` | durante toda a conversão |
| `/wx-claude-code:estilo-telas <projeto>` | paleta, tema e tipografia viram `PRODUCT.md` e `DESIGN.md` pelo Impeccable | quando a letra F foi «sim» |
| `/wx-claude-code:laudo-tokens [fase]` | auditoria de consumo em três fases, somente leitura | quando quiser medir o custo |
| `/impeccable <comando> <alvo>` | os 23 comandos de qualidade gráfica: `shape`, `polish`, `audit`, `critique`, `harden`… | em cada tela convertida |

Os comandos têm scripts por trás, que você também pode rodar direto. Todos
ficam em `$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/`:

| Script | Faz |
| --- | --- |
| `aplicar_questionario.py` | respostas do wizard viram manifesto, configuração, `CLAUDE.md` e `DESIGN.md` |
| `wx_preflight.py` | Gate G0: confere cada anexo fisicamente |
| `extrair_pdf.py` | texto por página com `arquivo#page=N` e hash |
| `query_wlanguage_help.py` | busca no corpus do Help por símbolo e tema |
| `golden.py` | captura resultados do legado e compara com o novo |
| `rotear_modelo.py` | escolhe o modelo Claude por classe de tarefa e orçamento |
| `pmo.py` | plano, sprint, kanban, PDCA, painel, entrega |
| `uso_de_tokens.py` | lê o consumo real das sessões e lança no orçamento |

**Atalhos.** Crie os seus em `.claude/commands/<nome>.md` no projeto. Exemplo
para polir telas com as regras da conversão já embutidas:

```markdown
---
description: Polir uma tela convertida preservando campos, textos e paleta
argument-hint: "<caminho-da-tela>"
---
Carregue a skill impeccable e execute `polish $ARGUMENTS`.
Preserve a ordem dos campos, os textos e as validações do legado.
Cores só do DESIGN.md; contraste mínimo 4,5:1, e diga o valor medido.
```

Aí `/polir-tela src/telas/Venda.tsx` faz a passada. O Impeccable também
cria atalhos sozinho: `node "$CLAUDE_PLUGIN_ROOT/skills/impeccable/scripts/pin.mjs" pin audit` gera `/audit`.

---

## 3. Como funciona a gerência de projeto

O PMO é código, não texto. Quem responde «em que pé está, quanto custou, o
que trava, quem decide» é o agente `pmo-gerente-de-projetos` rodando
`pmo.py`. Nenhum número do painel é digitado: cada linha cita a fonte, e o
que não tem fonte aparece como `INDISPONÍVEL`, nunca como zero.

**Começar:**

```bash
/wx-claude-code:pmo iniciar ./meu-projeto
```

Cria `.wx-migration/pmo/` com plano por gate, orçamento, RAID (riscos,
premissas, issues, dependências), backlog, base de conhecimento e a pasta de
ciclos PDCA.

**Gates.** O trabalho avança em oito portões, G0 a G7, e cada um depende de
um aprovador humano:

| Gate | O que acontece | Quem aprova |
| --- | --- | --- |
| G0 | anexos conferidos fisicamente (pré-flight) | responsável pelos anexos |
| G1 | inventário, hashes, extração de texto, índice do Help | líder técnico |
| G2 | regras, telas, dados, queries, integrações, conflitos | responsável de negócio |
| G3 | arquitetura-alvo, decisões, plano de ondas, rollback | arquitetura |
| G4 | piloto vertical: uma fatia com tela, regra, query e erro, comparada ao legado | técnico + negócio |
| G5 | ondas de implementação por módulo | líder técnico |
| G6 | segurança, desempenho, concorrência, testes | qualidade |
| G7 | ensaio, reconciliação, cutover e plano de retorno | patrocinador |

O piloto (G4) nunca é pulado numa conversão completa. Quem implementa não
aprova: o `quality-auditor` tenta refutar, o humano decide.

**Os dez papéis.** Em projeto de grande porte o trabalho é distribuído por
papéis com dono: A orquestrador, B engenheiro, C DBA, D zelador, E designer,
F prova real, G QA, H documentação, I versionador, J pesquisador. Cada papel
tem quatro subagentes, Plan, Do, Check e Act, e executa todo item como um
ciclo PDCA.

**Scrum.** Uma sprint por gate ou onda. O PMO abre a sprint atribuindo o
papel dono de cada item do backlog:

```bash
pmo.py sprint abrir --nome "Onda 1 · vendas" --objetivo "..." --gate G5 --item QRY-001:C --item UI-001:E --item BR-003:F
pmo.py sprint fechar --decisao APPROVED|CONDITIONAL|REJECTED --pedido "..."
```

O fechamento escreve o resumo de doze seções em `pmo/sprints/` e devolve ao
backlog o que não atingiu a definição de pronto: evidência com localizador,
implementação apontada, teste, resultado comparado, aprovação humana,
confiança nunca `low`.

**Kanban.** `pmo.py kanban` gera o quadro da matriz de rastreabilidade com o
papel em cada cartão (`[C dba] QRY-001 …`) e limite de WIP (6 em andamento,
4 em verificação). Coluna estourada não recebe cartão; item `[sem papel]`
ninguém pega até o PMO atribuir. O quadro não se edita: muda-se o estado na
matriz e o papel no backlog.

**PDCA.** Toda hipótese de trabalho abre um ciclo com critério numérico e
fecha como frutífero ou infrutífero, gravando uma linha na base de
conhecimento nos dois casos. Infrutífero sem a próxima hipótese não fecha.

```bash
pmo.py pdca abrir --gate G4 --hipotese "..." --medida "..." --criterio "ganho >= 1,5x"
pmo.py pdca fechar --id PDCA-001 --resultado infrutifero --medido "1,06x" --aprendizado "..." --proxima "..."
```

**Entrega ao stakeholder.** Fechada a sprint:

```bash
pmo.py entregar --sprint 2 --plugin-root "$CLAUDE_PLUGIN_ROOT"
```

Gera `pmo/entregas/sprint-02-G5-<data>.zip` com o resumo da sprint, as
técnicas aplicadas com números e fonte, a base de conhecimento, o que cada
ferramenta faz (lido do cabeçalho de cada script), decisões, lacunas, RAID,
backlog e o Kanban do fechamento.

**Painel.** `pmo.py status` regenera o texto e `pmo.py painel` gera o HTML
para o aprovador abrir no navegador, em tema claro ou escuro.

---

## 4. Como funciona a economia de tokens

Três mecanismos, todos medidos.

**Balanceamento de modelos.** Antes de cada delegação o orquestrador chama
`rotear_modelo.py` com a classe da tarefa e os sinais de risco. Haiku faz o
mecânico (hash, contagem, busca no corpus), Sonnet analisa e implementa,
Opus decide e revisa. Conflito, fiscal, permissão ou dado pessoal sobem um
degrau; padrão já aprovado ou volume grande descem. Acima de 80 % do
orçamento do gate rebaixa; acima de 100 % bloqueia e o PMO decide com o
número.

**Orçamento medido, não estimado.** O Claude Code grava o consumo de cada
resposta. `uso_de_tokens.py` lê esses registros, deduplica por mensagem e
lança no orçamento do gate:

```bash
uso_de_tokens.py --project-root . resumo
uso_de_tokens.py --project-root . lancar --gate G4
```

Numa sessão real deste projeto, um único `/wx-claude-code:pmo status`
delegado a um subagente custou 305.883 tokens. É esse tipo de número que o
orçamento por gate precisa ver.

**Hábitos que o plugin impõe.** Anexos e corpus são consultados por índice,
nunca abertos inteiros. Cada especialista WLanguage lê só a sua fatia do
Help (`--group`), o que reduziu uma busca de 5,4 s para 0,5 s. Saída longa
de comando vai para `.wx-migration/logs/` e volta como localizador. Quando
a letra J do wizard é «sim», o `CLAUDE.md` do projeto recebe o estilo de
resposta direto ao ponto.

**Laudo.** `/wx-claude-code:laudo-tokens` audita em três fases. A primeira é
somente leitura e termina numa tabela de problemas por impacto; então para
e espera o seu OK. A segunda propõe uma mudança por vez. A terceira entrega
até três hábitos, só os que tiverem evidência nas suas sessões. Todo número
é `MEDIDO`, `ESTIMADO` ou `INDISPONÍVEL`.

---

## 5. Como subir os arquivos

O plugin não lê o projeto WINDEV binário. Ele lê o que a plataforma exporta.
Crie uma pasta de evidências dentro do projeto de destino, por exemplo
`inputs/`, e coloque nela:

| Arquivo | O que é | Como gerar no WINDEV |
| --- | --- | --- |
| `banco.sql` | DDL do banco: tabelas, índices, constraints, triggers, views | análise → exportar script SQL, ou dump do HFSQL |
| `codigo.pdf` | só procedures, classes e eventos | documentação técnica → filtrar código |
| `interfaces.pdf` | só janelas, páginas, controles e relatórios | documentação técnica → filtrar telas |
| `queries.pdf` | só as queries: nome, SQL, parâmetros, onde são usadas | documentação técnica → filtrar queries |
| `completo.pdf` | a documentação técnica inteira; serve de reserva se algum dos três faltar | documentação técnica → tudo |
| `screenshots/*.png` | cada tela em cada estado (normal, vazio, erro) | capturas |
| `screenshots/screenshots.json` | para cada captura: `arquivo`, `tela`, `estado`, `plataforma` | à mão |
| `dados-de-amostra/` | dados sintéticos e resultados esperados do legado (golden master) | à mão ou exportação anonimizada |

Regras:

- Os PDFs precisam ser **pesquisáveis** (texto, não imagem). Sem isso a
  extração exige OCR e o pré-flight marca `OCR_REQUIRED`.
- Os anexos são **somente leitura**. O plugin nunca escreve neles; tudo que
  gera vai para `.wx-migration/`.
- Nada de senha, token, certificado ou dado real de pessoa. Dados de amostra
  são sintéticos ou anonimizados.
- Anexo dentro de zip: o plugin descompacta com `safe_unpack_bundle.py`
  numa pasta nova, com defesa contra travessia de caminho e zip bomb.

O projeto de exemplo `exemplos/estoque-wx/` tem tudo isso montado. Use-o
como modelo da pasta e como ensaio antes do seu projeto.

---

## 6. Como invocar o wizard

O wizard é o questionário A–J. É sempre o primeiro comando de um projeto:

```text
/wx-claude-code:questionario ./meu-projeto
```

**Como ele se comporta.** Pergunta **uma letra por mensagem** e espera. Você
responde, ele confirma em uma linha o que registrou (`A: inputs/banco.sql,
HFSQL 2025 → provided`) e só então faz a próxima. A resposta decide o
caminho: sem o PDF de código em B, ele avisa em E que o completo vai
cobrir; «não» ao Impeccable em F pula paleta e tipografia; mobile em I pede
versões de Android e iOS. Quem não tem um item responde «não tenho», e isso
vira `missing` no manifesto, nunca `not_applicable` por inferência.

Um caminho só conta como fornecido depois que o wizard **abre o arquivo**.

| Letra | Pergunta |
| --- | --- |
| A | caminho do `.SQL`, dialeto, versão do banco, encoding, collation |
| B | PDF só dos códigos; é pesquisável? |
| C | PDF só das interfaces |
| D | PDF só das queries |
| E | PDF completo |
| F | estilo das telas com o Impeccable: paleta, tema, tipografia, densidade, preservar ou redesenhar |
| G | usar o corpus do Help WLanguage 12k? há override da sua versão? |
| H | para qual linguagem converter o backend (capítulo 7) |
| I | para qual linguagem e plataforma converter o frontend (capítulo 7) |
| J | ativar a economia de tokens? |

E três perguntas de governança no fim: versão e idioma do WX; modo
(`inventário`, `plano`, `piloto`, `completo`); quem aprova.

**O que sai:**

```text
.wx-migration/
  questionario.json          suas respostas
  wx-inputs.manifest.json    manifesto que o pré-flight lê
  conversion.config.json     modo, destino, fidelidade
  gaps.md, traceability.csv  vazios, prontos para o G1
CLAUDE.md                    regras do projeto (com estilo de resposta se J = sim)
DESIGN.md                    esboço da paleta (se F = sim)
```

**Repetir o wizard** é seguro: o script nunca sobrescreve arquivo que já
existe. Para refazer do zero, apague `.wx-migration/` antes.

**Sem sessão interativa** (`claude -p`), o wizard faz a mesma coisa em
texto, uma letra por turno, e você continua com `claude -c "resposta"`.

---

## 7. Como definir a linguagem e a plataforma de destino

Esta é a decisão que mais muda o projeto, e por isso o wizard **orienta
antes de perguntar**, na letra H.

**Se você já sabe**, responda a linguagem e siga para framework, banco,
versões mínimas e implantação.

**Se não sabe**, o wizard faz quatro perguntas de sinal, uma por vez:

1. Quem vai manter o código depois: a equipe WINDEV de hoje ou outra?
2. O produto é desktop, web ou mobile?
3. Volume e desempenho importam, ou o prazo manda?
4. Há linguagem já em uso na empresa?

Com os sinais, mostra **três opções com o porquê em uma frase**, a
recomendada primeiro. Estas três estão sempre presentes:

| Perfil | Ganha | Custa | Serve para |
| --- | --- | --- | --- |
| **Rust** (Axum + PostgreSQL) | desempenho previsível, binário único, erros pegos em compilação | curva alta, equipe rara | volume alto, motor de cálculo, quem já usa o PhxSql |
| **Python** (FastAPI + PostgreSQL) | entrega rápida, biblioteca para fiscal, relatório e dados | desempenho por processo, deploy com runtime | sistemas de gestão que vão evoluir rápido |
| **C# (.NET 8) + WL_C#** | a biblioteca WL_C# porta mais de 480 funções do WLanguage com o mesmo nome; tradução das procedures quase mecânica | HFSQL e telas ficam fora da biblioteca; código fechado | a equipe WINDEV que vai manter o código; desktop Windows |

Go, Java e Node entram quando os sinais apontarem. A escolha é sua e vira
`DEC-0001` na abertura do G3.

**Plataforma e frontend (letra I).** React (TypeScript) é o padrão para
web. Blazor se H foi C#; Flutter se há Android e iOS; Tauri (Rust + React)
se o produto continua desktop; Vue ou Svelte para equipes pequenas. Depois:
plataformas (web, desktop, Android, iOS), navegadores e dispositivos
mínimos.

**Tabela de decisão** (a linha que mais casa é a recomendação):

| Se… | Backend | Frontend |
| --- | --- | --- |
| a equipe WINDEV de hoje mantém e quer a menor mudança | C# + WL_C# | Blazor ou React |
| é WEBDEV, ou vai para a web, e o time de front vai crescer | Python ou Node | React |
| há cálculo pesado, volume alto ou o motor é o PhxSql | Rust | React, ou Tauri se desktop |
| é WINDEV Mobile com Android e iOS | Python ou Go (API) | Flutter |
| muito relatório, fiscal e integração, e o prazo manda | Python | React |
| já existe Java ou .NET na empresa | Java ou C# | React ou Blazor |

**Sobre o WL_C#.** É a biblioteca de Bernard Sobra
(https://bernardsobra.github.io/WL-web/). O plugin traz um índice de 261
funções lido do `WL.dll` 1.0 e o hash da release; o DLL você baixa da
release oficial, e o especialista de funções padrão marca cada função como
`equivalente`, `adaptar` ou `substituir`. HFSQL, telas, comunicação e
relatórios seguem pelos outros especialistas, em qualquer perfil.

**Duas regras que não mudam com o perfil.** O banco de destino é decisão
separada, no G3. E regra de negócio não muda de comportamento por causa da
linguagem: o golden master compara o novo com o legado seja qual for o
destino.

---

## Apêndice: problemas comuns

- **`BLOCKED` no G0.** Leia `preflight/runs/<run>/report.md`; cada erro diz o grupo e o arquivo. Enquanto estiver bloqueado, o hook do plugin nega qualquer escrita de código fora de `.wx-migration/`.
- **Skill não aparece na sessão.** Descrição acima de 300 caracteres some da listagem; o validador avisa.
- **Corpus com hash divergente.** Não use; o zip certo tem 26.750.976 bytes.
- **Orçamento estourado.** `rotear_modelo.py` devolve `BLOQUEADO`; decida no PMO com o número.
- **Ciclo PDCA infrutífero não fecha.** Faltou `--proxima`.
- **`extrair_pdf.py` recusa.** Falta `pypdf` ou `pdfminer.six`; ele diz isso em vez de inventar texto.

## O que o plugin não faz

Não lê o formato binário do WX, não faz OCR sozinho, não certifica LGPD, não
aprova gate no lugar do humano e não afirma equivalência sem baseline
executável do legado.
