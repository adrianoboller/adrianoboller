# Parecer QA — auditoria da rodada de 24/09/2026 (papel G)

Auditoria só de leitura + catracas rodadas. Nenhum arquivo do repositório foi
alterado por este parecer. Árvore auditada: HEAD `a494f33` (18:30, 116 commits
de hoje) **e** a árvore de trabalho, que estava suja no momento da auditoria
(ver §5).

## 1. Nenhuma catraca subiu?

| Fonte | Achado |
|---|---|
| `TETO_*` do Rust (todo `crates/**/*.rs`, hoje) | **Nenhum TETO subiu.** 35 `TETO_*` novos apareceram hoje — todos constantes **novas**, não alterações de valor. A única linha removida com `const TETO_…` foi `TETO_DO_VALOR: usize = 48` em `error.rs`, consolidada em `TETO_DA_CITACAO` (mesmo valor, 48→48) — dedup, não ratchet. |
| `TETO_FSYNC_POR_FECHO_V2` (`phxsql-store/src/conferidor_fsync.rs`) | Não tocado hoje (última mudança 16/09, `b298b9a`). É o exemplo correto de aposentadoria já registrado no fonte (`V1=7` aposentada, `V2=8` nasce medida) — citado aqui só como referência, fora do escopo de hoje. |
| `bancada/catracas/`, `bancada/guardas/trecho-vivo.py`, `bancada/concorrencia/mapa-da-trava.py` | `mapa-da-trava.py` não foi tocado hoje. `catracas/` só recebeu mudança de ferramentaria (portão único de commit, 3 commits da manhã) — sem número de teto. |
| `PISO_DAS_ENTRADAS` (piso, sobe por desenho) | Subiu monotonicamente o dia inteiro: 204→213→222→228→236→242→249→265→270→271→273→286→297→309→318→325→329→345→352→368→374→386→**401** (HEAD). Toda subida tem linha de história nomeando o pedido e a conta do `--catraca`. Autoconferido: `python3 bancada/guardas/trecho-vivo.py --catraca` → `400 guardas + 1 aposentada = piso 401`, **ok**. |

**Achado B1 (severidade BAIXA, documentação):** o changelog de `trecho-vivo.py`
narra a contribuição do pedido 497 **duas vezes** com números diferentes e não
reconciliados: uma vez integrada (linha "SUBIU para 345 … pedido 497,
integrado sobre o 514") e depois, mais abaixo no arquivo, uma segunda
narrativa isolada ("SUBIU para 300 / 304 / 306 … pedido 497") que é a medida
**da árvore isolada do 497 antes da integração** (o mesmo padrão legítimo que
"SUBIU para 316" documenta explicitamente como "nesta árvore" para o
509+512). A diferença é que o bloco do 497 não diz "nesta árvore" nem aponta
para a linha combinada — fica parecendo uma queda de 401 para 300 a quem lê
em ordem. **Não é violação de catraca** (o valor de `PISO_DAS_ENTRADAS`
gravado nunca caiu — cada subida real, oldest→newest, é estritamente
crescente) — é um defeito de **auditabilidade da narrativa**: o próximo
integrador não consegue confiar no texto como linha do tempo sem recontar. Não
bloqueia; registrar como pedido de limpeza de doc (papel H/integrador).

## 2. Guardas novas de hoje

Comparado com `a272d8f` (fim de 23/09) até HEAD `a494f33`:

- **197 ids genuinamente novos** em `catalogo.py` (203→400 entradas; as 16
  linhas que aparecem como "-" no diff são as mesmas 16 ids reaparecendo nas
  213 linhas "+" — reordenação de bloco, não remoção). **0 removidos.**
  `PISO_DAS_ENTRADAS` bateu exato: 203+197=400, +1 aposentada = 401.
- **197/197 têm `porque`** nomeando o defeito. **195/197** nomeiam um número
  de pedido explícito no bloco (id+porque+troca); as 2 exceções
  (`aresta-velha-depois-do-savepoint`, `corrente-do-ciclo-atravessa-quem-nao-confirma`)
  nomeiam a condição do parecer do DBA ("R1 da re-checagem do papel C") em vez
  de um número de pedido — o defeito está nomeado e é rastreável ao parecer,
  então cumpre a lei ("cada guarda registra o defeito que a motivou"), mas sem
  o número cravado no texto.
- **Trecho único e vivo**: confirmado pela própria régua —
  `TETO_TRECHO_MORTO=0`, `TETO_TRECHO_AMBIGUO=0`, `TETO_TESTE_MORTO=0`,
  `TETO_TESTE_FORA_DO_BINARIO=0`, `TETO_TESTE_SEM_MODULO=0`, todos **ok**
  contra as 400 entradas de hoje.
- **`docs/TESTES.md` nomeia, não esconde**: `TETO_NAO_JULGADA_ESCONDIDA=0`
  **ok** — a última corrida do provador é de 16/09 (antes das 197 guardas de
  hoje), e a tabela publicada diz explicitamente **"Esta rodada NÃO julgou 76
  das 400 entradas do catálogo"**, nomeando as 76 uma a uma em vez de omiti-las.

## 3. Pétreas tocadas hoje × guarda que protege

| Pétrea tocada | Guarda(s) de hoje | Cobertura |
|---|---|---|
| Regra primordial da integridade / FK (nunca mata pai com filhos) | `fk-antes-do-default(-pelo-servidor)`, `cascata-sobre-calculada-na-declaracao`, `cascata-confere-a-filha-crua`, `auto-referencia-pulada-no-excluir(-pelo-servidor)`, `auto-laco-conta-como-filha`, `renomear-pula-a-auto-referencia`, `marca-do-disco-no-empilhar`, `upsert-solto-ressuscita-a-excluida`, `mescla-do-upsert-sobre-o-disco`, `elo-do-empilhar-pelo-disco`, `elo-implicito-sem-trava`, `ciclo-de-commits-sem-desempate`, `quem-cede-no-ciclo-segura-as-travas`, `aresta-velha-depois-do-savepoint`, `corrente-do-ciclo-atravessa-quem-nao-confirma`, `cascata-em-voo-ignorada-no-drop`, `cascata-em-voo-so-no-aplicar` | **Forte.** 17 guardas novas só hoje (lote 448, 514, integridade na transação 491/492/515/516/490). |
| Senha nunca em texto puro | `senha-sobra-no-erro-do-cadastro`, `senha-fora-de-aspas-simples-no-perfil`, `senha-depois-de-identified`, `parametros-irmaos-da-senha`, `portao-da-senha-por-espaco`, `portao-da-senha-pelos-simbolos`, `palavra-que-contem-a-senha`, `eco-do-sql-com-a-senha`, `jobs-json-antigo-derruba-o-arranque`, `job-recusado-roda-mesmo-assim`, `ficha-do-job-devolve-a-senha-do-disco`, `core-leva-a-senha-do-cofre`, `conferir-sem-o-teto-da-senha`, `criar-usuario-sem-o-teto-da-senha`, `login-sem-o-teto-da-senha` | **Forte.** Lote 497 (4 voltas + bloqueio SEC) inteiro dedicado a esta pétrea. |
| Redigir analisando, nunca recortando | `eco-do-sql-com-a-senha` (op `sql` ecoava o texto cru em vez de passar por `sem_a_senha_se_mencionada`) | **Coberta.** É o achado mais direto da pétrea hoje: o motor de redação (`redigir`) tinha um vizinho (op `sql`) que não passava por ele. |
| `fsync` sob a trava global | `fechar-baixa-o-byte-52-sem-fsync`, `atestado-sobrevive-a-escrita`, `atestado-pelo-caminho-e-nao-pelo-arquivo`, `atestado-de-antes-da-recusa-vale-depois`, `fts-fora-do-fecho-da-janela`, `fsync-recusado-repete-no-diario`, `fsync-recusado-repete-no-indice`, `drop-baixa-o-byte-52-depois-do-fsync-recusado`, `pagina-que-o-disco-recusou-sai-das-sujas`, `servidor-segue-de-pe-depois-do-fsync-recusado`, `panico-sob-a-trava-sem-reparo`, `reparo-da-trava-sem-as-marcas-orfas`, `job-corre-na-thread-de-servico`, `backup-corre-na-thread-de-servico` | **Forte, com gap ADMITIDO e rastreado, não escondido**: pedidos **533** (subida do byte 52 sem `fsync`), **534** (estado do cluster sem `fsync`) e **535** (posição do bidirecional antes do dado) foram julgados pelo PhxJev hoje e **ficam** em `PENDENCIAS.md` como `☐` — são gaps de durabilidade reais, sem guarda ainda porque o código ainda não foi corrigido. Isto é o comportamento certo (achado registrado, não maquiado), não uma guarda faltando por descuido. |
| Função e comando não se duplicam | Guardas pré-existentes de hoje cedo (`Canal::ler_decidindo`, pedido 442, ~01:00–02:00h) + a dedup ao vivo de `TETO_DE_COLUNAS` (mysql→`dblink::mod`, reusado por `pg/mod.rs`, ver §5) | **Coberta.** `TETO_LEITURA_FORA_DO_CANAL=0` e `TETO_INVENTARIO_DESCASADO=0` seguem **ok**; a rota nova de leitura por soquete (`ler_decidindo`) nasceu **dentro** do `Canal`, não ao lado dele. |
| Ordem de digitação (`.reg` nunca reaproveita slot excluído) | — | **Não tocada hoje.** `reg.rs` mudou (autonumber `proxima_sem_andar`/`anotada`, pedido novo de previsão de sequência dentro da transação), mas não mexe em reaproveitamento de slot excluído; o único teste com "slot excluido" no diff de hoje é um rename de tabela pré-existente, sem mudança de comportamento. Nada a reportar aqui — pétrea intocada, guarda não exigida. |

**Pétrea tocada sem guarda:** nenhuma das seis foi tocada sem guarda
correspondente. O único ponto de atenção é 533/534/535 (fsync sob a trava
global): são gaps **reais e não guardados ainda**, mas estão corretamente
listados como pendência aberta com `☐`, não como "resolvido" sem prova — não
é uma guarda faltando por omissão, é trabalho futuro nomeado.

## 4. Placar das catracas (`python3 bancada/catracas/todas.py`)

```
=== catracas em Python (6 achadas por varredura de --catraca) ===
  ok   bancada/concorrencia/mapa-da-trava.py       (4.52s)
  ok   bancada/concorrencia/mapa-das-threads.py    (1.02s)
  ok   bancada/guardas/debug-com-segredo.py        (2.37s)
  ok   bancada/guardas/pkill-sem-pid.py            (0.05s)
  ok   bancada/guardas/trecho-vivo.py               (0.40s)
  ok   docs/cognicao/avoid-e-reuse.py               (0.37s)

=== TETO_* do fonte Rust (36 achadas por varredura de 'const TETO*:') ===
  NAO rodadas por padrao (custam ~19s frias de cargo list); 15/36 sem teste
  candidato achado pelo crivo textual (não é falha — é "ninguém provou pelo
  nome ainda", registrado como tal, não escondido). Use --com-rust para rodar.
```

**Placar: 6/6 catracas Python OK. 36 `TETO_*` inventariadas, 0 executadas
nesta chamada** (comportamento padrão do script, não uma falha desta
auditoria — `--com-rust` não foi usado por orçamento de tempo; nenhum dos 36
tetos teve o **valor** alterado hoje, então a falta de execução não esconde
subida nenhuma, só deixa de reconfirmar a prova pelo nome).

## 5. Achado fora do escopo pedido, mas relevante: árvore de trabalho suja

No momento da auditoria a árvore tinha mudanças **não commitadas** (índice +
working dir) — aparentemente o próximo lote, "faceis D" (pedidos 523, 544,
530, resto do 463), em preparo:

- `catalogo.py`: **+6 guardas novas**, todas com `porque` (recusa de `fsync`
  por grafia do caminho/symlink — 523; DbLink MySQL `lenenc` em pânico — 544;
  DbLink PostgreSQL contagem negativa — 544; aperto de mão MySQL além do fim —
  544; `job_rodar` que dispara `job_rodar` sem teto — 530; SMTP sem prazo
  total da conversa).
- `trecho-vivo.py`: `PISO_DAS_ENTRADAS` 401→407, com linha de história
  nomeando os 4 pedidos e a conta do `--catraca`.
- `docs/TESTES.md`: 400→406 no total, 76→82 nas não julgadas, todas as 6 novas
  nomeadas na lista — nenhuma escondida.
- `dblink/mysql.rs`: `TETO_DE_COLUNAS` (4096) **movido**, não alterado, para
  `dblink/mod.rs` e reusado por `pg/mod.rs` novo — é a pétrea "função e
  comando não se duplicam" sendo seguida, não violada.

Rodei `trecho-vivo.py --catraca` **contra esta árvore suja** (é o estado real
do disco no momento da auditoria) e ela autoconfere: 406 catálogo + 1
aposentada = piso 407, **ok**. **Nenhum `TETO_*` do Rust mudou de valor nesta
mudança em revisão** — só constante movida (mesmo valor) e 6 guardas novas.

**Resposta direta à pergunta do orquestrador — "qual catraca subiria
indevidamente com a mudança em revisão":** **nenhuma**, no estado em que a
árvore foi encontrada. A única catraca que se move (`PISO_DAS_ENTRADAS`,
401→407) é piso, sobe por desenho, e a subida está documentada com pedido e
contagem no mesmo arquivo. Isto **não é uma aprovação de commit** — é
descrição do estado observado; quem commitar deve rodar `./portoes.sh` de
novo depois de qualquer edição posterior a esta leitura.

## Resumo

| Item | Resultado |
|---|---|
| 1. Catraca subiu hoje? | Não. Nenhum `TETO_*` teve valor elevado. `PISO_DAS_ENTRADAS` só subiu (por desenho), com histórico. |
| 2. Guardas novas de hoje bem formadas? | Sim — 197/197 com `porque`, trecho único e vivo confirmado pela régua, `docs/TESTES.md` nomeia as não julgadas. |
| 3. Pétreas tocadas sem guarda? | Nenhuma. Gap real em fsync/durabilidade (533-535) está registrado como pendência aberta, não escondido. |
| 4. Placar das catracas | 6/6 Python OK; 36 TETO_* inventariados (não executados nesta chamada, nenhum valor mudou). |
| 5. Mudança em revisão (árvore suja, não commitada) | Autoconsistente; nenhuma catraca subiria indevidamente. |

**O que precisa de pedido novo** (proposta, não conserto meu — vai ao juiz
PhxJev antes de entrar na conta):

- **QA-1** (baixa): reconciliar o changelog de `trecho-vivo.py` — a narrativa
  local do pedido 497 (300→304→306) não se identifica como medida de "árvore
  isolada" como a do 509+512 faz ("nesta árvore" / "a conta do integrador"),
  ficando ambígua para quem lê em ordem depois do 401. Sugestão: uma linha
  dizendo de qual árvore/branch aquela contagem é, como já se faz alhures no
  mesmo arquivo.
- **QA-2** (baixa): as 2 entradas novas de hoje sem número de pedido explícito
  no bloco (`aresta-velha-depois-do-savepoint`,
  `corrente-do-ciclo-atravessa-quem-nao-confirma`) cumprem a lei (nomeiam a
  condição "R1" do parecer do DBA), mas ficariam mais auditáveis com o número
  do pedido pai ao lado de "R1", como as demais entradas do mesmo lote fazem.
