# Parecer do DBA (papel C): pedido 496, o catálogo das catástrofes — 24/09/2026

Árvore medida: `8474487` (todas as âncoras `arquivo:linha` reconferidas nela). Kernel 6.18.44, rustc 1.94.1.
Provas contra o SO em montagem privada (`unshare -m`), nada no disco da máquina. O arnês inteiro está no
**Apêndice A** — o scratch desta frente foi apagado, e roteiro que resolveu algo não morre com a sessão.

## 0. Veredito

1. **O motor não prevê nada — confirmado.** Mede cru (`df` em `sistema.rs:213`, canário do 249, telemetria de
   200 s em `telemetria.rs:68-71`, `capacidade` no `esquema` em `servidor.rs:18929`) e avisa por **limiar fixo**,
   com o vigia de espaço **desligado de fábrica** (`config.rs:971`; `servidor.rs:23432` nem sobe a thread).
2. **Antes do previsor, o DBA diz NÃO a prever por cima de garantias que hoje não valem.** Quatro defeitos
   ativos, de perda de dado ou de parada, três deles provados contra o SO nesta frente (§2): o `fsync` que falhou e
   é repetido (C1, **novo**; irmão do 503), o `.ndx` marcado limpo sobre página que não foi ao disco (C2b, **novo**),
   o backup agendado que falha calado (C6, **novo**, lido; irmão do 502), e o «erro que grava» — linha no `.reg` sem
   evento no `.log` (C2a/C3), que **amplia o 498** aberto hoje: o 498 viu o `atualizar` no teto; aqui entram o
   `inserir`, o `excluir`, o disco cheio como segunda causa (22 de 41), a **órfã provada** e o `verificar()` cego.
   Mais um de baixa probabilidade (C13, novo).
3. **A camada é uma só com o 495**, e ela quase existe: fila com carteiro na saúde do disco, silêncio no
   `jobs::pode_avisar`. Correção ao J (495 §6): o silêncio **não** é um só — são **três** escritas da mesma
   decisão (§5).
4. **Primeira fatia:** `esgota_em`, a tendência do espaço livre, no vigia que já chama o `df`, entregue pelo
   carteiro que já existe (§6). A conta foi medida contra um tmpfs enchendo: erro ≤ 3,4% a taxa constante (6 corridas), e
   a janela curta **erra para o lado perigoso** (otimista em 5 de 5 corridas, até +1.281%) — o desenho sai
   dessa medida.

Falta da versão, pela fórmula do gerador (`pagina-dos-pedidos.py`, `percentual_falta`, lida sem gravar):
**20,8%** (feito 355 · parcial 16 · planejado 77 · depois 57). Se os cinco pedidos novos da §8.1 entrarem: **21,6%**
(98/453); o sexto item amplia o 498 e não muda a conta.

## 1. A tabela, por severidade × probabilidade

Severidade: **S1** perde dado ou garantia · **S2** para o serviço (tabela ou servidor) · **S3** degrada.
Probabilidade é **avaliação**, na data (como no `RISCOS.md`); o resto é medida.

| # | catástrofe | sev. | prob. | estado | prova |
|---|---|---|---|---|---|
| C2a/C3 | **erro que grava**: o `.log` falha (disco cheio ou teto de volumes) depois do `.reg`/`.ndx`; o cliente ouve erro com a linha gravada, a réplica nunca a recebe, e o `atualizar` pula a cascata → **filha órfã** | S1 | média | **ATIVO** — amplia o 498 | SO: 22 de 41 tamanhos de disco, 294 linhas; teto: 5/5; órfã 1/1 |
| C6 | **backup agendado que falha cala**: só `stderr`, nada no `acessos.log`, nenhum aviso, e só tenta de novo na janela seguinte | S1 | média | **ATIVO**, novo (irmão do 502) | lido, não exercitado |
| C11 | `UPDATE`/`DELETE` em massa por faixa (`WHERE id > 0`): irreversível no `UPDATE` sem backup | S1 | média | requisito 496 | lido |
| C8 | réplica para trás: o atraso é o RPO da promoção; laço estacionado por credencial só no `stderr` | S1 | média | requisito 496 | lido |
| C1 | **`fsync` que falhou, repetido, responde Ok**; o fecho apaga a marca; no arranque o `.log` perdeu 5.000/5.000 eventos confirmados e o `.ndx` está corrompido com a marca limpa | S1 | baixa–média | **ATIVO**, novo (irmão do 503) | SO: 3/3 no núcleo, 3/3 no motor |
| C13 | `rowstamp` empurrado ao fim por evento replicado: em release volta a **0**; em debug, pânico | S1 (garantia) | baixa | **ATIVO**, liga no 495 | medido: `u64::MAX, 0, 1` |
| C5 | **backup segura a trava global de ESCRITA a cópia inteira** (leitura para junto): 26–34 MB/s em zip, o padrão | S2 | certa, cresce com a base | **ATIVO** (travamento), hipótese a medir | medido: 26,0–33,6 MB/s |
| C4 | o `.log` nunca se poda (e a `.trash` só se esvazia à mão): o disco enche e cai em C2 | S2→S1 | alta | requisito 496 (fatia 1) | medido: 184 B por alteração com imagem |
| C2b | **disco cheio deixa o `.ndx` com CRC inválido e a marca de sujo em 0**: a tabela recusa ler pelo índice e gravar, e o arranque não a repara | S2 | média | **ATIVO**, novo | SO: com e sem o 2º fecho, os dois lados |
| C7 | tabela paginada cheia (`registros_por_arquivo × max_arquivos`) | S2 | baixa–média | requisito 496 | lido |
| C12 | descritores esgotados (`EMFILE`) | S2 | baixa–média | requisito 496 | **não medido** |
| C9 | trava global presa (posse longa) ou envenenada (toda operação erra até reiniciar) | S2 | baixa | requisito 496 | lido |
| C14 | memória (`memoria_max_mb: 0` = sem teto) | S2 | baixa | requisito 496 | lido |
| C10 | cascata do `ao_alterar` larga fora de transação: sem teto de linhas, sob a trava global | S3→S2 | baixa–média | requisito 496 | lido |
| — | contadores `u64` (rowid, versão, `rownum`, `Sequence`), certificado X.509 de 10 anos, senha sem validade | — | — | ⏸ | 2⁶⁴ a 1 M/s = ~584 mil anos |

Ordem: severidade primeiro, probabilidade dentro dela. O C1 fica atrás dos S1 de probabilidade média só pela
probabilidade; por episódio é o de maior dano — perde o que **já foi confirmado**, fora da janela declarada.

## 2. As provas contra o sistema operacional

### 2.1 C1 — o `fsync` repetido (o «fsyncgate» de 2018, neste motor)

**No núcleo, sem o motor** (`fsyncgate.py`, 3/3): ext4 num `loop` sobre um tmpfs com 64 KiB livres
(provisionamento fino, o caso canônico). O `write` de 1 MiB é aceito; `fsync` 1 → **EIO**; `fsync` 2, mesmo
descritor → **Ok**; libera-se espaço, `fsync` 3 num descritor **novo** → **Ok**; o cache devolve o escrito; depois
de remontar, o 1 MiB acrescentado **não existe** (o cabeçalho sincronizado antes sobrevive).

**No motor** (`p3b.sh`, 3/3), os mesmos passos que o servidor dá — `descarregar_sujas_com` **reabre** a tabela e
sincroniza (`servidor.rs:16649`), e com tudo Ok apaga as marcas dos commits (`servidor.rs:16726-16753`):

| passo | resultado |
|---|---|
| 5.000 inserções (janela `PorLote`) | 5.000 Ok ao cliente |
| fecho 1 | `erro de E/S: No space left on device (os error 28)` → a chave volta às sujas, a marca fica |
| operador libera espaço; fecho 2 (tabela reaberta) | **Ok** → o servidor apagaria a marca |
| o processo vê (cache) | 5.000 linhas, 5.000 eventos, `buscar porId=5 → [5]` |
| **depois de remontar** | `.log`: **0 eventos** (5.000 perdidos); `.ndx`: byte 52 = **0**, página 3 com **CRC inválido**, `indice_precisa_reconstruir=false`; `buscar`/`inserir` recusam; o `.reg` sobreviveu por posição de bloco, não por garantia |

O caso em que o SO nos salva também foi medido (`p3.sh`, a variante do Apêndice A, 3/3): com metadado do ext4 em
buraco, o fecho 1 dá `EIO`, o diário do ext4 aborta, a montagem vira só-leitura, a reabertura do fecho 2 dá `EROFS`
e a marca fica (`servidor.rs:16676-16680`: «não abriu … a marca tem de FICAR»). Não é garantia nossa.

**O defeito está na premissa escrita** em `servidor.rs:16726`: «o disco está em dia». Depois de um `fsync` que
falhou, um `fsync` Ok **não** prova isso — o PostgreSQL diz com as mesmas palavras no `data_sync_retry` (§4).

### 2.2 C2 — disco cheio de verdade (tmpfs), linha a linha

Tabela `pedidos` (`Int8` único, `Str(60)`, `Str(40)`), imagem no diário ligada (quem replica liga).

**C2a, varredura** (`p1d.sh`, tmpfs de 480 a 640 KiB, passo 4, 41 tamanhos): em **22** o episódio teve **erro sujo**
— a linha foi ao `.reg` e ao índice, o `.log` recusou (`ENOSPC`), o cliente ouviu erro — somando **294 linhas sem
evento**, até **25 por episódio**. Nos 22, o cliente que repete a mesma linha recebe `chave duplicada`. Em 516 KiB,
reaberta depois de liberar espaço: 1.471 vivas, 1.446 eventos, **25 linhas que réplica nenhuma jamais verá**.
A causa é de ordem: `inserir` grava `.reg` (`table.rs:3844`) e chaves, e só então `self.anotar(...)?`
(`table.rs:3896`) — sem desfazer. O caminho IRMÃO, o do índice, **já desfaz** (`table.rs:3868-3881`); o do diário
não. O mesmo `?` está no `atualizar` (`table.rs:4273`), no `excluir_de_vez` (`table.rs:4490`) e no `marcar`
(`table.rs:4633`).

**C2b, o `.ndx`** (`p1c.sh`, 512 KiB), os dois lados:

| caminho | byte 52 no disco | `precisa_reconstruir` | o que a tabela diz |
|---|---|---|---|
| **com** o fecho no mesmo punho (o `gravar_de_verdade`, `servidor.rs:16572`) | **0** | **false** | `CRC invalido na pagina 3` em `buscar`, `inserir` e `verificar` — manda procurar disco ruim |
| **só o `Drop`** | 1 | true | «reconstrua com `reparar indice`» — o recado certo |

Mecanismo, lido e confirmado pela diferença acima: `tirar_sujas` baixa a flag de **todas** as páginas antes de
gravar a primeira (`ndx.rs:301-306`); `descarregar` para no primeiro erro (`ndx.rs:1046-1051`); a página que falhou
e as seguintes **saem da lista**; o segundo fecho acha nada sujo e `pode_baixar_a_marca` (`ndx.rs:1187-1189`) não
sabe que houve falha — `sincronizar`/`fechar` gravam o cabeçalho limpo (`ndx.rs:1131-1147`, `1159-1174`). É o
fsyncgate **dentro do nosso cache**: o que falhou de gravar sai da lista do que falta gravar.

### 2.3 C3 — o teto do diário chega antes do teto do `.reg` (e faz órfã)

O `.log`, a `.trash` e o `.reason` herdam o `max_arquivos` da tabela (`paginacao.rs:417-424`, `diario.rs:66-75`),
e o corte deles pode ser encolhido pelo `recursos.diario_volume_mib` (`config.rs:3437`, «um TETO menor»). Medido
(`teto`, corte 64 KiB × 2 volumes, `.reg` de 2.000.000 linhas):

- a 2.977ª inserção falha com `diario de pedidos chegou ao teto de 2 volumes` (`log.rs:492`) com o `.reg` a
  **0,15%** da capacidade; **5 de 5** falhas são sujas; a repetição dá `chave duplicada`;
- `atualizar` responde erro e a linha **mudou** (`Blumenau` → `Joinville`);
- `verificar()` responde **Ok** com 2.981 registros e 2.976 eventos: a conferência é cega a isso;
- **órfã** (`orfa`): a inserção e 2.975 alterações de rotina (2.976 eventos) enchem o diário da mãe; alterar a chave 1→2 responde
  `limite excedido`, a mãe fica com `id=2`, a filha aponta para `1`, e não existe mãe `id=1`. É o **irmão do 486**
  um passo antes: o `.log` vem antes da trilha no `atualizar` (`table.rs:4273` e só depois `4279`), e o `?` pula
  `aplicar_ao_alterar`. Fere a regra primordial.

Não exercitei a órfã por `ENOSPC` (exige o alinhamento de página da §2.2); é o mesmo `?` da mesma linha.

### 2.4 As medidas que alimentam a conta

- **Crescimento** (`cresce`, release, 20.000 linhas): com imagem, **184 B por evento**; o `.log` já passa o `.reg`
  nas inserções (3.680.064 contra 3.160.960 B) e fica **4,66×** depois de 3 alterações por linha. Sem imagem, 44 B.
- **Backup** (`backup`, release, 94.625.732 B, cache quente): árvore 91,6–139,2 MB/s (5 corridas); **zip, o padrão**
  (`config.rs:863`), **26,0–33,6 MB/s** (4 corridas; a de 26,0 com outras frentes compilando). Com a trava de
  escrita na mão (`servidor.rs:5465`, `20425-20428`): **10 GB ≈ 5,0–6,4 min, 100 GB ≈ 50–64 min de servidor parado,
  leitura inclusive**.
- **Contador** (`carimbo`): `empurrar_carimbo(u64::MAX-1)` e depois `proximo_carimbo()` ×3 → release
  `18446744073709551615, 0, 1`; debug → `attempt to add with overflow` em `no.rs:41`. O caminho da réplica empurra
  sem teto (`table.rs:2825`).

## 3. O catálogo, item a item

Formato: **pior dia** (onde quebra) · **sinal e antecedência** · **já mede?** · **conta e limiar**.

**C2a/C3 — erro que grava.** Pior dia: acima. · Sinal: espaço livre caindo (horas a dias); volumes do diário
usados / `max_arquivos` (dias a meses). · Mede: `df` só com vigia ligado; volumes do diário **não** se medem em
lugar nenhum. · Conta: `ocupacao_diario = (volumes_fechados + fim_atual/corte) / max_arquivos`; `dias_ate_teto =
bytes_restantes / (eventos_por_dia × bytes_por_evento)`, 184 B com imagem. Limiar: aviso a 80% ou < 30 dias, crítico
a 95% ou < 3 dias. **O previsor não conserta a rajada**: o conserto é desfazer em lugar (§8.1).

**C1 — fsync repetido.** Pior dia: acima; a perda aparece **no próximo arranque**, dias depois, fora da janela
declarada do `PorLote`. · Sinal: o primeiro `fsync` recusado — **zero antecedência**; já vira evento
(`fecho_recusado`, `servidor.rs:16635`). · Mede: sim, o evento; **não** muda a conduta. · Conta: não há; é conduta
(§4, 10×0).

**C6 — backup que falha calado.** Pior dia: o disco morre e o último backup bom tem semanas. `Err` só no `stderr`
(`servidor.rs:5451`); o `acessos.log` só registra sucesso (o `?` sai antes do `anotar`, `servidor.rs:5462-5500`); o
relógio marca `ultimo = agora` **antes** de rodar (`servidor.rs:5447`), então a nova tentativa espera a janela
inteira. O `job_parado` (`jobs.rs:311`) não cobre: o backup agendado não é job, e job **pendurado** conta como
«rodando». · Sinal: `agora − último_backup_ok > 1,5 × período`. · Mede: não. · Conta: essa desigualdade.

**C11 — UPDATE/DELETE em massa.** Sem `WHERE` já se recusa (`dml.rs:408`, mais estrito que os três maduros); com
`WHERE id > 0` passa por faixa, e o `coletar_rowids` junta **tudo antes de aplicar**. O `UPDATE` não guarda o
«antes» (o `.log` guarda o depois; a `.lgpd` só coluna marcada). · Sinal: o **tamanho do plano**, conhecido antes
da primeira escrita — antecedência total. · Mede: o número existe, ninguém olha. · Conta: `linhas_do_plano / vivas`;
evento a ≥ 50% e ≥ 1.000 linhas. Observa e avisa; não recusa (guarda nova entra pedida).

**C8 — réplica para trás.** O mestre nunca poda o `.log`, então a réplica **não** perde o diário girado — a
catástrofe vira RPO: o que o mestre aceitou e a réplica não puxou morre na promoção (`cluster.rs`, cabeçalho). O
estado guarda a posição consumida (`bidirecional.rs:418-458`, `posicoes`) e **não** a cabeça da origem: o atraso não
é calculado em lugar nenhum. O laço parado por credencial só grita no `stderr` (`servidor.rs:2804`). · Conta:
`atraso = total_de_eventos_na_origem − posicao`, por tabela; `tendencia = d(atraso)/dt`; limiar: atraso crescendo
por 3 amostras seguidas, ou `atraso_s > 60`.

**C2b — `.ndx` limpo sobre página perdida.** Acima. · Sinal: o mesmo de C2a. · Conserto em §8.1.

**C4 — o `.log` sem poda.** Pior dia: vira C2. · Sinal: bytes por dia, por arquivo e por tabela. · Mede: não por
tabela; o `quanto-ocupa` (exemplo) mede à mão. · Conta: é a fatia 1 (§6), mais a decomposição «quem come o disco»
(tamanho por extensão entre duas amostras).

**C5 — backup com trava de escrita.** O comentário diz a intenção: «nenhuma escrita acontece no meio»
(`servidor.rs:20425`); a trava tomada (`dados.write()`, `servidor.rs:1701`) para **também a leitura**. · Sinal:
bytes da base. · Mede: não; e o `acessos.log` do backup grava `duracao_ms: 0` (`servidor.rs:5494`), o único número
que tornaria isto previsível. · Conta: `janela_s = bytes_da_base / vazao_medida` (26–34 MB/s em zip); aviso quando
passar da janela aceita (`recursos`), com a vazão re-medida em cada corrida.

**C7 — tabela cheia.** Recusa limpa (`reg.rs:2032`). · Sinal: `slots / capacidade` — os dois já saem no `esquema`
(`servidor.rs:18929`), a razão não. · Conta: contagem regressiva, `dias = (capacidade − slots) / insercoes_por_dia`;
aviso a 80%, crítico a 95%, no molde de dois degraus do PostgreSQL (§4).

**C9 — trava.** Envenenada: toda operação volta `trava suja` até reiniciar (`servidor.rs:25968`), e o erro não é
`Io`, então nem a saúde do disco avisa. Posse longa: backup (C5), cascata (C10), `reindexar`. · Sinal: a telemetria
tem `espera_maior_ms` e `trava_us`, 200 amostras de 1 s. · Não é previsível; é **ocorrência imediata** na mesma fila.

**C12 — descritores.** Por tabela aberta, ~8 arquivos, e cada conjunto de volumes guarda até 64 abertos
(`volume.rs:25`). O contêiner tem `ulimit -n` 20.000; o `systemd` dá 1.024 de fábrica. **Não medido.** · Conta:
`len(/proc/self/fd) / Max open files` de `/proc/self/limits`, ambos lidos com `std::fs`; aviso a 80%.

**C14 — memória.** `memoria_max_mb: 0` (`config.rs:3413`); 64 conexões × 100.000 linhas de transação
(`config.rs:3430`) ≈ 1,3 GiB no pior caso, fora o mapa de toques sem teto (pedido 330). · Conta: tendência do RSS
contra `MemAvailable`, a mesma função da fatia 1.

**C10 — cascata larga.** `TETO_DA_CASCATA = 16` é **profundidade** (`table.rs:180`); a largura é um `Vec` de todos os
rowids da filha (`table.rs:2456`), aplicado sob a trava global. Dentro de transação conta no teto de linhas
(`servidor.rs:15440`). · Sinal: o tamanho do plano — o **mesmo portão** de C11.

**C13 — contador empurrado.** Acima. · Sinal: salto do carimbo por evento. · É anomalia: a mesma família de
ocorrência do 495 (ataque ou defeito de par, a camada não precisa saber qual).

## 4. O que os maduros fazem para prever (fonte primária lida nesta rodada)

| comportamento | PG 4 | MariaDB 3 | MySQL 2 | SQLite 1 | decisão |
|---|---|---|---|---|---|
| `fsync` falhou: **nunca** tratar a repetição como sucesso | `data_sync_retry` padrão `off` → **PANIC**; a doc descreve «the second attempt may be reported as successful, when in fact the data has been lost» | `os_file_sync_posix` → `ib::fatal()` (`os0file.cc:841`, 11.4) | `ib::fatal … «fsync() returned EIO, aborting.»` (`os0file.cc:2865`, 8.0) | `unixSync` → `SQLITE_IOERR_FSYNC` (`os_unix.c:3934`) | **10×0, aceite automático** → C1 é defeito |
| expor o atraso da réplica | `replay_lag`/`flush_lag` em `pg_stat_replication` | `Seconds_Behind_Master` | `Seconds_Behind_Source` | não replica | **três convergem** → C8 entra sem pergunta |
| contagem regressiva até um limite duro | xid: aviso a **40 milhões**, recusa a **3 milhões**; `safe_wal_size` do slot | não conferido (a KB redirecionou) | `sys.schema_auto_increment_columns.auto_increment_ratio` | nada | **6 × 1** (MariaDB, qualquer lado, não vira) → C7/C3/C13 expõem a distância ao teto |
| diário retido para réplica | `max_slot_wal_keep_size` **−1 = ilimitado** de fábrica | não conferido | `binlog_expire_logs_seconds` = 2.592.000 (apaga) | — | diverge; o nosso «nunca poda» é o do PG, e o preço dele é o nosso: o disco (C4) |
| recusar `UPDATE`/`DELETE` em massa | nada no núcleo | `sql_safe_updates` (padrão desligado) | `--safe-updates` / `sql_safe_updates` | nada | **5 × 5, empate** — e não importa: nós já recusamos sem `WHERE`; o 496 só **observa** |
| prever esgotamento por **tendência** | não achei nos documentos lidos | idem | idem | idem | **decisão de produto do dono (496)**, não convergência |

**A divergência nossa, com a restrição que a causou:** os maduros contam regressivamente até um limite porque
têm quem **devolva** espaço (vacuum, purga do binlog, reuso de página). Aqui nada devolve — o `.reg` não reaproveita
slot e o `.log` não se poda, por ordem de digitação e por história. Num motor que só cresce, a pergunta útil não é
«quanto falta» e sim **«quando acaba»**. A tendência é a nossa resposta; a contagem regressiva entra junto porque
os três a dão.

## 5. Uma camada só com o 495

O desenho do J (495 §6) serve inteiro ao 496: `Ocorrencia{familia: Catastrofe, …}` → silêncio → fila → carteiro →
canais. O 496 **só pluga produtores**. Duas correções, medidas no fonte:

- **O silêncio não é um só.** Além do `jobs::pode_avisar` (`jobs.rs:320`, 4 chamadores), há duas cópias da mesma
  decisão: o vigia de disco reescreve o corpo dela em linha (`servidor.rs:23491-23499`) e o cluster tem a sua
  (`cluster.rs:1015-1023`). A fatia 1 passa pelo vigia e **absorve** a primeira no mesmo commit; a do cluster fica
  com as 7 chamadas de e-mail que o J já pôs em ⏸.
- **Ocorrências que não se preveem entram na mesma fila:** trava envenenada (C9), `fsync` recusado (C1, já é evento
  da saúde), `LimiteExcedido` do diário (C3, hoje não avisa ninguém, porque não é `Io`).

## 6. A primeira fatia: `esgota_em`

**O quê** (pequena, sem formato novo):

```rust
/// Quando o espaco livre cruza `piso_kb`, por minimos quadrados. `None` = sem
/// tendencia: menos de 4 amostras, inclinacao >= 0, ou R2 < 0,6.
pub fn esgota_em(amostras: &[(i64, u64)], piso_kb: u64) -> Option<Previsao>;
pub struct Previsao { pub horas: f64, pub kb_por_hora: f64, pub r2: f64 }
```

- **Onde:** a função pura ao lado de `sistema::espaco`; o produtor no vigia (`servidor.rs:23431`), que passa a
  **amostrar sempre** a cada `checar_minutos` — o `alertas.ligado` continua mandando só no e-mail, como o comentário
  de `discos_apertados` já promete (`servidor.rs:23412`: «Desligado quer dizer "nao manda e-mail", nao "nao olha"»).
- **Janela:** anel de 7 dias × 96 amostras por caminho (~10,5 KiB). A previsão é o **menor** `horas` entre a janela de
  24 h e a de 1 h — é a medida da §9 que manda: janela curta sozinha erra para o lado otimista.
- **Alvo:** o piso `livre_minimo_mb` que já existe (`config.rs:946-949`), não o zero — os defeitos C2 começam no
  primeiro `ENOSPC`.
- **Dois degraus**, no molde do 40M/3M: **aviso** abaixo de 24 h, **crítico** abaixo de 2 h. Observa e avisa; não
  recusa escrita.
- **Entrega:** pelo carteiro da saúde do disco (tipo novo), que a F2 do 495 generaliza. Não nasce segundo carteiro.

**Prova real, nos dois sentidos:**

1. **Unitário** — série linear decrescente acerta dentro de 2%; constante → `None`; crescente → `None`; rajada
   (liga/desliga) com a janela longa cobrindo dois ciclos → nunca otimista. Defeitos repostos, um por vez: sinal da inclinação invertido derruba
   o 1º; sem o portão de R² o ruído prevê e derruba o 3º; só a janela curta derruba o de rajada.
2. **Contra o SO** — bancada com privilégio, no molde do `prova-da-guarda-do-pulso.sh`: tmpfs de 64 MiB em
   `unshare -m`, escritor a 8 MiB/s, amostra a cada 0,5 s. A ocorrência `esgota_em` tem de chegar **antes** do
   `ENOSPC` com ≥ 50% da vida do disco de antecedência (medido: 63–91%, erro ≤ 3,4%); com o produtor desligado, nenhuma ocorrência
   antes do `ENOSPC` → vermelho. Sem `CAP_SYS_ADMIN` a bancada diz **«não provado»**, nunca verde por omissão.

**Custo:** um `df` a cada 15 min, que o vigia já paga quando ligado.

**Depois dela, na ordem do custo:** contagem regressiva da tabela e do diário (C7/C3, aritmética sobre cabeçalhos,
zero E/S nova); atraso da réplica (C8); tamanho do plano (C10/C11) no gancho da F2 do 495; descritores e memória
(C12/C14) na mesma função de tendência.

## 7. Formato e migração

- **Nenhum conserto proposto muda formato.** Desfazer o slot e a chave depois de o `.log` falhar é gravação no lugar
  (sem espaço novo, funciona com o disco cheio), e o slot morto é o preço que o `inserir` já paga e escreve
  (`table.rs:3808-3812`).
- **A fatia 1 não muda formato:** a série vive em memória. Depois de um reinício ela diz «sem tendência» por 1 h (4
  amostras de 15 min), com essas palavras.
- **Decisão de formato que convém tomar AGORA:** o teto de volumes do `.log` é herdado do `.reg`. O 368 parte B está
  tirando esse teto da trilha **nesta versão**; o `.log` tem o mesmo teto, e tirá-lo depois de haver tabela
  paginada com dado em produção vira migração. Se for para tirar, é agora, com a mesma frente.
- Se a série do previsor um dia for persistida: arquivo **próprio do nó** (JSON por linha + `rodizio.rs`, no molde do
  `jobs.log`), nunca dentro de `.reg`/`.pag` — a série é do servidor, não do dado (a mesma razão do corte global do
  diário, `diario.rs:18-24`).

## 8. Os «não» do DBA

### 8.1 Pedidos que eu abriria (ativos, na conta) — cinco novos e uma ampliação

| pedido | conserto que cabe na pétrea | prova que falha hoje |
|---|---|---|
| C1 (novo; o 503 cobre o `let _ = t.sincronizar()` e o `sync_all` da marca, não a segunda passada do fecho): `fsync` recusado não se repete como sucesso | convergência 10×0: no primeiro fecho recusado o servidor entra em só-leitura (o `somente_leitura` vivo já existe), mantém a marca, avisa, e só o arranque (que reindexa o que a marca nomeia) volta a gravar. **Dito com honestidade:** sem WAL, parar compra menos que no PostgreSQL — o evento que o núcleo já jogou fora não volta; compra não confirmar mais nada sobre disco que mente, e o bilhete que manda reindexar | `p3b.sh`: fecho 2 Ok e 5.000 eventos somem |
| C2a/C3 (**amplia o 498**): o `.log` que falha desfaz o que já gravou, em todo verbo e por qualquer causa (irmão do 486, o passo antes dele) | o desfazer que o caminho do índice já tem (`table.rs:3868-3881`), no `inserir`, `atualizar`, `excluir_de_vez` e `marcar`; e a cascata não pode depender do observador — mesma regra do 486 | `teto` e `orfa` desta frente; a varredura `p1d.sh` |
| C2b: página que não se gravou continua suja, e a marca não baixa depois de falha | `descarregar` devolve à lista o que não gravou e liga `escrita_interrompida` (o estado que `pode_baixar_a_marca` já lê) | `p1c.sh` com o fecho no mesmo punho: byte 52 = 0 hoje |
| C6 (novo; o 502 é o pânico no mesmo relógio, este é o `Err`): backup agendado que falha avisa, e tenta de novo antes da janela seguinte | a falha vira ocorrência na fila; `ultimo` só anda no sucesso, com recuo | exercitar com destino sem permissão: hoje, silêncio |
| C5: o backup para a leitura | **hipótese, não conserto**: trava de leitura basta para «nenhuma escrita no meio» **se** nenhum leitor escreve. O 164 mediu que a trava é ficha de exclusão global e o estado mora no disco — medir antes quem escreve sob trava de leitura (cura de cabeçalho, `.pag`) | medir a latência de um `ler` durante o backup de 1 GB |
| C13: carimbo replicado sem teto | recusar (ou contar e gritar) carimbo acima de `ultimo + teto_de_salto`, como o `carimbos_do_futuro` já faz para o relógio | `carimbo`: release devolve 0 hoje |

### 8.2 Recusas

- **NÃO** ao previsor no lugar do conserto: a 16 MiB/s o tmpfs de 64 MiB encheu em 4,07 s; um job em fuga passa por
  baixo de qualquer amostra de 15 min. Previsão compra antecedência para o lento; o rápido é o conserto de §8.1.
- **NÃO** a repetir `fsync` até dar Ok, em forma nenhuma (10×0).
- **NÃO** a consertar C2a gravando o evento **antes** do `.reg` com uma operação nova no `.log`: binário velho trunca
  em silêncio em tag desconhecida (o 292 mediu: `de_tag` → `Err`, `curar` faz `break` e grava cabeçalho encurtado).
- **NÃO** a previsão virar portão por padrão. Recusar escrita por previsão só se alguém pedir, e só no limite duro
  que já existe.
- **NÃO** a quarto silêncio nem a segundo carteiro.

## 9. Hipóteses que morreram

| hipótese | morreu com |
|---|---|
| a réplica atrasada perde o diário que o mestre girou | o mestre nunca poda o `.log`; a catástrofe vira disco (C4), teto (C3) e RPO (C8) |
| disco cheio só recusa limpo, porque o `.reg` vem antes e falha primeiro | 512 KiB deu 40/40 limpos; a varredura deu 22 de 41 com erro sujo |
| a marca de sujo do `.ndx` cobre o disco cheio | cobre com um fecho; com dois — o caminho do servidor — baixa sobre a página perdida |
| na tabela paginada o teto que chega primeiro é o do `.reg` | o do diário chegou com o `.reg` a 0,15% |
| o ext4 sempre salva, virando só-leitura | 3/3 salva com metadado em buraco; 3/3 perde com provisionamento fino fiel |
| janela curta de regressão basta | rajada 2 s liga / 2 s desliga, 5 corridas cada: a janela de 4 amostras (menos de um ciclo) sai **otimista em 5/5**, de +22,1% a **+1.280,9%**; a de 16 (dois ciclos) **nunca** sai otimista, pior de −6,5% a −7,5% — o lado seguro |
| «o silêncio já é um só» (495 §6) | três escritas da mesma decisão |

## 10. O que não medi, e por quê

- **O servidor inteiro** sob `ENOSPC` e `fsync` recusado: provei o motor com os passos que o servidor dá
  (`gravar_de_verdade` sincroniza no mesmo punho; `descarregar_sujas_com` reabre e sincroniza). O `phxsqld` não rodou.
- **EIO de setor:** o núcleo deste contêiner não tem device-mapper; o EIO veio do provisionamento fino (loop sobre
  tmpfs cheio), que é o caso de 2018.
- **Descritores (C12):** o contêiner tem 20.000; o 1.024 do `systemd` não foi exercitado.
- **Backup** em disco frio e acima de 95 MB; **falha do backup agendado** lida, não exercitada.
- **Commit em transação** com a marca `.tx` quando o `.log` falha no meio; **órfã por `ENOSPC`**.
- **MariaDB:** `sys.schema_auto_increment_columns` e a retenção do binlog não conferidos (a KB redirecionou).

Cognições candidatas (nascem **PENDENTE**; quem as escreve é o integrador): o `fsync` repetido que mente, medido no
6.18; a lista de sujas esvaziada antes da gravação, que é a mesma doença dentro do nosso cache; a janela curta que
erra para o lado perigoso.

---

## Apêndice A — o arnês, para refazer

Crate fora do repositório, com dependência por caminho. Tudo com root; montagens só em `unshare -m`.

```bash
S=/um/scratch; mkdir -p $S/prova/src $S/p1 $S/back $S/ext $S/pv
# Cargo.toml: [package] name="prova496" edition="2021"; [dependencies] phxsql-core e phxsql-store por path; [workspace]
cd $S/prova && CARGO_PROFILE_DEV_DEBUG=0 /home/user/adrianoboller/phxsql/cargo-da-frente.sh build --offline --target-dir $S/target
#            (e --release para `cresce`, `backup` e `carimbo`)
$S/target/debug/prova496 teto  $S/p2/db            # §2.3
$S/target/debug/prova496 orfa  $S/orfa             # §2.3, a orfa
$S/target/release/prova496 cresce $S/cr 20000 3    # §2.4 (SEM_IMAGEM=1 para 44 B)
ZIP=1 $S/target/release/prova496 backup $S/bk $S/bkdest 400000   # §2.4 (sem ZIP = arvore)
$S/target/release/prova496 carimbo x               # §2.4 (debug: o panico)
TAM=512k unshare -m --propagation private bash p1c.sh             # §2.2 C2b, com o 2o fecho
SEM_SYNC=1 TAM=512k unshare -m --propagation private bash p1c.sh  # §2.2 C2b, so o Drop
for k in $(seq 480 4 640); do TAM=${k}k unshare -m --propagation private bash p1d.sh; done   # §2.2 C2a
unshare -m --propagation private python3 fsyncgate.py $S          # §2.1, o nucleo
unshare -m --propagation private bash p3b.sh $S                   # §2.1, o motor
unshare -m --propagation private python3 previsor.py $S/pv 8 0.5  # §6 (taxa MiB/s, amostra s); 4 e 16 tambem
for w in 4 8 16; do RAJADA=1 unshare -m --propagation private python3 previsor.py $S/pv 16 0.5 $w; done  # §9
```

`p1c.sh` e `p1d.sh` (com `S` e `P=$S/target/debug/prova496` definidos):

```bash
# p1c.sh
mount -t tmpfs -o size=$TAM tmpfs $S/p1
$P enospc $S/p1/db 20000 | grep -E "^enospc|^fecho|repetiu"
mount -o remount,size=64m $S/p1
$P conferir $S/p1/db
umount $S/p1
# p1d.sh
mount -t tmpfs -o size=$TAM tmpfs $S/p1
SEM_SYNC=1 $P enospc $S/p1/db 20000 | grep -E "^enospc: vivas|repetiu" | head -3
umount $S/p1
```

`p3b.sh` (provisionamento fino fiel: todo bloco existe, os livres do ext4 viram buraco):

```bash
set -e
S="$1"; P="${P:-$S/target/debug/prova496}"; IMG=$S/back/disco.img
mount -t tmpfs -o size=80m tmpfs $S/back
truncate -s 64M $IMG
mkfs.ext4 -q -F -N 64 -J size=4 $IMG
fallocate -l 64M $IMG
LOOP=$(losetup -f --show $IMG)
trap 'umount $S/ext 2>/dev/null; losetup -d $LOOP 2>/dev/null; umount $S/back 2>/dev/null' EXIT
mount -t ext4 $LOOP $S/ext
$P criar $S/ext/db
sync; umount $S/ext
for r in $(dumpe2fs $IMG 2>/dev/null | sed -n 's/^ *Free blocks: //p' | tr ',' '\n' | tr -d ' '); do
  [ -z "$r" ] && continue; a=${r%-*}; b=${r#*-}
  fallocate -p -o $((a*4096)) -l $(((b-a+1)*4096)) $IMG
done
LIVRE=$(df -k --output=avail $S/back | tail -1)
dd if=/dev/zero of=$S/back/enchimento bs=1k count=$((LIVRE-64)) 2>/dev/null || true
mount -t ext4 $LOOP $S/ext
$P inserir $S/ext/db ${N:-5000}
$P fecho $S/ext/db
rm -f $S/back/enchimento
$P fecho $S/ext/db
SO_LER=1 $P conferir $S/ext/db
umount $S/ext; mount -t ext4 $LOOP $S/ext
$P conferir $S/ext/db
```

`p3.sh` (o SO salva) é o `p3b.sh` com três trocas: tmpfs de `24m` em vez de `80m`; `mkfs.ext4 -q -F -N 64 -J size=4
-E lazy_itable_init=0,lazy_journal_init=0 $IMG`; e sem o `fallocate -l 64M` nem o laço dos buracos. O segundo
`fecho` morre em `EROFS` (3/3).

`fsyncgate.py` (o núcleo, sem o motor):

```python
import os, subprocess, sys, errno
S = sys.argv[1]; back, ext = f"{S}/back", f"{S}/ext"
def sh(c): return subprocess.run(c, shell=True, check=True, capture_output=True, text=True).stdout
sh(f"mount -t tmpfs -o size=24m tmpfs {back}"); sh(f"truncate -s 64M {back}/disco.img")
sh(f"mkfs.ext4 -q -F -N 64 -J size=4 -E lazy_itable_init=0,lazy_journal_init=0 {back}/disco.img")
loop = sh(f"losetup -f --show {back}/disco.img").strip()
try:
    sh(f"mount -t ext4 {loop} {ext}")
    cab = b"PHXREG" + bytes(4090)
    with open(f"{ext}/tabela.reg", "wb") as f: f.write(cab); f.flush(); os.fsync(f.fileno())
    os.sync(); livre = int(sh(f"df -k --output=avail {back} | tail -1"))
    sh(f"dd if=/dev/zero of={back}/enchimento bs=1k count={max(livre-64,0)} 2>/dev/null || true")
    dado = bytes((i * 7 + 13) % 251 for i in range(1024 * 1024))
    fd = os.open(f"{ext}/tabela.reg", os.O_WRONLY | os.O_APPEND); os.write(fd, dado)
    for rotulo in ("fsync 1", "fsync 2 (mesmo fd)"):
        try: os.fsync(fd); print(rotulo, "OK")
        except OSError as e: print(rotulo, "FALHOU", errno.errorcode.get(e.errno))
    os.close(fd); os.remove(f"{back}/enchimento")
    fd = os.open(f"{ext}/tabela.reg", os.O_RDWR)
    try: os.fsync(fd); print("fsync 3 (fd novo) OK")
    except OSError as e: print("fsync 3 FALHOU", e.errno)
    print("cache == escrito:", os.pread(fd, len(dado), 4096) == dado); os.close(fd)
    sh(f"umount {ext}"); sh(f"mount -t ext4 {loop} {ext}")
    lido = open(f"{ext}/tabela.reg", "rb").read()
    print("cabecalho sobreviveu:", lido[:4096] == cab, "| acrescentado no disco:", len(lido) - 4096, "bytes")
finally:
    subprocess.run(f"umount {ext}; losetup -d {loop}; umount {back}", shell=True)
```

`previsor.py` (a conta da fatia 1 contra um tmpfs enchendo; argumentos: diretório, MiB/s, segundos por amostra,
janela em amostras; `RAJADA=1` liga o escritor em rajada, 2 s escrevendo e 2 s parado):

```python
import subprocess, sys, threading, time
import os
D, TAXA, AMOSTRA = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
JANELA = int(sys.argv[4]) if len(sys.argv) > 4 else 8; RAJADA = bool(os.environ.get("RAJADA"))
subprocess.run(f"mount -t tmpfs -o size=64m tmpfs {D}", shell=True, check=True)
fim = {}
def escritor():
    bloco = b"x" * (256 * 1024)
    with open(f"{D}/cresce.log", "wb", buffering=0) as f:
        while True:
            try: f.write(bloco)
            except OSError: fim["t"] = time.monotonic(); return
            time.sleep(0.25 / TAXA)
            while RAJADA and int(time.monotonic() - t0) % 4 >= 2: time.sleep(0.05)
def livre_kb():
    return int(subprocess.run(["df", "-k", "--output=avail", D], capture_output=True, text=True).stdout.split()[-1])
def esgota_em(am, piso=0):
    n = len(am)
    if n < 4: return None
    mt = sum(t for t, _ in am) / n; ml = sum(l for _, l in am) / n
    sxx = sum((t - mt) ** 2 for t, _ in am); sxy = sum((t - mt) * (l - ml) for t, l in am)
    syy = sum((l - ml) ** 2 for _, l in am)
    if sxx == 0 or sxy >= 0 or not syy or sxy * sxy / (sxx * syy) < 0.6: return None
    b = sxy / sxx; return (piso - (ml - b * mt)) / b
t0 = time.monotonic(); th = threading.Thread(target=escritor); th.start(); am, prev = [], []
while "t" not in fim:
    t = time.monotonic() - t0; am.append((t, livre_kb()))
    p = esgota_em(am[-JANELA:])
    if p: prev.append((t, p))
    time.sleep(AMOSTRA)
th.join(); real = fim["t"] - t0; subprocess.run(f"umount {D}", shell=True)
erros = [abs(p - real) / real for _, p in prev]
print(f"ENOSPC em {real:.2f} s; 1a previsao em {prev[0][0]:.2f} s ({100*(real-prev[0][0])/real:.0f}% de antecedencia); "
      f"erro medio {100*sum(erros)/len(erros):.1f}%, pior {100*max(erros):.1f}%, "
      f"pior OTIMISTA {100*max((p - real) / real for _, p in prev):+.1f}%")
# a rajada da §9: RAJADA=1 ... previsor.py DIR 16 0.5 4   (e 8, 16)
```

`src/main.rs` (modos `enospc`, `teto`, `orfa`, `criar`, `inserir`, `fecho`, `conferir`, `cresce`, `backup`,
`carimbo`; o esquema é sempre `pedidos(id Int8 único, nome Str(60), cidade Str(40))`):

```rust
use phxsql_core::paginacao::Paginacao;
use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::log::Operacao;
use phxsql_store::{diario, Table};
use std::path::Path;

fn esquema(pag: Option<Paginacao>) -> Schema {
    let e = Schema::new("pedidos", vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("nome", ColumnType::Str(60)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(40)),
    ], vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()]).unwrap();
    match pag { Some(p) => e.com_paginacao(p).unwrap(), None => e }
}
fn linha(id: i64) -> Vec<Value> {
    vec![Value::Int(id), Value::Str(format!("cliente numero {id:08}")), Value::Str("Blumenau".into())]
}

/// O que a replica veria contra o que o `.reg` tem, e o estado do indice.
fn conferir(dir: &Path) {
    let cru = std::fs::read(dir.join("pedidos.ndx")).unwrap_or_default();
    println!("byte 52 no disco = {}", cru.get(52).copied().unwrap_or(255));
    let mut t = match Table::abrir(dir, "pedidos") { Ok(t) => t, Err(e) => return println!("NAO ABRE: {e}") };
    let ev = t.diario(0, 0).unwrap_or_default();
    let inc: std::collections::BTreeSet<u64> =
        ev.iter().filter(|e| e.operacao == Operacao::Inclusao).map(|e| e.rowid).collect();
    let linhas = t.varrer().unwrap_or_default();
    let sem: Vec<u64> = linhas.iter().map(|(r, _)| *r).filter(|r| !inc.contains(r)).collect();
    println!("vivas={} eventos={} sem_evento={} precisa_reconstruir={}",
        t.registros(), ev.len(), sem.len(), t.indice_precisa_reconstruir());
    println!("buscar -> {:?}", t.buscar("porId", &[Value::Int(5)]).map_err(|e| e.to_string()));
    if std::env::var("SO_LER").is_ok() { return; }
    println!("inserir -> {:?}", t.inserir(&linha(999_999)).map_err(|e| e.to_string()));
    println!("verificar -> {:?}", t.verificar().map(|r| (r.registros, r.eventos)).map_err(|e| e.to_string()));
}

/// Insere ate errar; cada erro diz se a linha FICOU no `.reg` sem evento no `.log`.
fn inserir_ate_errar(t: &mut Table, max: i64, parar: usize) -> (u64, u64) {
    let (mut limpos, mut sujos, mut seguidos) = (0u64, 0u64, 0usize);
    for id in 1..=max {
        let (v, e) = (t.registros(), t.eventos().unwrap_or(0));
        if t.inserir(&linha(id)).is_ok() { seguidos = 0; continue; }
        seguidos += 1;
        if t.registros() > v && t.eventos().unwrap_or(0) == e {
            sujos += 1;
            if sujos == 1 { println!("o cliente repetiu -> {:?}", t.inserir(&linha(id)).map_err(|e| e.to_string())); }
        } else { limpos += 1; }
        if seguidos >= parar { break; }
    }
    (limpos, sujos)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = Path::new(&a[2]);
    match a[1].as_str() {
        "enospc" => {
            let mut t = Table::criar(dir, esquema(None)).unwrap();
            t.ligar_imagem_no_diario(true);
            t.sincronizar().unwrap();
            let (l, s) = inserir_ate_errar(&mut t, a[3].parse().unwrap(), 40);
            println!("enospc: vivas={} eventos={} limpos={l} SUJOS={s}", t.registros(), t.eventos().unwrap_or(0));
            if std::env::var("SEM_SYNC").is_err() { println!("fecho -> {:?}", t.sincronizar().map_err(|e| e.to_string())); }
        }
        "teto" => {
            diario::definir_bytes_por_volume(diario::CORTE_MINIMO);
            let mut t = Table::criar(dir, esquema(Some(Paginacao::nova(1_000_000, 2).unwrap()))).unwrap();
            let (l, s) = inserir_ate_errar(&mut t, 5_000, 5);
            println!("teto: vivas={} eventos={} limpos={l} sujos={s}", t.registros(), t.eventos().unwrap_or(0));
            let r = t.buscar("porId", &[Value::Int(1)]).unwrap()[0];
            let mut n = linha(1); n[2] = Value::Str("Joinville".into());
            let res = t.atualizar(r, &n).map_err(|e| e.to_string());
            println!("atualizar -> {res:?}, cidade agora {:?}", t.ler(r).unwrap().unwrap()[2]);
            println!("verificar -> {:?}", t.verificar().map(|r| (r.registros, r.eventos)).map_err(|e| e.to_string()));
        }
        "orfa" => {
            use phxsql_core::schema::{AcaoRi, ForeignKey};
            diario::definir_bytes_por_volume(diario::CORTE_MINIMO);
            let em = Schema::new("clientes", vec![Column::new("id", ColumnType::Int4).obrigatoria(),
                Column::new("nome", ColumnType::Str(40))],
                vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()]).unwrap()
                .com_paginacao(Paginacao::nova(1_000_000, 2).unwrap()).unwrap();
            let mut mae = Table::criar(dir, em).unwrap();
            let ef = Schema::new("pedidos", vec![Column::new("id", ColumnType::Int4).obrigatoria(),
                Column::new("cliente_id", ColumnType::Int4)],
                vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
                     IndexDef::new("porCliente", vec![IndexColumn::asc(1)])]).unwrap()
                .com_chaves_estrangeiras(vec![ForeignKey::new("fk_cliente", vec![1], "clientes",
                    vec!["id".into()]).ao_alterar(AcaoRi::Cascata)]).unwrap();
            let mut filha = Table::criar(dir, ef).unwrap();
            let r = mae.inserir(&[Value::Int(1), Value::Str("Adriano".into())]).unwrap();
            mae.sincronizar().unwrap();
            filha.inserir(&[Value::Int(10), Value::Int(1)]).unwrap();
            filha.sincronizar().unwrap(); drop(filha);
            let mut n = 0;
            while mae.atualizar(r, &[Value::Int(1), Value::Str(format!("Adriano {n}"))]).is_ok() { n += 1; }
            let res = mae.atualizar(r, &[Value::Int(2), Value::Str("Adriano".into())]).map_err(|e| e.to_string());
            mae.sincronizar().ok();
            let id = mae.ler(r).unwrap().unwrap()[0].clone(); drop(mae);
            let aponta = Table::abrir(dir, "pedidos").unwrap().ler(1).unwrap().unwrap()[1].clone();
            let existe = !Table::abrir(dir, "clientes").unwrap().buscar("porId", &[Value::Int(1)]).unwrap().is_empty();
            println!("teto apos {n}; 1->2 -> {res:?}; mae id={id:?}; filha aponta {aponta:?}; mae 1 existe? {existe}");
        }
        "criar" => { let mut t = Table::criar(dir, esquema(None)).unwrap(); t.sincronizar().unwrap(); }
        "inserir" => {
            let mut t = Table::abrir(dir, "pedidos").unwrap();
            let n: i64 = a[3].parse().unwrap();
            let ok = (1..=n).filter(|&id| t.inserir(&linha(id)).is_ok()).count();
            println!("{ok} de {n} Ok ao cliente, sem fsync");
        }
        "fecho" => println!("fecho -> {:?}", Table::abrir(dir, "pedidos").unwrap().sincronizar().map_err(|e| e.to_string())),
        "conferir" => conferir(dir),
        "cresce" => {
            let (n, voltas): (i64, i64) = (a[3].parse().unwrap(), a[4].parse().unwrap());
            let mut t = Table::criar(dir, esquema(None)).unwrap();
            t.ligar_imagem_no_diario(std::env::var("SEM_IMAGEM").is_err());
            let tam = |x: &str| std::fs::metadata(dir.join(format!("pedidos.{x}"))).map(|m| m.len()).unwrap_or(0);
            for id in 1..=n { t.inserir(&linha(id)).unwrap(); }
            t.sincronizar().unwrap(); println!("reg={} log={}", tam("reg"), tam("log"));
            for v in 1..=voltas {
                for id in 1..=n { let mut l = linha(id); l[2] = Value::Str(format!("cidade{v}")); t.atualizar(id as u64, &l).unwrap(); }
                t.sincronizar().unwrap(); println!("volta {v}: reg={} log={}", tam("reg"), tam("log"));
            }
        }
        "backup" => {
            let (destino, n): (&Path, i64) = (Path::new(&a[3]), a[4].parse().unwrap());
            let mut t = Table::criar(dir.join("loja"), esquema(None)).unwrap();
            for id in 1..=n { t.inserir(&linha(id)).unwrap(); }
            t.sincronizar().unwrap(); drop(t);
            let t0 = std::time::Instant::now();
            let r = if std::env::var("ZIP").is_ok() {
                std::fs::create_dir_all(destino).unwrap();
                phxsql_store::backup::executar_zip(dir, destino, "", "admin", 0).unwrap().1
            } else { phxsql_store::backup::executar(dir, destino, 0).unwrap() };
            let s = t0.elapsed().as_secs_f64();
            println!("{} bytes em {s:.3} s = {:.1} MB/s", r.bytes, r.bytes as f64 / s / 1e6);
        }
        "carimbo" => {
            phxsql_store::no::empurrar_carimbo(u64::MAX - 1);
            let v: Vec<u64> = (0..3).map(|_| phxsql_store::no::proximo_carimbo()).collect();
            println!("{v:?}");
        }
        _ => panic!("modo?"),
    }
}
```

Fontes primárias lidas (baixadas nesta rodada): PostgreSQL `routine-vacuuming`, `runtime-config-error-handling`,
`runtime-config-replication`, `view-pg-replication-slots`, `monitoring-stats`; MySQL 8.0
`storage/innobase/os/os0file.cc`, `mysql-tips` (`--safe-updates`), `sys-schema-auto-increment-columns`,
`replication-options-binary-log`, `show-replica-status`; MariaDB 11.4 `storage/innobase/os/os0file.cc`, KB
`server-system-variables` (`sql_safe_updates`), `show-replica-status`; SQLite `src/os_unix.c` (master).
