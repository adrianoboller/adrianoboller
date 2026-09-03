---
name: "laudo-uso-tokens"
description: "Audita o uso de tokens do Claude Code em fases aprovadas pelo usuario e propoe uma mudanca por vez."
license: "All rights reserved"
compatibility: "Claude Cowork and Claude Code"
metadata: {"author":"Adriano Boller","version":"3.0.0"}
allowed-tools: "Read, Glob, Grep, Bash"
---

# Laudo de uso de tokens (SKILL Laudo_Uso_Tokens)

Esta skill é de invocação manual. Use somente após o usuário chamar `/wx-claude-code:laudo-tokens`.

Antes de começar, confirmar:

1. o diretório do projeto autorizado;
2. a fonte autorizada de sessões, quando existir; e
3. se o usuário permite ler configurações do Claude Code fora do projeto que possam participar do contexto, como instruções globais e MCPs de usuário.

Se algum dado necessário não estiver autorizado, não o procurar por outros meios: marcar `INDISPONÍVEL` e pedir somente a autorização ou fonte mínima necessária.

Ler integralmente e seguir, nesta ordem:

- [PROMPT-MESTRE.md](PROMPT-MESTRE.md), que contém o contrato operacional completo das três fases — e [PROMPT-MESTRE-CURTO.md](PROMPT-MESTRE-CURTO.md), a versão que o usuário cola no Claude Code e que este contrato detalha;
- [references/measurement-method.md](references/measurement-method.md), antes de contar, estimar, agregar ou comparar tokens; e
- [templates/CLAUDE.response-style.md](templates/CLAUDE.response-style.md), antes de redigir cada entrega.

## Contrato invariável

- Executar somente leitura durante a auditoria. Não criar, editar, mover, apagar, instalar, habilitar, desabilitar nem reconfigurar arquivos, hooks, MCPs, skills, agentes, modelos ou preferências.
- Classificar métricas, achados e ganhos como `MEDIDO`, `ESTIMADO` ou `INDISPONÍVEL` e mostrar a fonte ou premissa.
- Preservar privacidade: extrair só metadados necessários, não exibir conteúdo de prompts, respostas, segredos ou identificadores sensíveis e não ler credenciais.
- Executar as fases em ordem e respeitar os pontos de parada. Ao final da Fase 1, parar; na Fase 2, apresentar uma única proposta e parar; na Fase 3, apresentar no máximo três hábitos de uma frase cada e encerrar.
- Não inferir contexto, custo, preço, modelo, effort, auto-switch, uso de ferramenta ou atividade de MCP sem fonte acessível.

Usar ferramentas nativas do Claude Code disponíveis no ambiente, como `Read`, `Glob`, `Grep` e `Bash`, apenas em operações sem efeito colateral. Se precisar mencionar um modelo, copiar o identificador da fonte; não pressupor que aliases como `opus`, `sonnet`, `haiku` ou `inherit` estejam habilitados.
