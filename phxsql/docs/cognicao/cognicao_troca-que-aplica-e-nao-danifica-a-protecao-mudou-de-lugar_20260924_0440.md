# A troca que ainda aplica e já não danifica: a proteção mudou de lugar

**Descoberto em 24/09/2026, 04:40**, papel F, conferindo duas guardas que o
provador deu como NÃO PEGOU na versão entregue `3dc0b2a`.

## 1. O que aconteceu

Duas entradas do `bancada/guardas/catalogo.py` repunham o defeito e o teste
ficava verde:

| guarda | troca | por que não danificava mais |
|---|---|---|
| `debug-da-cifra-mostra-a-senha` | `.field("senha", &self.senha)` no `Debug` da `Cifra` | desde o pedido 372 `senha` é um `Segredo`, cujo `Debug` redige: a troca imprimia `senha: (oculto)` |
| `teto-do-fio-sem-a-constante-no-soquete` | `Canal::ler` com `u64::MAX - 1` no lugar da constante | desde os pedidos 434/442 o laço da porta de dados não chama `Canal::ler`: lê por `ler_decidindo`, e a constante chega pelo `teto_da_linha` |

Nas duas o **trecho continuava vivo** — a `trecho-vivo.py` dava zero, o
`repor` achava o texto uma vez só e aplicava a troca. O que envelheceu não foi
o trecho: foi a **relação entre o trecho e o dano**.

## 2. O que eu concluí primeiro, e estava errado

Para a do teto, a hipótese do pedido era «outro teto (o do `Canal`, 434/442)
passou a segurar o mesmo pedido antes». Parecia certo: o `TETO_DO_APERTO`
nasceu justamente para ler antes do teto grande. Morreu medida: a prova que
ficou verde **afirma** `teto 134217728` na resposta e no `acessos.log`, e com o
teto do aperto ela cairia pelo número (65536). Nenhum teto segurou antes — a
constante é que não passava mais por ali.

## 3. O que a medição disse

- `Debug` da `Cifra` com a troca antiga: **0** ocorrências da senha, texto
  `senha: (oculto)` — o rótulo do tipo, não o `(oculta)` do `impl` da `Cifra`.
- Troca reapontada para `self.senha.valor()`: **1** ocorrência, o teste cai
  contando. Raio: 0 dos 1.391 testes do `--lib`.
- `Debug` do próprio `Segredo` imprimindo o valor: **0 dos 1.391** do `--lib`
  e 0 dos 5 do `cifra-pelo-config` caíam — a camada que tornava a troca antiga
  inocente não tinha prova nenhuma. Ganhou teste e entrada
  (`debug-do-segredo-mostra-o-valor`).
- `teto_da_linha` com `u64::MAX - 1`: o servidor leu a linha inteira
  (135.266.305 bytes) e respondeu `ESQUEMA_INVALIDO` no lugar do
  `LIMITE_EXCEDIDO`; a prova cai em 0,33 s.
- A régua `debug-com-segredo.py` não vê nenhuma das duas trocas do `Debug`:
  `Segredo` não é tipo portador para ela, e o campo do tipo se chama `valor`.

## 4. A regra

**Trecho vivo não é defeito vivo: quando a proteção muda de lugar, a guarda
tem de ir atrás dela — e a camada nova precisa de prova própria, senão a
refatoração que a criou deixa uma proteção que ninguém prova.**

## 5. Como está guardado hoje

As quatro entradas saem PROVADAS no provador (`--so`), 24/09/2026. O que pega
isto no futuro continua sendo **só o provador rodado**: a `trecho-vivo.py`
confere que o texto existe, não que a troca ainda causa dano, e nenhuma régua
estática consegue — é a mesma pergunta que só o vermelho responde. Buraco que
fica: a `debug-com-segredo.py` não conta campo `Segredo` lido pelo `valor()`
num `impl Debug`; é dono o papel G.
