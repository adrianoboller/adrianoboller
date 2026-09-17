# QA — onda 3, «Continue fazendo os gaps» (17/09/2026, ~03:58–04:30 UTC)

Papel G (QA), frente G2 da onda 3 da rodada «Replicação — bateria, revisão e
conclusão». Ordem do dono às 03:58 UTC. Fontes lidas: `docs/pmo/RODADA-2026-09-17-replicacao.md`
(linha de G da onda 1 e «Retorno da onda 2»), `docs/propostas/inventario-qa-replicacao-2026-09-17.md`,
`bancada/guardas/LEIA-ME.md`, `catalogo.py`, `provar-guardas.py`, `trecho-vivo.py`,
`tabela-no-testes.py`, e os `.json`/`.log` de
`$(ls -d /tmp/claude-0/-home-user-adrianoboller/*/scratchpad)/guardas-17-09/`.

**Regra desta frente, cumprida à risca**: nenhum `cargo`, nenhum
`provar-guardas.py` rodado. Todas as provas abaixo são Python puro
(`trecho-vivo.py --catraca`/`--autoteste`, e os dois autotestes novos do
`provar-guardas.py`), como o item de regras autorizava.

**Aviso que atravessa todo este parecer**: a frente B2 esteve mutando
`crates/` (`bidirecional.rs`, `blacklist.rs`, `config.rs`, `mensagens.rs`,
`replica.rs`, `servidor.rs`, `table.rs`, `paginacao.rs` — 976 linhas só em
`servidor.rs`, medido por `git diff --numstat` às 04:26 UTC) **durante** esta
frente. Uma das minhas âncoras (item 4, guarda 7) ficou desatualizada
*enquanto eu escrevia o parecer* — ver §4. **Antes de rodar o provador, rode
`python3 bancada/guardas/trecho-vivo.py --catraca` mais uma vez**: se B2
continuou mexendo depois deste parecer, pode haver uma nova âncora morta que
esta frente não viu.

---

## 1. `trava-atras-da-rede` reancorada (item 1)

O commit `49a3af7` (papel B, item 5 do contrato — a conferência de
continuidade pedida pelo papel C) reescreveu `alcancar_tabela` inteira:
ganhou `abrir_para_replicar`, a checagem de continuidade por
`diario_local_continua`, e o `puxar` do laço passou a começar em
`posicao - 1` (o evento de conferência). O ponto onde a trava podia voltar a
prender uma leitura de rede continua sendo o mesmo — o `crate::replica::puxar`
dentro do `while` —, só que o texto ao redor mudou byte a byte.

Reancorei o `trecho`/`troca` da entrada `trava-atras-da-rede` só nesse trecho
(não na função inteira, que agora faz coisas que não são o defeito desta
guarda): a mutação toma `self.travar_dados()` **antes** do `puxar` e a solta
**depois**, exatamente o defeito original — a trava presa atrás de uma ida e
volta de rede —, sem tocar no resto do laço (que continua chamando
`aplicar_lote_da_replica`, já fora da trava emprestada). Se eu tivesse
prendido a trava por cima da chamada a `aplicar_lote_da_replica` também, a
mutação teria disparado a detecção de reentrância do `RwLock`
(`trava_reentrante()`) em vez de pendurar — um defeito diferente do que a
guarda prova.

`caem`/`seguem` não mudaram (`source_mudo_nao_prende_a_trava_de_dados` e
`com_a_rede_sa_a_replica_conversa_e_o_servidor_atende` continuam existindo
com os mesmos nomes em `crates/phxsql-server/tests/trava-atras-da-rede.rs`).

**Prova estática** (não substitui o provador): `python3 bancada/guardas/trecho-vivo.py --catraca`
voltou a `TETO_TRECHO_MORTO: 0` (estava `1`, com a entrada nomeada). Veredito
da mutação em si: **NÃO JULGADA** até A rodar `--so trava-atras-da-rede`.

---

## 2. O provador não copiava o que os testes liam fora de `crates/` (item 2)

### O achado, medido

Das cinco guardas «sem veredito por defeito do provador», as cinco falhavam
pelo **mesmo** motivo, confirmado lendo o `.log` de `colisao-de-sequence-calada`
(`$(ls -d /tmp/claude-0/-home-user-adrianoboller/*/scratchpad)/guardas-17-09/colisao-de-sequence-calada.log`):
a árvore LIMPA da cópia reprovava em
`segredos::testes::todo_parametro_com_cara_de_segredo_esta_na_lista`
(**49,1 s**, 1130 testes do `--lib`, 1 falhou) porque esse teste lê
`bancada/guardas/debug-com-segredo.py` por
`CARGO_MANIFEST_DIR` + `"../../bancada/guardas/debug-com-segredo.py"`, e a
lista `COPIAR` do `provar-guardas.py` nunca levava `bancada/` — o diagnóstico
do integrador estava certo.

Antes de acrescentar só essa linha (a correção óbvia, e a terceira vez que
esta casa pagaria por essa mesma classe de falta — `docs/ROTEIRO-1.0.md` e
`testes-web/botoes-exercitados.txt` já tinham passado por isto), rodei um
grep completo de `CARGO_MANIFEST_DIR` em `crates/**/*.rs`: **15 ocorrências em
10 arquivos**. Duas liam fora do crate um caminho que `COPIAR` também não
cobria — `crates/phxsql-server/tests/catraca-do-mapa-das-threads.rs`, que
roda `python3 bancada/concorrencia/mapa-das-threads.py --catraca` como
subprocesso — um segundo buraco que ainda não tinha doído porque nenhuma
entrada do catálogo usa esse alvo `--test` hoje. Duas outras (em
`examples/`) apontam para `bancada/colmeia` e `bancada/vetorial`, e ficaram
de fora de propósito: `cargo test --lib`/`--test X` nunca compila
`examples/`.

### O conserto

`bancada/guardas/provar-guardas.py`:

- `COPIAR` ganhou os dois arquivos que faltavam (não a pasta `bancada/`
  inteira — **2,6 GiB** medidos agora por `du -sh bancada`, contra os 5 MB
  que o comentário de cima promete).
- `_sincronizar` ganhou um `os.makedirs(os.path.dirname(destino), ...)` antes
  de copiar um item que é ARQUIVO: os dois novos moram em subpastas
  (`bancada/guardas/`, `bancada/concorrencia/`) que a cópia nunca tinha
  criado, e sem isto o `shutil.copy` reprovaria com "No such file or
  directory" na primeira cópia nova.
- `verificar_copiar()` (com `_helpers_de_raiz`, `_leituras_fora_do_crate`,
  `_fontes_em_escopo`): varre `crates/*/{src,tests}/**/*.rs` — não
  `examples/` nem `build.rs`, que o provador nunca compila — atrás de todo
  `CARGO_MANIFEST_DIR` que sobe para fora do próprio crate (duas formas: a
  cadeia inline `.join("../../X")`, e o idioma desta casa `fn raiz*() ->
  PathBuf` com `.parent()` encadeado ou `.join("..").join("..")`), e confere
  que o caminho lido está coberto por `COPIAR`. `--conferir-copiar` roda e
  imprime o veredito; `--autoteste-copiar` é a prova real.

### Prova real (rodada agora, Python puro)

```
$ python3 bancada/guardas/provar-guardas.py --conferir-copiar
COPIAR cobre tudo que crates/*/{src,tests} le fora do proprio crate.

$ python3 bancada/guardas/provar-guardas.py --autoteste-copiar
=== autoteste do --conferir-copiar (pedido G, onda 3, 17/09/2026) ===
   ok    sem o arquivo em COPIAR, o conferidor reprova nomeando os dois
   ok    com a lista de hoje, nao sobra nenhuma leitura sem cobertura
   todos passaram
```

Contra a árvore de VERDADE, não uma sintética: o primeiro caso reproduz o
defeito de hoje (tira `bancada/guardas/debug-com-segredo.py` de `COPIAR` e
confere que o conferidor nomeia exatamente `segredos.rs` e o caminho), o
segundo confere que a lista de hoje fecha tudo.

Cognição nova: `docs/cognicao/cognicao_copiar-e-lista-que-precisa-de-conferidor_20260917_0426.md`
— o alcance é que a TERCEIRA falta da mesma classe («lista da qual um gerador
depende envelhece») não se resolve com uma quarta linha, e sim com um
conferidor que varre o código e prova que a lista está completa.

---

## 3. `--so … --json` deixava de mesclar (item 3)

### O desenho

Segui o precedente do `bancada/replicacao/achados-do-dba.py` (mescla por
nome, campo `preservados_de_corrida_anterior`), adaptado para até 187 ids em
vez de três nomes fixos, e para a disciplina da página de testes («cada
número traz a data em que foi medido»):

- `_gravar_json(caminho, so_ligado, resultados)`: com `--so` **e** um
  `--json` alvo que já existe, mescla por `id` — as guardas que a corrida
  tocou levam o veredito novo e o `quando` de agora; as que não tocou ficam
  como estavam. **Sem** `--so`, ou sem arquivo anterior, grava do zero, como
  sempre — uma corrida completa é o retrato inteiro, e não há nada para
  preservar.
- Cada entrada ganhou o próprio `quando` (chave nova; um arquivo de antes
  desta mudança, como o `ultima-corrida.json` de 16/09, não tem esse campo
  por entrada — a migração herda o `quando` do topo do arquivo antigo, em vez
  de inventar "agora" para uma entrada que não foi medida agora).
- O `quando` do TOPO deixa de ser sempre "agora": passa a ser o **mais
  antigo** entre as entradas que sobrevivem no arquivo — é a leitura que não
  superestima o retrato: se uma entrada de 07/09 continua ali, o topo não
  pode dizer que tudo é de hoje.
- `main()` chama `_gravar_json(opc.json, bool(opc.so), resultados)` e imprime
  quantas entradas foram preservadas e o `quando` do topo.

### Prova real, nos dois sentidos (autoteste em Python, sem cargo)

```
$ python3 bancada/guardas/provar-guardas.py --autoteste-mescla-json
=== autoteste da mescla do --json com --so (pedido G, onda 3, 17/09/2026) ===
   ok    corrida completa grava as duas entradas
   ok    --so preserva a entrada que nao rodou
   ok    a preservada mantem o veredito antigo
   ok    a que rodou de novo leva o veredito novo
   ok    o retorno nomeia quem foi preservado
   ok    o `quando` do topo e' o da entrada preservada (mais antigo)
   ok    corrida completa seguinte volta a ser so o que ela mediu
   ok    --so sem arquivo anterior grava normalmente
   todos passaram
```

Os oito casos cobrem os dois sentidos exigidos: uma corrida `--so` não pode
apagar o que não tocou (defeito reposto: sobrescrever apagava as outras 142+
guardas), e uma corrida COMPLETA continua sobrescrevendo por inteiro — não
pode arrastar preservados de uma rodada anterior para sempre.

---

## 4. As sete pétreas sem guarda + `cluster.rs` (item 4)

Sete entradas novas em `catalogo.py`, uma por pétrea nomeada em
`docs/propostas/inventario-qa-replicacao-2026-09-17.md` §2.2 (pedidos
211/214/217). `crates/phxsql-server/src/cluster.rs`, que tinha **zero**
entradas no catálogo, ganhou a primeira.

| id | pétrea | arquivo | teste que cai (`caem`) | seguem (comportamento velho / vizinho) |
|---|---|---|---|---|
| `replica-lista-e-pedida-nao-imposta` | `replicas_autorizadas` vazia libera (guarda pedida, não imposta) | `servidor.rs`, portão 2a-bis | `servidor::testes_papel::replica_de_fora_da_lista_nao_le_o_diario` | `sem_replicas_autorizadas_nada_muda`, `caminho_interno_sem_ip_nao_e_barrado_pela_lista`, `a_replicacao_aberta_se_anuncia_e_some_quando_a_lista_enche` |
| `posicao-nao-encolhe-em-silencio` | posição do diário não encolhe calada (pedido 211) | `servidor.rs`, `posicao_do_diario` | `servidor::testes_posicao_do_diario::tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio` | — |
| `eleicao-prefere-completa` | eleição prefere posição COMPLETA (pedido 211) | `cluster.rs`, `vencedor` | `cluster::testes::eleicao_prefere_completa_a_incompleta` | `cluster::testes::com_maioria_vence_a_maior_posicao` |
| `replica-nao-atende-escrita` | réplica não atende escrita, por PAPEL (pedido 214(c)) | `servidor.rs`, portão 2b-bis | `aplicar_num_source_trancado_por_administracao_e_recusado`, `aplicar_num_source_aberto_tambem_e_recusado` | `replica_trancada_continua_aceitando_o_diario_do_source`, `replica_destrancada_continua_aceitando_o_diario_do_source` |
| `spare-nao-atende-ninguem` | spare não atende ninguém (modo C, pedido 214) | `servidor.rs`, `OPS_NO_SPARE` | `servidor::testes_papel::spare_nao_atende_cliente_nem_de_leitura` | `spare_promover_vira_primario_e_abre_a_escrita`, `read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura` |
| `read-replica-recusa-escrita` | read replica recusa escrita apontando o master (modo D, pedido 214) | `servidor.rs`, match do papel | `servidor::testes_papel::read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura` | `spare_nao_atende_cliente_nem_de_leitura` |
| `pulso-fora-da-lista-e-recusado` | pulso de id fora da lista é recusado, e nó novo não precisa reiniciar (pedido 217) | `servidor.rs`, `op_cluster_pulso` | `servidor::testes_config_gravar::no_acrescentado_a_quente_passa_a_ser_aceito_no_pulso` | `origem_do_cluster_carrega_a_cifra_e_o_pino` |

(os `caem`/`seguem` acima estão sem o prefixo `servidor::testes_papel::` na
tabela por espaço; o `catalogo.py` grava o caminho completo, exigido pela
sexta régua, `TETO_TESTE_SEM_MODULO`.)

### O que mudou de âncora ENQUANTO esta frente rodava

`pulso-fora-da-lista-e-recusado` foi escrita contra o texto que eu tinha lido
minutos antes:

```rust
if estado.no(&id).is_none() {
    return Err(PhxError::Autorizacao(format!(
        "o no {id:?} nao esta na lista de nos deste cluster"
    )));
}
if id == estado.config.id {
    return Err(PhxError::Esquema(format!(
        "o no {id:?} e ESTE servidor -- dois nos com o mesmo id no ar"
    )));
}
```

`trecho-vivo.py --catraca` acusou `TETO_TRECHO_MORTO: 1` na primeira
conferência: a frente B2 tinha, no mesmo instante, fundido as duas recusas
numa condição só (revisão SEC, achado A11 — duas frases distintas faziam do
pulso um "oráculo de ids"):

```rust
if estado.no(&id).is_none() || id == estado.config.id {
    if id == estado.config.id { eprintln!(...); }
    if self.config.politica.contar_pulso_desconhecido && !sessao.ip.is_empty() { ... }
    return Err(PhxError::Autorizacao(...));
}
```

Reancorei na hora: o `trecho`/`troca` agora tocam só a condição
(`if estado.no(&id).is_none() || id == estado.config.id {` vira
`if id == estado.config.id {`), preservando a detecção de id duplicado que a
mesma linha carrega. É exatamente a lição do item 1, num arquivo diferente,
na mesma madrugada — e a prova de que B2 seguia mutando `crates/` **depois**
de eu ter lido o código pela primeira vez.

### Prova estática, e o piso

```
$ python3 bancada/guardas/trecho-vivo.py --catraca
   187 guardas no catalogo
   a ultima corrida (2026-09-16 15:25) julgou 143 das 187 entradas
   -- 44 sem veredito nessa corrida: 44 nomeadas na tabela publicada, 0 escondidas
   ok  TETO_TRECHO_MORTO: 0 (teto 0)
   ok  TETO_TRECHO_AMBIGUO: 0 (teto 0)
   ok  TETO_TESTE_MORTO: 0 (teto 0)
   ok  TETO_TESTE_FORA_DO_BINARIO: 0 (teto 0)
   ok  TETO_TESTE_SEM_MODULO: 0 (teto 0)
   ok  TETO_NAO_JULGADA_ESCONDIDA: 0 (teto 0)
   ok  PISO_DAS_ENTRADAS: 187 (piso 187)
```

`PISO_DAS_ENTRADAS` subiu de 180 para **187** (as sete novas), documentado no
próprio `trecho-vivo.py` no molde das subidas anteriores (170→177→187), com
o motivo e os sete ids. `TETO_NAO_JULGADA_ESCONDIDA` — que tinha subido para
`7` assim que as entradas novas entraram — voltei a `0` republicando a MESMA
corrida (`python3 bancada/guardas/tabela-no-testes.py bancada/guardas/ultima-corrida.json`,
0,2 s, sem prova nova e sem data nova): `docs/TESTES.md` agora nomeia as 44
entradas que a corrida de 16/09 15:25 não julgou (eram 37), sob o mesmo
aviso de sempre — nenhuma delas está escondida.

**As sete entradas, e a reancoragem de `pulso-fora-da-lista-e-recusado`,
estão NÃO JULGADAS até A rodar os `--so` da lista abaixo.**

---

## 5. A lista de `--so` para A rodar, em ordem de valor

Perda de dado e integridade primeiro, depois eleição/disponibilidade do
cluster, depois confidencialidade, depois indisponibilidade operacional —
mesmo critério do inventário de ontem. Tempo estimado: medido quando existe
registro de hoje ou de 16/09 para a mesma entrada/pacote-alvo; senão, a faixa
das vizinhas do mesmo pacote/alvo (`phxsql-server --lib`, 20–49 s, medido
agora em `colisao-de-sequence-calada.log`: a árvore limpa sozinha já leva
49,1 s com o binário reaproveitado).

```
python3 bancada/guardas/provar-guardas.py --so colisao-de-sequence-calada          # ~34 s  — perda de dado (autonumber, duas origens) — PROVADA esperada (só destravada pelo COPIAR novo)
python3 bancada/guardas/provar-guardas.py --so replica-nao-atende-escrita          # NOVA   — regra primordial: mata o pai com filhos pela rede
python3 bancada/guardas/provar-guardas.py --so eleicao-prefere-completa            # NOVA   — promove no incompleto, cluster.rs (0 entradas antes)
python3 bancada/guardas/provar-guardas.py --so posicao-nao-encolhe-em-silencio     # NOVA   — alimenta a de cima: o silencio que a bandeira `incompleta` fecha
python3 bancada/guardas/provar-guardas.py --so pulso-fora-da-lista-e-recusado      # NOVA   — REANCORADA hoje (B2, A11); no fantasma infla a maioria
python3 bancada/guardas/provar-guardas.py --so spare-nao-atende-ninguem            # NOVA   — dado velho servido como se fosse atual
python3 bancada/guardas/provar-guardas.py --so read-replica-recusa-escrita         # NOVA   — escrita indevida numa replica de leitura
python3 bancada/guardas/provar-guardas.py --so replica-lista-e-pedida-nao-imposta  # NOVA   — confidencialidade: IP fora da lista le o diario
python3 bancada/guardas/provar-guardas.py --so posicao-sem-portao                  # ~38 s  — vazamento de esquema/eventos de tabela negada — PROVADA esperada
python3 bancada/guardas/provar-guardas.py --so replicacao-do-cluster-em-claro      # ~38 s  — confidencialidade da replicacao entre nos — PROVADA esperada
python3 bancada/guardas/provar-guardas.py --so trava-atras-da-rede                 # ~17 s  — disponibilidade — REANCORADA hoje (B, item 5/continuidade)
python3 bancada/guardas/provar-guardas.py --so replica-insiste-na-credencial-recusada  # ~32 s — autobloqueio operacional — PROVADA esperada
python3 bancada/guardas/provar-guardas.py --so cluster-devolve-a-credencial-na-tela    # sem estimativa — NAO JULGADA desde 17/09, mesma familia (20-38 s)
```

Rode com `--json bancada/guardas/ultima-corrida.json` **de uma vez só** (os
13 num `--so` só, repetido 13 vezes, ou um `--so` por padrão que os case
todos) para aproveitar a mescla nova do item 3 — o arquivo vai ganhar
`quando` por entrada e vai preservar as 143 já julgadas em 16/09; **não**
rode cada `--so` com um `--json` PRÓPRIO e junte à mão depois, porque aí a
mescla de cada chamada mescla só contra o resultado da chamada anterior, e o
`quando` do topo vai oscilar sem necessidade.

Depois: `python3 bancada/guardas/tabela-no-testes.py bancada/guardas/ultima-corrida.json`
para publicar os vereditos novos em `docs/TESTES.md`.

**Antes de tudo isso**, com B2 possivelmente ainda mutando `crates/`:
`python3 bancada/guardas/trecho-vivo.py --catraca` — se voltar
`TETO_TRECHO_MORTO` diferente de 0, alguma âncora desta frente (ou de outra)
envelheceu depois deste parecer, e o provador vai devolver `QUEBRADA`
dizendo qual.

---

## 6. O que NÃO consegui, e por quê

- **Não rodei o provador em nenhum momento** (regra desta frente — B2
  mutando `crates/`), então nenhuma das 13 entradas acima tem veredito real
  ainda. Tudo o que afirmo sobre elas é: o `trecho` existe uma única vez no
  código de agora, o `troca` compila conceitualmente (li a função inteira ao
  redor), e o teste nomeado existe no binário certo — não é a mesma coisa que
  "a mutação derruba o teste".
- **Não posso garantir que nada mais envelheceu depois do meu último
  `trecho-vivo.py --catraca`** (04:xx UTC) — B2 seguia com `servidor.rs` em
  diff de quase mil linhas. Por isso a recomendação de rodar a régua de novo
  antes do provador, no §5.
- **`TETO_DO_LOTE_SERVIDO`/`TETO_DA_RESPOSTA` continuam sem prova nenhuma**
  (§2.2 do inventário de ontem, último item da tabela) — não escrevi guarda
  para eles nesta frente porque não achei NENHUM teste que sirva de `caem`
  (nem unitário, nem de bancada acessível ao provador): escrever a entrada
  sem um teste que caia seria "entrada remendada no chute", pior que a
  omissão. Fica nomeado para quem escrever esse teste primeiro.
- **Não conferi as 143 entradas já provadas em 16/09** continuam provadas
  hoje — só `TETO_TRECHO_MORTO`/`TETO_TESTE_MORTO`/etc. (texto puro) contra
  todo o catálogo, que voltou `0` em todas as réguas. Trecho vivo não é
  mutação: só o provador prova que repor o defeito ainda derruba o teste.

## Arquivos

- `bancada/guardas/catalogo.py` — reancoragem de `trava-atras-da-rede` e de
  `pulso-fora-da-lista-e-recusado` (achada mid-frente), sete entradas novas
  (181–187).
- `bancada/guardas/provar-guardas.py` — `COPIAR` com os dois arquivos que
  faltavam, `_sincronizar` com `makedirs`, `verificar_copiar()` +
  `--conferir-copiar`/`--autoteste-copiar`, `_gravar_json()` +
  `--autoteste-mescla-json`.
- `bancada/guardas/trecho-vivo.py` — `PISO_DAS_ENTRADAS` 180 → 187, com o
  parágrafo do motivo.
- `docs/TESTES.md` — republicado pelo `tabela-no-testes.py` (143/187, 44
  nomeadas, 0 escondidas).
- `docs/cognicao/cognicao_copiar-e-lista-que-precisa-de-conferidor_20260917_0426.md`.
- Este parecer: `docs/propostas/qa-onda3-replicacao-2026-09-17.md`.
