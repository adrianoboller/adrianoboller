# Projetos similares ao n8n no GitHub

**Data da consulta:** 09/10/2026  
**Escopo:** 15 alternativas e projetos relacionados, mais o Flowise como referência arquivada.  
**Finalidade:** comparar propostas, tecnologias, licenças e possível aplicação no Phoenix/PhoenixClaw.

## Visão geral

Para uma alternativa próxima ao n8n, a primeira opção sugerida para avaliação é o **Activepieces**, com construção visual, integrações, condições, loops, HTTP e recursos de IA. Para estudar automação com backend em **Rust**, destacam-se **Windmill e z8run**.

Essa prioridade é uma avaliação das funcionalidades descritas nos repositórios. Não foram executados benchmarks nem testes práticos de implantação.

## 1. Automação visual e integração de sistemas

| Projeto no GitHub | Tecnologia principal | Como funciona e onde se encaixa | Licença declarada |
| --- | --- | --- | --- |
| **[Activepieces](https://github.com/activepieces/activepieces)** | TypeScript | Construtor visual para conectar aplicativos e APIs. Tem gatilhos, condições, loops, tentativas automáticas, versionamento, código e ferramentas de IA/MCP. Uma das alternativas mais próximas ao uso do n8n. | **MIT na Community Edition**; recursos Enterprise comerciais. |
| **[Node-RED](https://github.com/node-red/node-red)** | JavaScript / Node.js | Editor visual de nós para eventos, APIs, serviços e dispositivos. Permite funções JavaScript e **importação/exportação dos fluxos em JSON**. Interessante para integrações, infraestrutura e IoT. | **Apache-2.0** no projeto principal. |
| **[Windmill](https://github.com/windmill-labs/windmill)** | **Rust**, Svelte e PostgreSQL | Transforma scripts em APIs, tarefas, workflows e interfaces. Combina código com composição visual e executa tarefas em linguagens como Python, TypeScript, Go, Bash, SQL e Rust. | **AGPL-3.0 no build aberto sem Enterprise**; distribuições oficiais Community têm termos adicionais. |
| **[Automatisch](https://github.com/automatisch/automatisch)** | JavaScript | Automação de processos entre serviços, com configuração visual e instalação própria por Docker. Tem proposta próxima de Zapier e n8n para integrações de negócio. | **AGPL-3.0 na Community Edition**; arquivos Enterprise sob licença comercial. |
| **[Kestra](https://github.com/kestra-io/kestra)** | Java; fluxos em YAML | Orquestra processos, dados e infraestrutura. Os fluxos são declarativos, editados pela interface web, com agendamentos, eventos e tarefas em diferentes tecnologias. | **Apache-2.0 no núcleo**. |
| **[Apache NiFi](https://github.com/apache/nifi)** | Java | Interface visual para receber, transformar, rotear e distribuir dados. Oferece filas, histórico da movimentação dos dados, controle de execução e processamento em cluster. | **Apache-2.0**. |

## 2. Projetos em Rust para estudar no Phoenix

Além do Windmill, foram encontrados os seguintes projetos:

| Projeto no GitHub | Tecnologia | Recursos descritos pelo projeto | Licença |
| --- | --- | --- | --- |
| **[z8run](https://github.com/z8run/z8run)** | **Rust + React + WebAssembly** | Motor visual apresentado como alternativa ao n8n e Node-RED. Declara editor de nós, comunicação por WebSocket, SQLite embutido, cofre de credenciais, nós de IA e **plugins executados em ambiente WASM isolado**. | **MIT ou Apache-2.0**, à escolha. |
| **[IronFlow](https://github.com/skitsanos/ironflow)** | **Rust + Lua** | Motor de workflows definido por scripts Lua, com execução por dependências, paralelismo, condições, novas tentativas, limites de tempo, subfluxos, CLI e API HTTP. É uma referência para o **motor de execução programável**. | **MIT**. |

O z8run é alinhado à combinação **Rust, React e WASM** considerada para o Phoenix. A indicação é para inspeção técnica pelo encaixe arquitetural; sua implementação e seu desempenho não foram validados nesta pesquisa.

## 3. Automação visual focada em IA e agentes

Estes projetos concentram-se em modelos de IA, agentes, documentos e ferramentas.

| Projeto no GitHub | Tecnologia principal | O que oferece | Licença declarada |
| --- | --- | --- | --- |
| **[Dify](https://github.com/langgenius/dify)** | Python + TypeScript / React | Editor de workflows, agentes, integração de modelos, ferramentas, APIs e **RAG — consulta a documentos e bases de conhecimento**. | **Dify Open Source License**, baseada na Apache-2.0 com condições adicionais. |
| **[Langflow](https://github.com/langflow-ai/langflow)** | Python + React / TypeScript | Editor visual para agentes e fluxos de IA. Permite componentes personalizados em Python, execução passo a passo e exposição dos fluxos como API ou servidor MCP. | **MIT**. |
| **[Sim](https://github.com/simstudioai/sim)** | TypeScript / Next.js / React / PostgreSQL | Ambiente para construir, executar e monitorar agentes e workflows. Inclui integrações, bases de conhecimento, agendamentos e acompanhamento das execuções. | **Apache-2.0 no arquivo de licença principal consultado**. |

### Situação do Flowise

O **[Flowise](https://github.com/FlowiseAI/Flowise)** também é uma referência de construção visual de agentes, mas **foi arquivado em 13/08/2026**, com fim de ciclo oficial em **31/08/2026**.

A recomendação é mantê-lo como material de estudo, sem colocá-lo entre as primeiras escolhas para uma implantação nova.

**Fonte primária:** [anúncio oficial “The Future of Flowise”](https://github.com/FlowiseAI/Flowise/discussions/6727).

## 4. Motores de execução, pipelines e monitoramento

Aqui a semelhança com o n8n está na automação das tarefas. **Temporal, Prefect e Trigger.dev definem os workflows principalmente em código**; suas interfaces servem para acompanhar e administrar execuções.

| Projeto no GitHub | Tecnologia principal | Uso principal | Licença |
| --- | --- | --- | --- |
| **[Temporal](https://github.com/temporalio/temporal)** | Servidor em Go; workflows por SDKs | Execução durável de processos: mantém o estado do workflow e permite continuidade diante de falhas. Referência para processos longos, workers e recuperação. | **MIT**. |
| **[Prefect](https://github.com/PrefectHQ/prefect)** | Python | Transforma funções e scripts em pipelines com agendamento, novas tentativas, cache, eventos e painel de acompanhamento. | **Apache-2.0**. |
| **[Trigger.dev](https://github.com/triggerdotdev/trigger.dev)** | TypeScript / JavaScript | Tarefas em segundo plano e workflows de IA, com filas, controle de concorrência, agendamentos, novas tentativas, logs e pausas para aprovação humana. | **Apache-2.0**. |
| **[Huginn](https://github.com/huginn/huginn)** | Ruby / Rails | Agentes configuráveis para monitorar sites, consumir RSS, acompanhar eventos, receber webhooks e disparar ações ou notificações. Os eventos circulam entre os agentes. | **MIT**. |

## 5. Prioridades para avaliação

| Objetivo | Primeira escolha sugerida |
| --- | --- |
| Montar automações visuais de negócio semelhantes às do n8n | **Activepieces** |
| Estudar editor de nós e persistência dos fluxos em JSON | **Node-RED** |
| Estudar backend Rust, PostgreSQL e execução de scripts | **Windmill** |
| Estudar a combinação Rust + React + plugins WASM | **z8run** |
| Construir visualmente agentes e componentes de IA | **Langflow** |
| Trabalhar com IA sobre documentos e bases de conhecimento | **Dify** |
| Estudar recuperação e continuidade de processos longos | **Temporal** |

Para o **PhoenixClaw**, a prioridade de estudo sugerida é **Node-RED, Activepieces, Windmill e z8run**, cobrindo editor visual, integrações, execução de tarefas e extensibilidade.

Essa seleção é uma avaliação arquitetural baseada nos recursos documentados pelos projetos.

## 6. Observações de licença para possível incorporação

- **Windmill:** distingue o código compilado sem a funcionalidade Enterprise das distribuições oficiais Community. Não deve ser resumido como um único conjunto integralmente AGPL. A documentação permite o uso interno da Community Edition nos termos apresentados, mas estabelece condições próprias para sua redistribuição, exposição a usuários ou incorporação comercial.
- **Dify:** usa uma versão modificada da Apache-2.0. O texto inclui condições adicionais para operação multitenant, conforme sua definição de workspace, e para alteração de marca/copyright no frontend.
- **Activepieces e Automatisch:** separam o núcleo comunitário dos recursos Enterprise.
- **Node-RED:** a licença do projeto principal não determina a licença de todos os nós de terceiros.
- **Sim:** a indicação de licença registra o arquivo principal consultado; não constitui garantia sobre todos os componentes, serviços ou ofertas comerciais associados ao produto.

## 7. Fontes primárias complementares

Os nomes dos projetos nas tabelas são links para seus repositórios oficiais. Para os detalhes que alteram a comparação:

| Assunto | Fonte |
| --- | --- |
| Editor Node-RED, funções JavaScript e fluxos JSON | [Node-RED — About](https://nodered.org/about/) |
| Activepieces Community e Enterprise | [Activepieces — README e licença](https://github.com/activepieces/activepieces) |
| Condições de distribuição do Windmill | [Windmill — LICENSE](https://github.com/windmill-labs/windmill/blob/main/LICENSE) |
| Divisão de licença do Automatisch | [Automatisch — LICENSE](https://github.com/automatisch/automatisch/blob/main/LICENSE) |
| Funcionalidades e licença do Kestra | [Kestra — README](https://github.com/kestra-io/kestra/blob/develop/README.md) |
| Funcionalidades do Apache NiFi | [Apache NiFi — repositório](https://github.com/apache/nifi) |
| z8run e dupla licença | [z8run — repositório](https://github.com/z8run/z8run) |
| IronFlow e definição de fluxos Lua | [IronFlow — repositório](https://github.com/skitsanos/ironflow) |
| Condições adicionais do Dify | [Dify — LICENSE](https://github.com/langgenius/dify/blob/main/LICENSE) |
| Licença principal do Sim | [Sim — LICENSE](https://github.com/simstudioai/sim/blob/main/LICENSE) |
| Arquivamento e fim de ciclo do Flowise | [Flowise — anúncio oficial](https://github.com/FlowiseAI/Flowise/discussions/6727) |

---

**Origem:** consolidação da pesquisa apresentada nesta conversa.  
**Data de geração:** 09/10/2026.
