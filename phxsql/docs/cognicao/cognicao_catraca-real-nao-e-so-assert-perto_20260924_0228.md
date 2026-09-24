# Catraca real não é «assert perto» — é `.len()` e a constante no MESMO assert

**24/09/2026, 02:28 UTC.** Frente do pedido 384 (papel H).

## 1. O que aconteceu

`TETO_TXT_CRU_EM_HTML` (`crates/phxsql-server/src/conferidor_texto_cru.rs:142`)
é catraca de código — imposta por dois testes (`assert!` e `assert_eq!`) —, mas
não tem exemplo em `crates/*/examples/*.rs` que a exponha ao `--numeros` do
`docs/qa/medir.py`. Por isso ela caía na lista de «constantes `TETO*` que
NENHUM conferidor reporta — não são catracas: são limites, ou promessas», e o
inventário total saía uma catraca a menos do que existe.

A tarefa era «conserte o lado certo — de preferência o `medir.py`, sem mexer em
`.rs`». Rodar o Rust (compilar um exemplo novo) estava fora de cota nesta
rodada (disco curto, três frentes compilando), e reescrever em Python a MESMA
varredura que o `.rs` já faz (`${txt(` fora de `esc(`) duplicaria a decisão —
a lição do rodapé que publicou 780 KiB de uma lista copiada. A saída: um crivo
**estrutural**, que não reimplementa o que o conferidor mede, só pergunta *«um
teste do próprio arquivo compara uma contagem contra esta constante?»*.

## 2. O que eu concluí primeiro, e estava errado

A primeira versão do crivo perguntava: depois do primeiro `#[cfg(test)]` do
arquivo, existe uma **linha** que cite o nome da constante, dentro de uma
**janela** de ±4 linhas de outra linha que contenha a palavra `assert`?

Rodei contra as 25-26 constantes `pub const TETO*` do repositório inteiro para
conferir que não estourava nada — e estourou. `TETO_DO_REGISTRO`
(`crates/phxsql-core/src/fio.rs`), que o próprio `docs/CATRACAS.md` já
classifica como **limite de funcionamento** (um teto de bytes aceito em
produção, não uma contagem de código-fonte), foi marcado como «catraca
testada» pelo crivo. A causa: um `panic!("...teto de {TETO_DO_REGISTRO}",
l.len())` no mesmo bloco de teste tinha `.len()` **três linhas abaixo** da
menção à constante — mas o `.len()` era de outra variável, e o `panic!` não é
um `assert!`. Uma janela de linhas não distingue «citado perto de uma
asserção» de «citado dentro dela».

Se eu tivesse publicado esse crivo, o inventário passaria a contar
`TETO_DO_REGISTRO` como catraca real quando ele é limite — trocando uma
subcontagem por uma SUPERcontagem, o mesmo defeito de família com o sinal
invertido.

## 3. O que a medição disse

Extraindo o interior de cada `assert!`/`assert_eq!`/`assert_ne!` com
parênteses **balanceados** (não linha, não janela) e exigindo que o nome da
constante e `.len()` apareçam **dentro do mesmo corpo**:

* `TETO_TXT_CRU_EM_HTML` → `testada = True` (correto: `crus.len() <=
  TETO_TXT_CRU_EM_HTML` e `assert_eq!(crus.len(), TETO_TXT_CRU_EM_HTML, ...)`
  estão os dois dentro do parêntese do `assert`).
* `TETO_DO_REGISTRO` e `TETO_DO_APERTO` → `testada = False` (corrigido: nenhum
  dos `assert!`/`assert_eq!` do arquivo tem `.len()` no mesmo corpo — o único
  `.len()` próximo mora num `panic!`, que o crivo novo não varre).
* Conferido contra as 26 constantes órfãs de hoje e contra as 28 do
  `docs/CATRACAS.md` («Os limites de funcionamento encontrados»): **zero**
  falsos positivos com o corpo balanceado, **um** com a janela de linhas.
* Das 8 constantes que o crivo novo marca como «testada», **7 já têm exemplo
  funcionando** (`textos-fora-da-fabrica.rs`, `botoes-sem-prova.rs`,
  `inventarios-descasados.rs`, `segredos-soltos.rs`,
  `grades-fora-do-padrao.rs`, `temporarios-sem-guarda.rs`,
  `vermelhas-sem-pedido.rs`) — na tabela real elas já caem em `vistos` e saem
  da lista nova. Só `TETO_TXT_CRU_EM_HTML` fica, que é exatamente o achado do
  pedido 384.

## 4. A regra

**Um crivo estático que decide «isto é catraca» pela PROXIMIDADE textual a um
`assert` erra pelo mesmo motivo que um `grep` na prosa erra: proximidade não é
associação.** A associação real, quando existe, está dentro do MESMO literal
sintático (aqui, o corpo balanceado do `assert!`) — nunca numa janela de
linhas, que um comentário, um `panic!` ou uma mensagem de erro vizinha
atravessam de graça.

## 5. Como está guardado hoje

* `docs/qa/medir.py`: `_corpos_de_assert` (extração balanceada) e
  `_imposta_por_teste_no_proprio_arquivo` (o crivo), com o caso
  `TETO_DO_REGISTRO` documentado no docstring como o que fundou a exigência do
  corpo balanceado.
* `constantes_teto()` devolve `{"onde": ..., "testada": bool}` por constante;
  `tabela()` separa `sem_numeros` (catraca real sem `--numeros`) de `orfas`
  (limite/promessa de verdade), e o total impresso passa a somar as duas
  categorias medidas em vez de só `len(vistos)`.
* **Onde o buraco ainda fica:** `TETO_TXT_CRU_EM_HTML` continua sem exemplo
  Rust que a exponha — o `medir.py` agora sabe que ela é catraca, mas não sabe
  o **medido de hoje**, só o valor declarado (1). Fechar isso pede um `.rs`
  novo (um `--numeros` a mais em `textos-fora-da-fabrica.rs`, no molde das três
  irmãs que já moram lá), fora do escopo desta rodada por decisão explícita
  (não mexer em `.rs`).
