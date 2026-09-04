# Skills de ERP (pacote erp-skills-pack)

Oito skills vendorizadas do pacote de pesquisa feito no skills.sh em 4 de setembro de 2026, entregue pelo dono do projeto. O corpo de cada `SKILL.md` está como veio; só a `description` foi encurtada para até 150 caracteres, porque descrição longa some da listagem de uma sessão nova (medido com a `impeccable`: 895 caracteres sumia, 150 aparece). As descrições originais estão em `descricoes-originais.json`.

O questionário (L6) gera no projeto de destino o esqueleto de ERP que este pacote descreve; o `CLAUDE.md` gerado diz qual skill usar para cada módulo.

---

# Instruções e skills para desenvolvimento de ERP

**Catálogo pesquisado:** skills.sh  
**Data da pesquisa:** 4 de setembro de 2026  
**Idioma:** pt-BR

## Finalidade

Oriente o desenvolvimento de um ERP confiável, auditável e evolutivo. Trate regras empresariais, integridade contábil e fiscal, isolamento entre empresas, autorizações, rastreabilidade e recuperação de dados como requisitos centrais — não como melhorias posteriores.

Este documento consolida uma pesquisa realizada no [skills.sh](https://www.skills.sh/) em 4 de setembro de 2026. Não foi encontrada uma única skill madura que cubra todo o ciclo de um ERP. A estratégia recomendada é combinar skills especializadas e manter skills próprias para regras de domínio que não estejam bem representadas no catálogo.

> Este arquivo é o índice detalhado da pesquisa. As oito skills instaláveis ficam em `skills/<nome>/SKILL.md`; cada pasta pode ser instalada ou copiada separadamente para o diretório de skills da ferramenta usada.

## Estrutura deste pacote

```text
erp-skills-pack/
├── instruções_skill.md
└── skills/
    ├── erp-accounting/SKILL.md
    ├── erp-inventory/SKILL.md
    ├── erp-brazil-fiscal/
    │   ├── SKILL.md
    │   └── references/official-sources.md
    ├── erp-multi-company/SKILL.md
    ├── erp-approval-workflows/SKILL.md
    ├── erp-lgpd/SKILL.md
    ├── erp-integration-reliability/SKILL.md
    └── windev-wlanguage-erp/SKILL.md
```

As skills são independentes. Instalar uma delas não instala nem invoca as demais. Se uma tarefa atravessar vários domínios, carregue apenas as skills necessárias e determine qual domínio é dono de cada regra.

| Skill | Use quando a tarefa envolver |
|---|---|
| [`erp-accounting`](skills/erp-accounting/SKILL.md) | razão, partidas dobradas, plano de contas, conciliação, estorno, fechamento ou multimoeda |
| [`erp-inventory`](skills/erp-inventory/SKILL.md) | saldo, reserva, movimentação, lote, série, depósito, inventário ou custeio |
| [`erp-brazil-fiscal`](skills/erp-brazil-fiscal/SKILL.md) | tributação brasileira, documento fiscal eletrônico, evento fiscal ou SPED |
| [`erp-multi-company`](skills/erp-multi-company/SKILL.md) | empresa, filial, estabelecimento, grupo econômico, intercompany ou isolamento de tenant |
| [`erp-approval-workflows`](skills/erp-approval-workflows/SKILL.md) | alçada, aprovação, rejeição, delegação, escalonamento ou segregação de funções |
| [`erp-lgpd`](skills/erp-lgpd/SKILL.md) | dados pessoais, retenção, direitos do titular, anonimização, operador ou incidente |
| [`erp-integration-reliability`](skills/erp-integration-reliability/SKILL.md) | API, webhook, fila, evento, lote, idempotência, retry, DLQ ou reconciliação |
| [`windev-wlanguage-erp`](skills/windev-wlanguage-erp/SKILL.md) | implementação ou revisão de ERP em WINDEV, WEBDEV, WINDEV Mobile ou WLanguage |

## Resultado esperado

Ao executar uma tarefa de ERP:

1. Entenda o processo empresarial e registre sua linguagem antes de escolher tabelas ou endpoints.
2. Defina módulos e limites de contexto. Em projetos greenfield, considere primeiro um monólito modular; em sistemas existentes, preserve o stack e a arquitetura salvo migração justificada.
3. Modele transações e invariantes de forma explícita.
4. Preserve isolamento multiempresa, autorização no servidor e trilha de auditoria.
5. Trate dinheiro, estoque, documentos fiscais e lançamentos contábeis com tipos e regras adequados.
6. Produza documentação, testes e evidências de verificação junto com a implementação.
7. Prepare migrações, observabilidade, backup, restauração e resposta a incidentes antes da entrada em produção.

## Princípios obrigatórios

### Domínio antes da infraestrutura

- Identifique atores, eventos, comandos, entidades, agregados, estados e exceções.
- Crie uma linguagem ubíqua; um mesmo conceito não deve ter nomes conflitantes no produto, código, banco e documentação.
- Registre invariantes que nunca podem ser violadas, por exemplo: a mesma quantidade não pode ser sobre-reservada; lançamento contábil deve ser balanceado; documento fiscal autorizado não pode ser alterado livremente.
- Modele processos completos, incluindo cancelamento, estorno, reprocessamento, conciliação e falha parcial.

### Arquitetura inicial ou existente

- Em um ERP greenfield sem restrição prévia, considere monólito modular, modelagem orientada ao domínio e banco relacional como defaults de avaliação, não como imposições.
- Em um ERP existente, preserve HFSQL, MySQL, SQL Server, Oracle, PostgreSQL, WLanguage ou outro stack adotado, salvo evidência de que uma migração oferece benefício maior que seu custo e risco.
- Separe domínio, casos de uso, interfaces, persistência e integrações.
- Não adote microsserviços, event sourcing ou CQRS apenas por tendência. Exija uma necessidade mensurável, como escala independente, isolamento operacional ou auditoria histórica incompatível com o modelo atual.
- Registre decisões significativas em ADRs.

### Integridade dos dados

- Use transações para alterações que precisam ser atômicas.
- Use `NUMERIC`/`DECIMAL` para valores monetários; nunca ponto flutuante binário.
- Em PostgreSQL, use `TIMESTAMPTZ` para instantes. Em outros bancos, escolha o tipo equivalente e mantenha regras explícitas para datas civis, competência e fuso horário.
- Imponha integridade também no banco: chaves estrangeiras, `NOT NULL`, `UNIQUE`, `CHECK` e índices coerentes.
- Torne operações externas idempotentes quando puderem ser repetidas.
- Não altere dados financeiros, fiscais ou auditáveis sem preservar o histórico apropriado.

### Segurança e multiempresa

- Negue acesso por padrão.
- Faça autenticação e autorização no servidor; a interface nunca é a autoridade final.
- Verifique empresa, filial, usuário, papel, ação e recurso em cada operação protegida.
- Nunca aceite `tenant_id` ou identificador equivalente sem validar que o usuário pertence à empresa indicada.
- Registre ações privilegiadas, mudanças de permissão, aprovações, exportações e operações financeiras relevantes.
- Proteja a trilha de auditoria contra alteração indevida; limite exclusão, registre origem e considere armazenamento imutável para eventos de alto risco.
- Aplique segregação de funções em fluxos sensíveis, evitando que a mesma identidade solicite, aprove e liquide a própria operação sem controle compensatório.
- Proteja segredos fora do repositório.
- Converta ameaças em requisitos e testes verificáveis.

### Experiência operacional

- Projete para usuários treinados que executam tarefas repetitivas e de alto volume.
- Priorize densidade informacional, consistência, atalhos de teclado, filtros persistentes, edição segura e feedback imediato.
- Formulários devem explicar obrigatoriedade, formato, erro e impacto antes da confirmação.
- Tabelas devem suportar ordenação, filtros adequados ao tipo, paginação ou virtualização, seleção segura, exportação autorizada e estados vazios claros.
- Siga WCAG 2.2: semântica, foco visível, navegação por teclado, contraste e anúncios acessíveis de erros e mudanças de estado.

## Fluxo de trabalho recomendado

### 1. Descoberta e requisitos

Antes de implementar, obtenha ou produza:

- objetivo do produto e métricas de sucesso;
- perfis de usuário e responsabilidades;
- empresas, filiais, moedas, idiomas e regimes relevantes;
- módulos do escopo e dependências entre eles;
- fluxos normais, exceções, cancelamentos e estornos;
- requisitos fiscais, contábeis, legais e de retenção;
- integrações, volumes, janelas operacionais e SLAs;
- critérios de aceite observáveis.

Se informação crítica estiver ausente, explicite a hipótese e confirme-a antes de uma decisão irreversível.

### 2. Linguagem e modelagem do domínio

Mantenha `UBIQUITOUS_LANGUAGE.md`, `CONTEXT.md` e `CONTEXT-MAP.md`. Para cada módulo, registre:

- responsabilidades e limites;
- entidades e identificadores;
- estados e transições permitidas;
- invariantes;
- comandos e eventos;
- políticas e aprovações;
- erros esperados e compensações;
- dados que pertencem ao módulo;
- contratos com outros módulos.

Exemplos comuns de contextos: identidade e acesso, cadastros, comercial, compras, estoque, preços, financeiro, contabilidade, fiscal, manufatura, serviços, ativos, folha, relatórios e integrações.

### 3. Decisões arquiteturais

Escreva ADR para decisões com custo de reversão significativo. Cada ADR deve conter contexto, decisão, alternativas, consequências, riscos e condição de revisão.

Decisões típicas:

- monólito modular versus serviços;
- estratégia multiempresa;
- banco e particionamento;
- mensageria e padrão de outbox;
- autenticação, autorização e SSO;
- auditoria e retenção;
- processamento síncrono versus assíncrono;
- abordagem de integração e idempotência.

### 4. Dados e transações

Produza modelo conceitual, ERD, dicionário de dados e migrações versionadas. Para cada tabela e fluxo crítico, verifique:

- chave primária e chaves naturais relevantes;
- escopo de empresa/filial;
- constraints e política de exclusão;
- índices alinhados às consultas reais;
- concorrência, locks e condição de corrida;
- precisão de valores e unidades;
- trilha de criação, alteração, aprovação e cancelamento;
- estratégia de migração e rollback ou roll-forward;
- impacto em tabelas grandes e operação online.

### 5. APIs e integrações

- Defina contratos antes de acoplar consumidores.
- Use OpenAPI 3.1 para APIs HTTP e AsyncAPI quando houver eventos relevantes.
- Padronize autenticação, paginação, filtros, ordenação, erros, correlação e versionamento.
- Valide entradas e saídas nos limites do sistema.
- Para webhooks, filas e provedores externos, defina idempotência, timeout, retry com backoff, limite de tentativas, dead-letter, reconciliação e observabilidade.
- Não esconda falhas externas; exponha estado pendente, falho e reprocessável quando fizer sentido.

### 6. Segurança e privacidade

Produza modelo de ameaças, matriz RBAC/ABAC e mapeamento LGPD. Verifique:

- separação entre autenticação e autorização;
- menor privilégio e segregação de funções;
- isolamento de tenant em consultas, cache, filas, arquivos, relatórios e logs;
- proteção contra enumeração de identificadores;
- criptografia em trânsito e, quando necessário, em repouso;
- classificação, finalidade, retenção e descarte de dados pessoais;
- exportação, anonimização e atendimento aos direitos do titular;
- auditoria de ações privilegiadas sem registrar segredos desnecessários;
- limites de taxa e proteção contra abuso.

### 7. Interface do usuário

Para cada tela, defina tarefa principal, frequência de uso, risco de erro e volume de dados. Exija:

- componentes consistentes;
- estados de carregamento, vazio, erro, sucesso e permissão negada;
- confirmação proporcional ao risco;
- prevenção de envio duplicado;
- preservação de rascunho quando necessário;
- filtros por tipo de dado;
- teclado e leitor de tela;
- formatação de moeda, quantidade, unidade, data e fuso;
- explicação clara de bloqueios contábeis, fiscais e de aprovação.

### 8. Testes e verificação

Use testes orientados a comportamento e risco. A pirâmide mínima deve cobrir:

- unidade e propriedade para regras e cálculos;
- integração para banco, filas, arquivos e provedores;
- contratos para APIs e eventos;
- end-to-end para jornadas críticas;
- segurança para autorização, isolamento multiempresa e abuso;
- acessibilidade automatizada e manual;
- migração com volume representativo;
- backup e restauração executados de verdade.

Não declare a tarefa concluída usando apenas inspeção visual ou resultados antigos. Execute comandos de verificação recentes e registre evidências relevantes.

### 9. Produção e continuidade

Antes do go-live, defina:

- logs estruturados com correlação;
- métricas técnicas e de negócio;
- rastreamento para fluxos distribuídos;
- SLI, SLO e alertas acionáveis;
- RPO, RTO, retenção e cópias de backup;
- teste periódico de restauração;
- runbooks de incidente e degradação;
- plano de rollback ou roll-forward;
- reconciliação financeira, fiscal e de integrações.

### 10. Migração de legado e cutover

Quando substituir ou consolidar um sistema existente, trate a migração como uma entrega própria:

- inventarie fontes, responsáveis, volumes, qualidade e sensibilidade dos dados;
- defina o de/para de entidades, códigos, unidades, contas, impostos, estados e históricos;
- identifique duplicidades, dados inválidos e regras de saneamento;
- estabeleça data de corte, freeze, responsáveis e janela operacional;
- migre cadastros, documentos necessários, saldos de abertura e vínculos de rastreabilidade;
- faça ensaios completos com medição de tempo e taxa de erro;
- reconcilie contabilidade, contas a receber/pagar, caixa, estoque, fiscal e totais de controle;
- obtenha aceite formal das áreas responsáveis;
- mantenha plano testado de rollback ou correção progressiva;
- preserve consulta ao histórico legado pelo prazo aplicável.

Nunca considere contagem de linhas como reconciliação suficiente. Compare também valores, saldos, estados, relações e amostras de documentos ponta a ponta.

## Arquivos importantes no repositório do ERP

Adapte a árvore ao stack; não crie arquivos vazios apenas para cumprir a lista.

```text
ERP/
├── AGENTS.md
├── CLAUDE.md
├── PRODUCT.md
├── CONTEXT.md
├── CONTEXT-MAP.md
├── UBIQUITOUS_LANGUAGE.md
├── ARCHITECTURE.md
├── DESIGN.md
├── SECURITY.md
├── docs/
│   ├── PRD.md
│   ├── adr/
│   │   ├── 0001-arquitetura-inicial.md
│   │   ├── 0002-postgresql.md
│   │   └── 0003-multiempresa.md
│   ├── domain/
│   │   ├── modules.md
│   │   ├── invariants.md
│   │   ├── workflows.md
│   │   ├── accounting.md
│   │   ├── inventory.md
│   │   └── fiscal.md
│   ├── data/
│   │   ├── erd.md
│   │   ├── data-dictionary.md
│   │   └── legacy-mapping.md
│   ├── api/
│   │   ├── openapi.yaml
│   │   └── events.asyncapi.yaml
│   ├── security/
│   │   ├── threat-model.md
│   │   ├── rbac-matrix.md
│   │   └── lgpd.md
│   └── runbooks/
│       ├── backup-restore.md
│       ├── incident-response.md
│       └── cutover.md
├── db/
│   ├── schema.sql
│   ├── migrations/
│   └── seeds/
└── tests/
    ├── domain/
    ├── integration/
    ├── contracts/
    └── e2e/
```

### Função de cada arquivo principal

| Arquivo | Conteúdo mínimo | Quando atualizar |
|---|---|---|
| `PRODUCT.md` | visão, usuários, problemas, escopo, métricas e restrições | mudança de direção do produto |
| `docs/PRD.md` | requisitos, jornadas e critérios de aceite | nova entrega ou alteração funcional |
| `UBIQUITOUS_LANGUAGE.md` | termos oficiais, sinônimos proibidos e exemplos | novo conceito ou ambiguidade descoberta |
| `CONTEXT.md` | visão dos domínios, entidades, invariantes e casos de uso | evolução da modelagem |
| `CONTEXT-MAP.md` | limites e relações entre módulos | criação ou mudança de integração interna |
| `ARCHITECTURE.md` | visão estrutural, dependências e requisitos não funcionais | mudança arquitetural ampla |
| `docs/adr/*.md` | decisão, alternativas e consequências | cada decisão relevante e durável |
| `DESIGN.md` | padrões de interação, componentes e comportamento das telas | evolução do sistema visual/UX |
| `SECURITY.md` | modelo de segurança, reporte e controles gerais | nova ameaça, controle ou incidente |
| `docs/security/rbac-matrix.md` | papéis, recursos, ações, escopos e segregação | mudança de permissão |
| `docs/security/threat-model.md` | ativos, fronteiras, ameaças e mitigações | nova superfície ou integração |
| `docs/security/lgpd.md` | dados pessoais, finalidade, base, retenção e direitos | mudança no tratamento de dados |
| `docs/data/erd.md` | relações e cardinalidades | mudança relevante de schema |
| `docs/data/data-dictionary.md` | semântica, tipo, unidade, origem e sensibilidade | coluna ou regra nova |
| `docs/data/legacy-mapping.md` | fontes, de/para, saneamento, saldos e reconciliação | migração ou consolidação de legado |
| `docs/api/openapi.yaml` | contrato HTTP, schemas, erros e segurança | mudança de API |
| `docs/api/events.asyncapi.yaml` | eventos, payloads, produtores e consumidores | mudança assíncrona |
| `db/migrations/` | mudanças versionadas e repetíveis | toda alteração de banco |
| `docs/runbooks/backup-restore.md` | RPO/RTO, procedimento e evidência de restauração | mudança operacional ou teste periódico |
| `docs/runbooks/incident-response.md` | detecção, contenção, comunicação e recuperação | aprendizado de incidente/exercício |
| `docs/runbooks/cutover.md` | ensaios, freeze, execução, aceite e retorno | cada versão do plano de virada |
| `tests/` | provas automatizadas por risco e camada | junto com cada mudança de comportamento |

`AGENTS.md` e `CLAUDE.md` são opcionais e dependem das ferramentas de IA usadas. Evite duplicar regras conflitantes entre eles; mantenha uma fonte canônica e referências curtas quando ambos forem necessários.

## Skills recomendadas do skills.sh

Os estados de auditoria abaixo são um retrato da data da pesquisa e podem mudar. A aprovação nos scanners reduz risco, mas não substitui a leitura do `SKILL.md`, scripts e dependências.

### Núcleo: produto e domínio

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [prd](https://www.skills.sh/github/awesome-copilot/prd) | PRD, escopo, requisitos e critérios de aceite | 3 auditorias aprovadas |
| [to-questionnaire](https://www.skills.sh/mattpocock/skills/to-questionnaire) | transforma lacunas do briefing em perguntas objetivas | 3 auditorias aprovadas |
| [ubiquitous-language](https://www.skills.sh/mattpocock/skills/ubiquitous-language) | cria e mantém `UBIQUITOUS_LANGUAGE.md` | 3 auditorias aprovadas |
| [domain-modeling](https://www.skills.sh/mattpocock/skills/domain-modeling) | contextos, entidades, invariantes, `CONTEXT.md` e `CONTEXT-MAP.md` | 3 auditorias aprovadas |

### Núcleo: arquitetura e decisões

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [codebase-design](https://www.skills.sh/mattpocock/skills/codebase-design) | estrutura coerente de módulos e dependências | 3 auditorias aprovadas |
| [architecture-decision-records](https://www.skills.sh/wshobson/agents/architecture-decision-records) | ADRs para decisões duráveis | 3 auditorias aprovadas |

Use [architecture-patterns](https://www.skills.sh/wshobson/agents/architecture-patterns) apenas após revisão: havia alerta no Snyk na data da consulta.

### Núcleo: banco de dados

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [postgresql-table-design](https://www.skills.sh/wshobson/agents/postgresql-table-design) | normalização, tipos, constraints, índices e particionamento | 3 auditorias aprovadas |
| [supabase-postgres-best-practices](https://www.skills.sh/supabase/agent-skills/supabase-postgres-best-practices) | consultas, locks, pooling, segurança e RLS; útil mesmo sem Supabase | 3 auditorias aprovadas |

Para migrações de produção, avalie também [postgres-database-migration](https://www.skills.sh/timescale/pg-aiguide/postgres-database-migration) e revise sua versão e auditorias antes da adoção.

### Núcleo: APIs, identidade e acesso

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [api-design-principles](https://www.skills.sh/wshobson/agents/api-design-principles) | recursos, paginação, erros e versionamento | 3 auditorias aprovadas |
| [openapi-spec-generation](https://www.skills.sh/wshobson/agents/openapi-spec-generation) | OpenAPI 3.1, componentes, documentação e SDKs | 3 auditorias aprovadas |
| [auth-implementation-patterns](https://www.skills.sh/wshobson/agents/auth-implementation-patterns) | sessão/JWT/OAuth, RBAC, SSO e multitenancy | 3 auditorias aprovadas |
| [access-control-patterns](https://www.skills.sh/afu-it/security-for-vibecoders/access-control-patterns) | negação por padrão, autorização server-side, tenant scope e auditoria | 3 auditorias aprovadas |

### Núcleo: segurança

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [stride-analysis-patterns](https://www.skills.sh/wshobson/agents/stride-analysis-patterns) | modelagem sistemática de ameaças STRIDE | 3 auditorias aprovadas |
| [security-requirement-extraction](https://www.skills.sh/wshobson/agents/security-requirement-extraction) | converte ameaças em requisitos, testes e rastreabilidade | 3 auditorias aprovadas |
| [code-security](https://www.skills.sh/semgrep/skills/code-security) | diretrizes de codificação segura e padrões OWASP em várias linguagens; não substitui um scanner SAST | 3 auditorias aprovadas |

Se precisar executar análise estática com Semgrep, avalie [semgrep](https://www.skills.sh/semgrep/skills/semgrep) separadamente e revise o alerta Snyk observado na data da consulta.

### Núcleo: interface operacional de ERP

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [operational-expert-tool-ui](https://www.skills.sh/dembrandt/dembrandt-skills/operational-expert-tool-ui) | telas B2B densas para usuários treinados e tarefas repetitivas | 3 auditorias aprovadas |
| [form-design](https://www.skills.sh/dembrandt/dembrandt-skills/form-design) | formulários empresariais, validação e redução de erro | 3 auditorias aprovadas |
| [data-display-and-selection](https://www.skills.sh/dembrandt/dembrandt-skills/data-display-and-selection) | exibição, comparação e seleção de grandes conjuntos de dados | 3 auditorias aprovadas |
| [table-filters](https://www.skills.sh/shipshitdev/skills/table-filters) | filtros coerentes por texto, status, data, valor e booleano | 3 auditorias aprovadas |
| [component-library](https://www.skills.sh/shipshitdev/skills/component-library) | biblioteca de componentes consistente | 3 auditorias aprovadas |
| [wcag-audit-patterns](https://www.skills.sh/wshobson/agents/wcag-audit-patterns) | auditoria WCAG 2.2 e plano de correção | revalidar antes da instalação |
| [better-accessibility](https://www.skills.sh/jakubkrehel/skills/better-accessibility) | semântica, teclado, foco e leitores de tela | revalidar antes da instalação |

[wcag-accessibility](https://www.skills.sh/dembrandt/dembrandt-skills/wcag-accessibility) passou nas três auditorias consultadas e pode apoiar implementação, mas seu enquadramento regulatório é centrado em EAA/EN 301 549. Não o trate como referência legal brasileira; mantenha requisitos nacionais em documentação interna validada.

Evite colocar [web-design-guidelines](https://www.skills.sh/vercel-labs/agent-skills/web-design-guidelines) ou `impeccable` no núcleo sem inspeção: foram observados alertas de auditoria.

### Núcleo: testes e evidência de conclusão

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [tdd](https://www.skills.sh/mattpocock/skills/tdd) | fatias verticais e testes de comportamento | 3 auditorias aprovadas |
| [risk-based-testing](https://www.skills.sh/petrkindlmann/qa-skills/risk-based-testing) | prioriza testes pelo impacto e probabilidade de falha | 3 auditorias aprovadas |
| [webapp-testing](https://www.skills.sh/anthropics/skills/webapp-testing) | testes funcionais reais com Playwright | 3 auditorias aprovadas |
| [verification-before-completion](https://www.skills.sh/obra/superpowers/verification-before-completion) | exige evidência recente antes de declarar sucesso | 3 auditorias aprovadas |

### Núcleo: entrega e operação

| Skill | Aplicação no ERP | Evidência observada |
|---|---|---|
| [github-actions-templates](https://www.skills.sh/wshobson/agents/github-actions-templates) | CI/CD, matrizes de build e varreduras de segurança | 3 auditorias aprovadas |
| [database-backup-restore](https://www.skills.sh/aj-geddes/useful-ai-prompts/database-backup-restore) | backup, retenção, RPO/RTO e testes de restauração | 3 auditorias aprovadas |
| [runbook-creation](https://www.skills.sh/aj-geddes/useful-ai-prompts/runbook-creation) | procedimentos operacionais e resposta consistente | 3 auditorias aprovadas |

## Skills condicionais ao stack ou à arquitetura

Adote apenas quando a condição for real:

| Condição | Skills candidatas | Observação |
|---|---|---|
| Frontend React/Next.js | [vercel-react-best-practices](https://www.skills.sh/vercel-labs/agent-skills/vercel-react-best-practices), [vercel-composition-patterns](https://www.skills.sh/vercel-labs/agent-skills/vercel-composition-patterns) | desempenho e composição de componentes |
| Backend Rust | [rust-async-patterns](https://www.skills.sh/wshobson/agents/rust-async-patterns), [memory-safety-patterns](https://www.skills.sh/wshobson/agents/memory-safety-patterns) | concorrência assíncrona e segurança de memória |
| ERP em Odoo | [odoo-development](https://www.skills.sh/mindrally/skills/odoo-development) | módulos, ORM, views, controllers e segurança Odoo |
| Integração com Odoo | [odoo-rpc-api](https://www.skills.sh/sickn33/agentic-awesome-skills/odoo-rpc-api) | JSON-RPC/XML-RPC |
| Fluxos empresariais difíceis de visualizar | [event-modeling](https://www.skills.sh/proophboard/skills/event-modeling) | comandos, eventos, telas e evolução de estado; requer conta/workspace e integração com Prooph Board |
| Serviços realmente independentes | [microservices-patterns](https://www.skills.sh/wshobson/agents/microservices-patterns) | Saga, retries, circuit breaker e contratos distribuídos |
| Auditoria histórica incompatível com CRUD | [event-sourcing-architect](https://www.skills.sh/rmyndharis/antigravity-skills/event-sourcing-architect) | alto custo; exigir justificativa, revisão forte e protótipo antes da adoção |

`saas-multi-tenant` apresentou alerta no Snyk na consulta. Não o instale como base sem revisar todo o conteúdo; prefira consolidar uma skill própria de multiempresa alinhada à arquitetura real.

## Lacunas do catálogo cobertas por skills próprias

A pesquisa não encontrou cobertura madura e completa para todos os aspectos de um ERP brasileiro. Este pacote inclui as seguintes skills internas:

- [`erp-accounting`](skills/erp-accounting/SKILL.md)
- [`erp-inventory`](skills/erp-inventory/SKILL.md)
- [`erp-brazil-fiscal`](skills/erp-brazil-fiscal/SKILL.md)
- [`erp-multi-company`](skills/erp-multi-company/SKILL.md)
- [`erp-approval-workflows`](skills/erp-approval-workflows/SKILL.md)
- [`erp-lgpd`](skills/erp-lgpd/SKILL.md)
- [`erp-integration-reliability`](skills/erp-integration-reliability/SKILL.md)
- [`windev-wlanguage-erp`](skills/windev-wlanguage-erp/SKILL.md)

Essas oito skills cobrem as lacunas priorizadas, mas não representam todos os módulos possíveis de um ERP. Vendas, compras, contas a receber/pagar, tesouraria, ativos, produção, folha, logística e suporte podem exigir skills próprias conforme o escopo. A skill contábil não deve ser usada para fingir que um estorno de venda também cancelou fiscal, recebível ou estoque.

### Conteúdo mínimo das skills internas

#### `erp-accounting`

- plano de contas e centros de custo;
- partidas dobradas e balanceamento;
- competência, liquidação, conciliação e fechamento;
- estorno em vez de alteração destrutiva;
- moedas, arredondamento e rastreabilidade da origem.

#### `erp-inventory`

- saldo físico, disponível, reservado e em trânsito;
- unidade de medida e conversões;
- lotes, séries, validade e rastreabilidade;
- custo e política de valorização;
- concorrência, inventário, ajustes e estoque negativo.

#### `erp-brazil-fiscal`

- NF-e, NFC-e, CT-e e demais documentos aplicáveis;
- autorização, rejeição, cancelamento, inutilização e contingência;
- SPED e obrigações acessórias do escopo;
- códigos e regras tributárias parametrizáveis;
- versionamento de schemas e regras por vigência;
- orientação para validar sempre em fontes oficiais atualizadas.

#### `erp-multi-company`

- empresa, filial, estabelecimento e contexto ativo;
- isolamento em banco, cache, filas, arquivos, relatórios e logs;
- cadastros globais versus exclusivos;
- operações intercompany;
- testes negativos de vazamento entre tenants.

#### `erp-approval-workflows`

- políticas por valor, área, cargo e risco;
- segregação de funções;
- delegação temporária e substituição;
- reabertura, rejeição, cancelamento e escalonamento;
- trilha imutável das decisões.

#### `erp-lgpd`

- inventário e classificação de dados pessoais;
- finalidade, base legal, acesso, retenção e descarte;
- direitos do titular;
- anonimização/pseudonimização;
- regras para logs, exportações, backups e ambientes não produtivos.

#### `erp-integration-reliability`

- idempotência, outbox/inbox e deduplicação;
- timeout, retry, backoff e circuit breaker;
- dead-letter e reprocessamento seguro;
- reconciliação e observabilidade;
- contratos, versionamento e teste de falha parcial.

#### `windev-wlanguage-erp`

- padrões de WLanguage e organização de projeto;
- transações, concorrência e acesso HFSQL ou bancos externos;
- testes possíveis no ecossistema;
- interoperabilidade, APIs e tratamento de erros;
- funções e capacidades confirmadas na ajuda oficial da versão WX e testadas no projeto.

## Protocolo de seleção e instalação de skills

O [skills.sh](https://www.skills.sh/docs) organiza skills e ranqueia o catálogo principalmente por telemetria anônima de instalações. Popularidade não equivale a qualidade, adequação ao ERP ou segurança. Os links deste documento são bibliografia de pesquisa: eles não instalam, importam nem tornam essas skills disponíveis automaticamente.

Antes de instalar qualquer skill:

1. Abra sua página no skills.sh.
2. Leia o `SKILL.md` completo.
3. Inspecione `scripts/`, `references/`, assets e dependências.
4. Confira as auditorias atuais no [painel de auditorias](https://www.skills.sh/audits).
5. Verifique se pede rede, credenciais, comandos destrutivos ou acesso amplo ao sistema.
6. Instale apenas a skill necessária, não o repositório inteiro.
7. Para reprodutibilidade, use uma referência imutável suportada pelo instalador ou faça vendoring da versão revisada; registre repositório, commit/hash e data.
8. Teste em projeto isolado antes de adotar no ERP.
9. Registre origem, versão, data de revisão e responsável.

Forma geral de instalação mostrada pelo site:

```bash
npx skills add <owner/repository> --skill <skill-name>
```

Esse comando é apenas a forma geral e não demonstra pinning reprodutível. Consulte a sintaxe atual do instalador antes de usar ref/commit e valide o resultado. Não trate um arquivo de lock como garantia de reprodução sem testar uma instalação limpa.

Não use `--all` sem ter revisado todos os itens do repositório. Auditorias automatizadas podem falhar em detectar instruções inadequadas ou produzir alertas falsos; a decisão final exige revisão humana.

## Pacote mínimo recomendado

Não carregue todas as skills em toda tarefa. Selecione o pacote da fase atual.

### Descoberta e domínio

- `prd`
- `ubiquitous-language`
- `domain-modeling`
- `to-questionnaire`, quando houver lacunas relevantes

### Design e implementação

- `codebase-design`
- `architecture-decision-records`
- `postgresql-table-design`, somente se PostgreSQL fizer parte do stack
- `api-design-principles` e `openapi-spec-generation`, quando houver API HTTP
- `auth-implementation-patterns` e `access-control-patterns`
- `code-security`
- `operational-expert-tool-ui`, `form-design` e `data-display-and-selection`, quando houver interface gráfica
- `tdd`

### Pré-produção e operação

- `risk-based-testing`
- `webapp-testing`, quando houver aplicação web
- `wcag-audit-patterns` e testes de acessibilidade aplicáveis
- `github-actions-templates`, quando o repositório usar GitHub Actions
- [observability-engineer](https://www.skills.sh/rmyndharis/antigravity-skills/observability-engineer)
- `database-backup-restore`
- [incident-response-plan](https://www.skills.sh/aj-geddes/useful-ai-prompts/incident-response-plan)
- `runbook-creation`
- `verification-before-completion`

Revalide conteúdo, requisitos externos e auditorias das skills adicionadas ao pacote. Use ferramentas reais de teste de API, banco, segurança e dados conforme o stack; diretrizes em Markdown não executam essas verificações por si mesmas.

## Critérios de conclusão de uma entrega

Uma funcionalidade de ERP só está pronta quando houver evidência de que:

- requisitos e critérios de aceite foram atendidos;
- termos e regras de domínio permanecem consistentes;
- permissões e escopo de empresa foram verificados no servidor;
- invariantes permanecem válidas sob concorrência e repetição;
- migrações foram testadas de forma representativa;
- contratos de API/eventos estão atualizados;
- logs não vazam segredos ou dados pessoais desnecessários;
- testes de unidade, integração, contrato e jornada crítica passam;
- acessibilidade relevante foi verificada;
- métricas, alertas e runbooks cobrem a operação;
- backup e restauração protegem os dados alterados;
- documentação e ADRs refletem a solução entregue.

## Fontes e limitações

- [Documentação do skills.sh](https://www.skills.sh/docs)
- [Auditorias do skills.sh](https://www.skills.sh/audits)
- Páginas individuais ligadas nas tabelas deste documento

Os dados de popularidade, conteúdo e auditoria do catálogo podem mudar após a data da pesquisa. Revalide antes de cada adoção importante. Para temas legais, fiscais, contábeis e de privacidade, use fontes oficiais atualizadas e revisão de profissionais habilitados; uma skill não substitui validação jurídica, fiscal ou contábil.
