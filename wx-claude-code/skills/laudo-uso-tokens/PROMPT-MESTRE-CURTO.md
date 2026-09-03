# Prompt-mestre curto (auditoria de tokens) — cola no Claude Code

Este é o texto que o usuário cola. `PROMPT-MESTRE.md` é o mesmo pedido, detalhado.

```text
Quero uma auditoria completa do consumo de tokens do Claude Code
neste projeto, em 3 fases.
Não altere nenhum arquivo, configuração, MCP, hook, skill ou agente
sem minha aprovação explícita.
Sempre diferencie:
MEDIDO = dado que você conseguiu obter diretamente
ESTIMADO = aproximação
INDISPONÍVEL = informação à qual você não tem acesso

FASE 1 — AUDITAR (somente leitura)

A. Custo inicial da sessão
Liste tudo que entra em contexto ou é disponibilizado em uma sessão
nova deste projeto: ferramentas de sistema, servidores MCP
configurados e ativos, número de ferramentas expostas por cada MCP,
CLAUDE.md do projeto, CLAUDE.md de pastas-pai, CLAUDE.md global,
arquivos importados por CLAUDE.md, skills e agentes.
Estime ou meça o tamanho em tokens de cada item e monte uma tabela
do maior para o menor.
Destaque:
- MCPs configurados que aparentemente não são utilizados
- CLAUDE.md individuais acima de ~5.000 tokens
- conjunto de CLAUDE.md acima de ~10.000 tokens
- skills ou agentes com descrições desnecessariamente grandes
- conteúdo duplicado entre arquivos
Informe qual é a janela de contexto disponível para o modelo atual
e estime: PRELOAD TOKENS / CONTEXT WINDOW.
Me entregue a porcentagem da janela que já nasce ocupada antes do
meu primeiro prompt.

B. Consumo durante o uso
Se os logs locais do Claude Code estiverem disponíveis, analise as
10 sessões mais recentes deste projeto. Para cada sessão, tente
levantar: input tokens, output tokens, cache creation, cache read,
duração ou número de turnos.
Procure padrões de desperdício, principalmente:
- arquivos grandes sendo relidos
- comandos com outputs enormes entrando no contexto
- MCPs ou ferramentas sendo carregados sem necessidade
- repetição de contexto
- sessões excessivamente longas
- baixa utilização de cache
- mudanças de modelo ou configuração que aumentem consumo
Informe o modelo atual, effort/configuração relevante e qualquer
mecanismo que possa alterar automaticamente o modelo durante uma
sessão, caso essa informação esteja acessível.

C. Diagnóstico
Termine a Fase 1 com uma tabela:
PROBLEMA | TOKENS/CUSTO ESTIMADO | FREQUÊNCIA | IMPACTO | EVIDÊNCIA
Ordene pelo maior impacto estimado.
Depois pare e espere minha aprovação.

FASE 2 — CORRIGIR (depois que eu disser OK)
Proponha cortes e otimizações específicos para ESTE projeto. Podem
incluir: MCPs para desativar, CLAUDE.md para enxugar, regras
duplicadas, arquivos que não precisam ser carregados por padrão,
skills ou agentes excessivamente grandes, outputs de comandos que
deveriam ser reduzidos e outras fontes de desperdício encontradas
na auditoria.
Para cada mudança mostre:
O QUE MUDAR | POR QUÊ | GANHO ESTIMADO POR SESSÃO | RISCO/TRADE-OFF
Não aplique nenhuma mudança automaticamente. Apresente uma por vez
e espere minha aprovação.

FASE 3 — HÁBITOS
Usando os dados reais encontrados nas minhas sessões, identifique
os 3 hábitos que mais reduziriam o MEU consumo de tokens neste
projeto. Uma frase por hábito. Ordene pelo maior impacto esperado.
Não me dê dicas genéricas que não tenham evidência na auditoria.
```

## Instrução de estilo (como o Claude deve falar) — cola no CLAUDE.md

```text
Estilo de resposta (vale pra sessão inteira):
Direto ao ponto: a resposta vem na primeira frase.
Frases curtas. Um assunto por parágrafo.
Problema em 1 linha; solução em passos numerados.
Termo técnico só se explicar em seguida, em uma frase.
Se faltar informação pra executar, pergunte ANTES de fazer.
```

`aplicar_questionario.py` instala este bloco no `CLAUDE.md` do projeto quando a letra **J** do questionário é «sim».
