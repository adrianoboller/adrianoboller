# O driver ODBC aprende o aperto reusando o `Canal` do core

## 1. O que aconteceu

O driver ODBC (`crates/phxsql-odbc/src/conexao.rs`) era o último cliente da
porta de dados que ainda falava só em claro — a §10 do `docs/CIFRA-DO-FIO.md`
o nomeava como gap. Com `cifra_fio.exigir: true` no servidor, ele parava no
primeiro pedido.

Fechei o gap fazendo o `Canal` do driver reusar o `fio` do `phxsql-core`
inteiro: `Iniciador::comecar`/`terminar` para o aperto, e o `fio::Canal`
(`Claro`/`Cifrado`) para a camada de registro — exatamente o caminho da
`replica::Cliente`. A connection string ganhou `CIFRA=1` e `CHAVE_DO_FIO=<hex>`
(o pino). Nenhuma linha de cripto nova aqui.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro achei que o defeito reposto (driver em claro contra `exigir: true`)
voltaria com um SQLSTATE da classe de conexão (`08*`)** — escrevi a conferência
da bancada assim: `estado.startswith("08")`. Parecia óbvio: é uma recusa de
conectar.

E também achei, antes de olhar, que dava para **manter o buffer `sobra` a mão**
do driver e só embrulhar a escrita/leitura com a cifra — o mínimo de mudança.

## 3. O que a medição disse

**O SQLSTATE mediu `28000`, não `08*`.** A recusa do `exigir` chega ao driver
pela via do `login` — a primeira coisa que um cliente com `UID` manda —, e o
`Canal::abrir` remapeia toda falha de login para `28000`. O `08*` nunca ia
aparecer nesse caminho. O que carrega o *porquê* é a **mensagem**, não o
estado: `[28000] ... este servidor exige a cifra do fio`. Reescrevi a
conferência para medir o que é verdade e é load-bearing — que o diagnóstico
**nomeia a cifra exigida** — em vez de um estado que eu tinha adivinhado.

**E o `sobra` a mão não sobrevive à cifra:** a distinção das três saídas de um
fim de fio (fim limpo / cortado / truncado) mora no `fio::Canal::ler` do core.
Manter o `sobra` obrigaria a reescrever essa lógica aqui — uma segunda cópia da
parte que menos se pode divergir. Troquei o `sobra` por um `BufReader` sobre um
clone do soquete, que é o que o `fio::Canal` consome — o mesmo desenho da
`replica::Cliente`. Menos código no driver, e a camada de registro num lugar só.

## 4. A regra

Quando um cliente da porta de dados aprender a cifra, ele **reusa o `fio` do
core pela via do `Canal`, não pela via do `Transporte` cru** — a distinção
fim/corte/truncado vem de graça e não vira segunda implementação. E a prova do
defeito reposto **mede a mensagem, não o SQLSTATE que se imaginou**: o estado
depende de por qual porta a recusa entrou.

## 5. Como está guardado hoje

* O código: `crates/phxsql-odbc/src/conexao.rs` — `Receita.cifra` +
  `chave_do_fio`, `Canal::cifrar`, `pino_da_receita` (pino torto é erro, não
  ausência), e `pedir` pelo `fio::Canal`.
* A prova em Rust, sem gerenciador de driver:
  `conexao::testes::aperto_pelo_canal_fecha_e_fala_por_dentro` (servidor de
  aperto em processo, só o `fio` do core) e
  `pino_errado_derruba_o_aperto_sem_vazar_chave`. O defeito reposto (comentar o
  `canal.cifrar`) derruba a primeira — medido nesta rodada.
* A prova real de soquete: `bancada/odbc/prova-cifra.py` sobe um `phxsqld` com
  `exigir: true`, monta os dados por dentro do túnel e confere pelo `ctypes` e
  pelo `isql -k` de verdade (unixODBC 2.3.12): cifrada trabalha (`COUNT(*)`=3),
  clara é recusada.
* A doc: `docs/ODBC.md` §1.1 e §7; `docs/CIFRA-DO-FIO.md` §5 (agora "Vale") e
  §10 (o gap marcado FEITO), com o limite escrito — o login do driver não amarra
  ao canal, então `exigir_amarra: true` ainda o recusaria.

### Onde o buraco ficou

O driver atende `exigir`, mas **não** `exigir_amarra`: ele faz o login com a
senha em claro dentro do túnel, não o desafio-resposta, então não há prova para
amarrar à transcrição. Fechar isso é ensinar o driver o desafio-resposta — está
nomeado na §10 e no `docs/ODBC.md` §1.1.
