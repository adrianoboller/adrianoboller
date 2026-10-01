# «O branch da frente é ancestral do HEAD» não prova que o trabalho dela foi integrado

**Estado:** INFRUTÍFERO
**Causa:** a limpeza de cópias de trabalho decidia «integrada» por
`git merge-base --is-ancestor worktree-agent-X HEAD`. As frentes NÃO comitam —
a regra da casa é que só o integrador comita —, então o branch de toda frente
fica parado no commit de onde ela saiu, e esse commit é SEMPRE ancestral do
HEAD. A conferência dizia «integrada» para frente cujo trabalho inteiro morava
só na árvore de trabalho, e o `git worktree remove -f -f` o apagou: em
01/10/2026, quatro frentes prontas e com portões verdes (513 passo 1, 329+331,
a catraca do 335, 600+601+245) sumiram assim. A prova de «nenhum processo usa»
(cwd, descritores, mapas) passou — ela responde outra pergunta.
**Prevenção:** antes de apagar uma cópia de trabalho, exigir as DUAS coisas:
`git -C <copia> status --porcelain` vazio (nada fora de commit) E o commit da
ponta alcançável do HEAD. Árvore suja nunca é «integrada», seja qual for o
grafo. E nunca `-f -f` numa cópia que a conferência não provou limpa: o `-f`
existe para pular exatamente a proteção que teria parado o erro. Por ordem do
dono, além disso, o trabalho de toda frente é salvo como objeto do git
(`phxsql/salvar-frentes.sh`, `refs/salvas/`) a cada 10 minutos e antes de
toda limpeza, e só `phxsql/limpar-frentes.sh` apaga cópia de frente.

## O que aconteceu

O integrador limpava o disco entre integrações (pétrea (B) do escopo
congelado: cópia de frente integrada se apaga depois de provar que nenhum
processo a usa). Até a 12ª limpeza do dia a conferência «ancestral» acertou
por acaso: o integrador sempre fazia `git add -A && git commit -m "WIP da
frente"` na cópia ANTES do merge, então só as frentes já mescladas tinham
commit próprio. Na 13ª, a limpeza rodou com quatro frentes ENTREGUES e ainda
não mescladas — sem o commit de WIP, o branch delas era a base, ancestral do
HEAD, e as quatro foram apagadas.

## O que eu concluí primeiro, e estava errado

Que «ancestral do HEAD» e «integrado» eram a mesma coisa. Valia só porque o
passo do commit de WIP vinha antes; a premissa nunca estava na conferência,
estava na ordem dos meus comandos — e a ordem mudou sem a conferência saber.

## A recuperação

Os agentes não puderam ser retomados enquanto a pasta deles não existia
(«its worktree no longer exists»). Recriada a cópia no MESMO caminho, com o
mesmo nome de branch, a partir do HEAD do dia, a retomada passou, e cada
agente refez o próprio trabalho a partir do contexto dele e dos scripts que
deixou na pasta de rascunho. Custo: a rodada de cada uma das quatro frentes,
de novo.

## O número

4 frentes apagadas, 0 bytes recuperáveis do disco (nada estava em objeto do
git), 4 retomadas possíveis pela recriação do caminho.

## O alcance, medido no mesmo dia (quarta prova)

As tres provas nao bastam para copia RECEM-CRIADA: ela esta limpa, a ponta e
o proprio HEAD, e o agente pode ainda nao ter entrado nela. Em 01/10/2026 o
`limpar-frentes.sh` chegou ao `git worktree remove` em tres copias de agentes
lancados segundos antes; quem segurou foi a tranca que o harness poe
(`git worktree lock`). Agora a tranca e a quarta prova, conferida pelo script
antes de tentar -- e nao mais uma recusa do git da qual se depende por sorte.
