# Um sentinela que significa duas coisas mente para o segundo chamador

**Descoberto em** 18/09/2026, 13:40 — pedido 367, a trilha `.lgpd` de coluna
externa marcada.

## 1. O que aconteceu

`Table::decodificar(payload, carregar_externos)` tem um modo barato: com
`false`, as colunas `Bin` e `Memo` voltam `Value::Null` em vez de custarem uma
leitura do `.bin`/`.memo` (`table.rs`, hoje na função `externo`). O modo nasceu
para o **índice**, que não indexa coluna externa, e ali ele está certo: o valor
não seria usado de jeito nenhum.

Depois a trilha de LGPD passou a usar a mesma decodificação — e por um bom
motivo, escrito no comentário: «o `valores_antigos` já está decodificado aqui
em cima por causa dos índices, então o par antes/depois não custa leitura
nova». Só que `Value::Null` já significava outra coisa: **coluna vazia**.

Três mentiras saíram da mesma linha, e um teste as separou:

| ação | o que a trilha gravava |
|---|---|
| alterar um laudo | `antes=""` — afirmava que o campo estava vazio |
| **apagar** o laudo (`Memo` → `Null`) | **nada**: `Null != Null` é falso, o filtro cortava |
| salvar sem tocar na foto | `antes="" depois="2048 bytes"` — dizia que mudou |

A terceira é a pior de ler: o comentário logo acima do filtro **jurava** que
isso não acontecia («salvar a ficha sem mexer em nada geraria seis registros
dizendo que nada aconteceu»). A promessa era verdadeira para as seis colunas
`Str` e falsa para a sétima, que mora fora do `.reg`.

## 2. O que eu concluí primeiro, e estava errado

Que dava para decidir «mudou?» **sem I/O**, comparando o `Ponteiro` de 16 bytes
do payload velho com o do payload novo: ele carrega `tamanho` e `crc` do
conteúdo, os dois já estão na mão, e a comparação custa nada. Cheguei a
desenhar a função.

Estava errado, e a razão está no fonte: `Reg::selar_externo` sela o conteúdo
com um **nonce sorteado a cada gravação** quando a coluna é externa **e
marcada** (`reg.rs`, `externa_marcada`). Numa tabela cifrada, o mesmo laudo
regravado sem uma letra de diferença dá bytes diferentes e CRC diferente — e o
falso positivo voltaria exatamente para as tabelas que mais se protegem. O
atalho valeria só com a cifra desligada, e aí seriam dois comportamentos com o
mesmo nome.

O agravante do diagnóstico errado: ele **teria compilado, e os três testes
novos teriam passado**, porque os três rodam sem cifra.

## 3. O que a medição disse

`--example custo-da-trilha`, seção 4, 7 rodadas de 2.000 linhas (mediana, faixa
min–max), com o **controle** sendo a mesma tabela e o mesmo trabalho de escrita
com a marca do laudo desligada:

| laudo | cenário | antes | depois |
|---|---|---:|---:|
| 2 KiB | marcado, intocado | 17,94 (16,82–19,77) | 16,60 (13,99–18,45) |
| 2 KiB | marcado, alterado | 17,67 (16,33–20,92) | 22,66 (18,31–23,88) |
| 64 KiB | marcado, intocado | 88,48 (83,53–93,97) | 128,47 (112,21–133,20) |
| 64 KiB | marcado, alterado | 88,37 (76,02–102,06) | 139,15 (124,89–148,43) |

Ler o bloco velho custa **0,59 µs/KiB**. E o número que põe isso em escala: o
`atualizar` **já pagava 0,99 µs/KiB** só para regravar o bloco novo da mesma
coluna (controle de 13,85 µs a 2 KiB contra 75,08 µs a 64 KiB). A leitura que a
honestidade comprou custa **60% do que a alteração já gastava naquela coluna**.

E no caso comum — um `Memo` de 2 KiB que ninguém tocou — o conserto saiu
**mais barato** que o defeito: ele deixou de escrever 2.000 registros falsos.
As faixas se cruzam ali, então a leitura honesta é: *não custou nada
mensurável, e metade dos registros que sumiu era mentira*.

## 4. A regra

**Antes de reaproveitar um valor calculado por outro caminho, pergunte o que o
sentinela dele significa para QUEM O PEDIU.** `Null` de «coluna vazia» e `Null`
de «não carreguei» são o mesmo byte e fatos opostos; o segundo chamador herda o
sentinela e não herda o significado.

E o corolário, que vale para qualquer registro de auditoria: **quando não se
consegue afirmar, diga que não se consegue** — o `.lgpd` ganhou um bit
(`FLAG_ANTES_INDISPONIVEL`) em vez de um `""` que passaria por «campo em
branco».

## 5. Como está guardado hoje

- **Prova real, nos dois sentidos**, em `crates/phxsql-store/tests/trilha-lgpd.rs`:
  `a_trilha_de_memo_marcado_nao_mente_sobre_o_valor_antigo`,
  `apagar_memo_marcado_gera_registro_de_trilha` e
  `salvar_sem_tocar_no_bin_marcado_nao_gera_trilha` — os três falhavam antes, e
  cada um traz escrito no doc-comentário **qual defeito reposto o derruba**.
  Mais três que fecham o cerco: `trocar_a_foto_por_outra_do_mesmo_tamanho_gera_registro`
  (senão «não gera registro» passaria até desligando a trilha da coluna
  externa), `coluna_inline_marcada_continua_trilhando_como_antes` (o
  comportamento velho) e `memo_marcado_ilegivel_sai_como_indisponivel_e_nao_como_vazio`.
- **A medição** é a seção 4 do `--example custo-da-trilha`, que imprime o
  número da rodada que o imprimiu — e conta os registros, que é o que mostra o
  defeito antes de o relógio mostrar o preço.
- **A recusa medida** do atalho pelo ponteiro está em `docs/LGPD.md` §5, para
  que a mesma ideia não volte sem o número.
- **Onde o buraco ficou:** `crates/phxsql-server/src/servidor.rs:18713-18714`
  publica `antes_redigido` e `depois_redigido` no JSON da op `trilha` e ainda
  **não publica** `antes_indisponivel`. A tela mostra a frase, que se lê, mas
  quem consome o JSON não tem o bit — e a lei desta casa é que texto se resolve
  por chave, nunca por comparação de frase. O arquivo estava com outra frente
  viva no dia, então a linha não entrou aqui.
