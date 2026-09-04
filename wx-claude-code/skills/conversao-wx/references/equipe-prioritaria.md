# A equipe prioritária: dez agentes, cada um com o seu gatilho

Existem por prioridade, e a letra é a ordem em que têm de existir num projeto de grande porte. Não substituem os papéis `papel-a` a `papel-j` com PDCA: a equipe prioritária governa; os papéis executam.

| Prioridade | Agente | Gatilho | Ferramenta própria |
| --- | --- | --- | --- |
| A | `equipe-a-zelador` | sinal de outro agente sem espaço, ou o hook diário | `zelador.py espaco`, `limpar` |
| B | `equipe-b-pesquisador` | todo ciclo PDCA infrutífero (o `pdca fechar` abre o pedido em `pmo/pesquisas.md`) | WebSearch, WebFetch, corpus por tema |
| C | `equipe-c-documentador` | código novo ou alterado numa sprint | `documentar_codigo.py` → `funcoes.md`, `funcoes.html`, `indice.json` |
| D | `equipe-d-supervisor-de-qualidade` | entrega de item; interjeição em `pmo/interjeicoes.md` | parecer em `pmo/qualidade/` |
| E | `equipe-e-gestor-de-tarefas` | item novo no backlog, mandado pelo GP | `pesar_tarefa.py` → grau e modelo |
| F | `equipe-f-gp` | sempre; dono do backlog, do Kanban e do versionamento | `pmo.py` inteiro, `git` |
| G | `equipe-g-testes` | item implementado | bateria do projeto, golden, `tests/testes.py`; entrega ao `papel-f-prova-real` |
| H | `equipe-h-status` | pedido do usuário ou fechamento de sprint | `pmo.py status --por-agente` |
| I | `equipe-i-base-de-conhecimento` | ciclo PDCA fechado | `pmo/conhecimento/frutiferos.md`, `infrutiferos.md`, `indice.md`, aviso ao GP |
| J | `equipe-j-tradutor` | **só pedido explícito do usuário** | `i18n.py` → `i18n/textos.json` |

## Como se falam

- Todo agente registra o que faz com `pmo.py atividade --agente <nome> --item <id> --estado iniciou|andamento|bloqueado|concluiu|falhou`; é daí que o Status (H) relata.
- Avisos entre agentes vão para `pmo/avisos.md`, uma linha datada por aviso, sempre endereçada («para o GP»). A base de conhecimento (I) e o Pesquisador (B) avisam o GP por lá.
- Sinal de espaço: quem não consegue gravar chama o Zelador (A) com o caminho que falhou; o Zelador devolve megabytes antes e depois.
- O Gestor de tarefas (E) escolhe o modelo por peso; o agente executor passa esse modelo ao `rotear_modelo.py --classe … --gate …`, que ainda pode rebaixar por orçamento.

## O que nunca muda

Nenhum deles aprova gate: o aprovador humano decide. Nenhum edita anexo. O Tradutor (J) não age sem pedido. Número sem fonte vira INDISPONÍVEL ou ESTIMADO, nunca zero.
