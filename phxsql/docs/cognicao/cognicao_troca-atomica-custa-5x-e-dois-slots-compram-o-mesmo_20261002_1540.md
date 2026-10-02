# A troca atômica custa 5× o `fdatasync` no lugar — e dois slots com CRC compram a mesma garantia

**Estado:** PENDENTE

- **Quando:** 2026-10-02, 15:40
- **Onde:** `crates/phxsql-store/src/sequencia.rs` (o `.seq` da sequência
  nomeada, pedido 229), `docs/FORMATO.md` §24
- **Custo:** uma medição de dois laços de 200 gravações, antes de qualquer
  byte de formato ir para o documento

## O que aconteceu

A sequência nomeada precisa durar cada número antes de devolvê-lo (numeração
de documento fiscal: repetir é pior que pular). A primeira versão usava o
motor durável que a casa já tinha para arquivo pequeno —
`sincronia::gravar_duravel`: temporário, `fsync` no descritor, `rename`,
`fsync` da pasta. É o caminho certo para o `config`, o `visoes.json` e a
marca do separador, e o teste de custo disse **788 µs por número**.

O `docs/AUTONUMBER.md` §B.2.4 tinha estimado **83,5 µs** de `fdatasync` por
número. Dez vezes menos. A diferença não era o disco: era o caminho.

## O que eu concluí primeiro, e estava errado

Que o `gravar_duravel` era «o motor único, logo o motor certo» — a lei
«função e comando não se duplicam» lida como «todo arquivo pequeno grava pelo
mesmo caminho». Quase escrevi o §24 do `FORMATO.md` com o arquivo de 128
bytes trocado por `rename`, e o número de 788 µs como preço aceito.

A lei protege a **decisão** de ser escrita duas vezes, não obriga todo
arquivo a pagar o mesmo `rename`. A decisão aqui é outra: «como sobreviver a
uma escrita rasgada». O `gravar_duravel` responde «deixando o arquivo
anterior inteiro»; um arquivo de dois slots responde «deixando o slot
anterior inteiro». São duas respostas para a mesma pergunta, e a segunda
custa um CRC em vez de dois `fsync` e um `rename`.

## O que a medição disse

Mesmo disco, mesma pasta de teste, 200 gravações de 128 bytes cada
(`sequencia.rs`, teste temporário, 02/10/2026):

| caminho | por gravação |
|---|---:|
| `escrever_em` no lugar + `sync_data` no descritor aberto | **147 µs** |
| `gravar_duravel` (temporário + `fsync` + `rename` + `fsync` da pasta) | **771 µs** |

5,2×. Depois de o arquivo virar dois slots de 128 bytes com `geracao` e
CRC-32 cada, o `custo_de_um_proximo` do módulo mede **169 a 230 µs** por
número (varia com a máquina ocupada; o teste imprime, não afirma). A escrita
rasgada é provada em `um_slot_rasgado_nao_perde_a_sequencia_e_nao_repete`:
o slot da última geração estragado, o anterior responde, e o número que
sairia dali é o que **ainda não tinha saído** — buraco nenhum, repetição
nenhuma.

## A regra

**Antes de pagar o `rename` durável num arquivo pequeno que se regrava a
cada operação, meça o `fdatasync` no lugar com dois slots — e pergunte qual
decisão a lei do motor único está protegendo, porque «mesmo caminho» não é
«mesma decisão».**

## Como está guardado hoje

- O formato (dois slots, `geracao`, CRC) está no `docs/FORMATO.md` §24, com
  os dois números.
- A guarda `sequencia-nomeada-proximo-sem-durar` (catálogo, provada 2/2) cai
  se o `proximo` deixar de gravar antes de devolver; a catraca
  `nenhum_caminho_de_escrita_novo_fora_do_volumes` (`volume.rs`) registra o
  `sequencia.rs` com 2 caminhos e o motivo.
- **Onde o buraco ficou:** o número do `custo_de_um_proximo` sai de um teste
  que imprime — não há bancada com `resultados.json` para a sequência
  nomeada, então a página dos testes não o mostra. Fica dito aqui, e não
  medido lá.
