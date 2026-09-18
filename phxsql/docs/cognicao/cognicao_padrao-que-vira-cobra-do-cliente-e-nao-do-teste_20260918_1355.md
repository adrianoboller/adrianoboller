# Cognição: padrão que vira cobra do CLIENTE — e escrever o escape no teste é o conserto que esconde

**Assunto:** a virada de `cifra_fio.exigir` para `true` de fábrica (pedido 370),
e onde o preço dela realmente aparece.
**Descoberto em:** 18/09/2026, ~13h55.
**Frente:** pedido 370 — as portas HTTP recusam, o campo para de mentir, o
padrão vira.

---

## 1. O que aconteceu

A ordem do dono foi *«A comunicação deve obrigatoriamente ser cifrada»*, com o
formato decidido por ele: `exigir` nasce `true`, e `"exigir": false` é o escape
**escrito**.

Depois de fechar os dois bloqueios reais (as portas HTTP recusando num lugar só
e o `encryption_exigida` dizendo a verdade da conexão), virei o padrão — uma
linha no `CifraFio::default()`. A suíte acusou, e eu tratei cada bateria
vermelha do mesmo jeito: **escrevi o escape no preparo do teste**, com o motivo
ao lado («esta bateria mede outra coisa; sem o escape ela mediria o portão
errado»). Para 14 arquivos de `phxsql-server` isso está certo — nenhum deles é
sobre a cifra, e todos eram *servidores de teste*, não clientes de gente.

Aí caiu o `crates/phxsql-cmd/tests/console.rs`, e eu fiz **a mesma coisa**. E
essa era diferente: ali o vermelho não estava dizendo «esta bateria mede outra
coisa». Estava dizendo **«o console deste projeto não sabe mais falar com um
servidor deste projeto»**.

O integrador pegou, e a frase dele é a regra: *«mudar o teste do comportamento
velho para ele parar de acusar é o pior conserto possível»*.

## 2. O que eu concluí primeiro, e estava errado

**«O ODBC já aprendeu o aperto no pedido 373, então a parte de cliente está
coberta; o console é um item para outro pedido.»**

Três erros numa frase.

O primeiro: eu tratei «coberto» como «o cliente que os outros usam». O console é
o cliente que **nós** usamos, e é o primeiro que alguém abre para ver se o banco
está no ar. Um servidor de fábrica que o próprio `phxsqlcmd` não alcança não é
uma pendência, é o produto quebrado na primeira tela.

O segundo, e é o mais caro: **eu olhei o custo antes de olhar a peça.** Supus
que ensinar o console a cifrar fosse trabalho de pedido — handshake, estado de
canal, testes —, porque foi isso que o 373 custou no driver. Não era: o console
usa `phxsql_server::replica::Cliente`, o **mesmo** cliente da réplica e do
cluster, e ele **já tem** `Cliente::cifrar(pino)` em produção desde a
replicação cifrada. O conserto inteiro foi uma chamada, um irmão
(`ligar_em_claro`) e uma bandeira no `main`. O que eu chamei de «outro pedido»
eram 40 linhas.

O terceiro: eu ia reportar isso como **ressalva nomeada** e achar que estava
cumprindo a lei por nomear. Nomear um defeito não o conserta; nomear serve para
o que **não se pode** consertar aqui.

## 3. O que a medição disse

Medido nos dois sentidos, no mesmo binário e no mesmo dia (18/09/2026), com o
escape retirado do teste para a corrida:

| cenário | `cargo test -p phxsql-cmd --test console` |
|---|---|
| com o padrão virado (`exigir: true`), console **sem** o aperto | **8 de 9 falham**, todas com `[SP000025] acesso negado: cifrar: … peca o aperto de mao com {"op":"cifrar"}` |
| com o padrão de ontem (`exigir: false`), console sem o aperto | **9 de 9 passam** |
| com o padrão virado, console **com** o aperto | **11 passam** — os nove de sempre e os dois novos da cifra |

Ou seja: o vermelho era 100% meu, e não da bateria. E o inventário dos clientes
desta casa, medido por varredura em vez de suposto:

| cliente | falava o aperto antes | agora |
|---|---|---|
| driver ODBC | **sim**, por padrão (pedido 373) | igual |
| console `phxsqlcmd` | **não** — zero menções a `Iniciador`, `"cifrar"` ou `fio::` | **sim**, por padrão, com `--sem-cifra` como escape |
| réplica / cluster / DbLink | pelo interruptor próprio, desligado de fábrica | igual (o `exigir` é *inbound-only*) |
| `phxsqlcli` | não abre soquete para a porta de dados — mexe em arquivo | não precisa |

## 4. A regra

**Quando um padrão de segurança vira, o teste do comportamento velho que fica
vermelho está apontando para um CLIENTE — e o conserto é o cliente aprender, não
o teste escrever o escape. Escape no preparo de teste só é honesto quando a
bateria não mede o caminho que quebrou.**

E o critério que separa os dois casos, porque ele decidiu 15 arquivos aqui: o
escape é honesto quando o vermelho vem de um **servidor de teste** montado para
medir outra coisa; é conserto-que-esconde quando o vermelho vem de um **cliente
de verdade** que este repositório entrega.

O corolário sobre o alcance da pétrea «guarda nova entra pedida, não imposta»:
quando o dono **manda** impor, ela não vira letra morta — ela muda de alvo.
Continua proibindo quebrar o dado que já está lá e o cliente de terceiro sem
saída (e por isso o escape escrito é obrigatório); e passa a **cobrar** que os
clientes desta casa aprendam a guarda **antes** de o padrão virar, em vez de
ganharem uma ressalva no relatório.

## 5. Como está guardado hoje

**Guardado:**

- `crates/phxsql-cmd/src/lib.rs`: `Console::ligar` faz o aperto de mão por
  padrão, pelo **mesmo** `Cliente::cifrar` da réplica — uma segunda
  implementação da cifra ali seria um segundo jeito de errar. O escape escrito é
  `Console::ligar_em_claro`, e `--sem-cifra` no `main.rs`.
- A recusa do aperto **diz a saída** em vez de vazar o texto da réplica (que
  fala em «source», vocabulário que não é do console).
- Dois testes pelo soquete em `crates/phxsql-cmd/tests/console.rs`:
  `o_console_atravessa_um_servidor_que_exige_a_cifra` — que **lê
  `Config::default().cifra_fio.exigir` antes de afirmar o que prova**, porque
  provar por consequência deixaria os nove testes do arquivo voltarem a passar
  no dia em que alguém escrevesse o escape no `subir` — e
  `com_o_servidor_sem_cifra_o_console_recusa_e_diz_a_saida`, o outro sentido.
- `crates/phxsql-cmd/tests/console.rs` **não tem escape nenhum no `subir`**: ele
  voltou a ser, byte a byte, o teste do comportamento velho que acusou.
- A tabela dos quatro clientes está em `docs/SEGURANCA.md` §7.0, com a data.

**O buraco, e ele está nomeado em vez de escondido:**

- **O console não aceita o PINO** da chave do servidor. O túnel dele é TOFU:
  protege de escuta passiva e nada mais, exatamente como a tabela da §7 descreve
  — o driver ODBC aceita (`CHAVE_DO_FIO`, pedido 373) e o console não. Está
  escrito no `--help` dele e como `DIVIDA:` no `ligar`, em vez de subentendido.
- **O `phxsql-cli` não foi tocado** porque não abre soquete para a porta de
  dados; se um dia abrir, ele nasce com o mesmo problema.
