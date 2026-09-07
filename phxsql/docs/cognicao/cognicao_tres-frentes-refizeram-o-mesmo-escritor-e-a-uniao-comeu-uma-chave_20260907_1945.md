# Cognição: três frentes refizeram o mesmo escritor, e a união comeu uma chave

**Descoberta:** 07/09/2026 19:45 UTC, na integração das seis frentes da rodada
das diretivas — o encontro que a cláusula diz que só o orquestrador vê.

## 1. O que aconteceu

Seis frentes mescladas no `main`, na ordem F6→F4→F1→F2→F5→F3. As quatro
primeiras entraram **sem conflito** — o `ort` uniu sozinho o que era aditivo em
lugares diferentes. F2, F5 e F3 conflitaram em `servidor.rs`, `config.rs` e
`usuarios.rs`, e o conflito de `config.rs` foi o mais fundo: **três frentes
refizeram o MESMO caminho de gravação do `config.json`**, com nomes e
assinaturas diferentes:

- F2: método `gravar_arvore(caminho, mudancas)` — 2 args, para o `gravar_nos_do_cluster`.
- F5: método `gravar_a_secao` + função livre `gravar_a_arvore(caminho, texto, arvore, caminhos)` — 4 args.
- F3: método `gravar_arvore(caminho, texto, arvore, campos)` — 4 args, um escritor **próprio** para a diretiva por banco.

O `gravar_arvore` de F3 (4 args) colide de nome com o de F2 (2 args) — Rust não
aceita dois métodos de mesmo nome no mesmo `impl`.

E no `servidor.rs`, a união do bloco grande (a seção de usuários da F5 seguida
da seção de diretivas da F3) deixou o `op_usuario` **sem o `}` de fechamento**:
a chave dele era a linha compartilhada DEPOIS do `>>>>>>>`, que a seção de
diretivas passou a consumir. O compilador só acusou como «unclosed delimiter»
no **fim do arquivo**, a 24 mil linhas de distância da causa.

## 2. O que eu concluí primeiro, e estava errado

Que resolver conflito de merge é escolher «o meu» ou «o dele» por bloco. Em
duas resoluções isso teria compilado e estado **errado**: pegar o lado da F3 no
`config.rs` traria um segundo escritor de mesmo nome (não compilaria, ok), mas
pegar o lado HEAD e ignorar a F3 teria **perdido a diretiva por banco** (o 220)
em silêncio — o merge escolhe um lado, e o lado perdido não deixa marca. É
exatamente o defeito que a cláusula descreve: «o dossiê perdeu uma seção porque
o merge escolheu o lado de quem não a tinha».

## 3. O que a medição disse

A resolução certa não era por bloco, era por **função**: um escritor só (a
função livre `gravar_a_arvore` da F5), e a diretiva por banco da F3
**religada** a ele (`Config::gravar_arvore(...)` → `gravar_a_arvore(caminho,
&texto, arvore, &[vec!["seguranca".to_string(), "comandos_proibidos".to_string()]])`),
descartando o escritor duplicado da F3. Medido depois: `cargo check` verde,
`clippy` **zero**, suíte inteira do workspace **exit 0**. O `git checkout
--ours config.rs` + splice de UMA função foi mais seguro que resolver três
blocos à mão, porque a única adição real da F3 ao arquivo era essa função — o
resto era o escritor duplicado.

E a chave que sumiu só apareceu porque **eu compilei entre os merges**, não só
no fim: o `cargo check -p phxsql-server` depois de cada frente arriscada é o que
achou o `}` do `op_usuario` antes de a F3 empilhar em cima.

## 4. A regra

**Conflito em que os dois lados refizeram a mesma função se resolve por
função, não por bloco — e sempre se compila ENTRE os merges, porque a união de
duas seções pode consumir a chave de fechamento de uma delas, e o compilador
acusa isso a milhares de linhas de distância da causa.**

## 5. Como está guardado hoje

- Os merges no `main` (`e48f586` e o `fmt` `e5ceb1c`), com as mensagens de
  merge dizendo a decisão.
- A função `acrescentar_proibidos_da_base` no `config.rs`, com o comentário
  «religado ao escritor livre na integração» explicando por que ela não usa o
  escritor que a F3 trazia.
- **O buraco:** não há guarda que impeça duas frentes de refazerem o mesmo
  escritor com nomes diferentes — isso é um custo do trabalho paralelo, e o que
  o pega é a compilação entre merges e a suíte no fim, não um conferidor. A
  cláusula já nomeia o alcance: a integração é papel do orquestrador **porque**
  o defeito do encontro não aparece para nenhuma frente sozinha.
