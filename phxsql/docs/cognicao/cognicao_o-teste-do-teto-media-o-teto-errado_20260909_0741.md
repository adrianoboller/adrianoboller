# O teste do teto media o teto errado

## 1. O que aconteceu

Achado A4 (`p08_juntar_memoria.py`): a junção `interno` de 1000 × 1000 com a
mesma chave materializava um milhão de linhas — **+561,6 MiB de pico, 1128
ms** no `VmHWM` do servidor — para então recusar contra `max_linhas` 1000. Só
o `cruzado` conferia antes (produto dos tamanhos). O conserto: o teto entra
em `consultar::juntar`, que para na linha `teto + 1` e devolve `None`.

## 2. O que eu concluí primeiro, e estava errado

**No desenho:** que o certo era estimar Σ |grupo_esq| × |grupo_dir| pelo mapa
de espalhamento antes de materializar, como o `cruzado` faz com o produto.
Daria o mesmo limite de memória com uma segunda passada e mais código — e as
órfãs do `direito`/`completo` exigiriam contar o segundo laço também. Parar
na linha `teto + 1` dentro do laço que já existe custa o que já custava e
cobre os quatro tipos com uma regra.

**No teste:** escrevi a prova de servidor com `servidor_com_teto(…, 3)` e
esperei que `esquerdo` (4 linhas) recusasse. Ela **passou com 3 linhas**. O
teto do servidor vale também para o `varrer` dos sub-pedidos — a esquerda
chegou cortada em 3, e a junção nunca viu a quarta. O teste media o teto do
`varrer`, não o da junção, e um teste que passa por engano é pior que teste
que falta. A prova certa é a auto-junção de `pedidos` por `cliente_id`: 4
linhas de cada lado viram 6, com o teto acima das entradas.

## 3. O que a medição disse

O mesmo pedido, depois: **+0,3 MiB e 8 ms** (`final/saida/p08…`); o SQL
`JOIN` 1000 × 1000, +0,0 MiB em 7 ms. Unidade: no teto exato os quatro tipos
devolvem tudo; uma linha acima, `None` — inclusive pelas órfãs do segundo
laço, que têm prova própria.

## 4. A regra

**Um teto se prova com o pedido que só o teto pode cortar** — se outro limite
alcança a entrada antes, o teste mede o outro limite.

## 5. Como está guardado hoje

`consultar::juntar(…, teto) -> Option<Vec<Linha>>`, o `servidor.rs` nomeia a
linha em que parou; guarda `juncao-materializa-antes-do-teto`, provada nos
dois lados (unidade e servidor); `docs/SQL.md` §8 com os dois números.
