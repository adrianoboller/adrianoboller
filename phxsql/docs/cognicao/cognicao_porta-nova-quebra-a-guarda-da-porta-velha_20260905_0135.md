# Porta nova quebra a guarda da porta velha — de dois jeitos, e um deles não compila

**Descoberto em 05/09/2026, ~01:35**, quando a varredura de mutação
(`bancada/guardas/provar-guardas.py`) julgou as 81 guardas do catálogo depois
de a trava de dados virar um `RwLock` com duas fichas.

## 1. O que aconteceu

`81 guardas: 75 provadas, 4 redundantes, 0 não pegaram, 0 estragaram,
**2 quebradas**` — e as duas quebradas eram do território que eu tinha acabado
de mexer:

| guarda | veredito | por quê |
|---|---|---|
| `trava-fora-do-ponto-unico` | QUEBRADA | *o código com o defeito reposto não compila:* `error[E0599]: no method named 'lock' found for struct 'std::sync::RwLock<T>'` |
| `trava-sem-guarda-de-reentrancia` | QUEBRADA | *o trecho aparece 2 vezes:* trocar a errada provaria outra coisa |

Nenhum teste caiu. Nenhum portão reprovou. `cargo fmt`, `clippy` e
`cargo test --workspace` passaram inteiros — **1.585 testes verdes** — com as
duas guardas mortas.

## 2. O que eu concluí primeiro, e estava errado

Que a lei da casa já cobria isto. Ela diz *«conserto entra no caminho que o
motivou, e o caminho IRMÃO fica»*, e eu a apliquei onde sempre se aplica: nos
**caminhos de código**. Procurei o irmão do `op_varrer`, o irmão do
`abrir_travada`, o irmão do `travar_dados` — e achei os três, e tratei os três.

O irmão que eu não procurei não era um caminho de código: era uma **entrada do
catálogo de mutação**. E ela quebrou por duas razões que não se parecem nada
uma com a outra:

* **por TIPO** — o defeito reposto chamava `self.dados.lock()`, e depois do
  `RwLock` esse método não existe mais. Uma guarda cujo defeito não compila
  não prova nada, e o pior é que ela some sem ruído: o `cargo test` continua
  verde porque a guarda **não roda no `cargo test`**;
* **por DUPLICAÇÃO** — a porta nova começa com a mesma pergunta de reentrância
  da porta velha, palavra por palavra. O `trecho` do catálogo, que precisa ser
  único no arquivo, passou a casar duas vezes. O executor recusou, e recusou
  **certo**: trocar a ocorrência errada provaria outra coisa.

A segunda é a mais traiçoeira, porque a duplicação foi **deliberada e certa** —
a `COM_A_TRAVA` é uma só para as duas fichas de propósito. O código está certo;
a guarda é que passou a apontar para dois lugares.

## 3. O que a medição disse

Não é medição de tempo, é a varredura de mutação, e o número dela é o
resultado:

* **antes do conserto:** 81 guardas, 2 quebradas — e as duas quebradas
  **invisíveis** para os três portões (`fmt`, `clippy`, `test`), que passaram
  com 1.585 testes verdes;
* **depois:** 82 guardas, **78 provadas, 4 redundantes, 0 quebradas**, 779 s de
  mutação.

A guarda de reentrância virou **duas**: `trava-sem-guarda-de-reentrancia`
(porta exclusiva, `trecho` agora carrega o comentário de cima para ficar único)
e `leitura-sem-guarda-de-reentrancia` (porta compartilhada, nova). A segunda
precisou de um teste que ainda não existia —
`as_duas_fichas_na_mesma_thread_viram_erro` —, porque o teste antigo pede a
**mesma** porta duas vezes e não cobre nenhum dos dois cruzamentos.

E o cruzamento importa mais que o caso antigo: num `RwLock`, pedir a leitura
com a leitura na mão **e um escritor na fila** trava três pontas em vez de uma.

## 4. A regra

> **Porta nova quebra a guarda da porta velha.** Quando uma operação ganha uma
> segunda entrada, rode a varredura de mutação — não o `cargo test`. As duas
> maneiras de quebrar são: o defeito reposto **deixar de compilar**, e o trecho
> do catálogo **passar a casar duas vezes**. Nenhuma das duas aparece nos
> portões.

E o corolário, que é o que dá para verificar sem lembrar da regra: **guarda que
o `cargo test` não roda é guarda que o `cargo test` não protege.** As catracas
que vivem dentro de um `#[test]` (`so_um_lugar_toma_a_trava` e a nova
`so_uma_operacao_usa_a_ficha_compartilhada`) sobreviveram à mudança porque elas
rodam junto da suíte; as do catálogo de mutação não rodam, e foram justamente
essas que quebraram.

## 5. Como está guardado hoje

* As duas entradas do catálogo foram consertadas **com o motivo escrito ao lado
  em comentário**, e não em silêncio: quem ler `catalogo.py` vê por que o
  `trecho` da reentrância carrega o comentário de cima, e por que o defeito
  reposto do ponto único passou a montar a ficha pelo `raiz.exclusiva()`.
* A guarda irmã nasceu junto (`leitura-sem-guarda-de-reentrancia`), com teste
  próprio, e as três foram provadas de novo pelo executor.
* A tabela de `docs/TESTES.md` §12 saiu do **`--json` de uma rodada de verdade**,
  como o gerador exige — 82 guardas, 779 s.

**Onde o buraco ficou:** nada roda a varredura de mutação automaticamente. Ela
não é item da `prova-bateria.py` (leva ~13 minutos e copia a árvore inteira), e
não há `cron` neste contêiner. Continua sendo *alguém lembrar de chamar* — e a
prova de que isso falha é esta própria rodada: as duas guardas ficaram
quebradas por horas, e só apareceram porque eu tinha um motivo independente
para rodar a varredura inteira. **Papel que não está cumprindo aparece como não
cumprindo.**
