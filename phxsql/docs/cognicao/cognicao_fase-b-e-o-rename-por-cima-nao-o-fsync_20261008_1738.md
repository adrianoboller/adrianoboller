# A FASE B da troca de volume é o `rename` por cima, e não o `fsync`

**Estado:** PENDENTE

## O que aconteceu

Pedido 647. A promessa de produto é «parada de escrita ≤ 1 s a 10 M de linhas»,
e o dono decidiu em 07/10/2026 que ela cobre só a trava global, onde roda a
FASE B (`aplicar_migracao_da_cifra`). Medido em 08/10/2026 pelo
`bancada/cifra-migracao/medir.py --grande` (chave `grandes` do
`resultados.json`), 3 voltas, máquina com outras frentes no ar (load 4,9 a 6,8):

| 10 M de linhas (~1,65 GB) | Criptografar | Descriptografar |
|---|---|---|
| 1 volume | **1.164 [1.051–1.498] ms** | 878 [728–939] ms |
| 10 volumes de 1 M | **1.339 [1.101–1.344] ms** | 913 [764–1.041] ms |

O MÁXIMO passa de 1.000 ms em três dos quatro casos (só o Descriptografar em 1 volume fica abaixo, 939 ms): a promessa **não vale**
hoje a 10 M.

## O que eu concluí primeiro, e estava errado

Que a reta da grade pequena (86,7 ms a 1 M, ×10 = ~0,87 s) chegaria abaixo de
1 s com folga pequena. Errou para baixo: o Criptografar a 10 M deu 1,16 s de
mediana — a reta subestima, e o parecer do C (0,88 a 1,0 s, «sem folga») era
o lado certo da dúvida.

## O que a medição disse

`strace -f -T` de uma corrida de 10 M em 1 volume: FASE B = 1.203 ms, e o
`rename("clientes.reg.novo", "clientes.reg")` sozinho custou **1,202 s**. O
`fsync` do diretório depois dele não chegou a 50 ms. A hipótese do C se
sustenta: o tempo é o núcleo soltando as extensões e o cache de páginas do
inode velho DENTRO do `rename`, não E/S de dado nem `fsync`.

## A regra

Antes de mexer no `fsync` de uma troca, meça o `rename`: por cima de um
arquivo grande ele é a operação cara, e cresce com o tamanho do que morre.

## O conserto, e a hipótese rival que morreu (08/10/2026, 18:50)

Duas hipóteses escritas antes de medir:

- **H1 (do C):** o custo é a liberação do inode velho, que acontece onde cai
  a ÚLTIMA referência; com um descritor aberto ela sai do `rename` e vai para
  o `close`.
- **H2 (rival):** o custo é do `rename` em si, qualquer que seja a
  referência (journal, `auto_da_alloc` do ext4 sobre o arquivo novo); segurar
  o descritor não mudaria nada.

Experimento isolado (Python, ext4, 1,6 GB, 2 voltas): sem segurar, `rename`
797–802 ms e `close` 0; segurando, `rename` 0,0–0,1 ms e `close` 828 ms. O
`fsync` do diretório ficou em < 1 ms nos dois. **H2 morreu; H1 se sustenta.**

No motor (`RegFile::volumes_velhos`, `soltar_volumes_velhos` chamado pelo
servidor depois de soltar a trava), `medir.py --grande`, 3 voltas, load ~5–7:
FASE B a 10 M passou de 1.164 [1.051–1.498] ms para **2,6 [2,1–2,8] ms**
(Criptografar, 1 volume); 1,1 [0,6–7,6] no Descriptografar; 24 [3–29] e
14 [2–40] em 10 volumes. A liberação foi para o `soltar`: 756–1.134 ms de
mediana, fora da trava. Prova nos dois sentidos:
`a_fase_b_segura_o_volume_velho_ate_soltar` (`tests/migracao-da-cifra.rs`)
reprova com o `segurar_o_velho` comentado (0 contra 3 descritores de
arquivo apagado no `/proc/self/fd`) e passa com ele.

O que eu concluí primeiro, e estava errado, também aqui: que bastava soltar no
`Drop` da tabela. No laço da migração v10 a mesma `Table` faz uma FASE B por
coluna, e só o `Drop` seguraria uma cópia morta da tabela inteira por passada
— daí o `soltar` explícito a cada passada e a soltura dos velhos anteriores
no começo de toda FASE B.

## Como está guardado hoje

Neste arquivo, no `resultados.json` (`grandes` e `grandes_antes_do_647`) e
no teste `a_fase_b_segura_o_volume_velho_ate_soltar`. Não há catraca que
acuse a FASE B acima de 1 s pelo tempo: o que trava a regressão é o teste do
descritor. Estado segue PENDENTE até alguém validar a evidência (pétrea de
24/09/2026).
