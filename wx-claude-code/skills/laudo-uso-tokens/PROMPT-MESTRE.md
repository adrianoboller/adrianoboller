# Prompt-mestre — auditoria de economia de tokens

Executar uma auditoria read-only de economia de tokens no Claude Code, restrita ao diretório e às fontes autorizadas pelo usuário. Obedecer a [references/measurement-method.md](references/measurement-method.md) em toda contagem, estimativa e comparação e usar [templates/CLAUDE.response-style.md](templates/CLAUDE.response-style.md) na redação.

## Objetivo e limites

Diagnosticar quanto contexto é carregado antes e durante o trabalho, como as 10 sessões recentes usam tokens e quais problemas têm evidência suficiente para merecer uma proposta. Não otimizar nem modificar nada durante esta auditoria.

Antes da coleta:

1. Confirmar o diretório autorizado do projeto.
2. Confirmar a fonte autorizada de sessões e se contém datas e campos de uso.
3. Confirmar autorização para ler configurações participantes do contexto fora do projeto, se necessária.
4. Informar que a auditoria extrairá somente metadados e métricas, sem reproduzir prompts, respostas, segredos ou identificadores sensíveis.

Usar apenas leitura. Não criar, editar, mover, apagar, executar arquivos do projeto, instalar dependências, chamar serviços externos, alterar configurações ou habilitar/desabilitar ferramentas. Comandos de inspeção devem ser sem efeito colateral. Não ler `.env`, credenciais, chaves, tokens de acesso, dumps, anexos binários ou conteúdo que não seja necessário à métrica. Quando uma fonte não puder ser lida com segurança, marcar `INDISPONÍVEL` e pedir uma exportação anonimizada.

Rotular toda métrica, conclusão, custo e ganho como:

- `MEDIDO`: valor de fonte primária acessível ou tokenizador exato confirmado;
- `ESTIMADO`: cálculo com premissa, fórmula, incerteza e limitação visíveis; ou
- `INDISPONÍVEL`: fonte, autorização, campo ou comparabilidade ausente, com motivo.

Nunca converter ausência em zero nem misturar estados sem separar as parcelas.

## Fase 1 — medir e diagnosticar

Executar A, B e C em sequência, somente leitura.

### Fase 1 A — inventário do contexto e das capacidades

Inventariar, quando acessível:

1. ferramentas nativas/de sistema expostas ao agente;
2. servidores MCP configurados e servidores MCP ativos, em colunas distintas;
3. quantidade total de ferramentas nativas, quantidade de MCPs configurados, quantidade de MCPs ativos e quantidade de ferramentas por MCP e no conjunto MCP;
4. `CLAUDE.md` do projeto, todos os `CLAUDE.md` de diretórios pais dentro do escopo autorizado, instruções globais/de usuário e cada arquivo alcançado por import explícito;
5. skills e agentes descobertos, distinguindo metadata/descrição pré-carregada de corpo carregado sob demanda; e
6. outros itens que a fonte demonstre participar do contexto inicial.

Para cada item, identificar categoria, origem/escopo, estado configurado/ativo quando aplicável, parcela efetivamente contada, modo de carga (`PRELOAD`, `SOB DEMANDA` ou `DESCONHECIDO`), tokens, estado de evidência e fonte/método. Medir também descrições e schemas de ferramentas separadamente quando estiverem acessíveis.

Apresentar a tabela principal ordenada por `TOKENS` em ordem decrescente:

| ITEM | CATEGORIA/ORIGEM | CARGA/ESTADO | TOKENS | EVIDÊNCIA/MÉTODO |
| --- | --- | --- | ---: | --- |

Depois, apresentar os totais sem dupla contagem:

| RESUMO | VALOR | ESTADO | FONTE/LIMITAÇÃO |
| --- | ---: | --- | --- |

Incluir no resumo as contagens de ferramentas e MCPs e os tokens do conjunto pré-carregado. Se uma parcela não for acessível, deixá-la `INDISPONÍVEL`, sem tratá-la como zero.

Destacar, com limiar e evidência visíveis:

- MCP configurado ou ativo sem chamada explícita nas sessões como `ocioso na amostra`, nunca como ocioso global;
- `CLAUDE.md` individual acima de aproximadamente 5.000 tokens;
- conjunto de instruções/contexto pré-carregado acima de aproximadamente 10.000 tokens;
- descrição de ferramenta, MCP, skill ou agente com pelo menos 200 tokens, além de manter o ranking completo;
- conteúdo exatamente duplicado e sobreposição parcial relevante, separando comparação determinística de hipótese semântica; e
- ferramenta configurada, mas ausente do runtime, sem chamá-la de ativa.

Informar a capacidade da `context window` somente se houver fonte acessível e aplicável ao modelo efetivamente usado. Nesse caso, citar a fonte e calcular `PRELOAD/CONTEXT % = tokens pré-carregados / context window × 100`, mostrando separadamente o estado do numerador e do denominador. Sem essa fonte, escrever `context window: INDISPONÍVEL` e `PRELOAD/CONTEXT %: INDISPONÍVEL`; não estimar pelo nome do modelo.

### Fase 1 B — 10 sessões recentes

Selecionar as 10 sessões mais recentes por timestamp confiável da fonte. Se houver menos de 10, usar todas as elegíveis e declarar `n/10 disponíveis`. Se não for possível ordenar de forma confiável, não escolher uma amostra substituta: marcar esta etapa `INDISPONÍVEL` e explicar a fonte mínima necessária.

Anonimizar as sessões como `S01` a `S10` e apresentar, da mais recente para a mais antiga:

| SESSÃO/DATA | INPUT | OUTPUT | CACHE CREATION | CACHE READ | DURAÇÃO | TURNOS | MODELO | EFFORT | CONFIG/AUTO-SWITCH | EVIDÊNCIA |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- | --- |

Cada valor deve carregar ou herdar claramente `MEDIDO`, `ESTIMADO` ou `INDISPONÍVEL`. Informar o campo da fonte para tokens de input/output/cache, duração e turnos. Não inferir modelo, effort, configuração nem auto-switch: relatar somente se a fonte os expuser. Se configuração ou troca automática variar dentro da sessão, registrar a sequência sem reproduzir conteúdo da conversa.

Apresentar totais e mediana por métrica apenas quando as unidades forem compatíveis e sem converter valores ausentes em zero. Separar cache creation de cache read.

Procurar padrões de desperdício ou oportunidade nos registros, incluindo repetição de contexto, reabertura de arquivo grande, saída de ferramenta volumosa, retry/reexecução, retrabalho, contexto estático reenviado, baixa reutilização de cache comparável, carregamento sem uso e resposta desproporcional. Tratar relações causais como hipótese salvo prova da fonte. Para cada padrão, mostrar frequência `n/N`, sessões afetadas anonimizadas, tokens atribuíveis quando calculáveis e evidência; não reproduzir prompts ou respostas.

Usar o inventário da Fase 1 A para cruzar chamadas observadas nas sessões. Um MCP, skill, agente ou ferramenta sem chamada só pode ser descrito como `sem uso observado na amostra`.

### Fase 1 C — diagnóstico priorizado

Consolidar apenas problemas sustentados pela Fase 1 A ou B. Ordenar pelo impacto provável, considerando magnitude, frequência e confiança da evidência.

Usar exatamente estas colunas:

| PROBLEMA | TOKENS/CUSTO ESTIMADO | FREQUÊNCIA | IMPACTO | EVIDÊNCIA |
| --- | --- | --- | --- | --- |

Em `TOKENS/CUSTO ESTIMADO`, separar tokens de custo monetário e preservar o estado. Só calcular moeda quando existir custo primário ou tabela de preço acessível, aplicável ao modelo/período/unidade e citada; caso contrário, declarar `custo: INDISPONÍVEL`. Em `FREQUÊNCIA`, usar `n/N sessões` ou outra unidade explícita. Em `IMPACTO`, combinar magnitude e confiança sem inventar precisão. Em `EVIDÊNCIA`, citar fonte/campo e distinguir observação de hipótese.

Depois da tabela, listar apenas limitações que alterem a interpretação. Não sugerir solução, não mostrar tabela de mudanças e não aplicar nada.

**PONTO DE PARADA OBRIGATÓRIO:** encerrar a resposta e pedir aprovação explícita para iniciar a Fase 2. Não avançar na mesma resposta, mesmo que a otimização pareça óbvia.

## Fase 2 — propor uma mudança por vez

Iniciar somente após aprovação explícita do diagnóstico da Fase 1. Escolher o problema aprovado de maior prioridade e apresentar exatamente uma proposta reversível e específica.

Usar exatamente estas colunas:

| O QUE MUDAR | POR QUÊ | GANHO ESTIMADO POR SESSÃO | RISCO/TRADE-OFF |
| --- | --- | --- | --- |

Basear `POR QUÊ` em evidência identificada na Fase 1. Em `GANHO ESTIMADO POR SESSÃO`, mostrar `ESTIMADO` com fórmula, linha de base e intervalo/limitação; usar `INDISPONÍVEL` quando não houver base comparável. Não alegar redução de custo sem preço aplicável. Explicitar como a proposta poderia ser verificada posteriormente.

Não editar, gerar, executar, aplicar nem configurar a proposta dentro desta auditoria. Não apresentar uma segunda proposta na mesma resposta.

**PONTO DE PARADA OBRIGATÓRIO:** encerrar após a proposta e pedir que o usuário escolha entre discutir essa proposta, solicitar outra proposta para um problema diagnosticado ou aprovar a passagem à Fase 3. Uma eventual implementação deve ser um pedido separado e explícito, fora desta auditoria.

## Fase 3 — hábitos baseados nos dados

Iniciar somente após aprovação explícita para entrar na Fase 3. Produzir uma lista numerada, em ordem de maior ganho provável, com no máximo três hábitos. Cada item deve ter exatamente uma frase e conter o hábito, a evidência que o sustenta e o estado `MEDIDO` ou `ESTIMADO`.

Não incluir subtópicos, explicações adicionais, recomendações genéricas ou hábitos sem suporte na Fase 1. Não completar a lista artificialmente: se houver suporte para dois hábitos, apresentar dois; se não houver suporte para nenhum, declarar em uma frase que os dados são insuficientes.

Encerrar sem aplicar mudanças. Se necessário, acrescentar uma única frase sobre a lacuna de medição mais importante e a autorização mínima para resolvê-la; essa frase não conta como hábito.

## Formato e linguagem

Ser direto, usar frases curtas e explicar o termo técnico na primeira ocorrência. Manter as tabelas e cabeçalhos exigidos. Identificar problema antes de proposta. Não repetir blocos extensos do inventário no diagnóstico; referenciar linhas ou identificadores. Se faltar informação para uma conclusão, perguntar ou marcar `INDISPONÍVEL` em vez de preencher por suposição.
