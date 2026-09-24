# A sobreposição que a leitura tolera, a conferência não tolera — e erra para os DOIS lados

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 448: a chave estrangeira da transação passou a se conferir na lista
inteira **antes** da marca (`Servidor::pre_conferir_a_lista`), lendo cada
tabela pela `Sobreposicao` com visibilidade de prefixo. O parecer do DBA
(`docs/propostas/parecer-dba-451-448-2026-09-24.md`, armadilha 1) nomeava dois
buracos da sobreposição — o `buscar` não via pela chave nova a linha do disco
que o prefixo alterou, e a conferência de «mãe viva» lia `reg.ler` por baixo da
marca pendente — e um custo a medir: o `buscar` dela era linear no conjunto de
escrita.

## 2. O que eu concluí primeiro, e estava errado

Que os dois buracos só faziam a pré-conferência **aprovar demais** — deixar
passar para a passada, depois da marca, o que ela devia recusar antes. É o que
o parecer descreve, e as provas foram escritas nesse sentido.

Medido com cada buraco reposto, sozinho, sobre o conserto inteiro: os dois
também fazem a pré-conferência **recusar transação válida**. Com o (a) reposto,
`[mãe nova, filha, alterar a chave da mãe com cascata, filha→chave nova]`
recusa no elo da cascata («nao existe clientes(id)»), porque a mãe alterada não
é achada pela chave nova. Com o (b) reposto, a mesma lista recusa na primeira
filha («rowid 2 fora da faixa 1..=1»), porque `reg.ler` de uma mãe que só
existe na sobreposição não acha slot. Uma sobreposição que mente sobre o
prefixo mente nos dois sentidos: o buraco que a leitura tolerava há semanas
virava, na conferência, regressão do comportamento velho.

E um segundo erro, este nas minhas provas: o retrato do disco que eu conferia
depois de um `COMMIT` bom esperava **zero marcas**, e falhava no HEAD por um
motivo que não era o defeito — a marca de um commit bom fica pendurada até a
janela de durabilidade fechar (o group commit). A prova passaria a falhar pelo
motivo errado e ninguém notaria; o `descarregar_sujas()` antes do retrato é o
que a faz medir o que diz medir.

## 3. O que a medição disse

`--example custo-da-pre-conferencia`, só o `COMMIT`, n/2 mães + n/2 filhas na
mesma lista:

| 10.000 escritas | intercalada | em blocos |
|---|---|---|
| antes do pedido | 72,6 ms | 73,2 ms |
| pré-conferência com o `buscar` linear | 6.550 ms (90×) | 8.380 ms (114×) |
| com o índice das chaves pendentes | 107,0 ms (1,47×) | 102,2 ms (1,40×) |

E a 100.000 (o teto padrão do conjunto de escrita), com o índice: 841,8 →
1.330,9 ms (1,58×) intercalada, 841,3 → 1.233,8 ms (1,47×) em blocos, faixas
que não se cruzam. A primeira medida da versão linear saiu com a suíte do
servidor rodando ao lado (6.807 e 8.570 ms); a da tabela é a refeita com a
máquina quieta.

O teto que o parecer pôs antes de embarcar era 2×. A busca linear, que na
leitura custava uma volta por consulta, na pré-conferência vira uma volta por
**escrita** — O(n²) numa lista que chega a 100.000.

## 4. A regra

Estrutura que a leitura tolera aproximada, confira-a nos dois sentidos antes de
pôr uma conferência em cima dela: repor o buraco tem de derrubar tanto a prova
do «recusa» quanto a do comportamento velho.

## 5. Como está guardado hoje

- `crates/phxsql-store/tests/sobreposicao-da-pre-conferencia.rs` — os dois
  buracos e o índice incremental, cada um com o vermelho descrito no teste.
- `crates/phxsql-server/src/servidor.rs::a_ordem_certa_continua_committed_e_inteira`
  — o comportamento velho que cai com qualquer um dos dois buracos reposto.
- O `ChavesPendentes` (`crates/phxsql-store/src/table.rs`) carrega o número
  que o trouxe no comentário.
- **Buraco que fica:** `varrer_indice`, `pagina_por_indice` e `intervalo` ainda
  põem a linha do disco alterada na posição da chave velha para quem **lê**. É
  imprecisão de ordem da leitura, escrita no próprio `varrer_indice`, e nenhuma
  conferência lê por eles hoje.
