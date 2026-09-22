# Cognição: a lição estava escrita no irmão que o zelador chama

**Descoberto em 22/09/2026 23:41 UTC**, na rodada em que o dono mandou ativar o
time inteiro. Achado pelo papel D; provado por mim (papel A) no código, não no
ambiente — havia `cargo` vivo, e ambiente com frente viva confunde a medição nos
dois sentidos.

## 1. O que aconteceu

O `zelador.sh` **nunca consegue limpar o `target/` da árvore principal**, com ou
sem compilação rodando, porque ele se acha a si mesmo.

Dois fatos, e o defeito é a soma deles:

- `zelador.sh:22` — `cd "$(dirname "$0")" || exit 1`. O processo do script passa
  a ter `cwd` = raiz do repositório.
- `zelador.sh:65-77` — `em_uso()` varre `/proc/[0-9]*/cwd` e casa
  `"$dir"|"$dir"/*`, **sem excluir o próprio PID nem a própria linhagem**.

Então `zelador.sh:94`, o `if em_uso "$RAIZ"` que guarda a árvore principal, casa
o *bash do próprio zelador* e responde «alguém trabalha aqui, não toco no
target». Sempre.

As outras três chamadas de `em_uso` **não** têm o defeito, e o motivo é
geométrico: `:86` pergunta por `$REPO/.claude/worktrees/*`, `:307` pelo cache das
guardas e `:365` por cópias — e o `cwd` do script (`$RAIZ`) não está dentro de
nenhum desses. Só a que pergunta pela própria raiz se enxerga.

## 2. O que eu concluí primeiro, e estava errado

Quando o zelador reportou «`target/` está em uso, não toquei», eu ia registrar
isso como **a proteção funcionando** — havia mesmo dois `cargo` e dois `rustc`
vivos, com `cwd` no repositório, e a resposta estava certa.

Estava errado, e o erro é do tipo pior: **a resposta certa pelo motivo errado**.
A checagem teria dito exatamente a mesma coisa com a máquina parada. Ela não
mediu nada — repetiu o próprio nome de volta.

E a consequência que eu não teria visto: a proteção parece redundante quando
alguém for otimizar o script («isso nunca libera nada, tira»), e aí o dia em que
ela importar é o dia em que ela não estiver mais lá.

## 3. O que a medição disse

- `em_uso "$RAIZ"` (`:94`): **1 de 1** chamada afetada.
- As outras chamadas de `em_uso`: **0 de 3** afetadas (`:86`, `:307`, `:365`).
- Liberado pelo zelador nesta corrida: **0 MiB** — 16 diretórios soltos apagados,
  todos vazios; 597 guardados por PID vivo ou por mtime recente.
- O que a seção travada guarda: **7,1 G** em `target/debug`, de 7,3 G de `target`.
- Livre antes 6,3 G, depois **6,1 G** — caiu porque as frentes escreveram no
  `target` durante a própria checagem. A proteção era necessária hoje; a checagem
  é que não era o motivo dela.

## 4. A regra

**Observador se exclui por LINHAGEM, e a exclusão vale para toda forma de se
procurar — `pgrep`, `/proc/*/cwd`, `/proc/*/fd`, `lsof`.** Guarda que responde
certo com a máquina parada não está medindo nada.

## 5. Como está guardado hoje, e onde o buraco ficou

**A lição já estava escrita nesta casa, com estas palavras**, no arquivo que o
próprio zelador consulta — `bancada/esta-medindo.sh:29`:

> «E a exclusão do próprio observador nunca é por texto — é por LINHAGEM. Nenhum
> ancestral deste processo conta […] Descendente, ao contrário, CONTA.»

E é aí que está o aprendizado **novo**, porque a pétrea «conserto entra no
caminho que o motivou, e o caminho IRMÃO fica» já existe e não precisava de
terceira cópia. O que se aprende aqui é o **alcance** dela:

> A lição do observador que se acha foi escrita no `esta-medindo.sh` e **só cobre
> o observador que procura por `pgrep`**. O `zelador.sh` procura por
> `/proc/*/cwd` — outra forma de olhar, mesmo defeito —, **chama** o
> `esta-medindo.sh` como portão, e mesmo assim não herdou a lição. Irmão não é
> quem tem nome parecido nem quem é chamado pelo outro: é quem responde à mesma
> pergunta.

É a **quinta** vez que o observador se acha nesta base (a quarta está no histórico
como «o portão “está medindo?” — a quarta vez do `pgrep` que se acha»). As quatro
primeiras foram por texto na linha de comando; esta é por `cwd`, e por isso
nenhuma guarda de texto a pegaria.

**O buraco:** não há conferidor que procure «observador que varre `/proc` sem
excluir a própria linhagem». Enquanto não houver, a sexta vez virá por
`/proc/*/fd` ou por `lsof`, e ninguém a acha por leitura. Pedido 389.

**Não consertei o `zelador.sh` nesta rodada, e é decisão registrada:** o zelador é
a única frente desta rodada que apaga arquivo, há nove frentes vivas no
repositório, e mexer na checagem que decide o que ele apaga **enquanto elas
compilam** é o desenho de acidente que a lei do papel D existe para impedir. O
conserto entra com o ambiente parado, e a prova real é o par nos dois sentidos:
com o `target` ocioso a checagem tem de liberar, e com um `cargo` vivo tem de
recusar — hoje ela recusa nos dois casos, e é isso que o teste tem de pegar.
