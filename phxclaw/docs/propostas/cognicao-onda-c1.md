# Onda C1 da cognição — desenho de implementação

Papel K, 09/10/2026. Só desenho: nada compilado nem testado (disco em 1,4 GB, árvore em conferência
pelo integrador). Base: `novas-fontes-2026-10.md` §4 (fontes 9, 10, 11 e reserva 13) e §5 «Ondas»;
decisões do dono de 09/10 (`diretivas/DECISOES_DO_DONO_20261009.md`, itens 7 e 8). Todo
`arquivo:linha` abaixo foi **lido nesta rodada**; o que não foi lido diz «a conferir».

**O que a onda entrega, e o que não entrega.** Cinco peças sem modelo e sem dependência nova:
`ativacao_base`, `saida_grande_em_arquivo`, `compactacao_contexto`, `recibo_ferramenta` e o
**desfecho do portão gravado em cada episódio**. Nada aqui aprende nem decide: a ativação só
desempata, a compactação só troca texto por ponteiro sem perder byte, o recibo só acusa, e o
desfecho só registra o rótulo que a C3 vai usar.

| # | Item | Hipótese vencedora | Linhas (código + teste) | Ligado por padrão? |
|---|---|---|---:|---|
| 4 | desfecho do portão no episódio | `Portao` nasce no `record()` do motor, vai para o `StepRecord` e espelha na gravação v3 | ~200 + ~170 | sim (só registra) |
| 2a | `saida_grande_em_arquivo` | acima de 6.000 caracteres a saída inteira vai para `work/_saidas/`; o modelo vê o MESMO começo de hoje e um ponteiro | ~70 + ~80 | sim (≤ 6.000 não muda nada) |
| 3 | `recibo_ferramenta` | HMAC-SHA256 do `canais/cripto.rs` com chave por tarefa; confere por pertença no `conferir_fim` | ~140 + ~110 | **não** até o A/B do gabarito |
| 2b | `compactacao_contexto` | mecânica, sem modelo: a 80% da janela, as 10 saídas mais antigas viram ponteiro para `_saidas/` | ~150 + ~120 | **não**: só com `agente.janela_contexto_tokens` |
| 1 | `ativacao_base` | Petrov (d = 0,5, k = 10) num arquivo lateral; desempata o `term_score` no lugar do `created_at` | ~210 + ~190 | sim (sem histórico = ordem de hoje) |

Total estimado: **~1.440 linhas, ~46% de teste**. Raciocinado, não medido.

---

## 1. `ativacao_base` (Soar, cap. 6)

### O que existe

- `memory-context/src/lib.rs:339-357` `term_score`: casamento por substring (namespace +8, chave +12,
  ocorrência no valor +2, teto 16). `:329-334` `relevance_score` soma a confiança por cima, só para o
  `ContextCompiler` (usado fora do agente: `research-pipeline/src/lib.rs:401`).
- `:394-424` `search_with`: filtra `term_score > 0`, ordena por pontos e, **no empate, pelo
  `created_at` mais novo** (`:419`). É a única busca que o agente usa (`memoria.rs:190`).
- `memoria.rs:207-224` `bloco_para_o_prompt` injeta as 3 primeiras (`:33`) pelo `motor.rs:480-495`;
  `:315-359` `memory_search` devolve até 10.
- `lib.rs:102-108` `refresh_hash`: o `sha256` cobre **o registro inteiro**; `:449-451` o arquivo não
  coordena processos; `:504-508` o teto de 200 tira **o mais antigo por posição**.

### O que a fonte diz (conferido hoje em `soar.eecs.umich.edu`, `reference/cli/cmd_smem` e cap. 6)

- Entre as memórias que casam a pista, ganha «the LTI which not only matches the cue, but also has
  the highest activation value» — **a ativação ordena dentro do casamento, não soma com ele**.
- `base-decay` 0,5; aproximação de Petrov (2006) com histórico de **10** («compile-time parameter
  (default=10)»); política `stable`: recalcula quando a memória é referenciada «through storage or
  retrieval»; `activate-on-query` **on**. O modo padrão do Soar é `recency`, não `base-level`.
- `BLA = ln[ Σ_{i=1..k} t_i^(-d) + (n-k)(t_n^(1-d) - t_k^(1-d)) / ((1-d)(t_n - t_k)) ]`, com `n` acessos,
  `t_i` a idade do i-ésimo mais recente e `t_n` a idade do primeiro.

### Hipóteses

**Onde gravar o acesso.**

- **H1 — dentro do `MemoryRecord`** (campo `acessos`, versão 3). **Morreu.** Toda busca passaria a
  regravar o arquivo de memórias, e ele não coordena processos (`lib.rs:449-451`): uma injeção num
  processo poderia apagar uma memória que outro acabou de gravar. Hoje ler nunca escreve; esse é o
  invariante que protege a memória. Além disso o `sha256` (`:102-108`) mudaria a cada leitura, e ele
  é a identidade que o `ContextItemRef` carrega.
- **H2 — diário só de acréscimo** (`<escopo>.acessos.jsonl`). **Morreu**: cresce sem teto (3
  injeções por tarefa) e toda busca relê o diário inteiro. Para ficar barata, precisaria de uma
  compactação, e aí vira a H3 com um passo a mais.
- **H3 — arquivo lateral resumido** `_memoria/<escopo>.ativacao.json`: `{chave: {n, primeiro,
  recentes[≤10]}}`, com os instantes em segundos Unix. **Venceu.** O tamanho é fixo por memória, a
  gravação atômica é a mesma do `gravar_atomico` e o pior caso entre processos perde **uma contagem
  de acesso, nunca uma memória**.

**O que conta como acesso.**

- **A1 — tudo o que a busca devolve** (injeção + `memory_search`), que é o `activate-on-query on`
  do Soar. **Morreu pela lei do papel K**: a consulta do `memory_search` é escolha do modelo, e
  contá-la faria a ordem aprender de uma escolha que nenhum portão conferiu. É a divergência, e a
  restrição que a causou tem nome.
- **A2 — gravação + injeção** (a injeção é função do objetivo do usuário e do arquivo, não do
  modelo). **Venceu.** O `memory_search` grava o acesso com `origem: "busca"`, mas a C1 não o soma.
  Fica guardado para a C2 decidir, com portão, sem migração.
- A gravação **não escreve** no lateral: memória sem histórico vale como `n = 1` no `created_at`.

**Como entra na ordem.**

- **O1 — soma ponderada** `term_score + w·BLA`. **Morreu**: mistura um inteiro em passos de 2 com
  um logaritmo, e o `w` não tem gabarito para sair medido. Uma memória muito usada passaria por
  cima de outra que casa melhor, e as 3 injetadas mudariam para todo objetivo.
- **O2 — lexicográfica**: primeiro o `term_score`, e a BLA no lugar do `created_at` (`:419`).
  **Venceu**: é exatamente a regra do Soar. Com o lateral vazio, a ordem fica **idêntica à de hoje**,
  porque com um acesso só a BLA é monotônica no `created_at`.

**Unidade de tempo.** Relógio de parede em segundos, com piso de 1 s (t = 0 daria infinito). A
ordem **não depende da unidade**: escalar t por c multiplica as duas parcelas por c^(-d) e só soma
uma constante ao log. O contador de tarefas, que é o análogo ao ciclo de decisão do Soar,
**morreu**: pediria um contador global compartilhado entre processos, que é mais um estado mutável.
Guarda numérica: com `t_n − t_k < 1 s`, a parcela vira `(n−k)·t_k^(−d)`.

**O teto de 200.** Despejar pela menor ativação, e não pela posição, fica **fora da C1**. Perder
memória é irreversível e pede gabarito; isso é da C2.

### Arquivos e funções

| Onde | O quê |
|---|---|
| `memory-context/src/ativacao.rs` (novo) | `Historico {n, primeiro, recentes}`, `bla(agora, d) -> f64` (Petrov), `Ativacoes::abrir(caminho)` (ausente → vazio; ilegível → `Err`, e quem chama cai na ordem de hoje **sem sobrescrever**), `registrar(chaves, agora, origem)`, `podar(chaves_vivas)` |
| `memory-context/src/lib.rs:394-424` | `search_with` ganha uma irmã `search_ranked(.., ativ: Option<&Ativacoes>, agora)`; o `search_with` vira ela com `None`. Ponto único de ordenação, troca só em `:419` |
| `memory-context/src/lib.rs:329-334` | **não muda**: o `ContextCompiler` desempata por confiança e só a `research-pipeline` o usa |
| `phxclaw-agent/src/memoria.rs:180-202` | `buscar_com` abre o lateral (`arquivo.with_extension("ativacao.json")`) sob a mesma `TRAVA` (`:37`) |
| `memoria.rs:207-224` | `bloco_para_o_prompt` registra `origem: injecao` das que entraram, poda as chaves que saíram pelo teto |
| `memoria.rs:315-359` | `memory_search` registra `origem: busca` (não somada na C1) |

**Migração:** nenhuma, porque o arquivo ausente vale como «ninguém acessou». **Custo:** uma leitura
a mais por busca e uma gravação atômica de ≤ ~40 KB por tarefa (200 chaves × ~200 B, raciocinado). A
conta são ≤ 2.000 `powf` por busca, desprezível perto da leitura do JSON.

### Teste nos dois sentidos

- **Aceite (§5):** A gravada há 90 dias e injetada ontem; B gravada há 30 dias e nunca usada; o
  mesmo termo. Esperado: **A antes de B** (BLA de A ≈ ln(7,776e6^-½ + 86.400^-½) ≈ −5,58; de B ≈
  ln(2,592e6^-½) ≈ −7,38). **REPOSTO:** volta o `created_at` em `:419` e B passa à frente, o teste cai.
- **Comportamento velho:** sem lateral, a ordem de 50 memórias aleatórias é igual à do `search_with`
  de hoje, elemento a elemento.
- **Casamento manda:** C casa dois termos e nunca foi usada; A casa um termo e foi usada ontem. C
  vem antes. Reposto com a soma O1, o teste cai.
- **Lei do K:** 20 `memory_search` sobre B não tiram A da frente.
- **Petrov contra a fórmula exata:** com n ≤ 10 as duas dão o mesmo resultado (diferença < 1e-12);
  com n = 50 acessos uniformes, a diferença fica abaixo de um limiar que **sai da primeira corrida**
  e se escreve no teste.
- **Lateral corrompido:** a ordem é a de hoje e o arquivo continua com os mesmos bytes.

---

## 2. `saida_grande_em_arquivo` e `compactacao_contexto` (goose)

### O que existe

- `motor.rs:67` corta em 6.000 caracteres; `:867-870` é o **único** ponto onde a saída de ferramenta
  volta ao modelo: `truncate_for_model(&texto, max)`. O `agent-core/src/lib.rs:256-265` guarda a
  **cabeça** e **perde o rabo**, e o rabo não fica em lugar nenhum: o ledger (`:1184-1199`) guarda
  só `chars` e `sha256`. Na saída de `cargo test` o veredito está no fim, então é o rabo que se perde.
- `:527` é o laço; `:531` é a única chamada ao modelo; `:543` acumula `usage.input_tokens`.
- O `read_file` (`ferramentas.rs:351-401`) lê com `start_line`/`end_line`.
- **Não há janela de contexto conhecida:** `LlmOptions` (`agent-core/src/lib.rs:139-142`) só tem
  `max_output_tokens` e `temperature`; o Ollama recebe só `temperature` e `num_predict`
  (`phxclaw-llm/src/ollama.rs:59`, `:108`). Vale o `num_ctx` padrão do servidor, e o agente **não o
  envia nem o lê**. Quando estoura, quem corta é o Ollama, calado.

### `saida_grande_em_arquivo`: hipóteses

- **S1 — goose literal:** acima do limite, a saída vai para arquivo e o modelo recebe o começo e um
  ponteiro. **Venceu**, mas com o começo **idêntico ao de hoje** (os mesmos 6.000) e o ponteiro
  depois. Assim nada some do que o modelo já via, e as notas do `avaliacao.rs` continuam
  comparáveis.
- **S2 — cabeça + rabo, sem arquivo.** **Morreu como substituta**: o meio continua perdido para
  sempre. Como **variante de pré-visualização** (cabeça 4.500 + rabo 1.500) volta como A/B no
  gabarito; ganhar sem cruzar faixas é condição para trocar.
- **S3 — rodar de novo com paginação.** **Morreu**: o shell não é idempotente, e a 3ª chamada
  idêntica já é bloqueada (`:728-739`).

**Desenho.** Uma função `entregar_ao_modelo(ctx, n_chamada, nome, texto) -> String` toma o lugar de
`:867-870`. Com `chars ≤ max`, devolve o texto como hoje. Acima disso, grava
`work/_saidas/c{n:03}-{nome}.txt` (0600, pelo `redigir_texto` da gravação, porque o arquivo persiste e a
cabeça vista é a de hoje) e devolve a cabeça de 6.000 +
`[full output: N chars saved at _saidas/c007-shell.txt; read it with read_file start_line/end_line]`.
O teto do arquivo é de 4 MiB, com aviso (raciocinado). Ler um `_saidas/` sem faixa não volta a
derramar: devolve a cabeça e pede a faixa, senão nasceria uma cópia da cópia. Sem `fs.read`, o
ponteiro diz que o arquivo existe e que a ferramenta não foi concedida.

**Teste.** Uma ferramenta de teste devolve 20.000 caracteres terminados em `FIM-7Q`. O resultado ao
modelo tem os mesmos 6.000 primeiros de hoje mais o ponteiro, e o arquivo existe e contém `FIM-7Q`.
Um `read_file` com faixa traz o rabo. **REPOSTO:** sem a gravação, o arquivo não existe e o teste cai.
**Comportamento velho:** com 5.999 caracteres, a mensagem sai byte a byte igual à de hoje.

### `compactacao_contexto`: hipóteses

- **C1-a — goose literal:** o próprio modelo resume as chamadas antigas em lotes de 10. **Morreu
  nesta onda**: o resumo seria texto do modelo no lugar de evidência, e o 3B narra o que não obteve
  (`motor.rs:552-553`). Fora isso, é uma chamada a mais, que a gravação teria de explicar. A ideia
  do lote veio de lá.
- **C1-b — mecânica e sem perda.** **Venceu.** Antes de `:531`, se a estimativa passar de 80% da
  janela, as 10 saídas mais antigas ainda inteiras viram
  `[compacted: <nome> → <outcome>, N chars, full output at _saidas/c003-web_fetch.txt] [receipt …]`.
  Repete enquanto passar do limite e houver o que compactar. Saída que ainda não estava em arquivo
  (≤ 6.000) é gravada **na hora de compactar**. O invariante é que todo ponteiro aponta para o texto
  inteiro que o modelo viu.
- **Regras duras do C1-b.** Troca-se **o conteúdo**, nunca se remove mensagem, porque toda chamada
  de ferramenta precisa do resultado par na API do provedor. Não se toca no sistema, no objetivo
  nem nas duas últimas voltas. A mensagem da 3ª chamada repetida (`:736`, «have its result above»)
  passa a citar o arquivo quando o resultado já foi compactado; sem isso, o modelo seria punido por
  pedir de volta o que a compactação tirou.
- **De onde sai o limite.** Tirar a janela do `/api/show` do provedor **morreu**: dá o máximo do
  modelo, não o `num_ctx` efetivo. **Venceu** a configuração explícita `agente.janela_contexto_tokens`,
  e o **mesmo número** vai como `num_ctx` ao Ollama, para que o limite que se compacta seja o limite
  que vale. Sem a chave, nada muda: não se compacta e não se manda `num_ctx`. É guarda pedida, não
  imposta.
- **A estimativa.** Usa o `input_tokens + output_tokens` da última resposta, mais os caracteres novos
  divididos pela razão caracteres/token **medida nesta tarefa** (caracteres do pedido anterior ÷
  `input_tokens`). Provedor que devolve 0 (o roteiro) cai em 4 caracteres/token, um número
  raciocinado e declarado.
- **Estourou mesmo com tudo compactado:** o passo se registra como `contexto` e a chamada segue.
  Falhar a tarefa mudaria um comportamento que hoje termina; o número fica no passo para a bancada.

**Arquivos.** Novo `phxclaw-agent/src/contexto.rs` (`estimar`, `compactar(&mut msgs, janela,
&saidas)`); `motor.rs:527-531` (uma linha antes do `chat`); `AgentConfig` (`motor.rs:22-59`) ganha
`janela_contexto_tokens: Option<u32>`; `LlmOptions` ganha `janela_contexto: Option<u32>`, que o
`ollama.rs:108` traduz para `num_ctx`; a leitura da configuração vai no `config.rs` (**a conferir**,
não lido).

**Teste.** Com o roteiro, uma janela de 2.000 tokens e 14 chamadas de 1.500 caracteres, o pedido
seguinte fica abaixo de 80% e todo ponteiro abre o texto original. **REPOSTO:** sem a compactação, o
teste que mede o tamanho do pedido no `ScriptedLlm` cai. **Comportamento velho:** sem a chave, a
sequência de mensagens é idêntica à de hoje. **Pares:** depois de compactar, toda chamada de
ferramenta do assistente tem o resultado par.

---

## 3. `recibo_ferramenta` (zeroclaw, reserva 13)

### Onde está o HMAC

Está em `phxclaw-agent/src/canais/cripto.rs:31-33` (`hmac_sha256`, sobre o `sha2`, conferido contra
a RFC 4231 pelo cabeçalho `:1-4`), público por `lib.rs:16` + `canais/mod.rs:23`. O `phxclaw-types`
**não tem HMAC**. O `getrandom` já é dependência direta do agente (`oauth.rs:101`, `gonogo.rs:476`).
**Zero crate nova.**

### O que o recibo prova, e o que não prova

Prova que **esta chamada (ordinal n, ferramenta, argumentos, desfecho e saída inteira) saiu do
portão `call_tool_com` desta tarefa**. O adversário é o **modelo**, não o operador: a chave nunca
entra no contexto, e o modelo não fabrica um recibo válido nem troca o `negado` de um recibo real
por `ok`.

**Não prova ausência.** Uma afirmação sem recibo («rodei os testes») não se acusa por ele. Essa
continua com o `arquivos_pedidos` e o `nada_rodou` (`motor.rs:552-575`) e com o `verificar`
(`:1274-1287`). O documento e a tela dizem isso.

### Hipóteses

- **R1 — id sequencial conferido no ledger.** Dentro do processo dá a mesma garantia, e esta foi a
  primeira conclusão, revista abaixo. **Morreu** por dois motivos. Primeiro, quem confere precisa
  do ledger da tarefa, e o pai de um subagente, o par A2A (C5) e o leitor da gravação (C3) não o
  têm. Segundo, a forma do token mudaria quando se quisesse selar, e mudança de formato entra cedo.
- **R2 — HMAC com chave efêmera por tarefa, conferido por pertença.** **Venceu.** O token é
  `[receipt n:<outcome>:<mac 16 hex>]`, com o mac sobre `n ‖ nome ‖ sha256(args canônicos) ‖ outcome
  ‖ sha256(saída inteira, antes do corte)`. A chave de 32 bytes vem do `getrandom` no começo do
  `run_inner`, e `AgentConfig.chave_de_recibo: Option<[u8;32]>` a fixa nos testes. Quem confere: o
  `conferir_fim` (`:1233`), por pertença ao conjunto emitido na tarefa.
- **R3 — chave persistida** (no `key-provider`), para o leitor da C3 conferir a gravação fora do
  processo. **Adiada, não morta**: é o que transforma o recibo gravado em selo de origem do
  rótulo. Entra com a C3. O formato da R2 já serve; só a chave muda de lugar.
- **R4 — nonce aleatório no lugar do HMAC.** Dentro do processo equivale à R2. **Morreu** porque não
  deixa porta para a R3: um nonce gravado não se reconfere depois.

**Armadilha achada no desenho: a repetição.** A repetição (`gravacao.rs:775-960`) roda o motor de
verdade com as respostas gravadas. Uma resposta final que cita um recibo da corrida original seria
recusada na repetição, porque a chave é outra e a saída gravada está **redigida**, com outro
`sha256`. **Conserto:** o recibo emitido vai na linha `passo` da gravação v3 (item 4), e o
`Repetidor::agente` (`:814-830`), que já troca o modelo e as ferramentas, passa a semear também
`recibos_aceitos`. O `conferir_fim` aceita o emitido ∪ o semeado.

**Achado colateral:** o `serde_json` do workspace vem com o `indexmap` (`Cargo.lock`, serde_json
1.0.151), ou seja, com `preserve_order`. `Value::to_string()` **não ordena chaves**, então o mac
precisa de uma canonização própria, chaves ordenadas recursivamente (~20 linhas).

### Arquivos e funções

| Onde | O quê |
|---|---|
| `phxclaw-agent/src/recibo.rs` (novo) | `Recibos {chave, emitidos, aceitos}`, `emitir(n, nome, args, outcome, texto) -> String`, `citados(texto) -> Vec<Token>`, `confere(&Token) -> bool`, `canonico(&Value)` |
| `motor.rs:521-527` | cria os `Recibos` da tarefa |
| `motor.rs:867-870` | `entregar_ao_modelo` (item 2) põe o recibo no fim; ele vale também para `negado`, `invalido`, `repetida` e `ask_user`, porque todos passam por ali |
| `motor.rs:1233-1290` | `conferir_fim`: cada recibo citado e não emitido é uma recusa, com `por: recibo` |
| `gravacao.rs:814-830` | `Repetidor::agente` semeia os recibos da gravação |

**Ligado por padrão: não.** O recibo muda o texto de **toda** saída que o modelo vê (~35
caracteres por chamada). Liga com a chave `agente.recibos` depois de uma corrida do gabarito do
`avaliacao.rs` com o 3B, com e sem recibo, em que a nota não caia com as faixas separadas. O
`PROMPT_BASE` **não muda** nesta onda. Pedir ao modelo que cite o recibo é outro A/B.

### Teste nos dois sentidos

- **Aceite (§5):** chave fixa, roteiro que chama `read_file` e depois `final_answer` com
  `[receipt 2:ok:0123456789abcdef]` forjado. Resultado: `final_answer REJECTED`, e na 3ª vez
  `Failed` com «recibo não emitido». **REPOSTO:** com o `confere` aceitando qualquer token bem
  formado, a tarefa conclui e o teste cai.
- **Adulteração:** o recibo real de uma chamada `negado`, com `negado` trocado por `ok`, é recusado.
- **Positivo:** com a chave fixa, o teste calcula o recibo verdadeiro antes de montar o roteiro. A
  resposta que o cita conclui.
- **Repetição:** gravar e repetir a tarefa positiva dá `Relatorio::igual()`. **REPOSTO:** sem a
  semeadura, aparece a divergência.
- **Vetor:** o `hmac_sha256` continua provado pela RFC 4231 do `cripto.rs`; aqui não se reescreve
  HMAC.

---

## 4. Desfecho do portão gravado em cada episódio

### O que existe, e o que concluí primeiro e estava errado

Primeiro concluí que a gravação já via as recusas. **Não vê.** O `GravadorTool`
(`gravacao.rs:416-480`) envolve o `run` da ferramenta, e chamada negada por capacidade, regra,
hook, segredo ou esquema **nunca chega ao `run`**. A recusa só aparece como texto
(«NEGADO pela politica: …») dentro do pedido seguinte ao modelo. O desfecho nasce no motor:

| Valor | De onde vem (lido) | `outcome` de hoje |
|---|---|---|
| `livre` | `call_tool_com` rodou sem regra que pergunte (`:1112-1131`, `aprovado == false`) | `ok`/`erro` |
| `aprovado` (+ `perguntado`) | regra `perguntar` e resposta afirmativa (`:779-797`) | `ok`/`erro` |
| `recusado` · `usuario` (+ `perguntado`) | resposta negativa ou prazo (`:799-813`) | `negado` |
| `recusado` · `capacidade` | `:1063-1068` | `negado` |
| `recusado` · `regra` / `regra_sem_aprovador` | `regras.rs` via `:1081-1090` | `negado` |
| `recusado` · `segredo` | `segredos::recusa_no_shell` `:1093-1095` | `negado` |
| `recusado` · `hook` | PreToolUse `:1100-1110` | `negado` |
| `recusado` · `inexistente` | ferramenta ausente ou ambígua `:1047-1060` | `invalido`/`erro` |
| `invalido` | esquema `:1076-1079`, ou `InvalidArguments` da ferramenta | `invalido` (`:227`) |
| `repetida` | 3ª idêntica `:728-739` | `repetida` |
| fim `recusado` · `arquivos`/`nada_rodou`/`argumento`/`esquema_saida`/`verificacao`/`hook_stop`/`recibo` | `:567-616`, `conferir_fim` `:1246-1287` | `recusado` |
| episódio `verificacao: verde` | `rodar_verificacao` Ok no fim aceito (`:1274`, `:1386-1418`) | — |
| episódio `verificacao: vermelho` | último `rodar_verificacao` Err | — |
| episódio `verificacao: ausente` | `task.verificar` vazio | — |

**`ausente` não é `verde`.** Uma tarefa `Completed` sem comando de verificação não ganha rótulo
verde: é «parcial não vira feito» aplicado ao dado da C3.

### Hipóteses

- **D1 — o gravador deduz o desfecho do texto do pedido seguinte.** **Morreu**: seria decidir por
  comparação de frase, e a primeira melhoria de redação quebraria o rótulo calado.
- **D2 — `GravadorTool` grava o desfecho.** **Morreu**: não vê o que não rodou, que é justamente
  metade dos rótulos.
- **D3 — o motor emite o desfecho num ponto só, e o gravador escuta.** **Venceu.**
  1. `agent-core/src/tarefa.rs:37-50` `StepRecord` ganha `portao: Option<Portao>`, com
     `#[serde(default, skip_serializing_if)]`, e o `task.json` velho continua lendo. `Portao {desfecho,
     por: Option<..>, perguntado: bool, resultado: Option<ok|erro|prazo>, recibo: Option<String>}`.
  2. `call_tool_com` (`:1033`) devolve o `Portao` junto da tupla de hoje. O `run_inner` o completa
     com o `perguntado`/`aprovado` de `:779-813`.
  3. `record()` (`:1203-1222`) se divide em `record_ferramenta(.., portao: Portao)`, com o portão
     **obrigatório no tipo**, e o de pensamento. Um caminho novo que esquecer o portão não compila.
  4. `AgentConfig` ganha `ouvinte: Option<Arc<dyn OuvinteDePassos>>`. O `record_*` e o
     `finish`/`encerrar` (`:888`, `:1319`) o chamam. O `gravando()` (`gravacao.rs:491`) o liga ao
     `Gravador`, que já é quem envolve o agente, e o laço continua um só.
  5. **Gravação v3:** linhas novas `{"tipo":"passo","tarefa","n","ferramenta","portao":{…}}` e,
     no fim, `{"tipo":"desfecho","tarefa","status","verificacao":"verde|vermelho|ausente",
     "fins_recusados":k}`. `VERSAO_GRAVACAO = 3`, `VERSOES_LIDAS = [1,2,3]` (`:37-38`). O leitor
     (`:613-672`) aprende os dois tipos e continua recusando o desconhecido (`:672`).

O dado da C3 passa a existir para **toda** tarefa (`task.json`) e não só para a gravada, e as duas
fontes saem do mesmo `record`, sem jeito de divergir. Na C3, o N0 lê argumento + `portao`.

**A conferir na implementação:** se o `equipe.rs:477` clona a configuração do pai (aí o ouvinte
iria junto, e o `tarefa` da linha separa filho de pai); e o caminho da aprovação de **plano** na
`api.rs` (`aprovar`, não lido), que é outro rótulo humano candidato (`plano: aprovado`).

### Teste nos dois sentidos

- **Tabela:** um roteiro por linha da tabela acima: capacidade negada, regra negar, `perguntar` com
  «sim» e com «não» (pelo mesmo caminho dos testes de `perguntar` que já existem, a localizar em
  `tests/`), esquema inválido, a 3ª repetida, e `verificar` com `test -f x` presente e ausente. Cada
  `StepRecord` traz o `portao` esperado, e a gravação v3 traz a mesma linha `passo`. **REPOSTO:**
  marcar `livre` no ramo da capacidade derruba a linha `capacidade`.
- **`ausente` não é `verde`:** uma tarefa concluída sem `verificar` grava `verificacao: ausente`.
  **REPOSTO:** derivar `verde` do `Completed` derruba o teste.
- **Comportamento velho:** uma gravação v2 do repositório continua lendo e repetindo igual. Um
  cabeçalho v4 é recusado com a mensagem de hoje.
- **Espelho:** em toda tarefa de teste, os `portao` do `task.json` são iguais aos das linhas
  `passo`.

---

## 5. Ordem de implementação

1. **Item 4, o desfecho do portão.** Vem primeiro porque é a fundação: a linha `passo` leva o
   recibo e semeia a repetição, e é o dado da C3.
2. **Item 2a, `saida_grande_em_arquivo`.** Toma o lugar de `:867-870` com `entregar_ao_modelo`.
3. **Item 3, `recibo_ferramenta`.** Usa a mesma função e a linha `passo`; nasce desligado.
4. **Item 2b, `compactacao_contexto`.** Usa o `_saidas/` e preserva o recibo; nasce desligado.
5. **Item 1, `ativacao_base`.** Não depende de nenhum dos outros e **pode correr como frente
   paralela**: os arquivos não se cruzam (`memory-context` + `memoria.rs` contra `motor.rs`,
   `gravacao.rs` e `tarefa.rs`).

Os portões são os de sempre, por alvo: `CARGO_INCREMENTAL=0`, fmt, clippy zero, e o teste com a
linha `// REPOSTO` provado e retirado.

## 6. O que precisa de decisão

- **Crate nova: nenhuma.** `sha2`, `getrandom`, `serde_json` e `chrono` já são dependências dos dois
  crates tocados; `ln`/`powf` são da `std`.
- **Sobe ao dono: nada.** Não há choque com pétrea, nem empate, nem produto. Ficam três medições, que
  são do pesquisador e da bancada, não do dono:
  1. o A/B do recibo no gabarito do `avaliacao.rs`, que decide se `agente.recibos` liga por padrão;
  2. o valor de `agente.janela_contexto_tokens` para o 3B na máquina do dono, porque o `num_ctx`
     custa memória na CPU; sem medida, fica desligado;
  3. o A/B da pré-visualização cabeça + rabo (S2) contra a cabeça só.
- **Do integrador**, porque é mudança de formato e entra cedo: a gravação v3, o `StepRecord.portao`
  e o arquivo lateral `<escopo>.ativacao.json`.
- **Da C3, não desta onda:** a chave persistida do recibo (R3), que é o selo de origem do rótulo
  para o treino.
