# O ciclo de qualidade desta casa

Este documento não substitui nada que já existe — só nomeia o ciclo que já
está rodando, com os arquivos e os números de quem faz cada parte:

- `provar.py` orquestra a bateria única, hoje **24 partes** (contadas em
  `parte(...)` em `provar.py`), em **14m35s** (`docs/TESTES.md` §7).
- `bancada/guardas/` é o catálogo dos defeitos já pagos e o executor que os
  repõe para conferir se o teste que cada um motivou ainda cai.
- `docs/TESTES.md` é a cobertura por área, medida e regenerada.

O que falta era escrever **por que** o ciclo tem essa forma — de onde vem cada
regra, e o que ela já custou quando não foi seguida. Por isso todo número
abaixo tem o arquivo ao lado: **regra da casa é número citado, não estimado**
(`CLAUDE.md`).

O PDCA da casa não tem quatro caixas iguais de tamanho. **Planejar** e
**Agir** são as mais baratas de pular e as mais caras quando se pula — é onde
ficam a maioria dos exemplos abaixo.

---

## Planejar — o pedido entra medido, não por palpite

Todo pedido do dono vira uma linha em `docs/PENDENCIAS.md`, na ordem em que
foi pedido, com um de três estados — `☑️ feito · ◐ parcial · ☐ planejado` — e
uma regra que já rendeu rebaixamento de estado por conta própria: **"o estado
é medido contra o código, não contra a lembrança"** (`docs/PENDENCIAS.md`,
linha 3). Foi assim que a chave estrangeira caiu de "pronto" para "parcial".

A parte que este ciclo acrescenta ao planejamento é uma trava, não uma
liberdade: **medir a premissa do item vem antes de implementar o item** — e
vale até quando o item é nosso, escrito por nós, com o alvo certo na primeira
leitura.

### O caso que prova a regra: o pedido 113

O pedido 113 (`docs/PENDENCIAS.md`, linha 122) chegou como hipótese
plausível: **"ordene as chaves do lote antes do `.ndx`"**, porque descer a
B+tree em ordem aleatória custa **localidade** de página. O alvo estava certo
— o `.ndx` respondia por **83,5%** do tempo de uma inserção
(`docs/DESEMPENHO.md`, §1) — e a causa proposta estava errada.

Medido com `cargo run --release --example ordem-da-chave` (`docs/DESEMPENHO.md`
§4.1), **antes** de qualquer outra mudança, a desordem custava só **1,06×** —
ordenar teria comprado quase nada, porque com tudo em RAM a ordem de chegada
não muda o trabalho. O custo real era outro: cada inserção **relia do arquivo
e recalculava o CRC-32 da mesma página** a cada descida da árvore, a raiz
incluída.

Um cache de páginas de leitura — sem tocar a ordem de digitação, sem mudar o
formato — levou a inserção de **44,4 para 18,5 µs por linha**, **2,4×**
(`docs/DESEMPENHO.md` §1 e §2 — o mesmo cache que `CLAUDE.md`, na regra
"Receita de fora se mede contra o nosso gargalo antes de virar plano", registra
como **"2,40×"**, mesma medição arredondada com uma casa a mais). Só
**depois** desse conserto a localidade passou a valer alguma coisa: a mesma
bancada, reordenada, mostrou **1,19×** — bem menor que o **2,4×** do
diagnóstico certo, e só visível depois dele (`docs/DESEMPENHO.md` §4.1). Nem
essa fração foi implementada: ordenar o lote
exigiria conhecer os rowids antes de gravar o `.reg`, o que rebaixaria uma
garantia real do formato — está registrado com o número na mão, para a
decisão ser tomada com ele, não sem ele.

O mesmo documento traz uma segunda hipótese que a própria casa gerou e
derrubou medindo: adiar o `.ndx` inteiro durante a carga e reconstruir no
fim. Medido com `cargo run --release --example indice-adiado`
(`docs/DESEMPENHO.md` §4.2), o ganho foi **1,02×** — **"um por cento"**,
porque reconstruir hoje é chave a chave, o mesmo trabalho movido de lugar, não
apagado. A hipótese morreu medida, e o documento registra onde estaria o
ganho de verdade (reconstrução em lote, sem descida de árvore) sem
implementá-lo — para ninguém propor "adiar o índice" de novo sem essa conta
na mão.

**O que planejar aprende com o 113:** uma proposta com o alvo certo pode ter
o mecanismo errado, e só a medição separa as duas. Planejar aqui não é
aprovar a ideia — é rodar o medidor antes.

---

## Fazer — os portões, e o binário tem de ser o de agora

Três portões, nesta ordem, descritos em `docs/PORTOES.md` e aplicados pelo CI
em `.github/workflows/portoes.yml` a cada `push` e `pull request`:

1. `cargo fmt --all --check` — formatação.
2. `cargo clippy --workspace --all-targets -- -D warnings` — **zero avisos**,
   não "poucos avisos".
3. `cargo test --workspace` — a suíte inteira, o único que exercita
   comportamento.

**Nenhum roda atrás de cano.** `docs/PORTOES.md` (linhas 8–14) é explícito:
"nada de `| tail`, `| grep -v ...` ou coisa parecida... **já foi defeito real
aqui**" — um cano troca o código de saída do comando pelo do último elo,
então `cargo test` reprova e o `tail` devolve sucesso do mesmo jeito, e o CI
marca verde um commit com teste quebrado dentro. O documento registra que o
defeito já aconteceu; não achei, em `docs/PORTOES.md`, `CHANGELOG.md` nem no
histórico de commits, um número medido para esse incidente específico — só a
frase "já foi defeito real aqui". Registro a ausência em vez de inventar um.

O toolchain é **pinado** (`rust-toolchain.toml`), não "o que estiver
instalado": sem isso, o mesmo commit passa numa máquina e falha noutra sem
nenhuma linha de código ter mudado, porque um clippy mais novo acrescenta
lints sozinho (`docs/PORTOES.md`).

### A regra do binário velho, e o que ela escondeu

`cargo build --release` **não recompila os `examples`** — e a bancada de
desempenho chama `target/release/examples/carga` direto. `docs/DESEMPENHO.md`
§4.8 documenta o dia em que isso escondeu uma rodada inteira de ganhos:

| | µs/linha |
|---|---:|
| 0.17.0 | 16,4 |
| + cabeçalho do `.ndx` fora do caminho da chave | 14,5 |
| + CRC slice-by-16 | 13,1 |
| + cache write-back | **7,5** |

**2,19×** de ganho real (`docs/DESEMPENHO.md` §4.8) — mas a bancada publicada
mal se mexeu, **265,2 → 261,8 s**, e a primeira explicação escrita foi "o
esquema da bancada custa 2,2× — o `Decimal` e o `Date` levam a inserção de
7,50 para 16,61 µs". Tinha tabela, tinha número, e estava **errada**. A prova
em três passos (mesmo documento): tabela recém-criada e reaberta inseriam
igual (**7,48 contra 7,46 µs**); um exemplo novo com o mesmo esquema dava
**8,0 µs** contra os **16,9 µs** do `carga`, na mesma máquina; e `ls -l` no
binário mostrou que `target/release/examples/carga` era **anterior ao
write-back**. Recompilado: **7,92 µs/linha, 126.280 linhas/s** — o esquema
custa ~5% a mais, não 2,2×.

`docs/DESEMPENHO.md` chama isso de "**o sétimo diagnóstico plausível que este
documento derruba**" — e o corolário virou regra permanente, citado
literalmente em `CLAUDE.md`: *"Medidor com binário velho mede o passado."*
Antes de qualquer bancada: `cargo build --release --examples -p
phxsql-store`.

### A bateria como Fazer disciplinado, não como martelo único

As guardas não rodam `cargo test --workspace` a cada defeito reposto — rodam
só o binário de teste nomeado pela entrada do catálogo. Medido em
`bancada/guardas/LEIA-ME.md`: o binário nomeado custa **8,1 s**, o workspace
inteiro **49,2 s**; para as entradas de então isso significava **~2 min
contra ~15 min** — a diferença entre caber dentro da bateria única e dobrá-la.
"Fazer" certo aqui não é rodar tudo sempre — é rodar o que prova o que está
sendo mudado, com prazo em cada rodada (a guarda `sujas-com-a-trava` sozinha
leva **35,3 s** porque o defeito dela pendura em vez de falhar; as outras 18
da mesma leva iam de 1,4 a 13,2 s).

---

## Conferir — prova real nos dois sentidos, e o que só o soquete acha

A regra é dupla, e as duas metades importam igualmente: **o teste tem de
FALHAR com o defeito reposto e passar com o conserto.** `bancada/guardas/`
existe para provar a primeira metade em escala — hoje o catálogo carrega
(medido nesta revisão, rodando `python3 -c "import catalogo;
print(len(catalogo.GUARDAS))"` em `bancada/guardas/`) **57 entradas**. O
executor devolve um de cinco vereditos por entrada (`bancada/guardas/LEIA-ME.md`):

| veredito | o que quer dizer |
|---|---|
| **PROVADA** | todos os `caem` caíram, todos os `seguem` continuaram de pé |
| **REDUNDANTE** | a guarda existe duas vezes no código; tirar uma só não muda nada — resultado medido, não falha |
| **NAO PEGOU** | um `caem` continuou passando — teste que passa por engano |
| **ESTRAGOU** | um `seguem` caiu junto — a troca quebrou mais do que o defeito original quebrava |
| **QUEBRADA** | a entrada envelheceu: o trecho não está mais no arquivo, ou nem compila |

E há uma terceira metade que ninguém pede por padrão, mas que o executor
confere sempre: a lista `seguem`, os testes que **têm** que continuar
passando. Sem ela, uma troca que quebrasse o arquivo inteiro pareceria uma
guarda excelente.

### NAO PEGOU: os dois achados que só a mutação, não a leitura, encontrou

**O teste da regra pétrea do fio passava por engano.**
`cliente_sem_cifra_continua_como_antes` deveria cair quando o padrão de
`cifra_fio.exigir` virasse `true` — e continuava verde. A causa:
`docs/CIFRA-DO-FIO.md` (linhas 573–578) mostra que o próprio teste montava o
`Config` na mão e escrevia `cifra_fio.exigir = false`, desfazendo o defeito
antes de exercitar qualquer coisa. Consertado subindo de um `config.json`
**sem** a seção `cifra_fio` — o arquivo real de quem atualizou o binário sem
mexer em nada. "Um teste que escreve o campo não pode provar o padrão dele"
(`docs/CIFRA-DO-FIO.md`, linha 577).

**O erro global por FFI escondia uma corrida.** Com a "vaga de erro global"
reposta como defeito, `ultimo_erro_e_por_thread` continuava passando, porque
os outros testes do mesmo binário rodam em paralelo e o `limpar()` de
qualquer um deles esvaziava a vaga bem a tempo (`docs/EMBUTIDO.md`, item (4)).
Consertado trocando "a outra thread vê vazio" por uma ordem estrita entre
duas threads.

### ESTRAGOU (a lista `seguem` provando a largura do estrago)

A guarda `regra-de-tabela-imposta` repõe o defeito mais repetido nesta casa:
regra de permissão nova que **impõe** em vez de **pedir**. O trecho reposto
faz `permissoes_em` **negar tudo** quando não há regra de tabela nenhuma, em
vez de cair na regra da base (`bancada/guardas/catalogo.py`, linhas 478–503).
Com o defeito reposto, **caem 14 dos 540 testes** do `--lib` — entre eles os
"continua valendo" do duplicar e do pivotar —, e o teste
`supervisor_passa_por_cima` (o `seguem`) sobrevive porque não depende de
regra de tabela nenhuma (`bancada/guardas/catalogo.py`, linhas 509–517;
`bancada/guardas/LEIA-ME.md`, últimas linhas). A largura do estrago **é** o
argumento: uma guarda imposta tira o direito de todo cliente que já
funcionava, e quem trava isso é justamente o teste do comportamento
**velho** — `sem_regra_de_tabela_nada_muda`, o teste que efetivamente cai.

### O que só o soquete acha, e não o teste de unidade

**BULKINSERT.** Dez testes de `servidor.rs` passavam exercitando a transação
pela API interna, com a `Sessao` montada à mão. A prova pelo soquete mostrou
que a queda da conexão **não soltava a reserva**. A causa não estava no
servidor: estava no teste — `socket.makefile()` do Python **segura o
descritor**, e fechar só o soquete deixa o `fd` aberto, então o servidor
nunca via o fim da conexão (`bancada/transacoes/provar.py`, linhas 6–18 e
120). Duas lições numa: o que depende do sistema operacional se prova contra
o sistema operacional, e um teste que passa por engano é pior que um teste
que falta.

**PostgreSQL(R), 19 conferências contra o `psql`.** `bancada/dblink/prova-postgres.py`
roda as cinco operações do DbLink contra um PostgreSQL(R) **16.13 real**, com
cada resultado conferido contra o `psql` — o oráculo independente, porque
conferir contra o que o próprio script espera só provaria que o script e o
servidor concordam (`docs/PENDENCIAS.md`, linha 95). A prova achou um defeito
com três sintomas — `dblink_tabelas` vazio, `dblink_estrutura` sem colunas,
`dblink_ler` derrubando "relation does not exist" — de uma causa só: `base`
significa *database* no MySQL(R) e *esquema* no PostgreSQL(R), e o código
tratava os dois como o mesmo campo. **Prova real nos dois sentidos:** com o
defeito reposto, a prova **reprova em 14 das 19** conferências e nomeia cada
uma (`docs/DBLINK.md`, linha 321; `bancada/dblink/LEIA-ME.md`, linha 87; a
mesma conta está em `docs/PENDENCIAS.md`, linha 95). Sem o defeito, passa 19
de 19.

**Os três motores, com a data constante reposta.** `bancada/comparacao/medir.py`
mede PhxSql, MySQL(R) e SQLite(R) **intercalados na mesma rodada**, e a
própria montagem da bancada achou uma violação da regra "mesmos dados": a
versão antiga gravava `'2024-10-04'` em toda linha, enquanto o `carga.rs` e a
bancada do SQLite(R) gravam uma data que varia por linha
(`bancada/comparacao/LEIA-ME.md`). A fase `conferir` do `carga.rs` obriga os
três motores a chegarem à mesma soma de `cadastro`, contra a forma fechada
calculada à parte (**20.199.500.000**). **Prova real nos dois sentidos:**
repor a data constante faz a soma de `cadastro` chegar a **400.000.000** em
vez de **403.990.000** — e 400.000.000 é exatamente 20.000 linhas × dia
20.000, a assinatura do próprio defeito (`docs/DESEMPENHO.md`, linhas
1990–1993; `bancada/comparacao/LEIA-ME.md`). Não achei, nesta bancada, uma
contagem "N de M" como a do PostgreSQL(R) — o veredito aqui é binário
(publica ou recusa publicar); registro a diferença em vez de emprestar o "14"
de uma prova para a outra.

### O que este ciclo achou rodando agora, não em documento antigo

Rodando o catálogo nesta revisão (`python3 -c "import catalogo;
print(len(catalogo.GUARDAS))"`, dentro de `bancada/guardas/`): **57
entradas**. A última tabela regenerada em `docs/TESTES.md` (marcada "medido
em 2026-08-30 17:28") mostra **43 guardas: 40 provadas, 3 redundantes**. A
diferença não é uma tabela errada — é uma tabela **esperando o próximo
`tabela-no-testes.py`**, depois da leva de guardas que a integração das
transações acrescentou (`git log` mostra o commit das transações depois do
commit que gerou aquela tabela). Registro aqui como achado desta rodada de
QA, não como defeito corrigido: quem fizer a próxima rodada de Fazer nesta
frente deve rodar `provar-guardas.py --json` e `tabela-no-testes.py` de novo
antes do próximo dossiê, porque **número visível que não sai do gerador da
vez é número que envelhece calado** — a mesma regra do 780 KiB, abaixo.

---

## Agir — onde o aprendizado fica, e a hipótese morta também é resultado

Duas gavetas, com papéis diferentes:

- **`CHANGELOG.md`** — uma entrada por versão, sempre nas mesmas quatro
  seções: **Corrigido, Adicionado, Mudado, Sabido** — "a seção *Sabido* lista
  o que ainda não funciona, para ninguém descobrir sozinho" (`CHANGELOG.md`,
  linhas 5–7). E a regra de abertura do arquivo, cumprida por este próprio
  documento: **"Os números são medidos, nunca estimados"** (`CHANGELOG.md`,
  linha 9).
- **O documento da área** — `docs/DESEMPENHO.md`, `docs/CIFRA-DO-FIO.md`,
  `docs/MENSAGENS.md`, `docs/EMBUTIDO.md`, `docs/DBLINK.md` e os outros —
  onde o número e a explicação ficam ao lado do código que os produz, para a
  próxima sessão medir de novo em vez de confiar na conversa passada.

A terceira gaveta é o **documento de tecnologias** que a cláusula pétrea
exige — o inventário do que se usou para fazer o produto **e** para fazer o
trabalho, com o que foi avaliado e recusado. Ele existe, e **fora daqui**:
`base-de-conhecimento/05-TECNOLOGIAS.md`, ao lado do extrator que refaz a base
do transcrito da sessão.

Vale registrar por que esta frase quase saiu errada. A primeira versão deste
documento afirmava que o tal inventário **não existia** — porque a busca que a
escreveu olhou só dentro de `phxsql/`, e a base de conhecimento mora um nível
acima, no repositório. *Ausência que se conclui de uma busca é do tamanho da
busca*, e afirmar ausência é uma afirmação como outra qualquer: precisa dizer
onde se procurou.

### A hipótese que morre medida é resultado — não é rodada perdida

Três casos, já citados acima por outro ângulo, valem repetidos aqui pelo que
**Agir** faz com eles:

1. **Ordenar as chaves do lote** (pedido 113) — seria 1,06× antes do cache
   certo, 1,19× depois. Não implementado; o número fica escrito para que
   ninguém precise medir de novo antes de propor a mesma coisa
   (`docs/DESEMPENHO.md` §4.1).
2. **Adiar o `.ndx` inteiro e reconstruir no fim** — 1,02×, "um por cento",
   porque reconstruir hoje é o mesmo trabalho movido de lugar
   (`docs/DESEMPENHO.md` §4.2). O documento não fecha a bateria aí: aponta o
   que a reconstrução em lote de verdade custaria (0,21 s para varrer e
   codificar 200 mil chaves, 0,03 s para ordenar) contra os 2,54 s de hoje —
   a hipótese morta **gerou a próxima hipótese**, ainda não implementada.
3. **"O esquema custa 2,2×"** — nasceu com tabela e número, e morreu com
   outra tabela e outro número três parágrafos depois (`docs/DESEMPENHO.md`
   §4.8, narrado acima em Fazer). Ficou registrado como "o sétimo diagnóstico
   plausível que este documento derruba" — o próprio documento se audita.

O padrão dos três: recusa medida **impede a mesma proposta de voltar sem
medição** — é o mesmo motivo pelo qual a base de conhecimento (papel J, e a
gaveta que falta acima) tem seção própria para "o que foi avaliado e
recusado".

---

## As armadilhas que este ciclo já pagou

Cada uma tem o número medido, e o arquivo onde ele mora.

1. **`socket.makefile()` segura o descritor.** Dez testes de unidade do
   `BULKINSERT` passavam; a prova pelo soquete achou que a queda de conexão
   não soltava a reserva. A causa era do teste, não do servidor — fechar só o
   `socket` em Python deixa o `fd` do `makefile()` aberto.
   `bancada/transacoes/provar.py`, linhas 6–18.

2. **Medidor com binário velho mede o passado.** `cargo build --release` não
   recompila `examples`; a bancada rodou com um `carga` **anterior ao
   write-back** e publicou 265,2 → 261,8 s quase parado, enquanto o ganho
   real por linha era **16,4 → 7,5 µs (2,19×)**. Recompilado: **7,92
   µs/linha, 126.280 linhas/s**. `docs/DESEMPENHO.md` §4.8; a regra
   permanente está em `CLAUDE.md`.

3. **`| tail` (e qualquer cano) troca o código de saída.** Os três portões
   rodam sem cano nenhum porque um cano já deixou um commit passar verde com
   teste quebrado dentro — o último elo do cano é quem decide o código de
   saída, não o `cargo test`. `docs/PORTOES.md`, linhas 8–14. Não achei um
   número medido para este incidente específico nos documentos disponíveis;
   registro a ausência em vez de estimar um.

4. **Número digitado à mão envelhece calado.** A receita do KiB de interface
   do dossiê era uma lista de três arquivos copiada dentro do gerador; o
   `http.rs` passou a embutir nove, e o rodapé publicava **780 KiB** quando a
   interface já tinha **1.032**. A lista sai do próprio `http.rs` hoje.
   `CHANGELOG.md`, linhas 1452–1458; `docs/dossie/LEIA-ME.md`, linha 84.

5. **Guarda nova imposta em vez de pedida quebra quem já funcionava — e a
   largura é o argumento.** Com a regra de permissão por tabela reposta como
   "nega sem regra declarada", caem **14 dos 540** testes do `--lib` de uma
   vez; o teste do comportamento velho é o que sobra de pé.
   `bancada/guardas/LEIA-ME.md`, últimas linhas.

6. **Teste que passa por engano por escrever o próprio campo que devia
   provar.** `cliente_sem_cifra_continua_como_antes` continuava verde com o
   padrão trocado para `exigir: true` porque montava o `Config` na mão e
   desfazia a troca antes de testar qualquer coisa.
   `docs/CIFRA-DO-FIO.md`, linhas 573–582.

7. **PostgreSQL(R) real, 19 conferências contra o `psql` — e a prova cai em
   14 quando o defeito volta.** Um campo (`base`) que significa *database*
   num motor e *esquema* no outro, sem tradução, derrubava três sintomas
   diferentes de uma causa só. `docs/DBLINK.md`, linha 321;
   `bancada/dblink/LEIA-ME.md`, linha 87.

---

## O que este documento não é

Não é um roteiro de comandos — esses estão em `provar.py`,
`bancada/guardas/provar-guardas.py` e no cabeçalho de cada `docs/*.md`
citado acima. É o porquê de cada portão existir, com o incidente que o
motivou e o número que ele deixou. Quando um portão novo entrar, a regra é a
mesma da cláusula pétrea: ele entra **pedido**, com o defeito que o motiva
escrito ao lado — não imposto, e não sem número.
