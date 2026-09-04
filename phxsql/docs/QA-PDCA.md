# O ciclo de qualidade desta casa

Este documento não substitui nada que já existe — só nomeia o ciclo que já
está rodando, com os arquivos e os números de quem faz cada parte:

<!-- bateria:inicio -->
- `provar.py` orquestra a bateria única, hoje **27 partes** — o número sai de `python3 provar.py --listar`, nunca digitado aqui.
<!-- bateria:fim -->
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
print(len(catalogo.GUARDAS))"` em `bancada/guardas/`) **60 entradas** — 57
mais as três que esta rodada de QA acrescentou (seção "As cinco, com guarda",
abaixo). O
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

## O catálogo de guardas — levantamento de 2026-09-02

O ciclo acima descreve **como** o PDCA roda. Faltava responder uma pergunta
mais simples e mais perigosa de deixar sem resposta: **quais guardas
existem, contra qual defeito cada uma protege, e quando foram provadas pela
última vez?** Sem essa lista, guarda vira uma coisa que ninguém sabe que
existe — e guarda que ninguém sabe que existe é guarda que alguém apaga sem
perceber, numa limpeza de "testes redundantes" ou numa reescrita de arquivo.

Esta seção é esse levantamento, com data e comando de cada número. Reproduza
com `cargo build --release --examples -p phxsql-store` primeiro (a regra do
binário velho vale para conferidor também) e as duas árvores estão sujeitas a
mudar **enquanto você lê**: outras frentes trabalham nesta mesma árvore, e o
número de testes mudou de **1.485 para 1.495** entre duas medições desta
própria rodada, minutos uma da outra (`git log -1` em `a727835`,
2026-09-02T18:52:38Z). Trate os números abaixo como uma fotografia datada, não
como uma verdade parada.

### As catracas numéricas — constante contra medido agora

Quatro catracas numéricas existem hoje na árvore (achadas varrendo `TETO` e
`catraca` em `crates/`; os outros `TETO_*` — `TETO_DO_CAMPO`,
`TETO_JUNCAO`, `TETO_PIVOT`, `TETO_DO_LOTE_SERVIDO`, `TETO_DO_REGISTRO` — são
**limites operacionais** do motor, não catracas de qualidade: não medem uma
dívida que só deve encolher, travam um comportamento em produção).

<!-- catracas:inicio -->

| Catraca | Onde mora | Valor | Medido hoje | Estado |
|---|---|---:|---:|---|
| `TETO_TABELA_NA_MAO` (tabelas montadas a mao em vez de PhxGrid) | `crates/phxsql-server/src/conferidor_grades.rs` | 0 | **0** | em cima, sem folga |
| `TETO_ROTULOS_E_CRASE` (textos cravados fora da fabrica de idiomas) | `crates/phxsql-server/src/conferidor.rs` | 1.707 | **1.707** | em cima, sem folga |
| `TETO_COLADO` (chaves com os seis idiomas identicos) | `crates/phxsql-server/src/conferidor.rs` | 0 | **0** | em cima, sem folga |
| `TETO_FRASE_REPETIDA` (frase longa repetida em tres ou mais idiomas) | `crates/phxsql-server/src/conferidor.rs` | 0 | **0** | em cima, sem folga |

*4 catraca(s) medida(s) por conferidor. Refaz com `python3 docs/qa/medir.py`.*

**Constantes `TETO*` que NENHUM conferidor reporta.** Elas não são
catracas: são limites, ou promessas. A diferença importa — catraca
sem medidor não segura nada e ainda parece que segura:

- `TETO_DO_REGISTRO` — `crates/phxsql-core/src/fio.rs:494`
<!-- catracas:fim -->

> **Esta tabela NÃO se edita à mão — ela se gera.** Com
> `python3 docs/qa/medir.py --gravar`, e nada entre as duas marcas acima
> sobrevive a isso.
>
> **A hora em que o número envelheceu diz por que o gerador precisou existir.**
> A tabela foi levantada com `TETO` em **1.577**; na mesma rodada, outra frente
> traduziu 28 textos e baixou a catraca para **1.549** — e nenhuma das duas
> podia ver a outra. Com trabalho paralelo, número digitado não envelhece
> «entre versões»: ele já nasce errado.
>
> **E o gerador não tem lista de catracas.** Ele varre `crates/*/examples/` atrás
> de quem imprime `catraca:`, e pergunta a cada um — cada conferidor se
> descreve. Lista digitada dentro de um script é exatamente a receita que
> envelhece, e esta casa já pagou por ela: uma lista de três arquivos copiada
> num gerador fez o rodapé publicar 780 KiB quando a interface tinha 1.032.
>
> **Ele acusa dois estados que uma tabela à mão esconde:** catraca **frouxa**
> (valor acima do medido, e a folga é onde uma regressão se esconde) e
> constante `TETO*` que **nenhum conferidor mede** — que não é catraca, é
> promessa. Na primeira corrida ele apontou duas assim, e as duas eram catracas
> de verdade: `TETO_COLADO` e `TETO_FRASE_REPETIDA` existiam, os testes as
> impunham, e o número delas aqui era **digitado**. Hoje os conferidores as
> reportam.

**Nenhuma catraca está frouxa nesta rodada** — as quatro batem exatamente
com o medido, o que é o estado saudável (a constante nunca fica **abaixo**
do medido, porque aí a suíte reprovaria sozinha; o achado seria uma catraca
**acima** do medido por mais do que a folga que cada teste tolera, e não foi
o caso). Comandos, na raiz do repositório:

```bash
cargo run --release --example textos-fora-da-fabrica -p phxsql-server   # imprime "catraca (so desce) ..... N"
cargo run --release --example grades-fora-do-padrao   -p phxsql-server  # imprime "catraca (so desce) ..... N"
cargo test --release -p phxsql-server --lib conferidor:: --lib conferidor_grades::
```

Não achei um quinto conferidor numérico na árvore — procurei por `TETO` e por
`catraca` em `crates/`, `bancada/` e `docs/`; os únicos candidatos adicionais
(`bancada/profiler/sonda-log.py`, que compara contra um `teto` calculado na
hora, e `docs/dossie/numeros-do-projeto.py`, que só **lê** a constante acima
para publicar no dossiê) não são catracas próprias — o primeiro é limiar de
uma bancada de desempenho, o segundo é consumidor da tabela acima, não
produtor. Registro a busca em vez de garantir que não existe um quinto em
algum canto que a varredura não cobriu.

### O catálogo de guardas nomeadas, por área

`bancada/guardas/catalogo.py` é o catálogo **oficial** de mutação — hoje
**60 entradas** (`python3 -c "import catalogo; print(len(catalogo.GUARDAS))"`
dentro de `bancada/guardas/`; eram 57 no início desta rodada de QA, que
acrescentou as três descritas em "As cinco, com guarda"), e a tabela em
`docs/TESTES.md` §8 já está fresca desta mesma rodada: **60 guardas: 56
provadas, 4 redundantes, 0 não pegaram, 0 estragaram, 0 quebradas** — a
defasagem que a revisão anterior deste documento registrou (43 contra 57)
**já foi corrigida** por outra frente antes desta rodada de QA começar.
Nenhuma entrada veio como `NAO PEGOU` ou `ESTRAGOU` nesta rodada; a corrida
completa só foi possível depois de um achado no caminho — o `COPIAR` do
executor não incluía `docs/`, e um teste que lê `docs/ROTEIRO-1.0.md` em
tempo de execução fazia a árvore limpa reprovar antes de qualquer defeito
ser reposto (consertado, ver a cognição desta rodada).

Esse catálogo mede **mutação de trecho**, não **nome de teste**. Só ele não
responde à pergunta desta seção, porque a maioria das guardas desta casa não
está no `catalogo.py` — está espalhada pelo código, como um teste cujo nome
**é** a frase do defeito, com o histórico no comentário `///` acima dele.
Uma varredura de `#[test]` cujo comentário traz palavras de incidente
(quebrava, defeito, escondia, vazava, achou, corrigiu, passava por engano…)
achou **109** candidatos em `crates/` (script descartável, reproduza com:
`grep -rln "#\[test\]" --include="*.rs" crates/` e leia o comentário de cada
achado — não escrevi um gerador para isto porque o crivo é de leitura, não de
sintaxe, e um gerador que só recorta a linha erraria a mesma forma que a
regra "rótulo se analisa, nunca se recorta" já condena). A tabela abaixo é uma
amostra de ~70 dessas guardas, organizada por área, com o defeito tal como o
próprio comentário do teste conta — é a parte navegável deste levantamento;
a lista completa de 109 fica no comando acima para quem for estender esta
tabela.

**Integridade referencial**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `a_mae_nao_gravada_recusa_dizendo_por_que` | `phxsql-store/tests/chave-estrangeira.rs:107` | filha aponta para mãe que não existe e a gravação passava calada |
| `sem_indice_na_mae_a_recusa_diz_qual_indice_falta` | `phxsql-server/src/servidor.rs:20039` | sem índice na mãe, conferir a chave varreria a tabela por linha — o motor recusa e nomeia o índice |
| `ao_excluir_so_aceita_restringir` | `servidor.rs:20072` | regra primordial: cascata, anular e nada aceitos no `ao_excluir` matariam pai com filhos |
| `ao_excluir_aceita_restringir_escrito_de_tres_jeitos` | `servidor.rs:20097` | o par do teste acima — sem ele, um portão que recusa TUDO passaria disfarçado de conferência |
| `a_chave_declarada_nasce_conferida` | `servidor.rs:19939` | chave declarada sem `"verificar"` deixava filha órfã entrar sem reclamação |
| `quem_pede_para_nao_conferir_continua_podendo` | `servidor.rs:19960` | a opção de **não** conferir tinha de continuar existindo, escrita — a decisão do dono não podia levar a opção junto |
| `declarar_fk_sem_tabela_ref_recusa_em_vez_de_apontar_para_si` | `servidor.rs:20220` | tabela-mãe inexistente na declaração apontava para si mesma em vez de recusar nomeando quem falta |
| `duplicar_tabela_preserva_a_chave_estrangeira` | `servidor.rs:20123` | duplicar tabela perdia a FK do original |
| `excluir_tabela_nao_deixa_arquivo_nenhum_para_tras` | `phxsql-store/src/catalogo.rs:1065` | `excluir_tabela` apagava só 6 extensões quando a tabela já tinha 9 (mesmo defeito que `excluir-tabela-lista-curta` no catálogo de mutação) |
| `excluir_tabela_deixa_o_nome_livre_para_a_proxima` | `catalogo.rs:943` | nome da tabela excluída continuava reservado, impedindo recriar com o mesmo nome |
| `chave_mal_escrita_recusa_e_a_tabela_nao_nasce` | `servidor.rs:19870` | `ao_excluir`/`ao_alterar` com valor inválido deixava a tabela nascer mesmo assim |

**Permissão e portão de acesso**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `sem_regra_de_tabela_nada_muda` | `servidor.rs:18200` | regra de tabela nova negando tudo por padrão tiraria o direito de todo cliente que já funcionava |
| `juntar_nao_e_a_porta_dos_fundos` | `servidor.rs:17983` | `juntar` guarda as tabelas em `a.tabela`/`b.tabela` — o portão que só olha `"tabela"` não via nenhuma das duas |
| `unir_nao_e_a_porta_dos_fundos` | `servidor.rs:18002` | `unir` guarda numa lista — mesma porta dos fundos, campo diferente |
| `pivotar_nao_e_a_porta_dos_fundos` | `servidor.rs:18028` | tabela de fatos passa pelo portão comum, mas a lista `juntar` do pivot tinha campo próprio sem conferência |
| `pivotar_na_tabela_permitida_continua_valendo` | `servidor.rs:18054` | garante que a conferência nova não vira parede: quem tem direito continua pivotando |
| `posicao_esconde_a_tabela_negada` | `servidor.rs:18104` | `posicao` devolvia eventos e o esquema cru de tabela negada |
| `sem_regra_de_tabela_posicao_e_sequencias_veem_tudo` | `servidor.rs:18141` | sem regra de tabela nenhuma, `posicao`/`sequencias` mostravam tudo em vez de cair na regra da base |
| `o_catalogo_esconde_a_tabela_negada` | `servidor.rs:17965` | `sistabelas`/`siscolunas` vazavam o nome da tabela negada |
| `leitor_com_administrar_no_curinga_nao_liga_o_profiler` | `servidor.rs:18467` | regra de `*` com `administrar` não podia ligar instrumentação por acidente |
| `os_campos_do_erro_saem_de_um_lugar_so` | `servidor.rs:16145` | três construtores de erro copiados — um quarto a mão caiu de calar um campo que os outros diziam |

**Transação e concorrência**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `a_transacao_enxerga_o_que_ela_mesma_escreveu` | `servidor.rs:21780` | isolamento próprio: a transação tinha de ver o que ela mesma gravou antes do commit |
| `escrita_comum_nao_anexa_enquanto_a_transacao_segura_o_fim` | `servidor.rs:22866` | escrita comum que anexa não olhava o fim travado por uma transação, arriscando colisão de rowid |
| `marca_que_nao_confere_e_commit_que_nunca_comecou` | `servidor.rs:22995` | COMMIT confirmando transação que nunca tinha começado de verdade |
| `so_um_lugar_toma_a_trava` | `servidor.rs:21505` | trava de dados tomada fora do ponto único — a mesma armadilha da guarda `trava-fora-do-ponto-unico` no catálogo de mutação |
| `duas_tabelas_na_mesma_janela_nao_travam_o_servidor` | `servidor.rs:21374` | duas tabelas na mesma janela de conflito travando o servidor inteiro |
| `a_cadeia_de_gatilhos_para_no_teto_e_avisa` | `servidor.rs:21436` | cadeia de gatilhos sem fundo abortava o binário em vez de parar e avisar |
| `sem_transacao_nada_muda` | `servidor.rs:21714` | comportamento velho (sem transação) tinha de continuar idêntico depois da integração |
| `sem_janela_a_marca_sai_no_commit` | `servidor.rs:22090` | sem janela de conflito configurada, a marca `.tx` tinha de sair só no commit |

**Exclusão e janela de conflito**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `sem_pedir_a_janela_cada_exclusao_espera_o_disco` | `phxsql-store/tests/exclusao.rs:238` | sem pedir a janela, cada exclusão pagava fsync — a guarda nova tinha de ser **pedida**, não imposta |
| `o_trash_fecha_antes_do_reg` | `exclusao.rs:276` | ordem de sincronização errada deixava o `.trash` atrás do `.reg` |
| `pedida_a_janela_o_fsync_sai_do_caminho` | `phxsql-store/tests/exclusao-na-janela.rs:110` | com a janela pedida, o fsync tinha de sair do caminho crítico de fato |
| `config_sem_o_campo_continua_esperando_o_disco` | `phxsql-server/tests/exclusao-na-janela-pelo-config.rs:46` | comportamento velho preservado quando o campo não está no config |
| `pedido_no_config_o_valor_chega_ao_motor` | `exclusao-na-janela-pelo-config.rs:71` | a mesma armadilha do `cache_paginas`: campo no config que ninguém lia |

**LGPD e trilha**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `insert_delete_e_soft_delete_nao_geram_trilha` | `phxsql-store/tests/trilha-lgpd.rs:164` | só UPDATE em coluna marcada gera trilha — inclusão e exclusão não, por decisão de custo |
| `tabela_sem_coluna_marcada_nao_paga_nada` | `trilha-lgpd.rs:210` | tabela sem nenhuma coluna LGPD não podia pagar overhead nenhum de trilha |
| `celula_vazia_nunca_vira_texto_vazio` | `phxsql-server/src/mensagens.rs:783` | célula de idioma vazia caindo para texto vazio em vez de cair para o português |

**Criptografia (cifra do fio, dos dados, dos diários)**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `o_nonce_nunca_se_repete_no_arquivo` | `phxsql-store/tests/cifra-dos-diarios.rs:304` | nonce repetido quebraria a garantia da cifra |
| `trocar_o_cabecalho_de_um_evento_cifrado_nao_passa` | `cifra-dos-diarios.rs:555` | cabeçalho de evento cifrado adulterado tinha de ser detectado |
| `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` | `phxsql-store/tests/cifra-dos-dados.rs:403` | trocar o corpo cifrado de uma linha por outra linha tinha de falhar a autenticação |
| `regravar_a_mesma_linha_nunca_repete_o_texto_cifrado` | `cifra-dos-dados.rs:459` | reuso de nonce/keystream ao regravar a mesma linha |
| `o_indice_sobre_a_coluna_marcada_continua_em_claro` | `cifra-dos-dados.rs:251` | decisão documentada: o índice não cifra, e o teste prende esse limite conhecido |
| `cliente_sem_cifra_continua_como_antes` | `phxsql-server/tests/cifra-do-fio.rs:258` | **histórico de vacuidade, já corrigido** — ver seção de guardas suspeitas abaixo |
| `fio_cortado_vira_erro_e_despedida_nao` | `cifra-do-fio.rs:454` | fio cortado no meio virando "despedida normal" em vez de erro |
| `a_privada_do_fio_nunca_sai` | `phxsql-server/src/config.rs:3331` | a chave privada do fio vazando pela resposta de configuração |
| `a_estatica_do_fio_nasce_no_arquivo_e_nao_muda` | `config.rs:3357` | chave estática do fio tinha de nascer fixa no arquivo, não gerada a cada subida |

**Senha e autenticação**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `a_ficha_nunca_devolve_a_senha` | `phxsql-server/src/usuarios.rs:1084` | a ficha do usuário vazando o hash da senha na resposta |
| `senha_em_texto_puro_funciona_mas_avisa` | `usuarios.rs:1069` | senha legada em texto puro tinha de continuar autenticando, mas com aviso |
| `a_senha_nunca_aparece` | `phxsql-server/src/profiler.rs:800` | Profiler expondo senha no registro de um pedido de login |

**Interface e a fábrica de idiomas**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `a_catraca_dos_textos_fora_da_fabrica` | `conferidor.rs:1188` | texto de tela cravado em português entrando sem passar pela fábrica |
| `nenhuma_chave_com_os_seis_idiomas_colados` | `conferidor.rs:1376` | tradução colada (copiar o português nos 6 idiomas) |
| `nenhuma_frase_longa_repetida_em_tres_idiomas` | `conferidor.rs:1407` | colagem parcial — 3 das 6 colunas com a mesma frase longa |
| `nenhum_texto_da_fabrica_traz_etiqueta_crua` | `phxsql-server/src/idiomas.rs:1676` | chave de tradução aparecendo crua na tela em vez do texto |
| `todo_texto_da_fabrica_e_pedido_por_alguem` | `idiomas.rs:1896` | chave morta na fábrica — traduzida nos 6 idiomas e nunca usada pela tela |
| `06-css-global.mjs` (caso de `testes-web/bateria.mjs`) | `testes-web/casos/06-css-global.mjs` | `input{width:100%}` global vira bolota o radio/checkbox em tabela; `label{text-transform:uppercase}` mostra dado em maiúscula (o caso «Blumenau» virando «BLUMENAU») |
| `09-cores.mjs` (idem) | `testes-web/casos/09-cores.mjs` | as 5 cores da ação existem, são distintas, o contraste mede ≥4,5:1 nos dois temas, e o preenchimento só aparece no hover |

**FFI (biblioteca embutida)**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `panico_nao_atravessa_a_fronteira` | `phxsql-ffi/src/testes.rs:255` | pânico em Rust atravessando a fronteira de C em vez de virar código de erro |
| `toda_funcao_exportada_e_blindada` | `testes.rs:315` | função exportada nova esquecida do catch-unwind |
| `ultimo_erro_e_por_thread` | `testes.rs:478` | **histórico de vacuidade, já corrigido** — vaga de erro global entre threads |
| `o_cabecalho_de_c_e_a_biblioteca_declaram_as_mesmas_funcoes` | `testes.rs:1076` | `.h` e o `.so`/`.dll` saindo dessincronizados |

**Replicação**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `sem_replicas_autorizadas_nada_muda` | `servidor.rs:16499` | comportamento velho preservado quando `replicas_autorizadas` não está configurado |
| `replica_de_fora_da_lista_nao_le_o_diario` | `servidor.rs:16516` | réplica não autorizada conseguindo ler o diário mesmo assim |
| `evento_nao_volta_para_quem_o_escreveu` | `servidor.rs:16632` | eco do próprio evento voltando para quem o escreveu na replicação bidirecional |

**REST, MCP e DbLink**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `toda_operacao_do_despachar_esta_na_especificacao` | `phxsql-server/src/rest.rs:823` | operação nova no despachar sem entrada correspondente no OpenAPI |
| `toda_rota_da_especificacao_existe_no_despachar` | `rest.rs:861` | OpenAPI prometendo rota que o servidor não atende |
| `corpo_com_outra_operacao_e_recusado` | `rest.rs:909` | corpo do pedido REST trocando a operação do caminho, em silêncio |
| `a_lista_pega_a_tabela_escondida_na_juncao_e_na_uniao` | `rest.rs:1009` | a mesma porta dos fundos de `juntar`/`unir`, agora na camada REST |
| `no_postgres_a_base_da_ligacao_nao_vira_esquema` | `phxsql-server/src/dblink/operacoes.rs:336` | `base` significa *database* no MySQL(R) e *esquema* no PostgreSQL(R) — tratados como o mesmo campo |
| `sem_base_padrao_o_mysql_nao_compara_com_o_database_nulo` | `phxsql-server/src/dblink/dialeto.rs:506` | comparação de base nula quebrando o dialeto MySQL |

**Configuração**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `config_sem_a_secao_rest_nao_escuta` | `phxsql-server/src/config.rs:3736` | webservice REST subindo sozinho quando a seção não existe no config |
| `campo_estranho_dentro_do_rest_avisa` | `config.rs:3881` | campo desconhecido dentro de uma seção válida passando sem aviso |
| `tipo_errado_e_recusado_antes_de_gravar` | `config.rs:4794` | tipo de coluna errado só sendo pego na gravação, não na declaração |
| `pino_torto_na_origem_e_erro_e_nao_ausencia` | `config.rs:3401` | endereço mal formado na origem de replicação sendo tratado como "ausente" em vez de erro |

**Profiler**

| guarda | arquivo:linha | o defeito que ela protege |
|---|---|---|
| `quebra_de_linha_no_pedido_nao_forja_linha_no_arquivo` | `phxsql-server/src/profiler.rs:1038` | campo livre do pedido com quebra de linha forjando uma linha inteira no `.txt` |
| `o_cabecalho_do_rodizio_nao_aceita_linha_forjada` | `profiler.rs:1227` | cabeçalho de rodízio do arquivo aceitando linha adulterada |
| `teto_zero_nao_rodizia` | `profiler.rs:1138` | `profiler.arquivo_mib: 0` deixando de significar "sem rodízio" |
| `toda_operacao_com_ponto_de_cancelamento_esta_na_lista` | `phxsql-server/src/telemetria.rs:1885` | operação cancelável nova esquecida da lista que a telemetria audita |

### As pétreas sem guarda — o achado principal desta rodada

Percorri as pétreas do `CLAUDE.md` uma a uma. A maioria tem guarda forte —
**zero dependências externas** dos exemplos do próprio pedido não entrou
nesta lista por acaso: é justamente uma das que **não** tem guarda, abaixo.
Os cinco achados reais:

1. **Zero dependências externas não tem guarda nenhuma.** Os oito
   `Cargo.toml` de `crates/*/` hoje só declaram `phxsql-*.workspace = true`
   entre si (conferido lendo os oito arquivos) — mas nada no repositório
   **impede** um `serde = "1.0"` de entrar num próximo commit. Não achei
   script, teste ou passo de CI que rode `cargo metadata`/`cargo tree` e
   reprove uma dependência de fora do workspace; `provar.py` e
   `empacotar.sh` só **citam** "zero dependência" em comentário, e
   `cargo build --offline` prova a garantia por acidente (falha se faltar
   crate no cache), não por regra. É a pétrea mais repetida do documento e
   a que depende inteiramente de revisão humana para não quebrar.

2. **O merge de conflito por coluna (`dialogoConflito`) não tem nenhum
   teste automatizado.** A função mora em
   `crates/phxsql-server/ui/index.html:8181` e implementa exatamente a
   regra "marca quem MEXEU, não quem perguntou por último" — compara coluna
   por coluna (`linhas = editaveis.map(...)`, `briga = linhas.filter(l =>
   !igual(l.outro, l.meu))`) para decidir o que precisa de escolha manual.
   Procurei "conflito"/"Conflito"/"dialogoConflito"/"mesclar" nos catorze
   casos de `testes-web/casos/*.mjs` e em `testes-web/*.mjs`: zero
   ocorrências. Não há teste de unidade possível aqui (é JS de tela, sem
   `cargo test` que o alcance) — a única prova real seria um décimo-quinto
   caso na bateria de frontend, e ele não existe.

3. **A metade "índice na filha" da regra de chave conferida não tem
   guarda**, embora a metade "índice na mãe" tenha
   (`sem_indice_na_mae_a_recusa_diz_qual_indice_falta`, achado acima). O
   código da recusa existe e está comentado —
   `crates/phxsql-store/src/table.rs:806-816`, a mensagem "crie o indice na
   filha ou desligue `verificar`" — mas os dez testes de
   `phxsql-store/tests/chave-estrangeira.rs` usam todos a mesma função
   auxiliar `filha()` (linha 38), que **sempre** cria o índice
   `porCliente`. Nenhum teste no arquivo constrói uma filha SEM esse
   índice para exercitar a recusa do outro lado. A regra pétrea diz
   textualmente "sem um deles o motor recusa dizendo qual falta" — hoje só
   metade dessa frase está provada.

4. **A própria armadilha que nomeou "configuração que não é lida mente" —
   `recursos.cache_paginas` — não tem o teste "chega ao motor" que ela
   inspirou em dois campos irmãos.** `exclusao_na_janela` e
   `corte_do_diario` ganharam, cada um, um teste
   `*_pelo_config.rs` que sobe um servidor com um `config.json` de verdade e
   confere o efeito no motor — e os dois comentam explicitamente "é a
   armadilha do `cache_paginas`" como motivação. Só que `cache_paginas` em
   si nunca ganhou o irmão: o valor é lido de verdade
   (`servidor.rs:664`, `definir_cache_paginas(config.recursos.cache_paginas)`)
   e exposto pela telemetria (`servidor.rs:12071`, campo
   `cache_ndx.paginas_teto`), mas procurei `paginas_teto`/`cache_ndx` em
   `crates/phxsql-server/tests/telemetria.rs` e não achei nenhuma
   ocorrência. O único teste que toca o campo
   (`config.rs:4771`, `campo_dentro_de_secao_muda_so_ele`) confere que o
   `Config` em memória guarda o valor — não que o valor chega ao
   `ndx::cache_paginas()` real. A ligação código→config existe; a prova
   ponta-a-ponta, não.

5. **"Instrumentação desligada tem de custar zero" é MEDIDA, não
   TRAVADA.** `bancada/profiler/custo.py` calcula exatamente essa
   pergunta (`tudo["desligado custa zero?"] = comparar(...)`,
   `bancada/profiler/custo.py:188`) — mas `custo.py` não aparece em
   `provar.py` (conferido com `grep profiler provar.py`: só `sonda.py` e
   `sonda-log.py` rodam na bateria única). Uma regressão de custo com o
   Profiler desligado não reprova nenhuma bateria automática — só aparece
   para quem lembrar de rodar essa bancada à parte. Não é ausência total
   como os quatro itens acima, mas é uma catraca sem trava: mede e não
   impede.

Não incluí aqui "interface só se prova exercitando" nem "rótulo se
estiliza, dado nunca" — as duas TÊM guarda real, `testes-web/casos/06-css-global.mjs`
e a bateria de 14 casos inteira (`testes-web/bateria.mjs`), como a tabela
acima mostra. Ficaram de fora da lista de achados por terem guarda, não por
esquecimento.

### As cinco, com guarda — a rodada que fechou este achado

As cinco confirmaram, medindo. Nenhuma morreu na medição desta vez — as
cinco levaram guarda nova, cada uma provada nos dois sentidos (falha com o
defeito reposto, passa com o código são; a saída de cada reprovação está
citada abaixo).

1. **Zero dependências externas.** `crates/phxsql-server/src/conferidor_dependencias.rs`,
   novo: compara os nomes do `Cargo.lock` contra os nomes que o
   `[workspace] members` do `Cargo.toml` raiz DECLARA — não contra o campo
   `source` do lock, que uma dependência de *caminho* de fora do workspace
   também não leva (os oito membros do próprio workspace já provam isso: são
   path deps entre si, e nenhum tem `source`). Dois testes: um contra o
   `Cargo.lock` de VERDADE
   (`conferidor_dependencias::testes::workspace_zero_dependencia_externa`,
   a guarda que protege o repositório hoje) e um contra um `Cargo.lock` de
   mentira embutido no teste
   (`conferidor_dependencias::testes::deteta_pacote_de_fora_do_workspace`,
   a guarda que entra no catálogo de mutação porque não depende de o
   binário conseguir compilar com o defeito).

   **Achado no caminho**: nenhuma das duas formas óbvias de mutação
   sobrevive. Acrescentar um pacote fantasma ao `Cargo.lock` (sem nenhuma
   dependência real apontando para ele) não sobrevive — o próprio
   `cargo test` reescreve o arquivo e PODA a entrada antes de qualquer
   teste rodar. E acrescentar uma dependência de verdade a um `Cargo.toml`
   quebra a resolução antes de compilar qualquer coisa, então nenhum teste
   chega a rodar para reprovar nada. Medido à parte, numa cópia isolada em
   `/tmp` (fora do `bancada/guardas/`, que não teria como provar isto):
   um `pacote-de-fora-do-workspace` de mentira, referenciado por *caminho*
   a partir de `phxsql-core` — que resolve **offline**, sem rede, sem
   precisar estar em cache — compila **sem erro nenhum**
   (`cargo build --offline` aceitou de bom grado), e só
   `workspace_zero_dependencia_externa` reprovou, dizendo qual pacote
   sobrou:

   ```
   test conferidor_dependencias::testes::workspace_zero_dependencia_externa ... FAILED
   thread '...' panicked: dependencia de fora do workspace no Cargo.lock:
   [("pacote-de-fora-do-workspace", "0.1.0")] -- zero dependencias externas
   e regra petrea do CLAUDE.md ("so a std e o proprio workspace")
   ```

   Essa é a prova de que a proteção de hoje era mesmo por acidente —
   `cargo build --offline` só recusa dependência de registro sem rede; uma
   dependência de caminho passa batida, e esta guarda é quem pega.

2. **O merge de conflito por coluna (`dialogoConflito`).**
   `testes-web/casos/19-conflito.mjs`, novo: duas ABAS do mesmo navegador
   editam a mesma linha — a aba B grava primeiro mudando só o UF, a aba A
   já tinha a ficha aberta com a versão de antes, edita só a CIDADE (coluna
   diferente) e tenta gravar por cima. O núcleo da prova é o pré-marcado
   dos rádios: `uf` tem de vir marcado "outro" (a aba A não tocou nele) e
   `cidade` marcado "meu" (foi a aba A quem digitou) — e depois de gravar o
   escolhido, as DUAS alterações sobrevivem. Reproduzi o defeito que a
   pétrea proíbe (`const mexi = true;` em vez de
   `!igual(l.meu, l.antes)`, marcando tudo "meu" — "quem perguntou por
   último") e a bateria reprovou exatamente onde devia:

   ```
   FALHOU conflito     1389 ms
       a coluna uf (que só a aba B mudou) não veio marcada com o valor do outro
   ```

   Não entra em `bancada/guardas/`: o executor de lá só sabe rodar
   `cargo test`, e isto é JavaScript de tela sem `cargo test` que o
   alcance — a prova real mora na própria bateria de frontend, como os
   outros guardas de UI desta casa (`06-css-global.mjs` e companhia).

3. **A metade "índice na filha".** Dois testes novos em
   `crates/phxsql-store/tests/chave-estrangeira.rs`
   (`sem_indice_na_filha_a_recusa_diz_qual_indice_falta` e o par do
   comportamento velho, `sem_conferir_a_mae_sai_mesmo_sem_indice_na_filha`),
   com uma segunda `filha_sem_indice()` que não cria o `porCliente`. Não
   precisa de linha nenhuma em `pedidos`: `indice_que_cobre` olha o
   ESQUEMA, não os dados — a recusa acontece antes de haver qualquer filha
   de verdade para procurar. Provado pelo catálogo de mutação
   (`sem-indice-na-filha-ignora-em-vez-de-recusar`, `table.rs`, trocando o
   `return Err(...)` por `continue` — ignorar em vez de recusar):

   ```
   sem-indice-na-filha-ignora-em-vez-de-recusar PROVADA   1.8 s   1/1 cairam
   ```

4. **`recursos.cache_paginas` chega ao motor.** Arquivo novo,
   `crates/phxsql-server/tests/cache-paginas-pelo-config.rs`, no molde dos
   dois irmãos — mas mais pesado que eles por necessidade: `cache_paginas`
   NÃO é aplicado dentro de `Recursos::aplicar` (o comentário do próprio
   `aplicar` diz isso — "o teto do cache de páginas continua sendo
   aplicado pelo servidor"), então `Config::ler` sozinho não bastava. Os
   três testes sobem um `Servidor::novo` de verdade e conferem
   `phxsql_store::ndx::cache_paginas()` — o mesmo global que a telemetria
   lê em `cache_ndx.paginas_teto`. Provado pelo catálogo
   (`cache-paginas-nao-chega-ao-motor`, comentando a chamada em
   `Servidor::novo`):

   ```
   cache-paginas-nao-chega-ao-motor PROVADA   6.0 s   2/2 cairam
   ```

5. **"Instrumentação desligada custa zero" — de MEDIDA a TRAVADA.**
   `bancada/profiler/custo.py` ganhou `falhou_desligado_custa_zero(medido,
   minimo=0.90)`: julga a mediana do par "atual/sem" contra um piso de
   90% (10% de folga para ruído de máquina compartilhada) e `main()` sai
   `1` quando reprova. `provar.py` ganhou a 25ª parte,
   `profiler-custo-zero`. A prova dos dois sentidos é o próprio
   `--autoteste` do script (não compila nada, roda em milissegundos —
   compilar as três variantes mexeria em `servidor.rs` três vezes, real
   demais para rodar de acompanhamento numa árvore compartilhada com outra
   frente):

   ```
   $ python3 bancada/profiler/custo.py --autoteste
   autoteste: ok -- par saudavel passa, par degradado reprova, borda do minimo (0.90x) confere
   ```

   Com o defeito reposto (`return []` em vez do julgamento):

   ```
   AssertionError: um par degradado (0,61x no lote) passou sem reprovar
   ```

   **Achado no caminho, fora do escopo dos cinco**: rodar o catálogo
   inteiro de `bancada/guardas/` para regravar a tabela do `docs/TESTES.md`
   §8 esbarrou num sexto buraco — `COPIAR`, em
   `bancada/guardas/provar-guardas.py`, nunca incluía `docs/`, e um teste
   de `error.rs` (`nenhuma_sprint_citada_e_inventada`) lê
   `docs/ROTEIRO-1.0.md` em tempo de execução. A árvore limpa reprovava
   antes de qualquer defeito ser reposto — não por contaminação entre
   rodadas concorrentes (cheguei a suspeitar disso primeiro, ver a
   cognição desta rodada), mas porque a lista nunca cobriu aquele arquivo.
   Consertado (`COPIAR` ganhou `"docs"`, 6,3 MB), e com o conserto o
   catálogo completo rodou até o fim nesta mesma rodada: **60 guardas, 56
   provadas, 4 redundantes, 0 não pegaram, 0 estragaram, 0 quebradas** —
   nenhuma regressão nas 57 antigas, e a tabela do `docs/TESTES.md` §8 já
   está regravada com este resultado.

### Guardas suspeitas de vacuidade

A pergunta certa aqui não é "este teste tem uma asserção fraca" — é "este
teste passaria com o defeito que ele diz proteger de volta?", e só mutação
responde isso com certeza. `bancada/guardas/` é a ferramenta que a casa já
tem para essa pergunta, e rodando o catálogo inteiro nesta rodada (a tabela
de `docs/TESTES.md` §8, fresca de 2026-09-01 21:52) o veredito é **zero**
`NAO PEGOU` e **zero** `ESTRAGOU` nas 57 entradas — nenhuma vacuidade nova
nas guardas que já estão catalogadas por mutação.

Isso cobre 57 guardas. As ~1.495 restantes não passaram por mutação nesta
rodada — reproduzir isso para todas exigiria um mutador genérico que este
levantamento não tinha orçamento para escrever (seria, ele mesmo, o próximo
item do papel J: pesquisar se existe um mutation tester para Rust que
funcione **sem dependência externa**, e medir o custo antes de adotar).
Fiz, em vez disso, uma leitura dirigida atrás do padrão que já quebrou duas
vezes nesta casa — teste que **desfaz o próprio defeito** montando o objeto
à mão antes de exercitar qualquer coisa — e o resultado é:

- **As duas ocorrências históricas já estão corrigidas e ficaram melhores
  que a média.** `cliente_sem_cifra_continua_como_antes`
  (`phxsql-server/tests/cifra-do-fio.rs:258`) hoje sobe um `Config`
  a partir de um `config.json` real **sem** a seção `cifra_fio`, em vez de
  montar o struct com o campo já desarmado — conferido lendo o teste
  inteiro. `ultimo_erro_e_por_thread` (`phxsql-ffi/src/testes.rs:478`) hoje
  usa duas `Barrier` para forçar a ordem A-escreve/B-escreve/A-lê, o que
  torna a corrida que escondia o defeito impossível de mascarar o teste de
  novo — também conferido lendo o teste.
- **Um vizinho de família (`config_de_ontem_continua_subindo_sem_cifra`,
  `phxsql-server/tests/cifra-pelo-config.rs:104`) foi ao mesmo arquivo
  histórico e escreveu certo desde o início**: sobe `Config::ler` de um
  `config.json` de verdade, sem seção `cifra`, e confere o `.log` gravado no
  disco byte a byte (`u16::from_le_bytes` na versão do cabeçalho) — não
  monta struct nenhum à mão. Não é um achado de defeito; é a confirmação de
  que a lição pegou na família de testes que mais fazia sentido repetir o
  erro.
- **Não achei uma terceira ocorrência do padrão específico "monta o objeto
  já desfazendo o próprio defeito"** nas ~20 guardas de config/comportamento
  velho que li por inteiro (`sem_*_nada_muda`, `*_continua_*`, listadas na
  varredura de nomes desta rodada). A maioria sobe o `Config` por
  `Config::ler`/`Config::de_json` a partir de texto, não por struct
  montado — o que é exatamente o padrão que fecha essa classe de defeito.
- **Busca ampla por asserção tautológica** (`assert!(true`,
  `assert_eq!(1, 1)`, comparação `>= 0` contra tipo sem sinal) não achou
  nada em `crates/`.
- **141 ocorrências de `assert!(...is_err());`** — a forma mais fraca de
  provar uma recusa, que confere que algo falhou sem conferir **por quê**.
  Li uma amostra e a maioria é validação de fronteira (caminho `".."`,
  arquivo inexistente, nome vazio) onde o "por quê" é óbvio pelo contexto —
  não vejo isso como vacuidade, mas como uma classe de teste mais fraca que
  a casa pratica em volume. Não listo as 141 individualmente porque isso
  seria ruído, não achado: sinalizo a classe, com o comando para quem quiser
  auditar por tabela (`grep -rn "assert!(.*\.is_err());" --include="*.rs"
  crates/ | grep -v target`), e deixo a decisão de endurecer alguma para
  quem tiver o contexto de cada uma.

**O que eu não posso afirmar**: que não existe uma terceira guarda vacía
nesta casa. Só afirmo que a busca dirigida ao padrão conhecido, mais a
mutação já catalogada em `bancada/guardas/`, não achou uma agora. Número
que não medi não vira "zero" por eu não ter achado.

### Como refazer este levantamento

```bash
# as catracas
cargo build --release --examples -p phxsql-store   # regra do binario velho
cargo run --release --example textos-fora-da-fabrica -p phxsql-server
cargo run --release --example grades-fora-do-padrao   -p phxsql-server

# o catalogo de mutacao (57 entradas hoje)
cd bancada/guardas && python3 -c "import catalogo; print(len(catalogo.GUARDAS))"
python3 provar-guardas.py --json /tmp/guardas.json && python3 tabela-no-testes.py /tmp/guardas.json

# a varredura de guardas nomeadas por comentario narrativo (109 hoje)
grep -rn "#\[test\]" --include="*.rs" crates/ | grep -v target | wc -l

# as pretreas sem guarda desta rodada, uma a uma
grep crates/*/Cargo.toml -A3 -n "\[dependencies\]"                     # (1) zero dependencia
grep -n "conflito" testes-web/casos/*.mjs testes-web/*.mjs             # (2) merge por coluna
grep -n "fn filha" crates/phxsql-store/tests/chave-estrangeira.rs      # (3) indice na filha
grep -n "paginas_teto\|cache_ndx" crates/phxsql-server/tests/telemetria.rs  # (4) cache_paginas
grep -n "profiler" provar.py                                           # (5) desligado custa zero
```

Nenhum destes seis comandos tem gerador — são consultas de auditoria, não
números que aparecem numa tela para o Adriano. Se este catálogo crescer a
ponto de precisar de painel próprio (o que a cláusula do papel H pediria no
dia em que ele virar número visível em algum lugar), aí sim caberia um
script em `docs/qa/` que produza a tabela de catracas e a lista de pétreas
sem guarda direto da árvore — hoje ele não existe porque cada número acima
tem comando de uma linha, e um gerador para uma linha seria indireção sem
ganho.

---

## O que este documento não é

Não é um roteiro de comandos — esses estão em `provar.py`,
`bancada/guardas/provar-guardas.py` e no cabeçalho de cada `docs/*.md`
citado acima. É o porquê de cada portão existir, com o incidente que o
motivou e o número que ele deixou. Quando um portão novo entrar, a regra é a
mesma da cláusula pétrea: ele entra **pedido**, com o defeito que o motiva
escrito ao lado — não imposto, e não sem número.
