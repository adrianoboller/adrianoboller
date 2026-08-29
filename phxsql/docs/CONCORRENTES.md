# O caminho de inserção do InnoDB e do Aria, lido no fonte

Documento de **leitura de código com medição**, não de opinião. Toda afirmação
sobre o código dos outros traz `arquivo:linha`. Toda afirmação sobre custo diz
se é **medida** ou **inferida** — e, quando é inferida, a frase diz isso.

A pergunta é uma só: **o que o caminho de inserção do MySQL(R)/InnoDB e do
MariaDB(R)/Aria faz que o nosso não faz, e por que eles inserem ~2,3× mais
rápido.** A bancada de 10 milhões diz PhxSql 265,2 s contra MySQL(R) 113,5 s.

> **A resposta curta.** Não é WAL, não é *group commit*, não é *change buffer*,
> não é LSM. É mais simples e mais barato do que qualquer um deles: **eles não
> escrevem página nenhuma no arquivo durante a inserção, e não calculam
> checksum de página nenhuma durante a inserção.** As duas coisas acontecem
> depois, uma vez por página, amortizadas por todas as linhas que caíram nela.
> Nós fazemos as duas por linha: **2,06 páginas de 4 KiB gravadas e seladas com
> CRC, mais uma página de cabeçalho inteira por índice.**
>
> Refazendo as três peças em cima do nosso próprio motor, medido:
> **16,2 → 7,2 µs por linha, 2,25×.**

---

## 1. O que foi lido

| | Versão | Commit | Onde |
|---|---|---|---|
| MySQL(R) / InnoDB | 8.0.46 | `9fc8d60` (2026-06-04) | `storage/innobase/`, `sql/` |
| MariaDB(R) / Aria e InnoDB | 11.4.13 | `3941f6c` (2026-08-26) | `storage/maria/`, `storage/innobase/` |
| PhxSql | 0.17.0 | este repositório | `crates/phxsql-store/` |

Os caminhos citados abaixo são relativos a `storage/innobase/` (MySQL(R)) e a
`storage/maria/` (MariaDB(R)). Os nossos são relativos à raiz do repositório.

Lidos por inteiro ou quase: `row/row0ins.cc`, `btr/btr0cur.cc`, `btr/btr0btr.cc`
(divisão de página), `page/page0cur.cc` (o registro de redo de uma inserção),
`mtr/mtr0mtr.cc`, `buf/buf0flu.cc`, `buf/checksum.cc`, `ut/crc32.cc`,
`ibuf/ibuf0ibuf.cc`, `include/ibuf0ibuf.ic`; e do lado do Aria `ma_write.c`,
`ma_page.c`, `ma_pagecache.c`/`.h`, `ma_pagecrc.c`, `ma_locking.c`,
`ma_open.c`, `ha_maria.cc`, `maria_def.h`.

Do nosso lado, e nesta ordem: `docs/DESEMPENHO.md` inteiro (é ele que já
derrubou quatro diagnósticos plausíveis, e nenhum deles se repete aqui),
`docs/FORMATO.md`, `crates/phxsql-store/src/table.rs`, `ndx.rs`, `reg.rs`,
`log.rs`, `crates/phxsql-core/src/crc.rs`.

### As medições deste documento

Todas na mesma máquina (Xeon 2,8 GHz, 4 núcleos, `sse4.2` e `pclmulqdq`
disponíveis), com o `--example onde-doi` do próprio repositório, 200.000 e
1.000.000 de linhas, três corridas por variante, a primeira depois de compilar
descartada. As variantes foram construídas numa **cópia** do repositório, em
`/tmp`, para não mexer no código — este documento não altera nenhum arquivo do
motor.

---

## 2. O caminho de uma inserção no InnoDB, passo a passo

### 2.1 Do `handler` até a folha

```
ha_innobase::write_row
  → row_insert_for_mysql
    → row_ins_step                          row/row0ins.cc:3648
      → row_ins                             row/row0ins.cc:3580
        → row_ins_index_entry_step          row/row0ins.cc:3474, chamado em :3610
          → row_ins_clust_index_entry       row/row0ins.cc:3116   (índice agrupado)
          → row_ins_sec_index_entry         row/row0ins.cc:3200   (cada secundário)
```

`row_ins` (`row0ins.cc:3610`) percorre os índices **um a um**, o agrupado
primeiro. É a mesma forma do nosso laço em `table.rs:784`.

### 2.2 Otimista primeiro, pessimista só se falhar

`row_ins_clust_index_entry` (`row0ins.cc:3116`) tenta **duas vezes**:

```c
/* Try first optimistic descent to the B-tree */
err = row_ins_clust_index_entry_low(flags, BTR_MODIFY_LEAF, ...);   // :3163
...
/* Try then pessimistic descent to the B-tree */
err = row_ins_clust_index_entry_low(flags, BTR_MODIFY_TREE, ...);   // :3188
```

`BTR_MODIFY_LEAF` trava **só a folha**: é o caso em que a chave cabe e nada mais
muda. `BTR_MODIFY_TREE` trava a árvore, e só acontece quando a folha encheu.

Isso importa para nós por um motivo de leitura, não de trava: **o caminho comum
toca uma página de escrita, e só uma.** O nosso `inserir_rec`
(`ndx.rs`, a partir de :740) já faz o mesmo — no caso sem divisão, um único
`gravar_pagina` na folha. Não é aqui que perdemos.

### 2.3 Onde a página é modificada, e o que vai para o disco

Dentro de `row_ins_clust_index_entry_low` (`row0ins.cc:2396`):

- `mtr.start()` — `row0ins.cc:2436`
- `btr_cur_optimistic_insert` — `row0ins.cc:2574` (a chamada), definido em
  `btr/btr0cur.cc:2662`
- `mtr.commit()` — `row0ins.cc:2617`

`btr_cur_optimistic_insert` confere o espaço livre (`btr0cur.cc:2769-2779`),
aplica a heurística de divisão para inserção sequencial (`btr0cur.cc:2785-2791`,
ver §5.4) e insere o registro na página **que está no buffer pool**
(`btr0cur.cc:2805-2812`). Nenhuma escrita em arquivo acontece aqui.

`mtr_t::Command::execute` (`mtr/mtr0mtr.cc:842`) é o commit da
mini-transação, e faz exatamente duas coisas:

```c
auto handle = log_buffer_reserve(*log_sys, len);      // mtr0mtr.cc:855
m_impl->m_log.for_each_block(write_log);              // mtr0mtr.cc:859
...
add_dirty_blocks_to_flush_list(handle.start_lsn, handle.end_lsn);  // mtr0mtr.cc:867
```

`add_dirty_blocks_to_flush_list` (`mtr0mtr.cc:830`) chama
`buf_flush_note_modification` para cada página modificada (`mtr0mtr.cc:338`):
**marca suja e põe na lista de descarga.** A página não vai ao arquivo.

**Contagem, no caminho comum de um índice:** páginas *tocadas* 3–4 (a descida),
páginas *modificadas* 1, páginas *escritas no arquivo* **0**.

### 2.4 O que é gravado, então?

Só o redo, e o redo de uma inserção é um **delta**, não a página.
`page_cur_insert_rec_write_log` (`page/page0cur.cc:854`) documenta o próprio
orçamento (`page0cur.cc:967-973`):

```
 11 -> REDO_LOG_INITIAL_INFO_SIZE
 2  -> cursor rec offset
 5  -> record end segment length
 1  -> info bits
 5  -> record origin offset
 5  -> mismatch index
```

E o corpo é só o pedaço do registro que **difere do registro sob o cursor**
(`page0cur.cc:1040-1049`): ele compara o novo registro com o vizinho e grava
apenas o sufixo diferente. Numa carga de linhas parecidas, isso são **dezenas
de bytes**.

Comparação com o que nós gravamos por linha (derivado de medida — as 2,06
páginas são medidas, o resto é aritmética do formato):

| | por linha inserida |
|---|---:|
| páginas do `.ndx` gravadas (medido, 2 índices) | 2,06 × 4096 = **8,4 KiB** |
| cabeçalho do `.ndx`, página 0 inteira, **por índice** | 2 × 4096 = **8,2 KiB** |
| slot do `.reg` + cabeçalho de 128 B + evento do `.log` | ≈ 0,3 KiB |
| **total** | **≈ 16,8 KiB** |

O InnoDB grava, na inserção, **o redo e nada mais**.

### 2.5 O checksum: uma vez por descarga, não por inserção

Esta é a diferença mais importante do documento.

O checksum de página do InnoDB é calculado em `buf_flush_init_for_writing`
(`buf/buf0flu.cc:993`), e o único caminho que o chama no regime normal é
`buf_flush_write_block_low` (`buf/buf0flu.cc:1169`, a chamada em
`buf0flu.cc:1243`) — a função que **escreve a página no tablespace**. O
algoritmo sai de `srv_checksum_algorithm` (`buf0flu.cc:1126-1131`):

```c
case SRV_CHECKSUM_ALGORITHM_CRC32:
case SRV_CHECKSUM_ALGORITHM_STRICT_CRC32:
  checksum = buf_calc_page_crc32(page);
```

`buf_calc_page_crc32` (`buf/checksum.cc:71`) é **CRC-32C**, e o `ut_crc32` que
ele usa é escolhido em tempo de inicialização (`ut/crc32.cc:794-806`):

```c
ut_crc32_cpu_enabled = hardware::can_use_crc32();
ut_poly_mul_cpu_enabled = hardware::can_use_poly_mul();
if (ut_crc32_cpu_enabled) {
  if (ut_poly_mul_cpu_enabled) ut_crc32 = hardware::crc32_using_pclmul;
  else                         ut_crc32 = hardware::crc32_using_unrolled_loop_poly_mul;
  return;
}
ut_crc32 = software::crc32;
```

`_mm_crc32_u64` em `ut/crc32.cc:440`, `pclmul` em `ut/crc32.cc:481`.

**Duas diferenças empilhadas, e a segunda é maior que a primeira:**

1. Eles usam CRC por **instrução de máquina**; nós usamos tabela em software.
2. Eles pagam o CRC **uma vez por página descarregada**; nós pagamos **por
   gravação**, e gravamos 2,06 páginas por linha.

Medido aqui (bancada isolada, página de 4096 bytes, 200.000 repetições):

| | µs por página | contra o nosso |
|---|---:|---:|
| o nosso `crc32` de hoje (slice-by-8, IEEE) | **2,35** | — |
| slice-by-16, **mesmo polinômio, mesmos valores** | **1,81** | 1,30× |
| CRC-32C por hardware (`_mm_crc32_u64`) | **0,41** | 5,7× |
| CRC-32C por hardware, 3 fluxos entrelaçados | **0,25** | 9,4× |

Os 2,35 µs batem com os 2,34 µs que o `onde-doi` mede há três versões — o
aparelho está calibrado.

### 2.6 Quando a página finalmente vai ao disco

O *page cleaner* decide, e a decisão **não olha o tamanho da tabela**:
`get_pct_for_dirty` (`buf/buf0flu.cc:2475`) olha a fração de páginas sujas
contra `srv_max_buf_pool_modified_pct`, e `get_pct_for_lsn`
(`buf/buf0flu.cc:2503`) olha a idade do redo. A vazão de descarga é uma fração
de `srv_io_capacity`, que é uma configuração — não uma função de N.

É por isso que a taxa deles é **plana** e a nossa cai (§7).

Depois disso a página passa pelo *doublewrite* (`dblwr::write`,
`buf0flu.cc:1252`) — que é um **custo** deles, não um ganho: cada página vai ao
disco duas vezes. Existe porque uma página de 16 KiB pode rasgar no meio numa
queda de energia e não há CRC por página que salve o conteúdo, só que o
detecte.

### 2.7 A caixa de mudanças (`ibuf`), e por que ela não é o nosso caso

`ibuf_insert` (`ibuf/ibuf0ibuf.cc:3283`) adia a manutenção de índice
secundário. Duas condições travam quando ele entra.

**A primeira**, em `ibuf_should_try` (`include/ibuf0ibuf.ic:116`):

```c
!index->is_clustered() && ... && (ignore_sec_unique || !dict_index_is_unique(index))
```

Índice agrupado, não. Índice **único**, não.

**A segunda**, e é a que decide tudo para nós, em `ibuf_insert`
(`ibuf0ibuf.cc:3369-3375`):

```c
buf_page_t *bpage = buf_page_get_also_watch(buf_pool, page_id);
if (bpage != nullptr) {
  /* ... the page has been read into the buffer pool.
     Do not buffer the request. */
  return false;
}
```

E, no chamador, `btr0cur.cc:944-952` só arma o caminho do `ibuf` com
`Page_fetch::IF_IN_POOL` — ou seja, ele só é tentado quando a folha
**não está na memória**.

> **A caixa de mudanças troca uma leitura aleatória de disco por uma escrita
> sequencial adiada. Ela não economiza CPU nenhuma.** O nosso gargalo, medido e
> registrado no `DESEMPENHO.md` §1, é 95% de CPU com **0,0 MiB lidos**. Não há
> leitura aleatória a economizar. Isto **confirma** o veredito do §4.4 do
> `DESEMPENHO.md` por um caminho independente: o análogo certo do que medimos
> como prejuízo é este, e ele mira um problema que não temos.

A fusão acontece em `ibuf_merge_or_delete_for_page`
(`ibuf/ibuf0ibuf.cc:3966`), quando a página finalmente é lida.

### 2.8 O índice adaptativo por hash

`btr0cur.cc:776-787`: quando o padrão de busca se repete, o InnoDB pula a
descida inteira e vai direto ao registro na folha, por uma tabela hash em
memória. Só vale para `latch_mode <= BTR_MODIFY_LEAF` — isto é, para o caminho
otimista, que é o da inserção comum.

Vale registrar que existe, mas não é candidato: é uma estrutura auxiliar em RAM
com invalidação por página, e o nosso cache de páginas já serve 8,80 páginas por
linha sem ir ao arquivo (medido).

---

## 3. O caminho do Aria, e por que ele é o parente próximo

O Aria é o motor de **arquivos separados** do MariaDB(R): `.MAD` para os dados,
`.MAI` para os índices. É a mesma família do HFSQL e do PhxSql. Ler o `ma_write.c`
é ler uma versão do nosso `table.rs` escrita em C.

### 3.1 `maria_write`, lado a lado com o nosso `Table::inserir`

| passo | Aria | PhxSql |
|---|---|---|
| marcar o arquivo como «em uso» | `ma_write.c:123` | — (não existe) |
| conferir as chaves únicas **antes** de gravar | `ma_write.c:126-144` | `table.rs:763-770` |
| gravar a linha, obter a posição | `ma_write.c:157` (`write_record_init`) | `table.rs:777-782` |
| inserir cada chave no B-tree | `ma_write.c:162-190` | `table.rs:784-797` |
| atualizar o estado do arquivo | `ma_write.c:306` (`_ma_writeinfo`) | `ndx.rs:723` + `reg.rs:850` |

A ordem é a mesma, o desenho é o mesmo, e a diferença está nas duas últimas
linhas da tabela.

### 3.2 O `.MAI` tem página suja, e o `.ndx` não

`_ma_write_keypage` (`ma_page.c:225`) grava a página de índice assim
(`ma_page.c:250-256`):

```c
res= pagecache_write(share->pagecache, &share->kfile, ...,
                     lock, pin, PAGECACHE_WRITE_DELAY, link, LSN_IMPOSSIBLE);
```

`PAGECACHE_WRITE_DELAY` está definido em `ma_pagecache.h:72` com o comentário
que basta: *«do not write immediately, i.e. it will be dirty page»*. A página
entra na lista de sujas em `link_to_changed_list` (`ma_pagecache.c:1290`),
marcada com `PCBLOCK_CHANGED` (`ma_pagecache.c:177`).

O `.MAD` faz igual: `ma_blockrec.c:2069`, `:2184`, `:3201`.

O nosso `gravar_pagina` (`ndx.rs:595`) sela e escreve na hora, sempre. Está
documentado como escolha em `FORMATO.md` e travado por teste
(`crates/phxsql-store/tests/ndx.rs:317`, `o_cache_nao_segura_gravacao`).

### 3.3 O CRC de página do Aria é *opcional*, e é gancho de arquivo

`ma_open.c:2098-2102`:

```c
file->post_read_hook = &maria_page_crc_check_index;
if (share->options & HA_OPTION_PAGE_CHECKSUM)
  file->pre_write_hook = &maria_page_crc_set_index;
else
  file->pre_write_hook = &maria_page_filler_set_normal;
```

Três coisas de uma vez:

1. É `pre_write_hook`/`post_read_hook` do **arquivo**, não do cache: o CRC roda
   quando a página cruza a fronteira do disco, nunca num acerto de cache. Igual
   ao InnoDB, e igual ao que já fizemos do lado da **leitura** (`ndx.rs:582-585`
   — o comentário lá está certo e a decisão está certa).
2. É **opcional por tabela**: `PAGE_CHECKSUM=1` no `CREATE TABLE`. Quem não
   pede, não paga; grava um preenchimento constante
   (`maria_page_filler_set_normal`).
3. `maria_page_crc` (`ma_pagecrc.c:29`) semeia o CRC com o **número da página**
   — assim uma página inteira trocada de lugar é detectada, e não só o byte
   trocado. O nosso `pag_crc` (`ndx.rs:280`) não semeia com o número da página.
   É uma linha de código e fecha um buraco que o nosso formato tem hoje.

### 3.4 O estado do `.MAI` **não** vai ao disco por linha

Este é o achado direto sobre o nosso `ndx.rs:723`.

O Aria guarda no `.MAI` o mesmo que nós guardamos na página 0 do `.ndx`: a raiz
de cada índice (`state.key_root[]`), a contagem de linhas, o tamanho do arquivo.
`maria_write` termina chamando `_ma_writeinfo` (`ma_write.c:306`), e
`_ma_writeinfo` (`ma_locking.c:292`) começa com esta guarda
(`ma_locking.c:301`):

```c
if (share->tot_locks == 0 && !share->base.born_transactional)
{
  /* transactional tables flush their state at Checkpoint */
  ...
}
else if (operation)
  share->changed= 1;			/* Mark keyfile changed */   // ma_locking.c:336
```

Traduzido: **enquanto a tabela está travada — isto é, durante qualquer comando
que insira mais de uma linha — o estado não vai ao disco.** Marca um `bool` em
RAM. Em tabela transacional ele vai no *checkpoint*, nunca por linha.

O nosso `inserir_ja_conferido` termina assim (`ndx.rs:722-723`):

```rust
self.indices[idx].qtd_chaves += 1;
self.gravar_cabecalho()
```

E `gravar_cabecalho` (`ndx.rs:544`) monta `vec![0u8; self.page_size]` — **uma
página inteira de 4096 bytes zerada** —, preenche 128 bytes de cabeçalho e o
diretório, e grava os 4096 no offset 0. **Por chave. Por índice. Por linha.**

É o mesmo defeito que o `DESEMPENHO.md` §2.0 achou no `.reg` e o §2.2 achou no
`.log`, na terceira casa da mesma rua — e é a única das três que ficou de fora
das duas caçadas anteriores. Medido em §5.1.

### 3.5 O que o Aria escreve no disco quando o processo morre no meio

A resposta é três bytes, escritos **uma vez**. `_ma_mark_file_changed_now`
(`ma_locking.c:429`):

```c
if (_MA_ALREADY_MARKED_FILE_CHANGED) DBUG_RETURN(0);      // ma_locking.c:435
...
share->state.open_count++;                                 // ma_locking.c:445
mi_int2store(buff, share->state.open_count);
buff[2] = 1;                          /* Mark that it's changed */
my_pwrite(share->kfile.file, buff, sizeof(buff),
          sizeof(share->state.header) + MARIA_FILE_OPEN_COUNT_OFFSET, ...);  // :460-464
```

Na abertura, `open_count != 0` significa «não foi fechado direito»:
`ha_maria::is_crashed` (`ha_maria.cc:2461-2464`) devolve verdadeiro, e o
`auto_repair` (`ha_maria.cc:2824`) dispara o reparo. O fechamento limpo
decrementa em `_ma_decrement_open_count` (`ma_locking.c:517`).

**Isto é a peça que falta no nosso argumento do §4.4 do `DESEMPENHO.md.**
Aquele parágrafo recusou adiar o índice porque «uma queda no meio da carga
deixaria uma árvore com chaves faltando e **nada dizendo isso**». O Aria mostra
o preço de fazer algo dizer: um `bool` em RAM, três bytes no arquivo, uma vez
por abertura.

### 3.6 O `bulk insert` do Aria é o §4.1 do nosso `DESEMPENHO.md`, implementado

`_ma_ck_write` (`ma_write.c:429`) desvia:

```c
if (info->bulk_insert && is_tree_inited(&info->bulk_insert[key->keyinfo->key_nr]))
  tmp= _ma_ck_write_tree(info, key);     // ma_write.c:1669
else
  tmp= _ma_ck_write_btree(info, key);
```

`_ma_ck_write_tree` (`ma_write.c:1677`) põe a chave numa **árvore balanceada em
memória** em vez de descer o B-tree. Quando ela enche ou o comando acaba,
`keys_free` (`ma_write.c:1699`) a percorre **em ordem** e chama
`_ma_ck_write_btree` para cada uma (`ma_write.c:1734`).

É exatamente «ordenar as chaves do lote antes do `.ndx`» — o item que o nosso
§4.1 mediu em **1,19×** e deixou registrado com o número. Aqui vale notar duas
coisas:

- **Só chave não única entra na árvore**: `maria_init_bulk_insert`
  (`ma_write.c:1743`) filtra com `if (! (key[i].flag & HA_NOSAME) && ...)` em
  `ma_write.c:1761`. `HA_NOSAME` é «única». O mesmo veredito do nosso §4.
- E o Aria **grava a linha primeiro** (`ma_write.c:157`), o que é a condição
  que o nosso §4.1 identificou como o preço: os rowids têm de existir antes das
  chaves. O preço que ele paga por isso está em §3.5 — a marca de arquivo sujo.

### 3.7 Desligar o índice: o Aria exige tabela **vazia**

`ha_maria::start_bulk_insert` (`ha_maria.cc:2164`) só desliga índices quando
(`ha_maria.cc:2218-2220`):

```c
if ((file->state->records == 0) &&
    (share->state.state.records == 0) && can_enable_indexes &&
    (!rows || rows >= MARIA_MIN_ROWS_TO_DISABLE_INDEXES) && ...
```

`MARIA_MIN_ROWS_TO_DISABLE_INDEXES` é 100 (`maria_def.h:1238`).

E `ha_maria::disable_indexes` (`ha_maria.cc:1988`) traz, em comentário e em
`DBUG_ASSERT`:

```c
/* unique keys cannot be disabled either */
for (uint i=0; i < table->s->keys; i++)
  DBUG_ASSERT(!(table->key_info[i].flags & HA_NOSAME) || map.is_set(i));
```

> **O `DESEMPENHO.md` §4.4 chegou às duas conclusões medindo, e o MariaDB(R)
> chegou às duas escrevendo `assert`.** Nós medimos o ponto de virada em
> M ≈ N/3; eles cortaram em N = 0, que é o caso mais conservador possível do
> mesmo resultado. Nada a mudar aqui — só a satisfação de a medição bater com
> quem já passou por isso.

---

## 4. A tabela: nós × InnoDB × Aria

| Decisão de projeto | PhxSql 0.17.0 | InnoDB 8.0 | Aria 11.4 |
|---|---|---|---|
| **Checksum de página, quando** | a **cada gravação** (`ndx.rs:596-597`) e a cada leitura do arquivo (`ndx.rs:585`) | só na descarga da página (`buf0flu.cc:1243`) e na leitura do arquivo | só no `pre_write_hook` do arquivo (`ma_open.c:2100`) |
| **Checksum de página, como** | CRC-32 IEEE por tabela, software, **2,35 µs / 4 KiB** (medido) | CRC-32C por **instrução** (`ut/crc32.cc:794-806`) | `my_checksum`, semeado com o **nº da página** (`ma_pagecrc.c:29`) |
| **Checksum de página, se** | sempre | configurável (`innodb_checksum_algorithm`) | **opcional por tabela** (`HA_OPTION_PAGE_CHECKSUM`) |
| **Cache de páginas** | leitura, *write-through*, 2.048 páginas | buffer pool, *write-back* | pagecache, *write-back* (`ma_pagecache.h:72`) |
| **Páginas escritas no arquivo por inserção** | **2,06** (medido) + 1 por índice de cabeçalho | **0** | **0** |
| **Cabeçalho / estado do índice** | página 0 inteira, **por chave** (`ndx.rs:723`) | no *tablespace*, com o resto | só quando destravado ou no checkpoint (`ma_locking.c:301`) |
| **Registro de recuperação** | `.log` da tabela, por linha, com imagem opcional | redo por **delta** de registro (`page0cur.cc:1040`) | WAL (transacional) ou `open_count` + reparo |
| **Marca de «não fechei direito»** | **não existe** | LSN do checkpoint | 3 bytes, 1× por abertura (`ma_locking.c:460`) |
| **Índice secundário adiado** | não | sim, se **não único** e **fora do pool** (`ibuf0ibuf.ic:116`, `ibuf0ibuf.cc:3375`) | sim, em árvore ordenada em RAM, se **não único** (`ma_write.c:1761`) |
| **Divisão de folha** | sempre ao meio (`ndx.rs:789`) | ao meio, **ou no ponto de inserção** se a carga é sequencial (`btr0btr.cc:1677`) | ao meio |
| **Ordem física dos dados** | ordem de digitação, slot fixo, `rowid` é endereço | ordem da chave primária | bitmap com reaproveitamento de espaço (`ma_blockrec.c`) |
| **Reaproveita espaço de linha excluída** | **nunca** (regra do projeto) | sim | sim |
| **Construção do índice em lote** | `construir_em_lote` (`ndx.rs:927`), 23–25× medido | `ddl/` (sort + build) | `maria_repair_by_sort` (`ma_check.c:3808`) |
| **Enchimento na construção em lote** | 80%, medido (`ndx.rs:309`) | reserva configurável | reserva por tipo de chave |
| **Transação / MVCC / rollback** | **não tem** | tem | tem (transacional) |

---

## 5. O que estamos errando, em ordem de valor

A ordem é **ganho medido ÷ custo de implementar**. As três primeiras foram
medidas no nosso próprio motor, empilhadas, na cópia em `/tmp`; a quarta foi
medida em espaço, não em tempo; a quinta é um teto e vem com um preço de
formato.

### O empilhamento, medido

`--example onde-doi 200000`, forma da bancada (2 índices), três corridas cada:

| variante | µs/linha | acumulado |
|---|---:|---:|
| como está hoje (0.17.0) | 16,1 · 16,4 · 16,2 | — |
| **+ cabeçalho do `.ndx` fora do caminho da chave** | 14,0 · 14,0 · 14,0 | **1,16×** |
| **+ CRC slice-by-16** | 13,0 · 12,9 · 13,0 | **1,25×** |
| **+ cache de páginas *write-back*** | 7,2 · 7,3 · 7,2 | **2,25×** |

Com 1.000.000 de linhas, para conferir que não é efeito de tamanho pequeno:
**16,3 → 7,5 µs por linha, 2,17×** (as duas medidas na mesma máquina, na mesma
sessão).

Para calibrar o que isso significaria na bancada: hoje ela mede PhxSql 265,2 s
contra MySQL(R) 113,5 s. **Inferido, não medido:** 2,25× no custo por linha
levaria os 265,2 s para perto de 118 s. Inferido porque a bancada carrega em
lotes de 50.000 abrindo e fechando a tabela, e o §4.6 do `DESEMPENHO.md`
registra 6,6 µs por linha ainda sem explicação nesse caminho — que estas três
mudanças podem ou não tocar. **Isso se mede rodando a bancada, não escrevendo.**

---

### 5.1 O cabeçalho do `.ndx` grava 4 KiB por chave — 1,16× medido

**O que eles fazem.** O Aria marca um `bool` em RAM e leva o estado ao disco
quando a tabela é destravada ou no checkpoint: `ma_locking.c:301` e
`ma_locking.c:336`. O InnoDB não tem esse arquivo separado: a raiz de cada
índice vive no dicionário de dados, e o dicionário é ele próprio uma tabela
gravada pelo mesmo buffer pool — ou seja, também não vai ao disco por linha.

**O que nós fazemos.** `ndx.rs:723` chama `gravar_cabecalho()` ao fim de toda
inserção de chave. `gravar_cabecalho` (`ndx.rs:544`) aloca `vec![0u8;
page_size]`, calcula dois CRCs curtos e escreve **4096 bytes** no offset 0.
Com dois índices são **duas** dessas por linha.

Medido, em bancada isolada: `vec![0u8; 4096]` + os dois CRCs curtos + `seek(0)`
+ `write(4096)` custa **0,69 µs**; a chamada de escrita sozinha, **0,48 µs**.
Vezes dois índices: **1,38 µs por linha**, de 16,2. No motor de verdade o
número saiu maior — **2,2 µs, 1,16×** —, provavelmente porque a página 0 também
suja a página do sistema de arquivos a cada linha.

**O que custaria em nós.** Cerca de dez linhas. O cabeçalho passa a ir ao disco
**quando a estrutura muda** — raiz nova ou página alocada, o que acontece uma
vez a cada ~118 chaves — e no `sincronizar`. Foi assim que medi:
`alocar_pagina` e a troca de raiz levantam um `estrutura_mudou`, e
`inserir_ja_conferido` só grava se ele estiver levantado.

**O que quebraria das nossas regras: só um contador.** Rodei a suíte inteira do
`phxsql-store` com essa mudança: **181 testes passam, 1 falha** — e o que falha
é `o_cache_nao_segura_gravacao` (`tests/ndx.rs:317`), na linha do
`qtd_chaves`, que ficou em 19.973 de 20.000. Instrumentei o teste para conferir
o resto: **`varrer` devolve as 20.000 chaves**. A árvore está inteira; o que
atrasa é o contador.

E o contador é exatamente a mesma natureza do que o §2.2 do `DESEMPENHO.md` já
decidiu adiar no `.log` — «o cabeçalho é um contador, e a leitura sabe
recalculá-lo». Aqui, `verificar()` (`ndx.rs:1286`) já o recalcula varrendo.

> **É o primeiro a atacar.** Maior razão ganho/custo da lista, nenhuma garantia
> trocada, nenhum byte novo no formato.

### 5.2 O CRC de página é 1,30× mais lento do que precisa, sem mudar nada — 1,08× medido

**O que eles fazem.** Instrução de máquina (`ut/crc32.cc:794-806`).

**O que nós fazemos.** `crc32_with` (`crates/phxsql-core/src/crc.rs:66`) é
slice-by-8. O comentário lá conta a história certa: byte a byte dava 10,0 µs por
página, slice-by-8 dá 2,3.

**A continuação da mesma ideia é slice-by-16**, e ela é literalmente a mesma
tabela com oito colunas a mais: `build_tabelas8` já gera a coluna `k` a partir
da `k-1`; basta ir até 16 e consumir 16 bytes por volta.

Medido: **2,35 → 1,81 µs por página, 1,30×**. Dentro da inserção, em cima da
mudança anterior: **14,0 → 13,0 µs, 1,08×**.

**O que custaria em nós.** Umas vinte linhas em um arquivo. Mais 8 KiB de tabela
estática (16 × 256 × 4 bytes = 16 KiB no total, contra 8 KiB hoje).

**O que quebraria: nada.** É o mesmo polinômio e o mesmo valor. Os testes de CRC
do próprio projeto passam **sem uma linha alterada** — inclusive
`slice8_concorda_com_o_laco_byte_a_byte`, que confere contra o laço de
referência em todo tamanho de 0 a 300 bytes, e `pagina_inteira_bate`. Nenhum
arquivo já gravado muda de valor.

### 5.3 O cache de páginas escreve através — 1,81× medido em cima dos dois

**O que eles fazem.** InnoDB: `buf_flush_note_modification` marca suja e a
página fica (`mtr0mtr.cc:338`); o cleaner descarrega no ritmo de
`srv_io_capacity` (`buf0flu.cc:2475`). Aria: `PAGECACHE_WRITE_DELAY`
(`ma_page.c:255`), `PCBLOCK_CHANGED` (`ma_pagecache.c:177`).

**O que nós fazemos.** `gravar_pagina` (`ndx.rs:595`) sela e escreve, sempre.

**Quanto vale.** Implementei o *write-back* na cópia — a página fica suja em
RAM, o CRC e o `write` acontecem no despejo ou no `sincronizar` — e medi:
**13,0 → 7,2 µs por linha, 1,81×**; com 1M de linhas, 7,5. A parcela do `.ndx`
na inserção cai de 11,5 µs para 2,6 µs, e a linha do relatório vira

```
  .reg + .log ................     4,6 us    63,0%
  primeiro indice ............     1,7 us    23,8%
  conferir a chave unica .....     0,1 us     0,8%
  segundo indice .............     0,9 us    12,3%
```

— isto é, **o `.ndx` deixa de ser o dono do tempo**, pela primeira vez desde que
o `onde-doi` existe.

**O que quebraria das nossas regras, e é a parte séria.** Exatamente a garantia
que o `FORMATO.md` descreve e que `tests/ndx.rs:317` trava: hoje uma queda do
**processo** não atrasa o `.ndx` em relação ao `.reg`, porque o `write` já
entregou a página ao núcleo. Com *write-back*, atrasa.

Rodei a suíte com a versão *write-back*: **181 passam, 1 falha**, e a que falha é
essa mesma. O teste está fazendo o trabalho dele.

**Como os dois compram essa garantia de volta**, e o que isso custaria aqui:

- **InnoDB** paga com redo: a página pode se perder porque o delta está no log
  (`page0cur.cc:854`). É o caminho caro — formato novo, recuperação nova.
- **Aria não transacional** paga com **três bytes e um reparo**
  (`ma_locking.c:460`, `ha_maria.cc:2461`). É o caminho barato, e o que **cabe
  aqui**: o `.ndx` inteiro é derivável do `.reg`, e desde a 0.17.0 a
  reconstrução em lote custa **0,31 s por milhão de chaves** (`DESEMPENHO.md`
  §4.3, 23–25× o `reindexar` antigo).

Ou seja: `abrir` acha a marca de sujo, chama `construir_em_lote`, e a tabela
volta correta. O §4.4 recusou um estado novo no formato porque ele comprava
1,22×. **Aqui o mesmo mecanismo compra 1,81×, e a reconstrução que o torna
barato já existe.** É outra conta, e merece outra decisão — mas a decisão é do
Adriano, não deste documento.

Uma observação de honestidade sobre o alcance da medida: 200.000 e 1.000.000 de
linhas cabem confortavelmente no teto de 2.048 páginas por índice. **Não está
medido** o que o *write-back* faz quando a árvore não cabe mais e o despejo
passa a escrever a toda hora — é a primeira pergunta do §7.

### 5.4 A divisão de folha desperdiça metade de cada página em carga crescente — 1,99× de espaço, medido

**O que eles fazem.** `btr_page_get_split_rec_to_right` (`btr0btr.cc:1677`), com
o comentário que explica a regra inteira (`btr0btr.cc:1694-1697`):

> *«We use eager heuristics: if the new insert would be right after the previous
> insert on the same page, we assume that there is a pattern of sequential
> inserts here.»*

Nesse caso a divisão acontece **no ponto de inserção**, não no meio — a folha
que ficou para trás sai cheia. Há a simétrica para a esquerda,
`btr_page_get_split_rec_to_left` (`btr0btr.cc:1639`), e as duas são consultadas
em `btr_page_split_and_insert` (`btr0btr.cc:2371-2375`).

**O que nós fazemos.** `ndx.rs:789` (folha) e `ndx.rs:878` (nó interno):

```rust
let meio = entradas.len() / 2;
```

Sempre ao meio. E **toda chave nossa termina com o rowid em big-endian**
(`FORMATO.md`, «chave completa»), que só cresce — então qualquer índice sobre
coluna crescente (`Sequence`, `Uuid` v7, um `Int8` de importação, a própria
chave primária de um ERP) é o caso sequencial puro, e cada folha fica pela
metade **para sempre**.

**Medido** (`--example densidade`, escrito para isto, chaves `Int8`
estritamente crescentes):

| chaves | hoje (meio) | com a heurística | |
|---:|---|---|---:|
| 1.000.000 | 8.436 páginas, **33,0 MiB**, 118,6 chaves/página | 4.238 páginas, **16,6 MiB**, 236,0 chaves/página | **1,99×** |
| 10.000.000 | 84.363 páginas, **329,5 MiB** | 42.360 páginas, **165,5 MiB** | **1,99×** |

**O que custaria em nós.** Seis linhas: quando a chave nova entra no fim da
folha, dividir no fim em vez de no meio. Implementei e rodei a suíte:
**182 testes passam, zero falham.** Nenhuma mudança de formato — a árvore
resultante é uma árvore válida com outra distribuição.

**O que NÃO está medido.** O ganho em **tempo**. A 1M e a 10M de chaves o tempo
ficou igual dentro do ruído (16,3 vs 16,5 µs/linha no `onde-doi`; 50,6 s vs
47,5 s no `densidade` de 10 milhões). A altura da árvore também não muda nessas
faixas: com `ck_len` de 17 bytes, os dois casos dão a mesma quantidade de
níveis. **Inferido, não medido:** metade do arquivo é metade da RAM para
cachear a mesma árvore, e é exatamente aí que a nossa taxa cai (§7) — mas
provar isso exige a bancada de 10 milhões inteira, não o `onde-doi`.

Fica na lista pelo espaço, que é medido e é 2×, e pela relação com o §7. Não
prometo tempo.

### 5.5 CRC por hardware: 5,7× no CRC, e um preço de formato

**O que eles fazem.** `ut/crc32.cc:794-806`, já citado.

**Quanto vale, medido:** o CRC de uma página de 4 KiB cai de 2,35 µs para
**0,41 µs** com `_mm_crc32_u64` serial, e para **0,25 µs** com três fluxos
entrelaçados (que é o que o `crc32_using_pclmul` deles faz). Se as três
mudanças de cima estivessem feitas, isto tiraria mais um pedaço do que sobrar
do CRC.

**O que custaria em nós.** Não uma crate: `std::arch::x86_64::_mm_crc32_u64` e
`std::is_x86_feature_detected!` são **da `std`**, e o projeto exige Rust 1.75
(`Cargo.toml`), muito acima do 1.27 em que isso estabilizou. Precisa de um
caminho de software para quem não tem `sse4.2` — que é o `crc32` de hoje.

**O que quebraria: o formato.** `_mm_crc32_u64` calcula **CRC-32C**
(polinômio de Castagnoli), não o CRC-32 IEEE que o `FORMATO.md` especifica em
`.reg`, `.ndx`, `.bin`, `.memo`, `.log`, `.trash` e `.reason`. Adotá-lo muda
**todos os CRCs já gravados**. Existe a saída de calcular o IEEE com
`PCLMULQDQ` (também na `std`), que dá velocidade parecida sem mudar o
polinômio, mas é código de dobra de polinômio — bem mais difícil de acertar do
que uma tabela, e a regra da casa é que criptografia e soma de verificação se
conferem contra vetor oficial.

**Recomendação:** fica para uma mudança de formato que já esteja acontecendo por
outro motivo, e com a mesma disciplina de vetor de teste que o resto. O §5.2
compra 1,30× do mesmo custo hoje, sem nada disso.

### 5.6 Duas linhas menores, achadas de passagem

- **O CRC de página não é semeado com o número da página.** `maria_page_crc`
  (`ma_pagecrc.c:29`) semeia; `pag_crc` (`ndx.rs:280`) não. Sem a semente, uma
  página inteira que apareça no lugar de outra — um `write` no offset errado,
  uma cópia de arquivo truncada e remendada — passa pelo CRC como boa. Uma
  linha, custo zero, e **é mudança de formato**: o CRC gravado muda de valor.
  Entra junto com qualquer outra mudança de formato.
- **`page_get_max_insert_size_after_reorganize`** (`btr0cur.cc:2769`): antes de
  desistir e dividir, o InnoDB pergunta se **reorganizar** a página resolveria.
  Não se aplica a nós: as nossas entradas têm largura fixa e a folha não
  fragmenta.

---

## 6. O que NÃO devemos copiar, e por quê

### 6.1 A caixa de mudanças (`ibuf`) — mira um gargalo que não temos

Já demonstrado em §2.7 com duas citações: ela só é armada quando a folha **não
está na memória** (`btr0cur.cc:944-952`, `ibuf0ibuf.cc:3369-3375`). Ela troca
uma **leitura aleatória de disco** por escrita adiada. A nossa carga é 95% CPU e
**0,0 MiB lidos** (`DESEMPENHO.md` §1). Não há o que economizar.

Isto **não contradiz** o §4.4 do `DESEMPENHO.md` — confirma-o por outra via.

### 6.2 Adiar o índice **único** — o MariaDB(R) também não faz

`ha_maria.cc:1988-1992`. E a nossa razão é mais forte que a deles: o `.reg`
nunca reaproveita slot, então descobrir a duplicata depois de gravar deixa um
buraco permanente por linha recusada. **A conferência antes de qualquer
gravação (`table.rs:763`) fica.**

### 6.3 Adiar o índice não único de tabela **cheia** — medido e recusado

`DESEMPENHO.md` §4.4 mediu o ponto de virada em M ≈ N/3 e o teto em 1,22×.
`ha_maria.cc:2218` corta em `records == 0`. Dois caminhos, o mesmo destino.

### 6.4 A ordem física da chave primária (InnoDB) e o bitmap de espaço livre (Aria)

O InnoDB guarda a linha **na ordem da chave primária**; o Aria reaproveita o
espaço de linhas excluídas por bitmap (`ma_blockrec.c`). Os dois quebram
frontalmente a regra que define o PhxSql:

> **A ordem de digitação é sagrada, e o `.reg` nunca reaproveita slot excluído.**

Sem ela caem quatro coisas que já funcionam e que o `DESEMPENHO.md` §5 lista:
o endereço por conta (`offset = data_offset + (rowid−1) × slot_size`), a
paginação por cursor, o salto por bissecção e a garantia de que a réplica chega
aos mesmos rowids sem ninguém transmiti-los. **Qualquer proposta que mexa nisso
está fora, e este parágrafo existe para não ser preciso reabrir a discussão.**

### 6.5 O *doublewrite buffer*

`dblwr::write` (`buf0flu.cc:1252`). É um **custo** deles: cada página vai ao
disco duas vezes, para sobreviver a uma escrita rasgada de 16 KiB. Copiar
seria copiar a conta sem a dívida — o nosso espelho de volume já cobre o caso
que ele cobre, e o CRC por página já detecta o rasgo.

### 6.6 Qualquer crate

`crc32fast`, `crossbeam`, o que for. **Zero dependências externas** é o que fez
a compilação cruzada para Windows funcionar de primeira e o que permite
`cargo build --offline`. Tudo o que este documento recomenda cabe na `std` —
inclusive o CRC por hardware do §5.5, que está em `std::arch`.

### 6.7 WAL, MemTable de escrita, group commit

Já resolvido, com número, em `DESEMPENHO.md` §3 e §7: eles atacam o `fsync` do
InnoDB, e o nosso não é `fsync`. **Não voltei a esse assunto neste documento e
não achei nada no fonte que mude o veredito** — pelo contrário: o
`mtr_t::Command::execute` (`mtr0mtr.cc:842`) mostra que o redo do InnoDB existe
justamente para que a **página** não precise ir ao disco. Nós já temos um
arquivo *append-only* (o `.reg`) e já sincronizamos uma vez por carga; o que nos
falta não é o log, é a página não ir.

---

## 7. Por que a taxa deles é plana e a nossa cai

A pergunta 7 do pedido. A evidência do fonte diz três coisas, e a terceira é a
que manda.

**1. O ritmo de descarga deles não olha o tamanho da tabela.**
`get_pct_for_dirty` (`buf0flu.cc:2475`) olha a fração de páginas sujas contra
`srv_max_buf_pool_modified_pct`; `get_pct_for_lsn` (`buf0flu.cc:2503`) olha a
idade do redo. Nenhum dos dois é função de N. A vazão é uma fração de
`srv_io_capacity`, uma configuração.

**2. A altura da árvore cresce como log(N), e devagar.** Com página de 16 KiB o
leque é grande e a descida fica em 3–4 níveis de 1 milhão a 100 milhões de
linhas. Isso vale para nós também, e por isso **não** é a explicação principal:
medi que, mesmo com o leque dobrando de 118 para 236 chaves por página (§5.4),
a altura não muda em 10 milhões de chaves.

**3. O custo por linha deles é constante e o nosso não é.** Esta é a resposta.
Por inserção o InnoDB gasta um trabalho **O(1) numa página que já está em
RAM**, mais dezenas de bytes de redo. O nosso gasta:

- **2,06 páginas de 4 KiB gravadas, cada uma selada com um CRC de página
  inteira** — 4,8 µs de 16,2, medido, e esse pedaço **não encolhe nunca**;
- **duas páginas de cabeçalho de 4 KiB por linha** (§5.1) — idem;
- e, à medida que a árvore passa do teto de 2.048 páginas, as descidas voltam a
  **ler do arquivo**, e cada leitura paga o CRC outra vez. O `DESEMPENHO.md` §2
  já registra o sintoma: na carga de 10 milhões o primeiro milhão entrou a
  16.051/s e o décimo a 9.311/s.

E o `.ndx` que precisa caber é **duas vezes maior do que precisaria** por causa
do §5.4 — 329,5 MiB em vez de 165,5 MiB para 10 milhões de chaves, medido.

> **Então a resposta é: eles são planos porque a inserção deles não escreve
> página, e nós caímos porque a nossa escreve — e porque a árvore que precisa
> caber em RAM é o dobro do que precisaria.** As três primeiras propostas do §5
> atacam exatamente isso, e as três estão medidas em 2,25×.

Não é buffer pool «porque tem mais RAM»: o nosso cache de 8 MiB já serve 8,80
páginas por linha sem ir ao arquivo (medido). É que, servida ou não, **toda
gravação nossa atravessa e paga CRC.**

---

## 8. As perguntas que a leitura levantou e só a medição responde

Em ordem de quanto mudariam a decisão.

1. **O *write-back* (§5.3) continua valendo quando a árvore não cabe no cache?**
   Medi a 200.000 e a 1.000.000 de linhas, onde ela cabe. Quando o despejo
   começar a escrever a cada inserção, o ganho pode encolher — ou pode crescer,
   porque o despejo escreve uma vez o que hoje se escreve muitas. **Roda a
   bancada de 10 milhões com a mudança e compara.**
2. **Quanto do 2,25× sobrevive à bancada?** A bancada carrega em lotes de 50.000
   abrindo e fechando a tabela, e o §4.6 do `DESEMPENHO.md` deixou 6,6 µs por
   linha sem explicação nesse caminho. O `onde-doi` roda um processo só.
3. **A heurística de divisão sequencial (§5.4) compra tempo, ou só espaço?**
   Medi 1,99× de espaço e nenhum tempo a 1M e 10M de chaves. A hipótese de que
   metade do arquivo vira mais acerto de cache na bancada é **hipótese**.
4. **O suspeito do §4.6 — o cache do `.ndx` nascer vazio a cada processo —
   continua sem medida.** É o mesmo eixo da pergunta 2 e ainda vale medir
   sozinho.
5. **Página de 8 ou 16 KiB no `.ndx` ajudaria ou atrapalharia?** O leque
   quadruplicaria; o CRC por página também. Com o CRC fora do caminho quente
   (§5.1 + §5.3) a conta muda de sinal, e ela hoje não está feita.
6. **O CRC semeado com o número da página (§5.6) — quantas corrupções reais
   isso pegaria?** Não é pergunta de desempenho, é de garantia, e a resposta
   honesta é «não sei; sei que hoje não pega nenhuma».

---

## Como refazer as medições deste documento

As três mudanças foram construídas numa cópia do repositório em `/tmp` e não
estão no código. Para refazer:

```bash
# a base, para comparar
cargo run --release --example onde-doi -- 200000
cargo run --release --example onde-doi -- 1000000
```

E, sobre uma cópia:

1. **§5.1** — em `ndx.rs`, levantar um `estrutura_mudou` em `alocar_pagina` e na
   troca de raiz; em `inserir_ja_conferido`, só chamar `gravar_cabecalho` se ele
   estiver levantado; chamá-lo também no `sincronizar`.
2. **§5.2** — em `crates/phxsql-core/src/crc.rs`, estender `build_tabelas8` de 8
   para 16 colunas e consumir 16 bytes por volta em `crc32_with`. Os testes do
   próprio arquivo provam a equivalência.
3. **§5.3** — em `ndx.rs`, `gravar_pagina` põe a página no cache marcada como
   suja; o despejo e o `sincronizar` selam e escrevem.
4. **§5.4** — em `ndx.rs:789`, `meio = entradas.len() - 1` quando a chave nova
   entra no fim da folha.

O medidor de densidade do §5.4 é um exemplo novo de vinte linhas que abre um
`NdxFile`, insere N chaves crescentes com `inserir_ja_conferido` e imprime
`paginas()`. As medidas de CRC e da escrita de cabeçalho do §5.1/§5.2 saem de
um programa avulso que cronometra as funções isoladas.

---

## Nota sobre os nomes

MySQL(R) e InnoDB são marcas da Oracle Corporation. MariaDB(R) e Aria são marcas
da MariaDB Corporation Ab. Este documento lê os fontes públicos dos dois sob as
licenças deles para entender decisões de projeto; nenhum código foi copiado para
o PhxSql, e as recomendações do §5 são reimplementações de ideias documentadas,
escritas do zero e só com a `std` do Rust.
