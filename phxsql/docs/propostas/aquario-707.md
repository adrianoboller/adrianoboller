# Aquário de monitoramento — desenho do pedido 707

08/10/2026 · papel J (pesquisador) · só leitura, sem `cargo` · nível: modelo forte (classificação,
segurança do «matar», custo no laço quente). Base de código lida: commit `68ad47da`; o
`servidor.rs` está sendo dividido na árvore **agora** (já caiu de 36.860 para 2.926 linhas no meio
desta leitura), então toda citação de `servidor.rs` vai como `servidor.rs@68ad47da:LINHA` e se acha
pelo nome da função depois da divisão.

**Fan-out:** dispensado e registrado — esta sessão não tem ferramenta de subagente; o domínio
«motor» (fontes dos quatro) e o «bancada» (física, SVG, p95) foram feitos pelo J direto, e cada
número abaixo diz se foi medido aqui ou raciocinado.

---

## 0. Veredito em cinco linhas

> **Revisto pela A0 (§11, 09/10/2026):** a base é a do 495/496 unificada (Welford sobre `ln(µs)`,
> z ≥ 4, n ≥ 20, piso 250 ms, ligada atrás do portão); `Alarme` é o tipo da `Ocorrencia`; a contagem
> dos gráficos fecha por hora em `aquario-horas.jsonl`. O item 3 abaixo é a hipótese Ha1, que morreu.

1. **80% dos dados já existem**: tarefa (`Atividade`), espera e posse da trava, prazo, encerrar
   cooperativo, duração e erro de toda resposta no **único sumidouro** (`anotar`). Faltam três peças
   de servidor: **o p95 por operação+tabela**, **os sinais vermelhos marcados na origem** e **o log
   de eventos do aquário**.
2. **A classificação mora no servidor, numa função só**, e serve a tela e o log (a cor já sai do
   servidor hoje, `telemetria.rs:588`; duas regras seriam duas cores).
3. **«Anormal» = tempo de serviço ≥ 2 × p95 da própria op+tabela, com ≥ 20 amostras na janela de
   30–60 min**, e as operações que esperam **por desenho** (`OPS_DE_REPLICACAO`) ficam fora. Medido
   num log real: sem essa exclusão, **14 dos 15** alarmes eram o `replicar_aguardar`.
4. **SVG continua** (medido: 60 fps com 150 bolhas; canvas perdeu a 500); grade espacial entra
   porque custa 0,06–0,09 ms e é o que segura 2.000.
5. **Nada sobe ao dono.** Duas decisões ficam visíveis para veto: «integridade recusando» vira
   **amarelo** (não vermelho), e o «custo zero com o aquário fechado» vale para a **tela**, não
   para o log — porque o log que o dono pediu tem de existir sem ninguém olhando.

---

## 1. De onde vêm os dados

### 1.1 O que já existe por tarefa

| Dado | Onde nasce | Custo hoje | Observação |
|---|---|---|---|
| identidade da tarefa | `Atividade.chave` + `serial` (`telemetria.rs:186-200`) | já pago | a bolha de hoje é a **conexão**; a do aquário é **chave#serial** |
| operação, database, tabela, usuário | `comecou_pedido` (`telemetria.rs:321`), chamado em `servidor.rs@68ad47da:13389` (web) e `:13877` (porta de dados) | já pago | |
| relógio de parede | `ha_ms` (`telemetria.rs:541`) | já pago | |
| tempo de serviço | `trabalhando_ha_ms` (`:551`); espera = `ha − trabalhando` (`:684`) | já pago | **é este que se compara com o p95** (ver §2.2) |
| esperando a trava / com a trava | `esperando_trava`/`com_a_trava` (`:379`, `:386`), chamados em `travar_dados` e `travar_dados_para_ler` (`servidor.rs@68ad47da:2760`, `:2915`) | dois `Instant::now()` por operação | |
| segurando todo mundo | `com_trava && ha_fila && Executando` (`telemetria.rs:611`) | já pago | é o «bloqueando» dos maduros (§2.6) |
| fase cancelável / tem ponto | `cancelavel`, `tem_ponto`, `OPS_CANCELAVEIS` (`:112`) | já pago | |
| prazo (STATEMENT TIMEOUT) | `prazo_ate_ms`, recusa em `siga` (`:423-431`) | `load` Relaxed por linha | |
| encerrar | `encerrar` (`:481`) → `Encerramento` (`:735`) | — | cooperativo, e diz quando não pode |
| duração final, ok, erro, código | `Acesso` (`acesso.rs:27-50`), gravado por `anotar` (`servidor.rs@68ad47da:3471`) | 1 linha JSON por pedido (**186–198 B**, medido em dois `acessos.log` reais) | **o estouro de toda tarefa já está no disco** |
| transação aberta, escritas, travas de tabela | `op_transacoes` + `juntar_travas` (`servidor.rs@68ad47da:19046`, `:20341`) | só quando perguntado | é o «o que se perde» do §5 |
| saúde do disco | `SaudeDoDisco` (`saude_do_disco.rs:296-533`): `ultimo_evento`, `erros_es`, `ultima_sonda` | sonda a cada 5 min | |
| réplica | `anotar_estado` em `uma_rodada` (`servidor.rs@68ad47da:4288`): `ultimo_erro`, `falhas_de_rede_seguidas`, `ultima_rodada_ms`; `continuidade_da_replica` (`:1635`) | por rodada | |
| threads do sistema | `Fio` (`telemetria.rs:773`), `familia = servico` | já pago | **não viram bolha hoje** (lacuna L3) |

### 1.2 O que falta

| # | Falta | Por que importa |
|---|---|---|
| L1 | **p95 por op+tabela em memória.** Hoje só existe o p95 global lido do arquivo inteiro (`op_estatisticas`, `servidor.rs@68ad47da:29225`), que relê o `acessos.log` a cada chamada | sem ele «anormal» vira limiar fixo, que o dono recusou |
| L2 | **Sinal na origem.** O código do erro não separa as causas: `trava_reentrante` devolve `Corrompido` (**1001**, `servidor.rs@68ad47da:34982`), o mesmo código do dado corrompido; o `Cancelado` (**6001**) é o mesmo para prazo e para encerrar manual | cor por código mentiria; cor por **texto** é proibida (texto se resolve por chave) |
| L3 | **Tarefas do sistema não são bolha.** A origem `fio` existe no enum (`telemetria.rs:189`) e ninguém a usa: só `web` e `dados` chamam `entrar` | réplica parada é vermelho do dono e não teria bolha |
| L4 | **Transação ociosa segurando trava não é tarefa.** Entre dois pedidos a atividade está `Ociosa` | é o *idle in transaction* que o PostgreSQL trata como estado próprio (§2.6) |
| L5 | **`encerrar_sessao` não grava o ALVO na trilha.** O `telemetria_encerrar` grava (`servidor.rs@68ad47da:28956-28975`); o `encerrar_sessao` só deixa a linha genérica do pedido, sem o id derrubado | o dono pediu «o ato vai à trilha» |
| L6 | **`ultimo_sucesso_ms` da réplica.** `ultima_rodada_ms` sobe no erro também (`:4307`), então «inalcançável há quanto tempo» não sai dele | |
| L7 | **O `fsync` recusado derruba o processo** (pedido 509, comentário em `fecho_recusado`, `servidor.rs@68ad47da:22015-22023`) | esse vermelho **não pode** aparecer ao vivo; aparece no arranque seguinte (§2.5) |

---

## 2. A classificação

### 2.1 Tamanho: pelo quê

| Hipótese | Resultado |
|---|---|
| H1 linhas (`passos`) | **morre**: `passos` só sobe no `siga`, que existe só nas fases canceláveis (`telemetria.rs:417`); um `inserir` teria tamanho zero. E a casa já recusou somar linhas: «linha lida e linha gravada custam coisas diferentes» (`telemetria.rs:514`) |
| H2 bytes | **morre**: não há contador de bytes por tarefa; o de bytes do processo é por segundo e por processo (`telemetria.rs:1383`) |
| H3 tempo de serviço da **tarefa corrente** (`trabalhando_ha_ms`) | **fica**. Moeda única, já medida, e a lição da casa: tempo de parede pintava as vítimas da fila do mesmo tamanho do culpado (`telemetria.rs:287-296`) |

O `peso_ms` de hoje soma as operações passadas da conexão; ele continua valendo para a lista de
conexões, mas **a bolha do aquário é a tarefa**, e nasce do zero a cada serial.

**Classes** (cor) e **área** (desenho) são coisas separadas:

| Classe | Tempo de serviço | Número e de onde sai |
|---|---|---|
| pequena | < 1 s | 1 s = `PERIODO_DA_AMOSTRA_MS` (`telemetria.rs:71`): abaixo disso a tarefa nasce e morre entre duas olhadas |
| média | 1 s a < 10 s | |
| grande | ≥ 10 s | **10 s é o `long_query_time` de fábrica do MySQL e do MariaDB** (`sql/sys_vars.cc`: `DEFAULT(10)` nos dois). O PostgreSQL não tem padrão (`log_min_duration_statement = -1`) e se abstém. 5 contra 0 |

Os dois cortes entram no bloco `telemetria` do `config.json` (`tarefa_media_ms`, `tarefa_grande_ms`),
pelo mesmo leitor do `alto_uso_ms` — campo sem leitor é proibido.

**Área com teto:** `r = rmin + (rmax − rmin) · √(min(t, 60 s) / 60 s)`, `rmin = 11 px` (o
`raioClique` que já existe, `telemetria.js:103`), `rmax = min(90 px, 0,09 · menor lado)`. Acima de
60 s a bolha para de crescer e ganha o anel de minutos (um traço por minuto, até dez). Área, e não
raio, pela razão que o `telemetria.js:309` já escreve: raio proporcional exagera sempre.
*Raciocinado, não medido:* o 60 s e o 90 px saem da TV de 1080 linhas; quem decide é o vídeo da F4.

### 2.2 «Anormal»: p95 da própria operação+tabela

> **Hipótese Ha1 — morreu na A0 (§11.1)** por custo (3,2×), memória (13,6×) e falso alarme em cauda
> larga (150×). Ficam desta seção: a chave op+tabela, o tempo de serviço, a janela de 30–60 min, as
> exclusões e o piso de 250 ms. Morrem: o histograma, o `2 × p95` como corte e o «a mais fria sai».

Decisão, com o número de cada parte:

| Parâmetro | Valor | Por quê |
|---|---|---|
| chave | `op` + `database.tabela` (a `op` sozinha quando não há tabela) | o «digest» do MySQL é o texto normalizado da instrução; aqui o pedido já chega estruturado, e op+tabela **é** o digest. Divergência por restrição nossa: protocolo JSON, não SQL |
| grandeza | tempo de **serviço** (`ms − espera`) | a vítima da fila não pode virar «anormal» pela espera — ela já tem cor própria (LOCK) |
| balde | logarítmico, **4 por oitava** (fator 1,19), de 1 ms a 2²⁰ ms + 1 de sobra = **81 baldes** | o MySQL usa 450 baldes de 4,7% a partir de 10 µs (`storage/perfschema/pfs_histogram.cc`, `BUCKET_BASE_FACTOR 1.0471285480508996`); a casa usa oitavas inteiras no `op_estatisticas`. Oitava inteira dá p95 com erro de 2×, que é do tamanho do próprio limiar; 4 por oitava dá 19% |
| janela | **duas metades de 30 min** que giram (o p95 olha 30 a 60 min) | 5 min absorve a própria tempestade: um lote lento de 5 min viraria «o normal» dele mesmo. O Query Store do SQL Server agrega em intervalos de 60 min por padrão |
| amostras mínimas | **20** | p95 pelo vizinho mais próximo (o método que a casa já usa, `servidor.rs@68ad47da:29272`) com n < 20 cai no **máximo** — um único caso fora da curva definiria o «habitual». Abaixo de 20, vale o limiar fixo de hoje (`alto_uso_ms`/`stress_ms`) |
| limiar | tempo de serviço **≥ max(2 × p95, 250 ms)** | ver a medição abaixo |
| exclusões | `OPS_DE_REPLICACAO` (`servidor.rs@68ad47da:584`) e as tarefas do sistema | esperam **por desenho** |
| onde mora | `Mutex<HashMap<chave, Histograma>>` no `Telemetria`, **teto de 1.024 chaves** (a mais fria sai) | 81 × 4 B × 2 metades = **648 B por chave**, **≤ 648 KiB** no teto |
| quem alimenta | **`anotar`** — o único sumidouro por onde passa toda resposta, com op, tabela, ms e código na mão | um ponto só; espalhar pelos dois laços (`web` e `dados`) seria o irmão esquecido |
| custo | 1 trava sem disputa (**13,2 ns**, medido na casa, CLAUDE.md) + 1 hash de chave curta, **atrás do portão `ligada()`** | *raciocinado, não medido*: decide a bancada A/B da F1 (critério: ≤ 1% no `inserir` em lote) |

**Medido aqui**, repetindo a regra sobre dois `acessos.log` reais (`/tmp/phx-207-quorum-real/no1`,
1.658 linhas; `/tmp/phx-odbc-sonda`, 184 linhas), em ordem de chegada e só com o passado de cada
linha:

| Regra | Alarmes no log do quórum (1.512 avaliadas) |
|---|---|
| tempo > p95 | 59 (**3,90%**) — ruído: por definição 5% passa do p95 |
| tempo > max(2 × p95, 250 ms) | 15 (**0,99%**) |
| idem, **sem** `OPS_DE_REPLICACAO` | **1 (0,07%)** — um `inserir` de 2.005 ms contra p95 de 5 ms, que é exatamente o que se quer ver |

Os outros 14 eram o `replicar_aguardar`, o canal longo do quórum, que segura **1.000 ms de
propósito**. Essa é a recusa medida mais útil deste desenho: **p95 sem a lista de quem espera por
desenho pinta a replicação de anormal a cada segundo.** (Ressalva: dois logs de bancada, não de
produção; a taxa de 0,07% é de carga sintética.)

**Por que p95 e não média ± desvio:** PostgreSQL (`pg_stat_statements`: `mean_exec_time`,
`stddev_exec_time`, sem percentil) e MariaDB (`events_statements_summary_by_digest`: MIN/AVG/MAX,
sem quantil) dão média; o MySQL dá `QUANTILE_95` a partir do histograma. A régua ponderada daria
média (4 + 3 = 7 contra 2) — **mas a régua decide comportamento de banco, e isto é método de
monitor**; nenhum dos três define «anormal». O que decide é a lei da casa já escrita: «média esconde
exatamente o que interessa» (`servidor.rs@68ad47da:29214-29219`), e a ordem do dono já aceitou o p95.
Registrado para não voltar como «os maduros usam média».

### 2.3 As cores, numa função só

`classificar(&Atividade, &Contexto) -> Classe { cor, tamanho, grupo, motivo_chave, dados }`, no
`telemetria.rs`, chamada pelo retrato **e** pelo log. Precedência de cima para baixo — a primeira
que casa vence:

| Ordem | Cor | Condição | Forma (o que não é cor) |
|---|---|---|---|
| 1 | **rosa** (`--acao-marcar`) | `Estado::Encerrando` | traço longo `10 4`, glifo ✕ — já existe |
| 2 | **vermelho** | algum sinal dos seis grupos (§2.4) | **octógono** de borda pontilhada grossa, glifo ■ + letra do grupo **por chave da fábrica** |
| 3 | **amarelo** | sinal de aviso (§2.4) | **losango** de borda tracejada `6 4`, glifo ▲ |
| 4 | **azul escuro** | grande **e** anormal (§2.2) | círculo de **borda dupla** clara + halo |
| 5 | **azul claro** | grande e normal | círculo de borda cheia |
| 6 | **verde** | média ou pequena, normal | círculo de borda cheia fina |

Pequena **e** anormal → amarelo (motivo `aquario.motivo.acima_do_habitual`), porque o dono reservou o
azul escuro para a grande. Média anormal → amarelo pelo mesmo motivo.

### 2.4 Os seis vermelhos e os avisos, mapeados em sinais

**Meio:** um `AtomicU32` de bits por `Atividade` (`sinais`), marcado **na origem** por
`telemetria::sinal(Sinal::X)` — um `fetch_or` Relaxed na tarefa corrente (`corrente()`,
`telemetria.rs:1744`), **só no caminho raro**; o caminho normal não paga nada. Os sinais que não
são de tarefa nenhuma (disco, réplica, firewall) vão para o **sedimento** do servidor (§3.5), com o
mesmo `Sinal`. Um enum só para os dois — uma decisão, um lugar. Nome `Sinal` colide com o
`sinais.rs` (SIGTERM, pedido 687): chamar `Alarme`. **A0 (§11.3): o `Alarme` é também o tipo da
`Ocorrencia` do 495-F2 — um enum, um produtor, dois arquivos com papéis diferentes.**

| Grupo | Sinal | Origem que já emite (arquivo:linha) | O que falta |
|---|---|---|---|
| **LOCK** | esperando a trava ≥ `stress_ms` | `telemetria.rs:1559` (`stress`) | nada: é estado |
| | segurando a trava com fila, ou ≥ `stress_ms` | `telemetria.rs:611-624` | nada |
| | trava reentrante | `trava_reentrante()`, `servidor.rs@68ad47da:34982`, chamada em `travar_dados` (`:2763`) e `travar_dados_para_ler` (`:2916`) | **marcar o bit**: o código 1001 não distingue (L2) |
| | trava envenenada | `depois_do_veneno`, `servidor.rs@68ad47da:2859`; `panicos_na_trava` (`:1504`) | bit na tarefa + sedimento enquanto `panicos > reparos` |
| | transação ociosa segurando trava de tabela que recusou alguém | `barrado_por_travas` (`servidor.rs@68ad47da:20376`) | bolha `tx:<id>` (L4) |
| **DISCO** | E/S, sem espaço, só-leitura | `anotar` → `CODIGO_DE_ES` 5001 (`servidor.rs@68ad47da:3495`) → `saude_do_disco::tipo_do_texto` (`saude_do_disco.rs:193`) e `classificar` (`:170`) | bit na tarefa (o gancho já existe, é o mesmo ponto) |
| | fecho da janela recusado | `fecho_recusado`, `servidor.rs@68ad47da:22024` | sedimento |
| | `fsync` recusado | derruba o processo (509) | **só no arranque seguinte**: o aviso «registrava um fsync recusado num boot ANTERIOR» (`servidor.rs@68ad47da:34663`) vira sedimento datado (L7) |
| **DADO EM RISCO** | marca impossível / não lida / parada | `Relatorio.impossiveis`, `sem_leitura`, `paradas` (`phxsql-store/src/marca.rs:1085-1097`) | sedimento desde o arranque |
| | índice que ficou para trás | `Relatorio.indices_pendentes` e `evento_do_arranque` (`saude_do_disco.rs:139-155`) | sedimento |
| | conferência que achou dado corrompido | `PhxError::Corrompido` 1001 **sem** o bit de reentrante | bit |
| **RÉPLICA** | continuidade rompida | `continuidade_da_replica` com `false` (`servidor.rs@68ad47da:1635`, conferida no `alcancar_tabela`) | bolha de sistema da origem (L3) |
| | origem inalcançável além do prazo | `Falha::Rede` (`replica.rs:1064`) + `falhas_de_rede_seguidas` | `ultimo_sucesso_ms` (L6); prazo = `Ritmo::TETO` × 3 = 3 min (*raciocinado*) |
| | transação acima do teto | `teto_da_transacao()` (`phxsql-store/src/log.rs:684-697`), recusa nos juntadores | bit |
| **SEGURANÇA** | força bruta | `Blacklist::tentativa_leve` (`blacklist.rs:831`) e `violacao_leve` (`servidor.rs@68ad47da:3439`) | bit na tarefa do login |
| | bloqueio do firewall | `aplicar_no_firewall` (`blacklist.rs:498`) | sedimento até vencer |
| | senha em claro | `conferir_o_fio_da_senha` (`servidor.rs@68ad47da:32113`, pedido 667) | bit |
| **PRAZO** | cancelada por STATEMENT TIMEOUT | `siga`, `telemetria.rs:423-431` | **bit** (6001 não distingue de encerrar manual) |
| | acima de 2 × p95 **3 vezes** na mesma op+tabela em 5 min | — | contador na própria chave do histograma (*raciocinado*: 3 em 5 min) |

**Avisos (amarelo):** esperando a trava ≥ `alto_uso_ms` (2 s) e < `stress_ms`; anormal pelo p95 sem
ser grande; `ha_ms ≥ alto_uso_ms` quando a chave tem < 20 amostras (o comportamento de hoje,
`telemetria.rs:625`); sonda do disco `Lento` (`saude_do_disco.rs:91`); recusa de **integridade**,
duplicado ou conflito ao terminar.

**Decisão visível para veto — «integridade recusando» é amarelo, não vermelho.** Duas hipóteses:
(a) toda recusa de FK/unicidade é vermelho, (b) é amarelo e o vermelho fica com o dado corrompido.
(a) morre por dois motivos: nos três maduros a violação de restrição é **erro do cliente** (classe
SQLSTATE 23 no PostgreSQL; `ER_NO_REFERENCED_ROW_2` no MySQL e no MariaDB), e nenhum deles a conta como
saúde do servidor — o motor fez o trabalho dele, o dado está **protegido**, não em risco; e a lição
da casa: vermelho que pinta todo cliente com defeito deixa de separar (`telemetria.rs:593-597`).
O que é vermelho de DADO EM RISCO é a **conferência** que acha `Corrompido`.

### 2.5 Recusas e números que mudaram o desenho

| Proposta | Recusa | Número |
|---|---|---|
| tamanho por linhas | `passos` só existe em fase cancelável | 0 passos num `inserir` |
| p95 sem exclusões | o canal longo do quórum vira alarme | 14 de 15 alarmes |
| limiar = p95 | 5% de toda tarefa por definição | 3,90% medidos |
| janela de 5 min | a anomalia vira o próprio habitual | *raciocinado* |
| cor pelo código do erro | 1001 cobre reentrante e corrompido; 6001 cobre prazo e encerrar | lido no código |
| `fsync` recusado ao vivo | o processo já morreu | pedido 509 |

### 2.6 «Bloqueado / bloqueando» nos monitores maduros

| Fonte | Bloqueado | Bloqueando | Ocioso com transação |
|---|---|---|---|
| PostgreSQL | `pg_stat_activity.wait_event_type = 'Lock'` | `pg_blocking_pids(pid)` | `state = 'idle in transaction'` |
| MySQL 8 | `performance_schema.data_lock_waits` / `sys.innodb_lock_waits` | idem, lado `blocking_*` | `information_schema.innodb_trx` sem instrução |
| MariaDB | `information_schema.INNODB_LOCK_WAITS` | idem | `innodb_trx` |
| SQL Server | `sys.dm_exec_requests.status = 'suspended'`, `wait_type`, `wait_time` | `blocking_session_id` | `open_transaction_count` |
| Idera SQL Check | a documentação pública lista «identificação e alerta de bloqueio»; **o significado de tamanho e cor das bolhas não está documentado** em fonte que se ache | | |

**Convergência dos três (9 de 10): mostrar quem espera E por quem.** Entra sem pergunta. Aqui fica
mais simples que lá, e a divergência é nossa: a trava de dados é **uma** (`RwLock` global), então o
bloqueador é exatamente quem tem `com_trava` — não há grafo para montar. O retrato ganha
`bloqueada_por: [chaves]` na tarefa que espera; com trava de leitura pode haver vários donos.
**Ociosa-em-transação** (L4) também converge nos três e entra como bolha `tx:<id>`.

---

## 3. A física

Medido nesta sessão (Node 22 / Chromium headless do `playwright` da casa, Xeon 2,1 GHz, 4 núcleos,
desenho por software — é o **piso**, uma GPU faz melhor), arquivos de prova no rascunho da sessão:

| N bolhas | colisão ingênua O(N²) | colisão com grade | SVG (fps / ms de JS por quadro) | canvas |
|---|---|---|---|---|
| 150 | 0,13–0,24 ms | **0,06–0,09 ms** | **60,1 / 0,27** | 59,8–60,1 / 0,53–0,72 |
| 500 | 1,0–1,3 ms | 0,23–0,28 ms | 46–52 / 0,84–1,0 | 29–31 / 1,7–2,0 |
| 2.000 | **17–19 ms** (estoura o quadro) | 2,3–2,8 ms | — | — |

Três corridas cada; colisão com 3 iterações de resolução por quadro.

**Decisões que saem da tabela:**

- **SVG fica** (acessível: cada bolha é um `<g>` focável com `aria-label`, como hoje). Canvas perdeu
  em 500 e não ganhou em 150. Recusa medida.
- **Teto de 150 bolhas visíveis** (`bolhasMax`, `telemetria.js:96`) **continua**: é onde o SVG
  segura 60 fps com folga. Acima disso, **cardume** (abaixo).
- **Grade espacial entra**: célula = 2 × `rmax`; custa menos que o ingênuo já em 150 e é a única que
  cabe em 2.000. Não é o gargalo — o gargalo é o desenho.
- **Orçamento de quadro:** 16,7 ms; medido, física + atualização ≈ **0,35 ms** em 150. Se a média
  de 1 s passar de 12 ms, desliga a deriva (o `bolhasParaDeriva` de hoje) e cai para 1 iteração.

**Peso pela gravidade.** Passo fixo de 1/60 s, Euler semi-implícito, amortecimento 0,98, resolução
por posição (como medido). Cada classe tem **empuxo** e uma **faixa de repouso** (fração da altura,
medida do fundo):

| Classe | Faixa de repouso | Comportamento |
|---|---|---|
| vermelho | 0–12% | afunda rápido e **se junta no fundo** |
| amarelo | 12–35% | afunda devagar |
| azul escuro | 35–50% | |
| azul claro | 50–65% | flutua no meio |
| verde | 55–75% | |
| terminou | sobe | empuxo negativo forte até a superfície |

**Colisão sempre**, nunca se atravessam — inclusive na transição de cor (a bolha muda de faixa
nadando, não teleportando; é a transição de lugar que o `telemetria.js:529` já faz).

**Estouro na superfície:** ao terminar, sobe; ao tocar `y = 0`: escala 1 → 1,3 e opacidade → 0 em
250 ms, com um anel que se abre. `prefers-reduced-motion`: some em 1 quadro, sem anel.

**Rastro do erro:** a tarefa que foi vermelha **em qualquer momento**, ou terminou com erro de
família 1xxx/5xxx/6xxx, estoura e **deixa uma pedra** na coluna lateral do fundo por 5 min (a mesma
janela da volta), clicável, abrindo a linha do log.

**Coluna lateral do fundo (sedimento):** à direita, 64 px. Mostra os **sinais do servidor** (disco,
réplica, firewall, marca de recuperação, `fsync` do boot anterior) e as pedras dos rastros, da mais
grave para a mais leve. Some sozinha quando o sinal some — exceto a pedra, que tem prazo.

**Cardume das pequenas:** passou de 150 visíveis, as **verdes pequenas** se juntam por `origem+op`
numa bolha com «×N». Nunca entram em cardume: vermelha, amarela, azul escura, rosa. Alarme dentro de
grupo é alarme escondido.

**Pausa quando a aba some:** `visibilitychange` → `hidden` para o `requestAnimationFrame` **e o
relógio de pedidos**. A multitela já desanexa aba escondida (`multitela.js`, decisão [2]) e o laço de
hoje para sozinho sem alvo (`telemetria.js:891`) — o aquário herda os dois.

### 3.1 Custo ZERO no servidor com o aquário fechado — e o alcance dessa frase

> **A0 (§11.2):** a linha «histograma p95» abaixo vira a base Welford — **37 ns** medidos, ligada
> atrás do mesmo portão.

| Peça | Com o aquário fechado | Com a telemetria desligada |
|---|---|---|
| pedido `telemetria`/vista do aquário | **zero** (ninguém pergunta) | zero |
| classificação para a tela | **zero** | zero |
| histograma p95 no `anotar` | **paga** (1 trava + 1 hash por pedido) | **zero** (portão `ligada()` antes) |
| classificação para o log, no amostrador de 1 s | **paga** (O(atividades) por segundo; o `amostrar` já percorre a mesma lista, `telemetria.rs:1426`) | **zero** |
| linhas do `aquario.log` | paga só para tarefa que viveu ≥ 1 s | zero |

O «custo zero com o aquário fechado», dito pelo integrador, **não cabe junto** com o item (5) do dono
(«o log fica, não se perde por ninguém estar olhando»): o log exige classificar sem tela aberta.
Pela hierarquia, a ordem do dono vence a melhoria do integrador, e por isso **não sobe**: o zero vale
para a tela, e o interruptor que zera tudo é o que já existe (`telemetria_desligar`). A bancada da F1
mede o preço do resto.

---

## 4. O log de tudo

### 4.1 Onde mora — três hipóteses

> **A0 (§11.3–11.4):** o mesmo escritor serve também o `ocorrencias.log` do 495; o `aquario.log`
> ganha a linha `contagem` por minuto, e a hora fecha em `aquario-horas.jsonl`, fora do rodízio.

| Hipótese | Resultado |
|---|---|
| H1 linhas novas no `acessos.log` com campo `evento` | **morre**: quatro leitores tratam toda linha como acesso — `op_acessos` (`servidor.rs@68ad47da:15743`), `op_ips`/`resumo_por_ip`, `op_estatisticas` (histograma e p95 globais). Cada um teria de filtrar; o que esquecesse inflaria a conta — o irmão esquecido |
| H2 tabela no `phxsys` | **morre** pelos três motivos já escritos em `diretivas.rs:10-26`, e um quarto: o vermelho de DISCO acontece justo quando o motor não grava tabela |
| H3 **`aquario.log`, JSON Lines, pelo MESMO motor** do `acessos.log` | **fica** |

«O mesmo motor» quer dizer o **escritor** do `LogAcessos` (`acesso.rs:152-239`: abrir, `registrar`,
rodízio por `crate::rodizio`, conta de falha) numa segunda instância apontando para outro arquivo,
**e** a falha de gravação indo para o mesmo `evento_de_disco` que o `anotar` já chama
(`servidor.rs@68ad47da:3475-3484`). Não nasce um terceiro escritor ao lado do `LogAcessos` e do
`Diario`: o escritor se generaliza para `registrar_json`, e o aquário é cliente dele.

**O estouro não se duplica.** Toda tarefa já tem a linha do estouro no `acessos.log` (§1.1). O que
falta é **ligar** as duas: o `Acesso` ganha o campo opcional `tarefa` (`"dados:17#42"`) — aditivo, e o
`de_json` já tolera campo ausente (`acesso.rs:96-98`). O `aquario.log` só grava `estourou` para
tarefa que **nasceu** no aquário (viveu ≥ 1 s), com a cor final e o motivo.

### 4.2 Formato da linha

```json
{"quando":"2026-10-08 14:03:12,345","quando_ms":1791468192345,
 "evento":"nasceu|mudou|estourou|morta|retrato|sedimento",
 "tarefa":"dados:17#42","sessao":"dados:17","origem":"dados",
 "usuario":"ana","ip":"192.168.50.20","op":"varrer","database":"loja","tabela":"vendas",
 "ms":12345,"espera_ms":300,"tamanho":"grande","cor":"azul_escuro","antes":"azul_claro",
 "grupo":null,"motivo":"aquario.motivo.acima_do_habitual","dados":{"p95_ms":800,"n":214},
 "ok":null,"codigo":null,"quem":null}
```

- **Motivo por CHAVE da fábrica + dados**, nunca frase: o log é neutro de idioma e a tela traduz
  (lei «texto se resolve por chave»).
- **Nunca o texto do pedido** — só op, tabela e tamanho. A redação do Profiler (`profiler.rs:60-71`)
  não se repete porque o campo não existe.
- Todo campo livre reduzido a uma linha antes de entrar (o furo da linha forjada do Profiler,
  `profiler.rs:104-117`).
- Campo vazio não entra (a regra do `acesso.rs:67-69`). Estimativa **~220 B** por linha (as do
  `acessos.log` medem 186–198 B).
- `retrato`: a cada **30 s**, **só se houver tarefa viva**, uma linha com a lista compacta das vivas.
  É o quadro-chave da volta de 5 minutos; sem ele, tarefa que nasceu antes da janela não aparece.

### 4.3 Retenção e consulta

- **Rodízio por tamanho**, como o resto da casa (`profiler.rs:88-103`: disco se mede em bytes):
  **8 MiB × (7 + 1) = 64 MiB** de teto. Nasce **ligado** — arquivo novo não quebra cliente nenhum.
  *Raciocinado:* 10 eventos/s sustentados × 220 B ≈ 7,9 MB/h; 64 MiB ≈ 8 h de carga ruim e dias de
  carga calma. Decide o vídeo de carga da F2, que conta linhas por minuto.
- **Consulta: `aquario_log`** com filtros `desde`, `ate`, `tarefa`, `sessao`, `op`, `tabela`, `cor`,
  `grupo`, `evento`, `max`. Lê os arquivos do rodízio **de trás para a frente** e para quando passa
  de `desde`. **Não** repete o `op_acessos`, que lê o arquivo inteiro para devolver os N últimos
  (`servidor.rs@68ad47da:15745`) — com 8 MiB isso é parse de dezenas de milhares de linhas por clique.
- **A volta de 5 minutos:** a tela pede `aquario_log desde=agora−5min`, acha o último `retrato`
  anterior e dobra os eventos por cima. Pausar congela a tela; o servidor continua gravando; voltar
  ao vivo é um clique.

---

## 5. Matar

**Convergência dos três maduros — entra sem pergunta:**

| Comportamento | PostgreSQL | MySQL | MariaDB | Aqui |
|---|---|---|---|---|
| dois níveis: cancelar a instrução × derrubar a sessão | `pg_cancel_backend` × `pg_terminate_backend` | `KILL QUERY` × `KILL CONNECTION` | idem | `telemetria_encerrar` × `encerrar_sessao` — **já existem os dois** |
| ver todos ≠ matar os outros | `pg_read_all_stats`/`pg_monitor` × `pg_signal_backend` | `PROCESS` × `CONNECTION_ADMIN` | `PROCESS_ACL` × `PRIV_KILL_OTHER_USER_PROCESS` (= `CONNECTION_ADMIN_ACL`, `sql/privilege.h:431`) | **lacuna**: hoje os dois exigem administrador (`portao_da_telemetria`, `servidor.rs@68ad47da:28719`) |

Fontes: `src/backend/storage/ipc/signalfuncs.c` (PostgreSQL), `sql/sql_parse.cc` `kill_one_thread`
(MySQL 8.0 e MariaDB 11.4), documentação de privilégios dos dois.

**Tarefas do sistema protegidas — divergem, decide a régua:** PostgreSQL **não** sinaliza processo
auxiliar (`BackendPidGetProc` devolve nulo: «PID %d is not a PostgreSQL backend process»); MariaDB
**não** mata `COM_DAEMON` nem com privilégio; MySQL deixa o `SUPER` matar thread de sistema
(comentário em `kill_one_thread`: «If we're SUPER, we can KILL anything, including
system-threads»). **Proteger: PG 4 + MariaDB 3 = 7 contra MySQL 2.** O SQL Server converge com a
maioria («System processes … can't be ended»). Então: **tarefa de sistema não se mata, por ninguém**
— recusa no **servidor**, não só botão escondido.

| Protegida | Como se reconhece |
|---|---|
| threads `servico` (amostrador, réplica, cluster, sonda, carteiro, jobs-relógio) | `Fio.familia == "servico"` |
| operações de replicação vindas de outro nó | `OPS_DE_REPLICACAO` |
| a própria sessão de quem pergunta | já recusada (`servidor.rs@68ad47da:28905-28914`) — PG e MySQL deixam; SQL Server proíbe; fica como está porque o pedido morreria sem responder |

**O que mostrar ANTES de confirmar** (na própria página, sem `confirm()` do navegador):

1. sessão, usuário, IP, origem, op, tabela, há quanto tempo, quanto foi espera, fase;
2. **o que a promessa vale**: `cancelavel`/`tem_ponto` — «aborta na próxima unidade»,
   «a marca fica posta», ou «**não cancelável: vai terminar**» (os textos que o `Encerramento` já dá);
3. **a transação aberta** da ligação, de `op_transacoes`: id, idade, **N escritas** e **linhas
   travadas por tabela** (`juntar_travas`);
4. **o que volta**, que difere entre os dois botões e foi lido no código:
   - **cancelar dentro de transação** → o `Cancelado` (6001) é erro de classe TRANSAÇÃO
     (`transacao.rs:144-152`) e leva a transação a **`ABORT_ONLY`**: as N escritas vão se perder no
     `ROLLBACK`. É o comportamento do PostgreSQL; no MySQL o `KILL QUERY` desfaria só a instrução.
     A tela tem de dizer isso, porque quem pensa em MySQL espera o contrário;
   - **derrubar a conexão** → `soltar_transacao_da_ligacao` (`servidor.rs@68ad47da:20604`): N
     escritas descartadas, travas soltas;
5. quem vai assinar o ato.

**Permissão:** nasce o direito **`monitorar`** (ver tudo, matar nada), pela convergência acima.
**Entra pedido, não imposto:** administrador continua com os dois poderes; ninguém perde direito.
O teste que mais importa é o do comportamento velho — admin vê e mata como antes.

**Trilha:** os dois atos vão ao `acessos.log` (o `telemetria_encerrar` já vai; o `encerrar_sessao`
passa a gravar o ALVO, L5) **e** ao `aquario.log` como `morta`, com `quem`, o desfecho
(`marcada`/`posta`/`nao_cancelavel`/`derrubada`) e as N escritas perdidas.

---

## 6. Tela

- **Redimensionável** no console (alça no canto, medida guardada por usuário) **e** em **rota
  própria**: `?tela=aquario`, entrando no `CATALOGO` da multitela, que já abre aba como janela
  destacada pela URL (`multitela.js:1085-1093`).
- **Modo TV**: `?tela=aquario&tv=1`, login de um usuário só com `monitorar` — **sem botão de matar,
  e o servidor recusaria mesmo que houvesse**. Selo de frescor fixo no canto: «atualizado há N s»
  a partir do `agora` do servidor; passou de 3 s, o aquário inteiro escurece e ganha a borda de
  VELHO — a lógica que já existe em `marcarVelho` (`telemetria.js:910`). Painel congelado mente
  pior que painel vazio (`telemetria.rs:1053`).
- **Uma chamada só por volta**, do mesmo motor: a `telemetria` ganha `vista: "aquario"` e aceita
  `amostras: 0` (hoje o mínimo é 1, `servidor.rs@68ad47da:28738`). Nada de operação paralela que
  serialize atividades por conta própria — a lei «função e comando vêm do mesmo motor».
- **Texto pela fábrica**, seis idiomas, chaves `aquario.*`; a catraca `textos-fora-da-fabrica` não
  pode subir.

### 6.1 Cores do dono, contraste medido

Medido aqui (fórmula WCAG 2.x). Gráfico pede ≥ 3:1 contra o fundo; rótulo dentro da bolha, ≥ 4,5:1.

| Cor | Escuro (`#010418`) | vs fundo | tinta do rótulo | Claro (`#f7f5f2`) | vs fundo | tinta do rótulo |
|---|---|---|---|---|---|---|
| azul escuro | preench. `#1d4ed8`, borda `#9cc3ff` | **3,04** (borda 11,30) | branco 6,70 | `#1f3f7a` | 9,41 | branco 10,24 |
| azul claro | `#5fa6e8` (`--reg`) | 7,86 | fundo 7,86 | `#1f6fb8` | 4,80 | branco 5,22 |
| verde | `#6cc98c` (`--ok`) | 10,05 | fundo 10,05 | `#2f7a3e` | 4,86 | branco 5,28 |
| amarelo | `#ffc43d` (`--ambar`) | 12,81 | fundo 12,81 | `#a06a00` | 4,24 | branco 4,61 |
| vermelho | `#ff5f5f` (`--vermelho`) | 6,84 | fundo 6,84 | `#b71414` | 6,18 | branco 6,72 |

- O azul escuro passa raspando no escuro (3,04) — **por isso a borda dupla clara (11,30) não é
  enfeite, é o que o torna visível**, como o integrador previu.
- **Azul escuro × azul claro: 2,59 no escuro e só 1,96 no claro.** A cor sozinha não separa as
  duas; separa a **forma** (borda dupla + halo × borda cheia). É a razão medida de a gravidade ir
  também na forma.
- Branco sobre o vermelho do tema escuro dá **2,98** — o rótulo vai em tinta escura; a escolha da
  tinta medindo já existe (`telemetria.js:488-506`).
- Variáveis novas do tema claro (`#1f6fb8`, `#1f3f7a`) entram como variáveis do console, nunca em
  hexadecimal no módulo (`telemetria.js:108-110`).

---

## 7. Fatias

| Fatia | O que entra | Prova (RED onde há guarda: falha com o defeito reposto) |
|---|---|---|
| **F0** bancada | script que repete um `acessos.log` e conta alarmes por regra (o que este documento fez à mão, virando arquivo da `bancada/`) | números da §2.2 regerados por comando |
| **F1** servidor: classificar | `Alarme` (bits na origem), base no `anotar` (**Welford, §11**), `classificar()` único, `vista: "aquario"` com `amostras: 0`, `bloqueada_por` | RED: (1) vítima da fila **não** vira anormal — repor «comparar `ha_ms`» faz falhar; (2) n < 20 não dá anormal; (3) `replicar_aguardar` nunca é anormal; (4) reentrante ≠ corrompido; (5) **telemetria desligada: zero atualizações de histograma** (contador), no molde do `testes_profiler_desligado`. **Bancada A/B** do custo, critério ≤ 1% |
| **F2** servidor: log | escritor generalizado, `aquario.log` com rodízio, campo `tarefa` no `Acesso`, `aquario_log` lendo de trás para a frente, `retrato` de 30 s | RED: nascer/mudar/estourar gravam **sem nenhum cliente perguntando**; linha forjada com `\n` no `op` não vira duas; disco cheio conta a falha e chama `evento_de_disco` (tmpfs pequeno, **contra o SO**) |
| **F3** servidor: matar | direito `monitorar`; recusa de tarefa de sistema no servidor; `encerrar_sessao` grava o alvo; ficha do «o que se perde» | RED: `monitorar` vê e **não** mata; admin continua matando (**comportamento velho**); `servico` recusado mesmo para admin; alvo na trilha. Queda de conexão **pelo soquete**, não unitário |
| **F4** tela | física (grade, faixas, estouro, pedra, cardume), formas, alça, rota, pausa por visibilidade, textos pela fábrica | navegador **nos dois temas**; **vídeo curto** com carga real (consulta longa segurando a trava + fila + uma recusa de disco simulada); RED: aba escondida faz **0 pedidos em 10 s** (repor o relógio sem `visibilitychange` faz falhar); contraste dos cinco pares nos dois temas contra o `PISO_CONTRASTE`; catraca de textos não sobe; chave morta |
| **F5** TV e volta | `tv=1`, selo de frescor, volta de 5 min pelo `aquario_log` | vídeo: derrubar o servidor com a TV aberta → selo de VELHO em ≤ 3 s; voltar 5 min mostra a bolha que já estourou |
| **F6** documentação | `docs/TELEMETRIA.md` (cores, sinais, log), `MENSAGENS.md`, cognição do «p95 sem a lista de quem espera por desenho» | — |

Ordem: F0 → F1 → F2 → F3 → F4 → F5 → F6. F4 pode andar contra um retrato inventado (o módulo não
fala com o servidor, `telemetria.js:11-15`), em paralelo a F2/F3.

---

## 8. O que diverge das fontes, e qual restrição nossa causou

| Divergência | Restrição |
|---|---|
| encerrar é **marcar** e esperar ponto seguro; há fase não cancelável | Rust não mata thread, e a **integridade** não aceita slot sem índice (`telemetria.rs:35-45`) |
| «bloqueando» é exato, sem grafo | a trava de dados é **uma** só |
| o «digest» é op+tabela, não texto normalizado | o protocolo é JSON estruturado |
| log em arquivo JSON Lines, não em tabela | o vermelho de disco chega quando o motor não grava; molde do `diretivas.rs` |
| SVG à mão, física à mão | **zero dependências** |
| ninguém mata tarefa de sistema, nem o administrador | a régua (7 × 2) e o fato de a réplica ser o que segura a **ordem de digitação** do outro lado |
| cancelar dentro de transação aborta a transação inteira | o modelo de erro por classe já decidido (`transacao.rs:144`), igual ao PostgreSQL e oposto ao MySQL |

---

## 9. Lacunas que este documento não fecha

- **Idera SQL Check**: o significado de tamanho e cor das bolhas não está em fonte pública achada;
  o desenho não depende dele.
- **Custo do histograma no laço quente** e **volume do `aquario.log`**: raciocinados; a F1 e a F2
  medem.
- **Taxa de falso alarme em produção**: os 0,07% são de dois logs de bancada.
- **Prazo de «origem inalcançável»** (3 min) e **«3 vezes em 5 min»** do PRAZO: raciocinados.

## 10. O que sobe ao dono

> **A0 (§11.6):** continua **nada**; três decisões ficaram visíveis para veto.

**Nada.** Comportamento de banco decidido pelas réguas (dois níveis de matar, ver ≠ matar, proteger
tarefa de sistema, mostrar quem bloqueia, ociosa-em-transação); método de monitor decidido pela lei
da casa e pela medição. Ficam **visíveis para veto**, sem pergunta: integridade = amarelo (§2.4) e o
alcance do «custo zero» (§3.1).

---

## 11. A0 — uma base, um alarme, uma contagem com o 495/496 (09/10/2026, papel J)

Lei aplicada: **função e comando vêm do mesmo motor.** Esta seção **revoga**, onde contradiz, o §2.2,
o nome do §2.4, o §3.1 e o §4.1 deste documento, e o §0 item 6, o §2 linha 5, o §4 e a F4 do
`ia-495-496-desenho.md`. Árvore lida: `5569ee93`. Fan-out dispensado e registrado: esta instância
não tem ferramenta de subagente; motor e bancada pelo J direto. Scripts no Apêndice B.

### 11.1 (a) A linha de base — hipóteses escritas antes de medir

| # | hipótese | veredito, com o número |
|---|---|---|
| Ha1 | a do aquário: histograma 4/oitava, `2 × p95`, n ≥ 20, piso 250 ms | **morre.** Custo **118,7 ns** contra **37,0 ns** (3,2×; faixas 97,6–128,5 × 28,9–39,1, não se cruzam); **652 B** contra **48 B** por chave (13,6×); falso alarme em lognormal σ = 1: **0,612%** contra **0,004%** (150×). No log real empata com a vencedora (1/9.317) |
| Ha2 | a da IA: Welford **cumulativo** sobre `ln(µs)`, z ≥ 4, n ≥ 30, sem exclusão, sem piso, **desligada** | **morre em pedaços.** Sem a exclusão: **51** alarmes contra 37 nos 24 logs, 14 deles o `replicar_aguardar`; sem piso: **40/9.317** — ruído de 1 ms de resolução (29 `commit`) — contra **1**; n ≥ 30 não compra nada: aquecimento **0,018–0,020%** com 20 e com 30 (4.000 chaves × 60); cumulativo não esquece o habitual de ontem (raciocinado); desligada: §11.2 |
| Ha3 | duas bases, uma por consumidor | **morre pela lei**: duas respostas para «isto é anormal?» pintariam a bolha de uma cor e mandariam o e-mail por outra |
| **Ha4** | **Welford sobre `ln(µs)` em duas metades de 30 min, n ≥ 20, z ≥ 4 e serviço ≥ 250 ms, sem o que espera por desenho** | **entra.** **1/9.317** (o `inserir` de 2.005 ms contra habitual de 5 ms — o mesmo único alarme da Ha1); 37,0 ns |

E uma premissa que os dois documentos supunham, **medida**: a grandeza tem de ser **µs**. No log da
sonda ODBC, **170 de 184** linhas (92%) têm `ms = 0`, e **152** delas são `sql` — em ms a base do
`sql` é cega. O `Acesso` ganha `us` (aditivo; o `de_json` tolera ausente, `acesso.rs:96-98`), da
mesma medida que já vai ao `duracao_ms`: nenhum `Instant` novo.

**A base única:**

| parâmetro | valor | de onde |
|---|---|---|
| chave | `op` + `database.tabela`; para `op = sql`, a **digital** (FNV-1a 64 sobre os símbolos, F1 do 495, sem `String`) | aquário §2.2 + IA §2 linha 3 (converge 9 × 0) |
| grandeza | tempo de **serviço** (duração − espera na trava), µs; e **linhas devolvidas** | aquário (a vítima da fila não vira anormal) + IA |
| estatística | Welford sobre `ln(µs)` e `ln(1 + linhas)` | Ha4 |
| janela | duas metades de 30 min, unidas na leitura pela fórmula de Chan (O(1)) | aquário (5 min absorve a tempestade); *raciocinado, não medido* — os logs têm minutos |
| n mínimo | **20**; abaixo vale o limiar fixo de hoje (`alto_uso_ms`/`stress_ms`) | medido (Ha2) |
| anormal | **z ≥ 4 e serviço ≥ 250 ms**, com o desvio sob um **chão de 0,1 em `ln`** (≈ 10%; *raciocinado*) — sem ele a série constante dá desvio 0, e o `z` do Apêndice A da IA devolve 0: 20 × 1 ms exatos e um de 10 s **não alarmaria**; nas linhas, **z ≥ 4 e linhas ≥ 1.000** (o «≥ 1.000» do C10/C11; *raciocinado*: o log não guarda linhas) | Ha4 |
| exclusões | `OPS_DE_REPLICACAO` (`servidor.rs:389`) e as tarefas do sistema | aquário; a IA não tinha |
| teto | **5.000 chaves + linha-coringa** | IA C1 (5 × 4). O «1.024, a mais fria sai» do §2.2 **morre**: despejo é o do PG, que perdeu o voto, e quem inunda chaves apagaria o habitual dos outros |
| memória | 2 grandezas × 2 metades × 24 B + máx, erros, primeira/última ≈ **128 B/chave**, ≈ **640 KiB** no teto | raciocinado |
| onde mora | o `anotar` (único sumidouro) | aquário |
| o que mostra | `z`, `n` e o **p95 habitual estimado** = `exp(média + 1,645 · desvio)` | a tela fala a língua que o dono aceitou («p95 da própria operação e tabela»), e o corte é o z |

O 707 diz «critérios de grande, anormal e warning **decididos pelo pesquisador**»: trocar `2 × p95`
por `z ≥ 4` sobre `ln` é decisão do J, não sobe.

<!-- aquario-regra:inicio (gerado por bancada/aquario/regra.py --gravar; nao editar) -->

logs reais lidos: 24 (esperados 24)

| regra sobre os logs reais | alarmes | de | % | ops mais frequentes |
|---|---|---|---|---|
| A hist 2xp95 piso250 excl | 1 | 9317 | 0.01% | [('inserir', 1)] |
| A sem excl | 15 | 10282 | 0.15% | [('replicar_aguardar', 14), ('inserir', 1)] |
| A sem piso excl | 39 | 9317 | 0.42% | [('commit', 30), ('inserir', 6), ('sql', 2), ('atualizar', 1)] |
| B welford ln z4 sem excl | 51 | 10282 | 0.50% | [('commit', 27), ('replicar_aguardar', 14), ('inserir', 9), ('sql', 1)] |
| B welford excl | 37 | 9317 | 0.40% | [('commit', 27), ('inserir', 9), ('sql', 1)] |
| C unificada n20 z4 piso250 excl | 1 | 9317 | 0.01% | [('inserir', 1)] |
| C sem piso | 40 | 9317 | 0.43% | [('commit', 29), ('inserir', 9), ('sql', 1), ('atualizar', 1)] |

| caso do F4 (30 amostras ~1 ms) | 200 ms alarma? | 1,3 ms alarma? |
|---|---|---|
| Hist piso 250000 | False | False |
| Hist piso 0 | True | False |
| Welf piso 0 | True | False |

| sintetico (semente 7, 200.000) | estatistica | falso alarme |
|---|---|---|
| lognormal s=1 | hist 2xp95 | 0.612% |
| lognormal s=1 | welford z4 | 0.004% |
| lognormal s=0,3 | hist 2xp95 | 0.001% |
| lognormal s=0,3 | welford z4 | 0.003% |
| bimodal 90/10 400us/6ms | hist 2xp95 | 0.001% |
| bimodal 90/10 400us/6ms | welford z4 | 0.000% |
| bimodal 99/1 400us/6ms | hist 2xp95 | 1.028% |
| bimodal 99/1 400us/6ms | welford z4 | 1.028% |

| aquecimento (semente 11, 4.000 chaves x 60) | falso alarme |
|---|---|
| welford z4 n>=20 sigma=0.3 | 0.020% (32/160000) |
| welford z4 n>=20 sigma=1.0 | 0.019% (31/160000) |
| welford z4 n>=30 sigma=0.3 | 0.018% (21/120000) |
| welford z4 n>=30 sigma=1.0 | 0.020% (24/120000) |

| conferencia contra o par 11 | escrito | regerado | |
|---|---|---|---|
| Ha1 alarmes (hist 2xp95 piso250 excl) | 1 | 1 | ok |
| Ha1 avaliacoes | 9317 | 9317 | ok |
| Ha4 alarmes (unificada n20 z4 piso250 excl) | 1 | 1 | ok |
| Ha4 avaliacoes | 9317 | 9317 | ok |
| sem piso, Ha4 (C sem piso) | 40 | 40 | ok |
| sem piso, Ha1 (A sem piso excl) | 39 | 39 | ok |
| Welford sem exclusao | 51 | 51 | ok |
| Welford com exclusao | 37 | 37 | ok |
| Hist sem exclusao | 15 | 15 | ok |
| lognormal s=1 hist 2xp95 (%) | 0.612 | 0.612 | ok |
| lognormal s=1 welford z4 (%) | 0.004 | 0.004 | ok |

<!-- aquario-regra:fim -->

### 11.2 Quem nasce ligado

- **A base nasce LIGADA, atrás do portão `telemetria.ligada()`**, que já nasce ligado
  (`Telemetria::default()` = `nova(true)`, `telemetria.rs:984`; `servico_nucleo_01.rs:457`).
  `telemetria_desligar` a zera: **0,51 ns** (IA, Apêndice A).
- **Morre** a «desligada 7 × 2» da IA. Não é a régua errando: é a régua **alcançada por ordem do
  dono posterior** — 707, 08/10: «anormal pelo p95 da própria operação e tabela (**não por tamanho
  fixo**)». Desligada, o padrão de fábrica seria justamente o tamanho fixo que o dono recusou.
- **Preço ligado:** protocolo **37 ns** (medido hoje, carga 5,1) ≈ 0,011% de um pedido de 322 µs
  (citado de 24/09); `sql` + digital ≈ 741 ns (citado, teto) ≈ 0,23%; somado ao detector de 4
  classes, que já nasce ligado (885 ns, citado), ≈ **0,5%** — abaixo do critério **≤ 1%** que os
  dois documentos já tinham. Decide a bancada A/B da A4.
- **Pétrea:** o portão vem antes do hash, da digital e da trava. RED: telemetria desligada → zero
  atualizações (contador), no molde do `testes_profiler_desligado`.
- O e-mail do `fora_do_habitual` continua atrás de `alertas.ligado` (o padrão da IA para o vigia:
  amostra sempre, o interruptor decide só o e-mail).

### 11.3 (b) `Alarme` = o tipo da `Ocorrencia`

| # | hipótese | veredito |
|---|---|---|
| Hb1 | um log só para tudo | **morre.** (1) Retenção: o rodízio de 64 MiB cobre ≈ 8 h de carga ruim (§4.3, raciocinado) — a ocorrência de força bruta seria apagada pelo volume do próprio ataque. (2) Leitor: o aquário é lido pelo `monitorar` e pela TV, com usuário pseudonimizado e **sem IP** (decisão do dono 09/10); a ocorrência de segurança leva IP ao administrador. Um log só obriga um filtro na leitura, e o filtro esquecido vaza o IP. (3) Maduros: MySQL separa por papel (seis logs, [server-logs](https://dev.mysql.com/doc/refman/8.4/en/server-logs.html)), MariaDB põe a auditoria em arquivo próprio (`server_audit`), o PG usa um log com severidade → separar **5 × 4** |
| Hb2 | dois enums, um por documento | **morre pela lei** |
| **Hb3** | **um enum, um produtor, dois arquivos com papéis diferentes** | **entra** |

- **`enum Alarme`, um só** (nome do aquário: `Sinal` colide com `sinais.rs`). É o `tipo` da
  `Ocorrencia` da F2. Cada variante responde `gravidade()` (vermelho/amarelo), `grupo()` (LOCK,
  DISCO, DADO, RÉPLICA, SEGURANÇA, PRAZO e as famílias da IA: ataque, defeito, previsão) e
  `escopo()` (tarefa → bit no `AtomicU32` da `Atividade`, ≤ 32 variantes; servidor → sedimento).
  A chave da fábrica `aquario.motivo.<variante>` é o mesmo texto do e-mail.
- **Um produtor:** `telemetria::sinal(Alarme::X, dados)` marca o bit (quando é de tarefa) **e**
  entrega a `Ocorrencia` à camada (silêncio → fila → carteiro). Nenhum outro caminho cria ocorrência.
- **`ocorrencias.log`** guarda o **fato** (o alarme e por quê), redigido, para o `administrar`.
  **`aquario.log`** guarda a **linha do tempo** das tarefas e a contagem; a linha `mudou` leva a
  chave do `Alarme` e `"ocorrencia": <id>` — repete o valor, nunca a decisão.
- Os dois (e o `acessos.log`) pelo **mesmo escritor** (`registrar_json` do `LogAcessos`
  generalizado, §4.1). O «grande e anormal» (azul escuro) é **classe**, não alarme: o
  `fora_do_habitual` é `Alarme` amarelo, e a `classificar()` única o pinta de azul escuro quando a
  tarefa é grande.

### 11.4 (c) A contagem dos três gráficos

| # | hipótese | veredito |
|---|---|---|
| Hc1 | unidade = **linha** (PG `tup_inserted`… «Number of rows», [monitoring-stats](https://www.postgresql.org/docs/current/monitoring-stats.html)) | **morre 5 × 4**: MySQL e MariaDB contam **instrução** (`Com_xxx`, «number of times each xxx statement has been executed», [status](https://dev.mysql.com/doc/refman/8.4/en/server-status-variables.html)); e a ordem do dono põe na mesma régua backup, erro e aviso, que não têm linha |
| Hc2 | barra = **tentativas** (`Com_stmt_xxx`: «correspond to the number of requests issued») | **morre por razão nossa**: o backup que falhou subiria a barra de backup — mentira sobre a cópia (C6 do 496). A página não diz isso do `Com_xxx` geral, só do `Com_stmt_xxx` |
| Hc3 | erro/aviso = vermelho/amarelo do aquário | **morre.** Erro de sintaxe e login recusado sumiriam do gráfico «erros»; e os três maduros definem pelo **desfecho da instrução** — PG `ERROR` «caused the current command to abort», `WARNING` «warnings of likely problems» ([severidade](https://www.postgresql.org/docs/current/runtime-config-logging.html)); MySQL `ERRORS` = SQLSTATE fora de `00`/`01`, `WARNINGS` = «the number of warnings, from the statement diagnostics area» ([events_statements_current](https://dev.mysql.com/doc/refman/8.4/en/performance-schema-events-statements-current-table.html)); MariaDB, a mesma P_S → **converge 9 × 0, aceite** |
| Hc4 | **dia** fora do rodízio (`aquario-dias.jsonl`, plano) | **morre.** O motor não tem fuso (`datahora.rs:164`: «em lugar nenhum deste motor existe fuso»; tudo UTC). Dia UTC põe **3 das 24 h** do dia de Brasília (UTC−3) no dia errado: 12,5% |
| **Hc5** | **hora** fora do rodízio; a tela soma no fuso do navegador | **entra** |

**O motor da contagem — um acumulador, três retenções:**

- **Um acumulador** no `Telemetria`: oito `AtomicU64` do minuto corrente, somados pelo `anotar`
  (um ponto), atrás do mesmo portão. A categoria **decide-a a op que executou** e devolve-a com a
  resposta (`categoria` no `Acesso`, em memória) — o modo do `excluir` mora na resposta, e o
  `anotar` adivinhando pelo nome da op seria a segunda decisão.
- **Minuto, no `aquario.log`:** `{"evento":"contagem","minuto_ms":…,"c":{"select":…,"insert":…,
  "update":…,"excluir_suave":…,"excluir_fisico":…,"backup":…,"erro":…,"aviso":…}}`, escrita pelo
  amostrador de 1 s na virada, **sempre que a telemetria está ligada, inclusive com zeros**. É isso
  que separa **0** (ligado, nada aconteceu) de **ausente** (`null`: servidor fora ou telemetria
  desligada). ≈ 150 B × 1.440 ≈ 216 KB/dia (raciocinado).
- **Hora, em `aquario-horas.jsonl`**, fora do rodízio, só acrescenta: uma linha por hora UTC
  fechada, com as oito somas e `minutos_medidos` (0–60). ≈ 4 KB/dia, ≈ 1,5 MB/ano (raciocinado). A
  hora fecha da **memória**; no arranque no meio da hora, refaz-se das linhas de minuto, e minuto
  comido pelo rodízio fica fora de `minutos_medidos` — o gráfico desenha parcial, nunca inventa.
- **Dia, semana e mês: a tela soma as horas**, no fuso do navegador. O gráfico A (ao vivo) = horas
  fechadas de hoje + minutos da hora corrente + o minuto parcial, que vem na mesma chamada
  `telemetria` com `vista: "aquario"`.
- «Motor único: nada de segundo contador» (dono, item 8): a hora é o **fecho** do minuto, não outro
  contador.

**As oito séries — conta-se um por pedido que passou pelo `anotar`** (o derivado do
`executar_derivado` não conta de novo; lote de 5.000 = 1; `UPDATE` por faixa = 1):

| série | conta |
|---|---|
| `select` | `ok`, op que devolve linhas — a lista sai de **uma** constante do código (hoje `OPS_QUE_DEVOLVEM_LINHAS`, local em `servico_consulta_01.rs:404`; sobe ao módulo e recebe as que faltam, como `ler`, `juntar`, `unir`, `pivotar`, com teste contra o despacho) —, ou `sql` cuja instrução **analisada** é `SELECT` |
| `insert` | `ok`, `inserir` ou `sql` `INSERT` |
| `update` | `ok`, `atualizar` ou `sql` `UPDATE` |
| `excluir_suave` | `ok`, `excluir` cuja **resposta** diz `"modo":"suave"`; `sql` `DELETE` (o derivado é suave, `servico_sql_01.rs:446/541`) |
| `excluir_fisico` | `ok`, `excluir` cuja resposta diz `"modo":"fisico"` — o desfecho, não a bandeira do pedido |
| `backup` | `ok`, backup terminado (op `backup` e job) |
| **`erro`** | **`ok: false`, qualquer op e qualquer código** — inclusive login recusado, sintaxe e integridade. **Não** conta na barra da categoria |
| **`aviso`** | **`ok: true` com `aviso` ou `avisos` não vazio no nível de cima da resposta.** Conta **também** na barra da categoria |

Transação: conta na hora do pedido, mesmo que um `ROLLBACK` depois desfaça (a instrução executada,
como o `Com_xxx`). Linhas devolvidas/gravadas por período: ⏸. A legenda diz, pela fábrica, que
erro/aviso é o desfecho da instrução e vermelho/amarelo é a gravidade para o servidor.

**RED da A8:** `excluir` físico contado como suave → cai; backup que falhou → barra de backup
parada e `erro` +1; servidor derrubado 2 min → 2 minutos `null` e `minutos_medidos` = 58, nunca
zero; telemetria desligada → acumulador parado; hora UTC 01:00 cai no dia anterior na tela de
Brasília.

### 11.5 O que muda nas fatias (o `plano-0.21.md` foi atualizado)

- **A1** regera esta seção pelo Apêndice B (Ha1 × Ha4: 1/9.317 cada; sem piso 39 × 40; sem
  exclusão 15 × 51), não mais «59/15/1».
- **A2** nasce com o `enum Alarme` e o `registrar_json`: A3, A4, A6 e a F2 os usam, e quem os
  escrevesse primeiro viraria dono por acidente.
- **A4 ≡ F4 do 495**: uma fatia, um arquivo, Welford.
- **A8**: `aquario-horas.jsonl`, não `-dias`.

### 11.6 Sobe ao dono?

**Nada** — nem produto nem choque com pétrea. Ficam visíveis para veto: (1) a base **ligada**
contra o voto 7 × 2, pela força do 707; (2) erro/aviso do gráfico ≠ vermelho/amarelo, com a
legenda; (3) o «p95» que o dono leu vira **estimado e mostrado**, e o corte é `z ≥ 4`.

### 11.7 Lacunas

- **µs não existe em log nenhum**: a comparação Ha1 × Ha4 em tráfego real foi em ms, com `0 → 0,5`.
  Decide a A1, com o campo `us`.
- **Moda rara** (1% das execuções 15× mais lentas): as duas estatísticas acusam **1,03%** — nenhuma
  separa; o piso de 250 ms é o que protege quando a moda rara é rápida.
- Janela de 30 min, piso de 1.000 linhas e os volumes dos arquivos: raciocinados.
- 322 µs, 741 ns e 885 ns: de 24/09, não remedidos. A bancada de hoje rodou sob carga 5,1.
- 24 logs de bancada, nenhum de produção.

## 12. Pedido 780 — depois do teto, ANÉIS até a bolha preta (09/10/2026, papel E)

Ordem do dono: *«o tamanho aumenta com o tempo de execução até uma medida máxima que ainda
permita ver as outras bolhas; daí começa a ter mais cores internas ou camadas até ficar preta».*
O raio já parava aos **30 s** (`raioAlvo`, 9 → 46 px, logarítmico). O tempo a mais vira anel.

**A escala: cada DOBRA de tempo além do teto é um anel, cinco até a preta.**

| anel | 1 | 2 | 3 | 4 | 5 (preta) |
|---|---|---|---|---|---|
| rodando há | 1 min | 2 min | 4 min | 8 min | 16 min |

- **Dobra, e não minuto:** o raio é logarítmico até o teto, e o anel continua a mesma régua —
  cada anel diz «demorou o dobro». Um anel por minuto pintaria de preto aos 6 min uma carga que
  é normal levar 15, e diria a mesma coisa para 6 min e para 6 h.
- **Cinco:** com o raio no teto (46 px) os anéis ficam a ~7,7 px um do outro; com a `escala`
  do tanque encolhendo tudo à metade, ainda ~3,8 px. Seis ou mais viram borrão, e deixam de ser
  forma.
- **Desenho:** anel *i* é a forma da bolha (círculo, losango, octógono) no raio
  `r·(1 − i/6)`; cada um escurece o miolo (`fill #000` a 0,22, acumulando para o centro); no
  quinto a bolha inteira fica `#000` opaca. O traço do anel é **duplo** — escuro 2,6 px por
  baixo, claro 1,1 px por cima —, e por isso aparece no papel e no miolo preto. A cor de
  gravidade fica na **borda**, intacta. A preta ganha **halo** na cor do texto a `r + 2,5`.
- **Contraste medido** (`testes-web/prova-780-aneis.mjs`, Chromium, 09/10): preto × fundo
  **1,03:1** no escuro (`#010418`) — é por isso que o halo existe —; halo × fundo **15,65:1**
  (escuro) e **16,96:1** (claro); borda azul-escuro × fundo **11,30:1** / **9,41:1**; traço
  claro do anel × preto **19,23:1**.

**Onde mora — um motor só.** A escala é do servidor (`aquario/anel.rs`): o `aquario.log` tem
de gravar a troca de anel sem ninguém olhando (item 5 do dono), então quem decide é quem grava.
O amostrador, de segundo em segundo e só com a telemetria ligada, grava a linha `anel`
(`dados: {anel, de}`, com `ms`, a cor e o motivo da classe) uma vez por subida — o serial do
pedido mora no mesmo atômico do anel gravado, e o pedido seguinte recomeça do zero. O retrato
manda `anel` em cada bolha e a tabela `limiares.aneis_ms`; a tela desenha o `anel` e, na volta
de 5 min (sem retrato), conta os limites da tabela — lê, não recalcula. O teto de 30 s existe
nos dois lados (`msGrande` do JS e `TETO_DO_RAIO_MS`), e um teste lê o literal do JS e compara.

**O motivo** «rodando há N min, anel K de M» sai pela chave `tela.aq_anel_motivo` no `<title>`,
no cartão de quem encerra, na **dica do toque** (quem não pode encerrar — a TV, quem só
monitora — toca na bolha e lê o que o `<title>` diz: no toque não há hover) e na linha `anel`
da linha do tempo. Custo com o aquário fechado: zero na tela (o desenho só roda no laço); no
servidor, uma leitura de relógio e um atômico por tarefa viva por segundo.

## 13. Pedido 783 — o PERFIL VISUAL configurável (09/10/2026, papel E)

Ordem do dono: *«configuração de cores, tamanhos, texturas, efeitos de animação, tipografia de
fontes, cores e tamanhos para não serem valores fixos».* O que era `CORES`/`M`/folha cravados no
`ui/aquario.js` passa a ter um perfil, com os valores de hoje como fábrica.

**Onde mora — e por quê.** No bloco `aquario` do `config.json`, gravado pelo `config_gravar`, o
mesmo caminho das cores do painel da telemetria (`telemetria.cor_*`), que é a preferência de
tela que a casa já guardava no servidor. Uma tabela `phxsys` pediria de novo o portão
(`administrar`), a trilha (diário das diretivas, com valor de antes e de depois) e a escrita
atômica; o `config_gravar` já tem os três. O perfil é do **servidor** (a TV e o administrador
veem o mesmo aquário); preferência por pessoa continua no `localStorage` (a altura da alça).

**Um motor só.** A regra mora em `crates/phxsql-server/src/aquario/perfil.rs`: a `Perfil::ler`
devolve o perfil **e** as recusas; o arranque transforma recusa em aviso e cai no de fábrica
campo a campo (cor torta não derruba o servidor, a mesma decisão do `telemetria.cor_*`); a
gravação (`Config::gravar_arvore`, quando muda algum `aquario.*`) recusa com todos os motivos e
o arquivo não muda. O retrato (`aquario_retrato`) traz sempre a digital (`perfil_versao`, FNV-1a
do bloco) e o perfil inteiro **só** quando a tela manda uma digital diferente — a tela normal, a
alça e a TV leem pelo pedido que já fazem, e a TV aberta muda sozinha no retrato seguinte.
Digital e não contador: contador recomeçaria no arranque e a TV acharia que já tem o perfil.

**O que se configura, com a faixa** (fábrica entre parênteses; um teste lê cada literal do `M`):

| campo | faixa | por que o teto |
|---|---|---|
| `cor_<cor>` / `cor_<cor>_claro` (6 × 2) | `#rrggbb` ou vazio (= a do tema) | ≥ **3:1** contra `#010418` / `#f7f5f2`, e matiz na **família** da cor |
| `raio_min` (9) | 4–20 px | a nova tem de ser vista e não nascer grande |
| `raio_max` (46) | 24–64 px, e ≥ 2× `raio_min` | no menor tanque da TV (320 px, 5 faixas de 64) a maior não passa de duas faixas — é o que o 780 protege |
| `ocupacao` (0,46) | 0,2–0,6 | acima, sem água para as bolhas se moverem |
| `cresce` (2,2) · `mola_faixa` (5) · `atrito` (1,6) | 0,5–6 · 1–12 · 0,3–4 | física legível |
| `quique` (0,35) | 0–0,9 | 1 = colisão sem perda, as bolhas nunca assentam |
| `estouro_ms` (380) | 120–1500 | abaixo não se vê; acima a que acabou parece viva |
| `rotulo_min` (15) | 10–30 px, e < `raio_max` | senão nenhuma bolha mostra o nome |
| `espessura` · `traco_escala` (1 · 1) | 0,5–2 × | escalam o traço; o **padrão** do tracejado é sinal e não muda |
| `preenchimento` (0,14) | 0,05–0,4 | acima vira fundo cheio — a casa pinta contorno |
| `fonte_rotulo_px` (10) · `fonte_faixa_px` (11) | 8–16 · 9–18 | — |
| `fonte` (Exo 2) | Exo 2, IBM Plex Mono, Helvetica Neue, Arial, system-ui | só o nome na bolha e o das faixas; título é Exo 2, é marca |

**O que NÃO se configura:** o **significado** da cor (vermelho é alarme e afunda — a janela de
matiz recusa um «vermelho» verde ou cinza); a **forma** e o **padrão** do traço; o `msGrande`
(= `TETO_DO_RAIO_MS` do 780 — duas fórmulas se um lado mudasse); `passagens` e `folga` (a
garantia de não sobrepor, não aparência).

**A tela.** Configurações → Gerais do servidor → «Aparência do aquário», com **dois tanques de
pré-visualização** (escuro e claro, porque a regra do contraste vale nos dois) que leem o
**rascunho** por `op.perfil` — o aquário de verdade aberto ao lado, na multitela, não muda antes
de salvar. Salvar é amarelo (altera), contorno. A faixa de cada controle vem do servidor.

**Provas.** Rust: `aquario::perfil::testes` (fábrica = literais do `M` e do CSS do `index.html`;
recusa de contraste nos dois temas, de família, de faixa, de fonte, do par de raios; sem bloco
nada muda; arranque avisa) e `testes_config_gravar` (grava, vale no retrato pela digital, fica no
diário; recusa sem tocar no arquivo; sem `administrar` não grava). RED medido: sem a
`conferir_secao` na gravação, `#2a0606` grava e o arranque o descarta calado (`no_arquivo`
acusa); sem o `definir_perfil_do_aquario` no `config_gravar`, o retrato continua no de fábrica.
Navegador: o caso `perfil-do-aquario` da bateria (TV aberta antes de salvar muda sozinha para o
tom e o raio 60; aquário normal igual; contraste ruim recusado pelo servidor com o motivo) e
`testes-web/prova-783-fabrica.mjs`, que roda o `aquario.js` de antes e o de agora com as mesmas
tarefas e semente: o SVG de fábrica sai **idêntico** (5.872 × 5.872 caracteres).

**Custo com o aquário fechado:** zero na tela (o perfil só é lido no retrato); no servidor, uma
`String` de 16 bytes comparada por retrato, e o perfil montado uma vez por gravação.

## Apêndice B — os medidores da A0 (refazer: `python3 regras.py`, `python3 nmin.py`, `rustc -O --edition 2021 bench.rs && ./bench`)

Vão para `bancada/aquario/` pela A1 (script que resolveu não morre com a sessão).

`regras.py` — as regras sobre os `acessos.log` e os sintéticos:

```python
import json, math, glob, random, sys, collections
REPL={'posicao','replicar','retrato_da_replica','aplicar','cluster_pulso','replicar_aguardar'}  # OPS_DE_REPLICACAO, servidor.rs:389
def balde(x):  # 4 por oitava, x em unidade >=1
    if x<1: return 0
    return int(math.floor(4*math.log2(x)))+1
def p95_hist(h):
    n=sum(h.values()); alvo=math.ceil(0.95*n); a=0
    for b in sorted(h):
        a+=h[b]
        if a>=alvo: return 2**(b/4) if b>0 else 1  # topo do balde
class Hist:
    def __init__(s): s.h=collections.Counter(); s.n=0
    def anormal(s,x,piso):
        if s.n<20: return False
        return x>=max(2*p95_hist(s.h),piso)
    def somar(s,x): s.h[balde(x)]+=1; s.n+=1
class Welf:  # a da IA: n>=30, z>=4 sobre ln, sem piso
    NM=30
    def __init__(s): s.n=0; s.m=0.0; s.m2=0.0
    def anormal(s,x,_):
        if s.n<s.NM: return False
        sd=math.sqrt(s.m2/(s.n-1)) if s.n>1 else 0
        if sd==0: return False
        return (math.log(x)-s.m)/sd>=4
    def somar(s,x):
        v=math.log(x); s.n+=1; d=v-s.m; s.m+=d/s.n; s.m2+=d*(v-s.m)
class WelfPiso(Welf):  # a unificada (A0): n>=20, z>=4 sobre ln, piso
    NM=20
    def anormal(s,x,piso): return x>=piso and Welf.anormal(s,x,piso)
def rodar(eventos, Cls, piso, excluir):
    base={}; al=0; av=0; quais=[]
    for op,tab,x in eventos:
        if excluir and op in REPL: continue
        k=(op,tab); b=base.setdefault(k,Cls()); av+=1
        if b.anormal(x,piso): al+=1; quais.append((op,x))
        b.somar(x)
    return al,av,quais
def logreal(f, minimo):
    for l in open(f):
        d=json.loads(l); yield d['op'],d.get('tabela',''),max(d.get('ms',0),minimo)
if __name__=='__main__':
    fs=['/tmp/phx-207-quorum-real/no1/acessos.log','/tmp/phx-odbc-sonda/acessos.log']+sorted(glob.glob('/tmp/phx-custo-tx-4992-*/acessos.log'))
    for nome,Cls,piso,exc,minimo in [('A hist 2xp95 piso250 excl',Hist,250,1,1),('A sem excl',Hist,250,0,1),('A sem piso excl',Hist,0,1,1),
                                    ('B welford ln z4 sem excl',Welf,0,0,0.5),('B welford excl',Welf,0,1,0.5),
                                    ('C unificada n20 z4 piso250 excl',WelfPiso,250,1,0.5),('C sem piso',WelfPiso,0,1,0.5)]:
        tot=[0,0]; q=collections.Counter()
        for f in fs:
            a,v,qq=rodar(list(logreal(f,minimo)),Cls,piso,exc); tot[0]+=a; tot[1]+=v; q.update(o for o,_ in qq)
        print(f'{nome:28} alarmes {tot[0]:4} de {tot[1]:5} ({100*tot[0]/max(tot[1],1):.2f}%)  {q.most_common(4)}')
    # casos sinteticos do F4 da IA (em us)
    for Cls,piso in [(Hist,250_000),(Hist,0),(Welf,0)]:
        r=[]
        for ult in (200_000,1_300):
            b=Cls(); 
            for i in range(30): b.somar(1000*(1+0.05*((i%5)-2)))
            r.append(b.anormal(ult,piso))
        print(Cls.__name__,'piso',piso,'-> 200ms alarma?',r[0],' 1,3ms alarma?',r[1])
    # sintetico: bimodal (90% 0,4 ms acerto de cache, 10% 6 ms falta) e lognormal sigma 1, 200k cada
    random.seed(7)
    for nome,gen in [('lognormal s=1',lambda: math.exp(random.gauss(math.log(800),1.0))),
                     ('lognormal s=0,3',lambda: math.exp(random.gauss(math.log(800),0.3))),
                     ('bimodal 90/10 400us/6ms',lambda: random.gauss(400,40) if random.random()<.9 else random.gauss(6000,600)),
                     ('bimodal 99/1 400us/6ms',lambda: random.gauss(400,40) if random.random()<.99 else random.gauss(6000,600))]:
        ev=[('x','t',max(gen(),1)) for _ in range(200_000)]
        for cn,Cls in (('hist 2xp95',Hist),('welford z4',Welf)):
            a,v,_=rodar(ev,Cls,0,0); print(f'  {nome:24} {cn:11} falso alarme {100*a/v:.3f}%')
```

`nmin.py` — o aquecimento com n ≥ 20 e n ≥ 30:

```python
import math,random
from regras import Welf, Hist
random.seed(11)
for nmin in (20,30):
  for sig in (0.3,1.0):
    al=0;av=0
    for k in range(4000):   # 4000 chaves, 60 amostras cada (aquecimento)
        b=Welf(); 
        for i in range(60):
            x=math.exp(random.gauss(math.log(800),sig))
            if b.n>=nmin:
                sd=math.sqrt(b.m2/(b.n-1)); av+=1
                if sd>0 and (math.log(x)-b.m)/sd>=4: al+=1
            b.somar(x)
    print(f'welford z4 n>={nmin} sigma={sig}: falso alarme no aquecimento {100*al/av:.3f}% ({al}/{av})')
```

`bench.rs` — o custo das duas estatísticas (mediana de 9 voltas × 2 M):

```rust
use std::collections::HashMap; use std::hint::black_box; use std::sync::Mutex; use std::time::Instant;
#[derive(Default,Clone,Copy)] struct W{n:u32,m:f64,m2:f64}
impl W{fn somar(&mut self,x:f64){let v=x.ln();self.n+=1;let d=v-self.m;self.m+=d/self.n as f64;self.m2+=d*(v-self.m);}
 fn z(&self,x:f64)->f64{if self.n<30{return 0.0}let s=(self.m2/(self.n-1) as f64).sqrt();if s==0.0{0.0}else{(x.ln()-self.m)/s}}}
#[derive(Default,Clone,Copy)] struct Wb{a:W,b:W} // duas metades
impl Wb{fn z(&self,x:f64)->f64{ // Chan: une as duas metades
 let (a,b)=(self.a,self.b); let n=a.n+b.n; if n<30{return 0.0}
 let d=b.m-a.m; let m=a.m+d*b.n as f64/n as f64; let m2=a.m2+b.m2+d*d*a.n as f64*b.n as f64/n as f64;
 let s=(m2/(n-1) as f64).sqrt(); if s==0.0{0.0}else{(x.ln()-m)/s}}}
#[derive(Clone,Copy)] struct H{c:[[u32;81];2],n:u32}
impl Default for H{fn default()->Self{H{c:[[0;81];2],n:0}}}
fn balde(us:u64)->usize{ if us<1000{return 0} let ms=us/1000; let l=63-ms.leading_zeros() as usize; let f=((ms<<2)>>l) as usize & 3; (1+4*l+f).min(80)}
impl H{fn somar(&mut self,x:u64){self.c[0][balde(x)]+=1;self.n+=1}
 fn p95(&self)->usize{ if self.n<20{return 0} let alvo=(self.n as u64*95).div_ceil(100) as u32; let mut a=0; for i in 0..81{a+=self.c[0][i]+self.c[1][i]; if a>=alvo{return i}} 80}}
fn med(mut v:Vec<f64>)->(f64,f64,f64){v.sort_by(|a,b|a.partial_cmp(b).unwrap());(v[0],v[v.len()/2],v[v.len()-1])}
fn main(){ const N:usize=2_000_000; let ks:Vec<u64>=(0..5000u64).map(|i|i.wrapping_mul(0x9E3779B97F4A7C15)).collect();
 let mut r:Vec<(&str,Vec<f64>)>=vec![("welford 1 metade",vec![]),("welford 2 metades (Chan)",vec![]),("histograma 81x2 + p95 por varredura",vec![])];
 for _ in 0..9{
  let t:Mutex<HashMap<u64,W>>=Mutex::new(HashMap::with_capacity(5000)); let i0=Instant::now(); let mut z=0.0;
  for i in 0..N{let x=((i%97)*30+400) as f64; let mut g=t.lock().unwrap(); let b=g.entry(ks[i%5000]).or_default(); z+=b.z(x); b.somar(x);} black_box(z); r[0].1.push(i0.elapsed().as_nanos() as f64/N as f64);
  let t:Mutex<HashMap<u64,Wb>>=Mutex::new(HashMap::with_capacity(5000)); let i0=Instant::now(); let mut z=0.0;
  for i in 0..N{let x=((i%97)*30+400) as f64; let mut g=t.lock().unwrap(); let b=g.entry(ks[i%5000]).or_default(); z+=b.z(x); b.a.somar(x);} black_box(z); r[1].1.push(i0.elapsed().as_nanos() as f64/N as f64);
  let t:Mutex<HashMap<u64,H>>=Mutex::new(HashMap::with_capacity(5000)); let i0=Instant::now(); let mut z=0usize;
  for i in 0..N{let x=((i%97)*30_000+400) as u64; let mut g=t.lock().unwrap(); let b=g.entry(ks[i%5000]).or_default(); z+=(balde(x)>=b.p95()+4) as usize; b.somar(x);} black_box(z); r[2].1.push(i0.elapsed().as_nanos() as f64/N as f64);
 }
 for (k,v) in r{let (a,m,z)=med(v);println!("{k:40} min {a:7.2} med {m:7.2} max {z:7.2} ns");}
 println!("bytes por chave: W {} Wb {} H {}",std::mem::size_of::<W>(),std::mem::size_of::<Wb>(),std::mem::size_of::<H>());
}
```
