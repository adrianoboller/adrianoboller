# Testes de relógio — pedidos 703 e 675

**Papel J, 08/10/2026.** Só leitura; nada foi compilado nem rodado (a máquina
estava com o provador). Tudo abaixo é **raciocinado sobre o fonte, não medido**,
salvo os dois números do próprio pedido 703 (1,69 s; 6,18 s × 2,09 s). Cada
seção nomeia o que decidiria o número na bancada.

## Resumo

1. `o_recuo_nao_atrasa_seguir_o_master_eleito`: o prazo de 1,3 s mede «acordou
   no passo de 1 s do sono vigiado», com **300 ms de folga**. O defeito que ele
   caça daria **≥ 3,7 s**. O conserto é ancorar o limite no fim do recuo, uma
   constante do `Ritmo`, e trocar o segundo prazo por uma **contagem**
   (`falhas_de_rede_seguidas`).
2. `replica_atrasada_promovida_nao_reemite_o_que_o_master_entregou`: o mais
   provável é que **não seja lentidão, e sim uma corrida de ORDEM** que a carga
   torna provável. Se o `posicao` da réplica cai no meio dos três `inserir`, o
   `replicar` daquela rodada fica preso no portão por 30 s (`SILENCIO_DA_REPLICA`),
   e o contador certo não chega dentro dos 5 s. O conserto é segurar também o
   `posicao` durante os inserts e esperar o **evento** «um `posicao` passou
   depois dos inserts».
3. Varredura: 21 testes têm **teto** de tempo de parede. 4 são frágeis, porque a
   folga é menor que 1 s ou que 3× o tempo esperado; 2 deles são os do 703.
   O **piso** de tempo (`>=`) é robusto à carga e fica como está.
4. 675: o `portoes.sh` passa a gravar a suíte em `target/portoes/suite-<carimbo>.log`
   e, se sair VERMELHO, imprime o caminho do log, os binários e os testes caídos,
   e a carga da máquina.
5. Os maduros não usam teto de parede em teste de replicação. Esperam o evento
   com paciência longa, sem afirmar rapidez: o PostgreSQL com
   `poll_query_until` e `$timeout_default = 180` s, o MySQL com
   `wait_condition.inc` (30 s, consulta a cada 0,1 s).

---

## 1. `o_recuo_nao_atrasa_seguir_o_master_eleito`

`crates/phxsql-server/src/servidor/testes_do_recuo_no_cluster.rs:178`

**O que o código faz.** O laço é `laco_da_replica_do_cluster`
(`servidor/servico_cluster_01.rs:787`). Na falha de rede, ele dorme o recuo do
`Ritmo` (`replica.rs:1152`; base `pulso_s` = 1 s, ×2 a cada falha) por
`dormir_vigiando_com` (`servidor/servico_replicacao_01.rs:413`). Esse sono vai
em **passos de até 1 s** e, a cada passo, pergunta se o master mudou.

**O que cada prazo mede de verdade.**

| Linha | Asserção | O que mede | Valor certo | Valor do defeito | Folga |
|---|---|---|---|---|---|
| :196 | `atraso <= 1.300 ms` | resto do passo corrente (≤ 1 s) + volta do laço + `connect` + *accept* do ouvinte falso + agenda de 3 threads | ≤ 1,0 s + jitter | sono cego: ~3,7 s (4 s − 300 ms) | **0,3 s** |
| :200 | `iv[0] <= 1.500 ms` | a primeira espera no no3 é a base (o `ritmo.sucesso()` da troca de par, :833) | 1 s + jitter | recuo herdado: 8 s (2³) | **0,5 s** |

**Por que a carga estoura.** São três threads acordando em série: o laço depois
do `sleep`, o `TcpStream::connect` dentro do `rodada_classificada`, e a thread do
ouvinte, que só carimba `Instant::now()` depois do `accept`. Com a suíte cheia
rodando N binários em paralelo, cada acordar pode atrasar dezenas a centenas de
milissegundos, e 300 ms de folga acabam. O medido no pedido foi 1,69 s, ou seja,
0,39 s acima do teto e **2 s abaixo** do valor do defeito. O teste mediu o
escalonador, não a vigilância.

**Hipóteses (escritas antes de concluir).**

- H1: a vigilância falhou sob carga; o laço não acordou no passo. → Morta pelo
  número: 1,69 s está longe dos ~3,7 s do sono cego.
- H2: o teto foi tirado do valor certo mais uma margem fixa, e não do ponto
  médio entre o certo e o defeito. → Sustenta-se.

**Conserto.**

- (:196) Limite **derivado do defeito, não do caso bom**. Exigir
  `vindas3[0] < vindas2[2] + RECUO_CORRENTE`, com `RECUO_CORRENTE` = base·2²
  = 4 s, o fim do sono que o defeito esperaria inteiro.
  - Os dois carimbos saem da mesma família de thread (os ouvintes), e não do
    `eleito_em` da thread de teste.
  - Folga: de 0,3 s para ~2,7 s. Prova real preservada, porque o sono cego chega
    em `vindas2[2] + 4 s` ou depois.
  - Melhor ainda: expor o passo como constante (`PASSO_DO_SONO_VIGIADO`, hoje um
    `Duration::from_secs(1)` literal em :431) e calcular o recuo pelo próprio
    `Ritmo`, para o teste nunca carregar o número digitado.
- (:200) **Contagem no lugar do relógio.** Depois de `vindas3.len() >= 1`, esperar
  com paciência longa por `estado_replicacao["cluster:no3"].falhas_de_rede_seguidas >= 1`
  e afirmar que ele vale **1 ou 2**.
  - Com o recuo herdado, valeria 4, porque o `Ritmo` já estava em 3 seguidas.
  - É a mesma régua que `o_master_que_derruba_e_retentado_com_recuo` já usa em
    :151.
  - Que o valor 1 de seguidas dê a espera base já está provado sem relógio nos
    testes unitários do `Ritmo` (`replica.rs:1204` e seguintes).
- O irmão `a_falha_unica_volta_no_pulso_de_sempre` (:166) tem o mesmo teto
  frágil (`iv[0] <= 1.500 ms`). Conserto: o mesmo contador
  (`falhas_de_rede_seguidas == 1` depois da primeira volta) mais o piso de
  900 ms, que é robusto e fica.

**O que decidiria na bancada.** Rodar o teste 50× com `stress-ng --cpu $(nproc)`
ao lado e registrar a distribuição de `atraso`. A previsão de H2 é: p99 < 2,5 s
e nunca ≥ 3,7 s. Com o defeito reposto (passo = recuo inteiro), sempre ≥ 3,7 s.

## 2. `replica_atrasada_promovida_nao_reemite_o_que_o_master_entregou`

`crates/phxsql-server/tests/contador-na-promocao.rs:260`

**O que o código faz.** Uma rodada da réplica (`rodada_da_replica`,
`servico_replicacao_01.rs:573`) segue esta ordem:

1. Pede `posicao` (:601).
2. Adota o contador do source em `abrir_para_replicar` (:661), **antes** de puxar.
3. Só pede `replicar` se o source tem mais eventos do que ela (`alcancar_database`,
   :1271).

O repetidor do teste (:80) segura **só** o `replicar` e deixa o `posicao` passar.
O cliente da réplica tem prazo de leitura de `SILENCIO_DA_REPLICA` = 30 s
(`replica.rs:976`, via `ligar`).

**O que o prazo de 5 s (:271) mede.** «Uma rodada que começou **depois** dos três
`inserir` acontece em até 5 s.» Mas o teste não controla em que ponto da rodada
os inserts caem. Há dois cenários:

- **Caminho bom.** O `posicao` vem antes do 1º insert ou depois do 3º. Ele vê 2
  eventos, a rodada sai com `Ok(0)` e dorme 1 s. O `posicao` seguinte vê 5 e
  adota 6. Total ≤ ~1–2 s, compatível com os 2,09 s sozinho.
- **Corrida.** O `posicao` cai **entre** o 1º e o 3º insert e vê 3 ou 4 eventos.
  A réplica adota 4 ou 5, pede `replicar`, e o pedido **fica preso no portão**.
  A conexão fica pendurada até 30 s. Não há rodada nova, então o `proxima`
  fica < 6. Aos 5 s, `chegou = false` e o teste cai. Isso bate com os ~6,18 s:
  5 s de espera mais o arranque.

**Por que a carga estoura.** Cada `inserir` é uma conexão TCP nova com gravação
em disco. Sob carga, os três inserts levam mais tempo, a janela da corrida se
alarga, e cresce a chance de o `posicao` periódico (a cada ~1 s) cair dentro
dela. A carga não deixa o caminho bom lento: ela **sorteia o caminho ruim**.

**Hipóteses.**

- H1: pura lentidão; os 5 s não bastam para a rodada seguinte. → Fraca. Pediria
  mais de 4 s de atraso de agenda numa espera de 1 s.
- H2: corrida de ordem com `replicar` preso (acima). → Sustenta-se e explica os
  6,18 s.

Decide na bancada, sem carga nenhuma: pôr um `sleep(1,5 s)` entre o 1º e o 2º
insert. Se H2 vale, o teste cai **sozinho**, de forma determinística. Contar no
repetidor os `posicao` e o `proxima_sequencia` de cada resposta mostra qual rodada
adotou qual número.

**Conserto (ordem/evento, sem prazo).** O repetidor ganha um segundo portão,
`segurar_tudo`, e um contador `posicoes_depois: AtomicUsize`. O teste fica assim:

1. `segurar_tudo = true`. Qualquer pedido da réplica em voo espera.
2. `fechado = true` (o portão do `replicar`), e os 3 inserts.
3. Zerar `posicoes_depois` e fazer `segurar_tudo = false`.
4. Esperar, **com paciência longa** (30 s, só desistência e não afirmação de
   rapidez), `posicoes_depois >= 1` e depois `proxima >= 6`. O segundo passo
   acontece na mesma rodada, antes do `replicar`, então não há outra corrida.

Agora todo `posicao` que conta foi respondido **depois** dos inserts, por
construção. A prova real fica intacta: sem o contador no `posicao`, `proxima`
fica em 3 e o `assert` final cai igual.

Alternativa mais fraca: subir os 5 s para `SILENCIO_DA_REPLICA` +
`reconectar_em` + margem, ~35 s. É um prazo justificado por constante, mas faz o
caminho ruim custar 31 s e esconde a ordem em vez de fixá-la. **Recusada** como
primeira escolha.

## 3. Varredura da suíte

Critério: teste `#[test]` com `assert!(… < / <= Duration|elapsed|ms …)` ou
`recv_timeout`. Achados por varredura de padrão, então podem faltar asserções
com outra forma. Classificação:

- **A**: paciência. Espera o evento com prazo longo; o tempo é só desistência.
- **B**: mede tempo de propósito (canal lateral, bancada).
- **C**: tempo usado no lugar de evento. A coluna «folga» compara o teto com o
  esperado.
- **D**: `recv_timeout` curto para provar **ausência** de evento. A carga não deixa
  vermelho, mas pode deixar **verde falso** com o defeito reposto.

**Frágeis, folga < 1 s ou < 3× (consertar):**

| Arquivo:função | Teto | Esperado | Defeito | Conserto |
|---|---|---|---|---|
| `server/src/servidor/testes_do_recuo_no_cluster.rs:o_recuo_nao_atrasa_seguir_o_master_eleito` | 1,3 s / 1,5 s | ~1 s | 3,7 s / 8 s | §1 |
| `server/src/servidor/testes_do_recuo_no_cluster.rs:a_falha_unica_volta_no_pulso_de_sempre` | 1,5 s | 1 s | 2 s | contador `falhas_de_rede_seguidas == 1` |
| `server/tests/contador-na-promocao.rs:replica_atrasada_promovida_…` | 5 s | ~1–2 s | corrida (ordem) | §2 |
| `server/src/servidor/testes_das_threads.rs:acima_do_teto_a_web_responde_503_…` (:351) | **100 ms** | ~0 (recusa imediata) | 100 ms (esperar a fila) | teto = `fila_web_ms` do próprio teste, menos ε; o piso de :342 fica |

**C com folga folgada (vigiar; ponto médio derivado do defeito é melhor que número solto):**

- `core/src/semaforo.rs:prazo_zero_nao_espera`: teto 50 ms.
  - O defeito seria esperar sem fim, então qualquer volta finita já prova.
  - Trocar por «voltou» (`recv_timeout` de 10 s numa thread) elimina o teto.
- `server/src/servidor/testes_transacoes.rs:a_escrita_comum_respeita_a_trava_e_nao_espera`:
  teto 200 ms. O defeito é esperar a trava, que só solta no `rollback`, ou seja,
  nunca dentro do teste. Mesmo conserto: «voltou com `EM_TRANSACAO`» já prova.
- `server/src/blacklist.rs:a_lista_fica_livre_enquanto_o_firewall_roda`: teto
  500 ms. O defeito daria 30 s (`sleep 30`), então o teto justificado é 15 s.
- `server/tests/quorum-de-escrita.rs:duas_replicas_e_depois_…`: degradado
  `< 250 ms`, contra um defeito que espera o `PRAZO` de 1.500 ms. Teto
  justificado: `PRAZO/2`.
- `server/src/quorum.rs:duas_replicas_fecham_o_quorum_de_dois` (< 2 s, prazo 5 s)
  e `:sem_replica_conhecida_degrada_sem_esperar_o_prazo` (< 1 s, prazo 5 s).
  Folga ≥ 2,5×.
- `server/tests/argumento-desconhecido.rs` (2 testes): teto 1 s para **lançar um
  processo**. Arranque sob carga pode passar de 1 s. O defeito é subir o
  servidor e não sair, que um `wait` com prazo de 10 s já pega.
- `phxzip/tests/phz.rs:ciclos_do_arquivo_acima_do_padrao_recusam_sem_derivar`
  (500 ms) e `server/src/pg/scram.rs:iteracoes_acima_do_teto_recusam_antes_de_derivar`
  (1 s). O defeito são 2^24 rodadas, segundos. Um contador de rodadas derivadas
  seria contagem pura.
- `server/src/saude_do_disco.rs:entregar_acorda_o_carteiro_…` (< 2 s contra
  defeito de 10 s), `server/tests/firewall-que-pendura.rs` (< 2 s contra 30 s) e
  `server/src/replica.rs:ligar_nao_fica_pendurado_…` (< 3 s contra prazo de
  500 ms × defeito pendurado). Folga ≥ 4×.

**C com folga ≥ 5× (ficam):**

- `core/src/prazo.rs:o_total_recomeca_…` (3 s)
- `semaforo.rs:adquirir_ate_devolve_none_…` (2 s contra 60 ms; o piso é a prova)
- `gancho.rs:programa_que_estoura_…` (5 s contra 30 s)
- `blacklist.rs:o_firewall_roda_pelo_motor_do_gancho` (5 s)
- `config_phz.rs:phz_hostil_…` (10 s)
- `retrato.rs:sem_escritor_na_fila_…` (30 s)
- `testes_da_ficha_compartilhada.rs:o_escritor_nao_passa_fome_…` (20 s)
- `testes_da_saude_do_disco.rs` :229 e :709 (5 s)
- `testes_do_terceiro_na_tabela_que_nasce.rs:criar_e_usar_…` (5 s)
- `testes_janela_e_cadeia.rs:gatilho_before_…` (5 s)
- `trava-atras-da-rede.rs` (2 testes, 5 s)
- `quorum-de-escrita.rs` (os de 3 s)
- `phxsql-sql/src/rotina.rs:o_corpo_lento_para_no_prazo_…` (1 s contra 50 ms;
  o comentário já diz 20×)

**B, mede tempo de propósito:**

- `server/tests/identidade-do-pulso.rs:o_pulso_nao_diz_quais_nos_tem_pino` e
  `:o_pulso_sem_prova_…` (`LIMITE_DO_RELOGIO_SEM_PROVA`, canal lateral).
  - Sob carga o ruído é simétrico e o vício não deveria subir.
  - Se cair na suíte, o caso pede repetição estatística, não um teto maior.
- `server/tests/carimbo-e-faixa.rs:mil_linhas_…` (conta carimbos distintos).

**A, paciência** (prazo ≥ 5 s esperando evento, padrão `Instant::now() < ate`
ou `recv_timeout(10 s+)`). São ~45 casos: `continuidade-da-replica.rs`,
`venda-inteira-*`, `teto-*`, `dblink-*-no-fio.rs`, `odbc/conexao.rs`,
`testes_do_panico_sob_a_trava.rs` e os demais da lista da varredura. Só caem se
o evento **não acontecer**. É o padrão dos maduros, e não se mexe.

**D, ausência por janela curta** (verde falso possível, não vermelho):

- `odbc/src/lib.rs:delete_nao_anuncia_colunas_…` (300 ms)
- `testes_da_saude_do_disco.rs:erro_de_es_…_uma_vez_so` (700 ms), `:o_sms_sai_…`
  (500 ms) e `:sem_email_ligado_…` (500 ms)
- `testes_firewall_e_mensagens.rs:comando_proibido_avisa_…` e `:sem_avisar_…`
  (700 ms)

A prova de ausência mais forte seria um evento-sentinela: mandar algo depois e
esperar que **ele** chegue primeiro. Fica `⏸`: não é defeito ativo.

## 4. Pedido 675: o `portoes.sh` que diz qual teste caiu

**Defeito.** `passo "suite" … cargo test --workspace --offline --no-fail-fast`
manda a saída só para o terminal. Com saída 101, o nome do teste caído se perde.

**Conserto proposto**, no passo da suíte e só nele:

- Criar `log="$raiz/target/portoes/suite-$(date +%Y%m%d-%H%M%S).log"`.
  - Fica dentro do `target`, que é ignorado pelo git e não suja a árvore.
  - O `zelador.sh` precisa **poupar** `target/portoes/`, ou guardar os N mais
    novos. Senão o log some antes de alguém lê-lo.
- No cabeçalho do log: `git rev-parse HEAD`, `nproc`, `/proc/loadavg` antes e
  depois, e `RUST_TEST_THREADS` se houver. A causa do 703 é a carga; o log tem
  de trazer a carga junto.
- Rodar `cargo test … 2>&1 | tee "$log"` e tirar o código por
  `${PIPESTATUS[0]}`. Sem isso, o `tee` engole o 101 e o portão fica verde.
  - É o defeito que esta casa já pagou: gerador que diz sucesso fazendo menos.
- Se `rc != 0`, imprimir:
  - o caminho do log;
  - `grep -E '^test .+ \.\.\. FAILED$'`, os nomes dos testes;
  - o bloco `error: N targets failed:` com as linhas `--test X` / `--lib`, os
    binários;
  - para cada nome, a linha pronta para rodar sozinho:
    `cargo test -p <crate> <alvo> <nome> -- --exact`.
  - Sem nenhum `FAILED` e com rc ≠ 0, dizer isso por escrito («caiu sem nome de
    teste: pânico de harness, sinal ou compilação, ver o log»), em vez de
    silenciar.
- Sair VERDE também imprime o caminho do log em uma linha. Uma corrida verde
  vizinha de uma vermelha é o par que se compara.

**Depois do conserto, para fechar o 675:**

1. Rodar `./portoes.sh` (ou só a suíte) em laço até reproduzir.
2. Cruzar os nomes caídos com a tabela «frágeis» da §3. A previsão é que o
   vermelho sem nome seja um dos quatro.
3. Se não for, o log diz qual é. Nada se marca resolvido por palpite.

## Lacunas

- Nenhum número novo foi medido aqui. A distribuição de `atraso` sob carga e a
  reprodução determinística do H2 da §2 são as duas medições que confirmam ou
  matam as hipóteses.
- O 703 não diz **qual** asserção do teste do contador caiu. A previsão desta
  análise é a de :280 (`chegou == false`), com `proxima` em 4 ou 5 na mensagem.
  Se a mensagem mostrar `proxima` = 3 ou -1, H2 morre e volta H1.
- A varredura é por padrão de texto. Asserções com `as_millis()` comparado a
  inteiro em variável com outro nome podem ter escapado.

## Fontes

- Fonte desta casa: arquivo:linha em cada seção.
- PostgreSQL, `src/test/perl/PostgreSQL/Test/Utils.pm`: `$timeout_default =
  $ENV{PG_TEST_TIMEOUT_DEFAULT}`, 180 se ausente (lido em
  https://raw.githubusercontent.com/postgres/postgres/master/src/test/perl/PostgreSQL/Test/Utils.pm).
- MySQL, `mysql-test/include/wait_condition.inc`: 30 s por omissão, consulta a
  cada 0,1 s (lido em
  https://raw.githubusercontent.com/mysql/mysql-server/trunk/mysql-test/include/wait_condition.inc).
- Os dois convergem em **esperar evento com paciência longa e nunca afirmar
  rapidez**. MariaDB herda o `wait_condition.inc` do MySQL (não conferido no
  fonte dela nesta rodada). Quanto ao método, os três concordam e nada nosso se
  opõe.
