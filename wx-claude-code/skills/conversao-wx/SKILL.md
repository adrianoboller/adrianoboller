---
name: "conversao-wx"
description: "Inventario, plano, piloto e conversao rastreavel de projetos WINDEV, WEBDEV e WINDEV Mobile, com evidencias e gates."
license: "All rights reserved"
compatibility: "Claude Cowork and Claude Code; Python 3.10 or newer for bundled helper scripts"
metadata: {"author":"Adriano Boller","version":"3.0.0"}
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, Agent, AskUserQuestion"
---

# Conversão de projeto WX

Trate `$ARGUMENTS` como indicação, nunca como confirmação de completude.

## 1. Intake obrigatório no contexto principal

No plugin, o intake começa pelo **questionário A–J** (`/wx-claude-code:questionario`), que grava `.wx-migration/questionario.json` e gera manifesto e configuração pelo `scripts/aplicar_questionario.py`. Se esse arquivo existir, pule para as perguntas abaixo que continuarem em aberto. Se não existir, faça-as todas.

Antes de delegar ou alterar arquivos, faça perguntas ao usuário. Subagentes não devem conduzir esta etapa.

Use `AskUserQuestion` no contexto principal quando estiver disponível; se não estiver, faça as mesmas perguntas em texto e aguarde as respostas antes do G0.

Pergunte em rodadas curtas:

1. “Você já disponibilizou os anexos em uma pasta que o Claude Code consegue ler? Qual é o caminho autorizado? Qual é a raiz do projeto de destino?”
2. “Qual é a versão/update do WINDEV, WEBDEV ou WINDEV Mobile e o idioma do projeto? O corpus WLanguage 12k incluído no plugin será a fonte técnica auxiliar; existe também Help específico da release que precise ser tratado como override?”
3. “Os anexos estão separados em uma pasta ou dentro de um ZIP? Foram entregues os PDFs completos de código, telas, eventos, queries e regras de negócio, o script SQL, screenshots de todos os estados e os links/documentações de APIs ou fontes auxiliares?”
4. “Há também o projeto WX original, procedimentos, classes, relatórios, componentes, DLLs, webservices, assets, exemplos de dados e resultados esperados? O que não existe ou não se aplica?”
5. “Qual é o destino: linguagem, framework de UI/web/mobile, banco, plataformas, arquitetura, versões mínimas e forma de implantação?”
6. “Existem dados anonimizados, baseline, build/configuração exatos, ambiente reinicializável e autorização para executar o legado? Há uma referência segura para credenciais de teste, sem enviar valores secretos?”
7. “Qual modo deseja agora: inventário, plano, piloto vertical ou implementação completa? Quem aprova regras, divergências e cada critério de aceite?”
8. “Deseja habilitar algum companion opcional — Impeccable, Taste Skill, Higgsfield ou Sheets — e possui autorização, conta/licença e orçamento/créditos quando aplicável? A ausência deles não bloqueia a conversão.”

Não aceite “está tudo anexado” sem verificar caminhos, formatos, legibilidade e contagem. Antes de existir um caminho verificável, use o estado `INTAKE_PENDING`: faça somente perguntas e não prometa relatórios ainda. Se a pessoa não souber, ofereça gerar os modelos de manifesto e configuração e pare no inventário.

Mapeie os nomes informados pelo usuário para o arquivo de configuração: `inventário → inventory`, `plano → plan`, `piloto → pilot`, `completo → complete`.

## 2. Gate G0 — pré-flight

Leia [intake-and-evidence.md](references/intake-and-evidence.md). Use os recursos autocontidos da skill por `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx`.

Se a entrada for um ZIP misto, peça autorização e descompacte-o primeiro em uma pasta nova dentro de `.wx-migration/imports/`:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/safe_unpack_bundle.py" \
  --archive <zip-dentro-da-raiz-autorizada> \
  --allowed-evidence-root <pasta-que-contém-o-zip> \
  --workspace-root <projeto> \
  --output <projeto>/.wx-migration/imports/<id-da-importação>
```

O utilitário recusa travessia, links, colisões, segredos evidentes, ZIP bombs e saídas existentes; nunca executa os anexos. Use a nova pasta como raiz de evidências. O corpus WLanguage que acompanha o plugin não deve ser extraído nem copiado para essa raiz.

Se ainda não houver manifesto, após confirmar a raiz do projeto e a raiz de evidências execute o bootstrap seguro (ele não sobrescreve arquivos):

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/bootstrap_workspace.py" \
  --project-root <projeto> \
  --evidence-root <anexos-ou-importação-descompactada> \
  --install-claude-md
```

Peça ao usuário para completar os campos desconhecidos. Então execute o pré-flight sem sobrescrever arquivos existentes:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/wx_preflight.py" \
  --manifest <caminho>/wx-inputs.manifest.json \
  --allowed-evidence-root <anexos-ou-importação-descompactada> \
  --workspace-root <projeto> \
  --output <projeto>/.wx-migration/preflight
```

Cada execução é versionada e não sobrescreve relatórios anteriores. No Windows, substitua `python3` por `py -3`.

- `BLOCKED`: não escreva código de produção. Entregue inventário, perguntas e `gaps.md`.
- `CONDITIONAL`: só prossiga no escopo explicitamente aprovado; marque riscos e itens sem prova.
- `READY`: significa apenas pronto para G1/inventário, não pronto para implementação.

## 3. Orquestração

Leia [agent-orchestration.md](references/agent-orchestration.md). Delegue primeiro ao agente `wx-claude-code:wx-orchestrator` com o caminho do manifesto, modo escolhido, status do pré-flight e respostas do usuário.

O orquestrador deve usar agentes e subagentes especializados, paralelizando somente investigações independentes. Se os agentes do plugin não estiverem instalados, crie subagentes `general-purpose` com as funções e modelos de [agent-orchestration.md](references/agent-orchestration.md). Escritas concorrentes devem ocorrer em módulos separados ou worktrees. Nenhum agente decide requisito ausente.

## 4. Conversão por gates

Leia [conversion-workflow.md](references/conversion-workflow.md) e [deliverables-and-gates.md](references/deliverables-and-gates.md).

Execute apenas até o gate autorizado:

- G1: inventário, hashes, extração, índice do Help e mapa de evidências.
- G2: especificação comportamental, dados, telas, queries, integrações e lacunas.
- G3: arquitetura-alvo, ADRs, plano incremental e estratégia de rollback.
- G4: piloto vertical com equivalência demonstrada.
- G5: ondas de implementação por módulo.
- G6: segurança, desempenho, concorrência, observabilidade e suíte com cobertura e critérios aprovados.
- G7: ensaio de migração, aceite, cutover e rollback.

Nunca pule G4 em uma conversão completa.

Em G1, verifique primeiro o corpus fixo e somente leitura:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" --verify
```

O hash esperado é fixo. A edição distribuída remove 15 blocos de chaves privadas
demonstrativas em duas páginas. Ela ainda possui uma página inválida em
quarentena e uma lacuna de índice conhecida; portanto, trate-a como
`DEGRADED/CONDITIONAL`, não como coleção completa. Leia
[bundled-help-corpus.md](references/bundled-help-corpus.md). Para pesquisar
símbolos sem extrair o ZIP:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" \
  --query <simbolo-ou-termo> --version <versao-WX> --limit 10
```

Se o usuário fornecer um conjunto específico de Help como override da release, valide-o no manifesto e gere seu índice separado, sem sobrescrita:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/build_help_index.py" \
  --manifest <projeto>/.wx-migration/wx-inputs.manifest.json \
  --allowed-evidence-root <anexos-ou-importação-descompactada> \
  --workspace-root <projeto> \
  --output <projeto>/.wx-migration/evidence/help-override-index.jsonl \
  --summary <projeto>/.wx-migration/evidence/help-override-index.summary.json
```

## 5. Rastreabilidade e qualidade

Leia [traceability.md](references/traceability.md). Cada item implementado precisa ligar evidência → regra → decisão → código → teste → resultado. Rode:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/validate_traceability.py" \
  <projeto>/.wx-migration/traceability.csv \
  --project-root <projeto> \
  --inventory <projeto>/.wx-migration/preflight/<run>/inventory.csv
```

Antes de declarar um gate concluído, o agente `wx-claude-code:quality-auditor` deve revisar em modo somente leitura e emitir recomendação. Somente o aprovador humano decide o gate. Uma conclusão deve informar: escopo concluído, build/configuração/ambiente/dataset/tolerâncias, provas, testes executados, lacunas, decisões pendentes, riscos e próximo gate.

## 6. Regras de parada

Pare o item afetado e pergunte quando houver: anexo ilegível; hash do corpus divergente; símbolo atingido por lacuna/quarentena sem outra fonte; destino indefinido; regra conflitante; segredo ou dado pessoal não protegido; schema incompleto; dependência sem fonte/licença; consulta sem parâmetros conhecidos; comportamento impossível de reproduzir; ou teste de equivalência falhando.

Não transforme uma conversão parcial em “concluída”. Use os estados `não iniciado`, `inventariado`, `especificado`, `implementado`, `verificado`, `aceito` ou `bloqueado`. O plugin não certifica conformidade LGPD nem substitui revisão jurídica, de segurança ou do responsável de negócio.
