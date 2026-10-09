# Hipótese morta: o custo da FASE B é do `rename` em si, qualquer que seja a referência (H2)

**Estado:** INFRUTÍFERO

**Causa:** O custo está na liberação do inode velho, que acontece onde cai a ÚLTIMA referência: com o descritor aberto ela sai do `rename` e vai para o `close`. O `rename` em si custa 0,0–0,1 ms.

**Prevenção:** Antes de atribuir o custo de uma troca de arquivo ao `rename` ou ao `fsync`, meça o `rename` e o `close` isolados, com e sem um descritor segurando o inode velho; só então escolha onde mexer.

## 1. O que aconteceu

Pedido 647. Para a FASE B a 10 M de linhas (1.164 ms de mediana, o `rename` sozinho com 1,202 s), escreveram-se duas hipóteses antes de medir. A H2 (rival da do papel C) dizia que o custo é do `rename` em si (journal, `auto_da_alloc` do ext4 sobre o arquivo novo), de modo que segurar o descritor do volume velho não mudaria nada. Origem: `cognicao_fase-b-e-o-rename-por-cima-nao-o-fsync_20261008_1738.md`, seção do conserto.

## 2. O que eu concluí primeiro, e estava errado

Que a lentidão fosse propriedade do `rename` como chamada de sistema, e que o conserto estivesse em outro lugar (o `fsync` ou o journal).

## 3. O que a medição disse

Experimento isolado (Python, ext4, 1,6 GB, 2 voltas): sem segurar o descritor, `rename` 797–802 ms e `close` 0; segurando, `rename` 0,0–0,1 ms e `close` 828 ms. O `fsync` do diretório ficou abaixo de 1 ms nos dois casos. No motor (`resultados.json`, chaves `grandes` e `grandes_antes_do_647`), a FASE B foi de 1.164 [1.051–1.498] ms para 2,6 [2,1–2,8] ms. H2 morreu.

## 4. A regra

Antes de atribuir tempo a uma chamada de sistema, meça quem libera o inode (último `close`/`unlink`) com e sem referência segurada.

## 5. Como está guardado hoje

O número está em `bancada/cifra-migracao/resultados.json` e o teste `crates/phxsql-store/tests/migracao-da-cifra.rs::a_fase_b_segura_o_volume_velho_ate_soltar` trava o conserto. Esta entrada existe para a hipótese morta alimentar o `AVOID.md`; o aprendizado vivo continua no arquivo do #4, ainda PENDENTE (sem guarda no catálogo).
