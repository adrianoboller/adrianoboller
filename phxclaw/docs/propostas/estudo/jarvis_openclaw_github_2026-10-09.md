# Projetos Jarvis e OpenClaw no GitHub

**Data da consulta:** 09/10/2026  
**Idioma:** Português do Brasil  
**Escopo:** seleção de 18 repositórios de assistentes de IA, agentes, complementos e alternativas.

Jarvis é um nome usado por vários projetos independentes. O repositório principal do OpenClaw é [openclaw/openclaw](https://github.com/openclaw/openclaw).

As funcionalidades e licenças abaixo são as descritas pelos próprios projetos na consulta realizada em 09/10/2026. Esta relação não representa uma validação prática de funcionamento nem uma comparação de desempenho.

## Sumário

1. [Projetos Jarvis](#1-projetos-jarvis)
2. [OpenClaw e seus repositórios oficiais](#2-openclaw-e-seus-repositórios-oficiais)
3. [Catálogo comunitário e alternativas ao OpenClaw](#3-catálogo-comunitário-e-alternativas-ao-openclaw)
4. [Prioridade de estudo para o PhoenixClaw](#4-prioridade-de-estudo-para-o-phoenixclaw)
5. [Referências e observações sobre licenças](#5-referências-e-observações-sobre-licenças)

## 1. Projetos Jarvis

| Projeto no GitHub | Tecnologia principal | O que oferece | Licença declarada |
| --- | --- | --- | --- |
| **[microsoft/JARVIS](https://github.com/microsoft/JARVIS)** | Python | Projeto de pesquisa da Microsoft que inclui o **HuggingGPT**: um LLM planeja tarefas, seleciona modelos especializados do Hugging Face, executa e reúne os resultados. | MIT. |
| **[open-jarvis/OpenJarvis](https://github.com/open-jarvis/OpenJarvis)** | Python | Plataforma desenvolvida em laboratórios de Stanford para **agentes pessoais com execução local**, ferramentas, avaliações de eficiência e aprendizado a partir do histórico de execução. | Apache-2.0. |
| **[skyfireitdiy/Jarvis](https://github.com/skyfireitdiy/Jarvis)** | Python; serviço auxiliar em Go | Plataforma de **colaboração entre agentes para desenvolvimento**. Inclui análise e alteração de código, testes, automação do computador e navegador, memória e ferramentas de **migração de C para Rust**. | MIT. |
| **[isair/jarvis](https://github.com/isair/jarvis)** | Python | Assistente de voz com execução local, memória pessoal, interação por conversa, controle do Chrome e integração com ferramentas **MCP**. | Licença própria **não comercial**; uso comercial exige licença separada. |
| **[PanPenek/JarvisAi](https://github.com/PanPenek/JarvisAi)** | Python / FastAPI | Assistente voltado ao Windows com voz, leitura da tela por OCR, controle de aplicativos e interface inspirada no Homem de Ferro. Usa Ollama, Whisper e Kokoro. | **MIT, segundo o README**. |
| **[vierisid/jarvis](https://github.com/vierisid/jarvis)** | TypeScript / Bun / React; auxiliar em Go | Agente persistente com múltiplos agentes especializados, percepção da área de trabalho, fluxos visuais e conexão a várias máquinas. | **Jarvis Source Available License 2.0**, com restrições próprias. |
| **[Priler/jarvis](https://github.com/Priler/jarvis)** | **Rust / Tauri** | Assistente de voz offline para desktop. O projeto se identifica como **em desenvolvimento — WIP**. | **CC BY-NC-SA 4.0**, não comercial. |
| **[Sycatle/local-jarvis](https://github.com/Sycatle/local-jarvis)** | **Rust** | Assistente de voz local para **Linux**, com serviço persistente, reconhecimento de fala, modelo local e controle do desktop por ferramentas. Está em **fase alpha**. | MIT **ou** Apache-2.0. |
| **[sukeesh/Jarvis](https://github.com/sukeesh/Jarvis)** | Python | Assistente de linha de comando com plugins, consultas, cálculos e voz opcional. O próprio projeto o descreve como um **assistente sem IA**. | MIT. |

**Total desta seção:** 9 repositórios.

## 2. OpenClaw e seus repositórios oficiais

| Projeto no GitHub | Tecnologia principal | O que oferece | Licença |
| --- | --- | --- | --- |
| **[openclaw/openclaw](https://github.com/openclaw/openclaw)** | TypeScript / Node.js | **Projeto principal.** Assistente instalado no computador ou servidor, com agentes, memória, ferramentas, skills, plugins e canais como WhatsApp, Telegram, Discord e Slack. | MIT. |
| **[openclaw/clawhub](https://github.com/openclaw/clawhub)** | TypeScript / React / Convex | Registro para **publicar, versionar, pesquisar e instalar skills**, além de catálogo de plugins do OpenClaw. | MIT. |
| **[openclaw/mcporter](https://github.com/openclaw/mcporter)** | TypeScript | Ferramenta para descobrir e chamar servidores **MCP** pelo terminal ou código. Também gera clientes TypeScript e comandos independentes. | MIT. |
| **[openclaw/openclaw-windows-node](https://github.com/openclaw/openclaw-windows-node)** | C# / WinUI | Aplicativo complementar para **Windows**, conectando o computador ao gateway e disponibilizando chat e capacidades locais conforme as permissões. | MIT. |
| **[openclaw/openclaw-ansible](https://github.com/openclaw/openclaw-ansible)** | Ansible / YAML / Shell | Automatiza a instalação em servidores Debian/Ubuntu, incluindo configuração de Tailscale, firewall e integração com Docker. | MIT. |

**Total desta seção:** 5 repositórios.

## 3. Catálogo comunitário e alternativas ao OpenClaw

Estes projetos são independentes da implementação principal.

| Projeto no GitHub | Tipo / tecnologia | O que oferece | Licença |
| --- | --- | --- | --- |
| **[VoltAgent/awesome-openclaw-skills](https://github.com/VoltAgent/awesome-openclaw-skills)** | Catálogo em Markdown | Lista organizada de skills do ecossistema OpenClaw, por categoria e finalidade. | MIT **da lista**; as skills vinculadas podem ter outras licenças. |
| **[neul-labs/openclaw-rs](https://github.com/neul-labs/openclaw-rs)** | **Rust**, com integração Node.js | Implementação comunitária do OpenClaw em Rust, incluindo gateway, agentes, canais, provedores e memória. O README declara sua independência do projeto original. | MIT. |
| **[zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw)** | **Rust** | Infraestrutura para assistente pessoal com execução em um binário, provedores de modelos, canais, ferramentas e integrações. Alternativa independente ao OpenClaw. | MIT **ou** Apache-2.0. |
| **[MatrixCoreX/RustClaw](https://github.com/MatrixCoreX/RustClaw)** | **Rust** | Plataforma apresentada no README como **Agent Runtime**, com execução de tarefas, ferramentas, skills, memória, agendamento e interface web. | Código disponível sob licença **não comercial**. |

**Total desta seção:** 4 repositórios.  
**Total geral:** 18 repositórios.

## 4. Prioridade de estudo para o PhoenixClaw

Pelo foco do PhoenixClaw, a ordem sugerida de estudo é:

1. **OpenClaw:** arquitetura do gateway, canais de comunicação, plugins e skills.
2. **ZeroClaw e openclaw-rs:** implementação de agentes e serviços em **Rust**.
3. **skyfireitdiy/Jarvis:** colaboração entre agentes, desenvolvimento de software e migração **C → Rust**.
4. **OpenJarvis:** execução local de modelos e avaliação de eficiência.

Essa prioridade é uma avaliação baseada nas propostas dos repositórios, sem comparação prática de desempenho.

### Links diretos dos projetos priorizados

- [OpenClaw — repositório principal](https://github.com/openclaw/openclaw)
- [ZeroClaw — implementação em Rust](https://github.com/zeroclaw-labs/zeroclaw)
- [openclaw-rs — implementação comunitária em Rust](https://github.com/neul-labs/openclaw-rs)
- [Jarvis — colaboração e desenvolvimento](https://github.com/skyfireitdiy/Jarvis)
- [OpenJarvis — agentes com execução local](https://github.com/open-jarvis/OpenJarvis)

## 5. Referências e observações sobre licenças

Os nomes dos projetos nas tabelas são links diretos para as fontes primárias: os respectivos repositórios no GitHub, que apresentam o README, os arquivos de código e as informações de licença.

As seguintes referências complementam as observações de licença registradas na pesquisa:

| Projeto | Referência | Observação registrada |
| --- | --- | --- |
| isair/jarvis | [LICENSE](https://github.com/isair/jarvis/blob/main/LICENSE) | O texto distingue usos não comerciais e uso comercial, que exige licença separada. |
| PanPenek/JarvisAi | [README](https://github.com/PanPenek/JarvisAi/blob/main/README.md) | A indicação MIT foi encontrada no README; por isso a tabela mantém a expressão “segundo o README”. |
| vierisid/jarvis | [LICENSE](https://github.com/vierisid/jarvis/blob/main/LICENSE) | Licença própria denominada Jarvis Source Available License 2.0, baseada em RSALv2. |
| Priler/jarvis | [Repositório e seção de licença](https://github.com/Priler/jarvis#license) | CC BY-NC-SA 4.0: a designação inclui a condição de uso não comercial. |
| Sycatle/local-jarvis | [LICENSE-MIT](https://github.com/Sycatle/local-jarvis/blob/main/LICENSE-MIT) e [repositório](https://github.com/Sycatle/local-jarvis) | O README apresenta a escolha entre MIT e Apache-2.0. |
| VoltAgent/awesome-openclaw-skills | [Repositório](https://github.com/VoltAgent/awesome-openclaw-skills) | A licença da lista não determina a licença de cada skill ou projeto vinculado. |
| zeroclaw-labs/zeroclaw | [Repositório e arquivos de licença](https://github.com/zeroclaw-labs/zeroclaw) | O repositório apresenta MIT e Apache-2.0. |
| MatrixCoreX/RustClaw | [Repositório e seção de licença](https://github.com/MatrixCoreX/RustClaw#license) | O README declara uma licença de código disponível para uso não comercial. |

---

**Origem do documento:** consolidação da pesquisa apresentada nesta conversa, com os links preservados em Markdown.  
**Data de geração:** 09/10/2026.
