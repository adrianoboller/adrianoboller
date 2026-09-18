# Padrão que vira muda o aviso de lugar — e o aviso velho passa a acusar a decisão certa

**18/09/2026, 17h20.** Frente da virada da SAÍDA (`replicacao.origens[].cifra`,
`cluster.cifra`, `web.servidores[].cifra` nascendo `true`).

## 1. O que aconteceu

O pedido 370 virou a **entrada** (`cifra_fio.exigir` nasce `true`) e deixou um
aviso de arranque nomeando cada **saída em claro** configurada:

> «estas saídas estão em claro […] se o outro lado for um PhxSql desta versão,
> ele vai RECUSAR esta conexão»

Quando a **saída** também virou, esse aviso continuou compilando, continuou
passando no teste, e passou a dizer uma coisa errada de um jeito que nenhum
portão pega: com o padrão ligado, **saída em claro só existe escrita**
(`"cifra": false`). O aviso que nomeava o *esquecimento* passou a nomear a
*decisão* — e avisaria, todo arranque, exatamente a instalação que está em
transição por escrito.

## 2. O que eu concluí primeiro, e estava errado

**Duas coisas.**

A primeira: «viro os três padrões e o aviso continua valendo, porque saída em
claro continua sendo saída em claro». Errado — o aviso não é sobre o *estado*,
é sobre a *omissão*. Estado igual, omissão invertida: o que era esquecimento
virou escolha, e o que virou esquecimento (ninguém escreveu nada) não tinha
aviso nenhum.

A segunda: «`web.servidores` em texto solto (`"host:porta"`) fica em claro, por
retrocompatibilidade». Também errado, e pior: é a forma **mais escrita** das
duas. A virada teria alcançado só quem já usava o objeto — isto é, quem já
tinha decidido. O texto solto não é uma escolha pelo claro; é alguém
escrevendo um endereço.

## 3. O que a medição disse

* A virada dos três padrões derruba **5** testes numa árvore de **2.585**
  verdes — e **1 dos 5** estava fora do arquivo do assunto (`servidor.rs`).
* Os **três estados** do interruptor passaram a existir porque `booleano_ou`
  fundia *ausente* e *torto*: com o padrão desligado dava no mesmo, com ele
  ligado `"cifra": "sim"` seria rebaixamento silencioso — a mesma armadilha
  que o pedido 373 pagou no ODBC.
* Procurando o **irmão** (quem chama as mesmas funções na mesma ordem), a sonda
  `replicacao_testar` apareceu: monta uma `Origem` do pedido e cai no mesmo
  `replica::ligar`. E apareceu uma **quarta saída** que não é dos três e não
  tem interruptor nenhum: o **DbLink para outro PhxSql** (`dblink/phx.rs`),
  que abre a conexão e nunca chama `cifrar`.
* Prova pelo soquete, um master e uma réplica de verdade: com o padrão novo,
  **`"op":"cifrar"` 19×** no `acessos.log` do master e a linha atravessa; com o
  padrão velho reposto por escrito, `[SP000025] este servidor exige a cifra do
  fio: peça o aperto de mão` e **nada atravessa**.

## 4. A regra

**Quando um padrão vira, o aviso que nomeava a omissão tem de mudar de
omissão** — e a omissão nova é «ninguém escreveu decisão nenhuma», nunca «não
escreveu o valor que eu queria». Por isso as **duas** decisões escritas calam:
o que se cobra é o registro, não o valor.

E o corolário que a implementação obriga: **guardar a procedência do valor não
é enfeite** — sem distinguir *ausente* de *escrito*, não há como calar para
quem decidiu e falar para quem herdou.

## 5. Como está guardado hoje

* `CIFRA_DE_SAIDA_PADRAO` e `LeituraDasSaidas` em
  `crates/phxsql-server/src/config.rs`: a procedência sai da leitura do JSON e
  morre no aviso — o `Config` pronto não a carrega.
* Pares travados nos dois sentidos:
  `saida_no_padrao_de_fabrica_avisa_que_vai_pedir_o_aperto` ×
  `as_duas_decisoes_escritas_calam_o_aviso_da_saida`;
  `valor_torto_no_cifra_de_saida_nao_rebaixa_e_avisa` (os três estados);
  `a_sonda_de_replicacao_tem_o_mesmo_padrao_de_cifra_do_arquivo` (o irmão, e
  ele compara os **dois caminhos**, não o valor).
* **Onde o buraco ficou:** o **DbLink para outro PhxSql** não tem interruptor
  de cifra. Contra um PhxSql desta versão ele já não entra, e o único escape é
  `"exigir": false` do outro lado. Está escrito em `docs/SEGURANCA.md` §7.0 e
  **não** foi remendado: dar-lhe cifra é formato (`dblink.json`) e pino
  próprio, e meia cifra ali faria o painel dizer «cifrado» para uma ligação que
  não é.
