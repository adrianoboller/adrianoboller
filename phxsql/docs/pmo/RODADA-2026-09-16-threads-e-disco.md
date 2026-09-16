# Rodada de 16/09/2026 — threads com semáforo, e a saúde do disco do banco

Dois pedidos do dono, na mesma mensagem (16/09/2026, ~05:40 UTC):

1. *«Multi threads devem ter um controle altamente validado com semáforos
   adequados do Rust para um ótimo funcionamento.»*
2. *«Monitor de status da saúde do disco onde o banco de dados está sendo
   gravado; em caso de log de erro, aviso ⚠️ imediato por e-mail e SMS.»*

Este quadro é apoio de processo (pétrea dos `.md` de controle). Os números
abaixo foram **medidos** antes de qualquer contrato, com `grep`/`sed` no
fonte em 16/09/2026 05:40 UTC.

## O que já existe, medido

### Threads

| onde | como nasce | teto | fonte |
|---|---|---|---|
| porta de dados | uma thread por conexão, via `telemetria.subir(...)` | `recursos.conexoes_max` (64) por `AtomicUsize conexoes`: `load >= max` recusa e anota no `acessos.log`; `fetch_add` ao aceitar, `fetch_sub` ao fim do fecho | `servidor.rs:817`, `:1587`, `:1619` |
| porta web (interface) | uma thread por pedido HTTP | **nenhum** — o próprio comentário confessa: *«esta thread nasce SEM TETO … uma enxurrada de pedidos vira uma enxurrada de threads»* | `servidor.rs:6854–6888` |
| REST (6000) e docs (7000) | uma thread por pedido HTTP | **nenhum** — mesmo molde | `servidor.rs:7138–7162` |
| fecho da janela de durabilidade | `std::thread::scope`, **K** fios (uma por tabela suja) | K, sem teto — comprou 2,5× em K=16 (pedido 180) | `servidor.rs:14216` |
| varredura em memória | `paralelo::mapear_faixa` | `paralelo::nucleos()` ← `recursos.threads`/`cpu_percentual` | `phxsql-core/src/paralelo.rs` |
| serviços de fundo | `telemetria.subir` (réplica, cluster, jobs, vigia de disco, telemetria) | um de cada | `telemetria.rs:1211` |

Primitivas no servidor (contagem de ocorrências): `Mutex<` 51, `RwLock<` 3,
`AtomicBool` 47, `AtomicU64` 57, `AtomicUsize` 14, `mpsc::` 4, **`Condvar` 0,
semáforo 0**. A `std` do Rust não tem semáforo (o `std::sync::Semaphore` foi
removido antes do 1.0); o semáforo «adequado» aqui é o de contagem sobre
`Mutex<usize>` + `Condvar`, escrito nesta casa, sem crate — a mesma regra do
SHA-256.

O gestor de threads da telemetria (`registrar_fio`/`subir`) **já é o registro
de todas as threads de serviço** — o que falta não é registro, é **teto** e
**prova**.

### Disco

| o que | existe? | fonte |
|---|---|---|
| espaço livre por caminho (`base`, destino do backup, `alertas.caminhos`) | sim, pelo `df -k`, a cada `alertas.checar_minutos` (15), silêncio `repetir_horas` (6) por caminho | `sistema::espaco`, `ligar_vigia_de_disco` (`servidor.rs:20068`), `conferir_disco` |
| aviso por e-mail | sim, SMTP escrito aqui, sem TLS, `alertas.email` | `email.rs`, `config.rs:834` |
| aviso por SMS | **não existe** — zero ocorrências de `sms` no código | — |
| saúde (erro de E/S, montagem só-leitura, latência do disco, SMART) | **não existe** — o vigia só olha espaço | — |
| gancho no erro de E/S do motor | **não existe** — `PhxError::Io` (código 5001, `SP000010`) só vira resposta e linha no `acessos.log` | `error.rs:11,152,264` |
| cartão do disco no painel | sim, só espaço (`tela.pa_espaco_disco`, `m.discos`) | `op_painel` (`servidor.rs:20213`), `ui/index.html:3687` |

## Papéis convocados e dispensados (registro do orquestrador)

| papel | decisão | motivo |
|---|---|---|
| A orquestrador | eu, modelo forte | contratos, integração, encontro das frentes |
| B engenheiro — frente T (threads) | convocado, **modelo forte** | concorrência e trava: projeto e risco |
| B engenheiro — frente D (disco) | convocado, **modelo médio** | sonda de disco e canais de aviso: trabalho delimitado, com um ponto de risco (comando externo do SMS) que vai à revisão SEC |
| SEC | convocado, modelo médio, depois da frente D | executar comando vindo do `config.json`, texto de alerta sem segredo, canal SMS |
| C DBA | **dispensado** | nenhuma frente toca formato em disco, chave, índice ou migração |
| E designer | **dispensado, com o dever transferido** | a tela muda em um cartão do painel que já existe; a lei «só se prova exercitando» vai para a frente D como aceite (caso de navegador nos dois temas) |
| F prova real | cada frente prova RED/GREEN; auditoria cruzada leve na integração | custo |
| G QA | dentro da frente T: catraca do mapa das threads; guardas no catálogo nas duas frentes | — |
| H documentação | escalão leve, na integração | propagação de fato decidido |
| J pesquisador | **dispensado** | o consenso dos três motores já é conhecido e vai escrito no contrato: `max_connections` recusa na hora no PostgreSQL («too many clients already»), no MySQL e no MariaDB («Too many connections»); fila de espera só existe em servidor HTTP (nginx/Apache: workers + backlog), que é o molde da porta web |
| D zelador | rodou às 05:09; 5,7 GB livres | — |

## Contrato da frente T — semáforo e teto das threads (pedido 248)

**Aceite:**

1. `phxsql-core/src/semaforo.rs`: `Semaforo::novo(teto)`, `adquirir()`
   (espera), `adquirir_ate(Duration) -> Option<Permissao>`, `tentar()`,
   `em_uso()`, `teto()`; `Permissao` solta no `Drop` — **inclusive em
   pânico** (prova com `catch_unwind`). Mutex envenenado não derruba
   (`into_inner`). Zero crate.
2. Porta de dados: o `AtomicUsize conexoes` vira `tentar()` no semáforo, com
   a permissão viajando para dentro da thread da conexão; recusa continua
   **imediata** (consenso dos três motores) e continua anotando no
   `acessos.log`. Prova real: com o defeito reposto (pânico dentro do
   `atender` sem soltar), a porta deixa de aceitar depois de N pânicos; com o
   RAII, aceita.
3. Porta web e REST/docs: ganham teto (`recursos.conexoes_web_max`, padrão a
   decidir medindo — sugestão 64) com **fila curta**
   (`recursos.fila_web_ms`, padrão 2.000 ms): `adquirir_ate`; estourou, 503
   com `Retry-After` e linha no `acessos.log`. O teste do comportamento
   VELHO: abaixo do teto nada muda.
4. Fecho da janela (K fios): **medir antes de limitar** — K=16 com teto 4, 8,
   16 e sem teto, três corridas cada, na bancada que o pedido 180 já usa. Se
   qualquer teto custar mais de 5%, fica sem teto **e o mapa diz por quê**.
5. `bancada/concorrencia/mapa-das-threads.py`: varre `thread::spawn`,
   `Builder::spawn`, `thread::scope` e `telemetria.subir` fora de `#[cfg(test)]`,
   e exige que cada sítio esteja no catálogo com o seu teto (ou a dispensa
   com motivo). `--catraca`: `spawn-sem-teto = 0` — no molde exato do
   `mapa-da-trava.py`, ligado à `prova-bateria.py` como item 0c.
6. Bancada `bancada/concorrencia/enxurrada-web.py`: 500 conexões HTTP
   simultâneas contra o `phxsqld` de pé; mede threads vivas (`/proc/<pid>/status`
   `Threads:`), RSS e quantas receberam 503 — antes e depois. Grava
   `resultados.json` com data.
7. `docs/CONCORRENCIA.md` §17 «O mapa das threads», com os números. Guardas
   no `bancada/guardas/` para cada prova real. `fmt`, `clippy` zero,
   suíte verde. Não comita.

**Fora do escopo:** substituir a trava de dados; thread pool para a porta de
dados (medir primeiro, noutra rodada).

## Contrato da frente D — saúde do disco do banco (pedido 249)

**Aceite:**

1. `crates/phxsql-server/src/saude_do_disco.rs`, thread própria pelo
   `telemetria.subir`, a cada `alertas.disco.checar_segundos` (padrão 60):
   - **canário**: escreve, `fsync`, relê e apaga `<base>/.saude/canario`,
     medindo a latência em ms; recusa (`EROFS`, `EIO`, `ENOSPC`) vira estado
     `erro` na hora;
   - **só-leitura**: a montagem que contém `base` em `/proc/mounts` com `ro`;
   - **espaço e inodes**: `df -k` (existe) e `df -i`;
   - **SMART**: `smartctl -H <dispositivo>` **se** o binário existir
     (`Command`, zero crate); senão `nao_medido`, nunca «ok».
2. **Gancho no erro de E/S**: onde a resposta vira `PhxError::Io` (um lugar
   só, o irmão do `acessos.log`), o módulo recebe `(op, database, tabela,
   texto)`, incrementa o contador e dispara o aviso **imediato** — sem
   esperar o relógio. Silêncio `alertas.repetir_horas` por **tipo** de
   problema (E/S, só-leitura, canário, latência, SMART), primeira ocorrência
   sempre imediata; quando o disco volta ao normal, o silêncio zera (como o
   `conferir_disco` já faz).
3. **Canais**: e-mail (o que existe) e **SMS**, novo em `alertas.sms`:
   `ligado`, `numeros: []`, `comando: []` (programa + argumentos, **sem
   shell**, com `{numero}` e `{texto}` substituídos por argumento — nunca
   interpolados numa string de shell), `prazo_s` (mata o filho), e
   `gateway_email: "sms.exemplo"` (manda `numero@gateway` pelo SMTP que já
   existe, o caminho sem programa externo). Zero TLS aqui: HTTP de gateway
   é decisão do dono e fica escrita na doc como recusa com motivo.
   Texto do SMS: uma linha, ≤ 160 chars, sem segredo, sem caminho completo
   quando ele contém nome de usuário.
4. `op_painel` devolve `saude_do_disco: {estado: ok|aviso|erro|nao_medido,
   canario_ms, so_leitura, erros_es, ultimo_erro, smart, medido_em_ms,
   avisos: {email_ms, sms_ms}}`; op nova `saude_disco` (só leitura) para o
   MCP/REST.
5. Cartão no painel (`tela.pa_saude_disco`), pino de estado por forma e cor,
   **todo texto pela fábrica de idiomas nos seis idiomas**, catraca
   `TETO_ROTULOS_E_CRASE` não sobe. **Exercitado no navegador nos dois
   temas** (`testes-web/casos/`), com o estado `erro` forçado por um canário
   que falha.
6. Provas reais nos dois sentidos: canário sobre diretório só-leitura
   (`chmod 0555` no teste) vira `erro`; um `PhxError::Io` injetado numa op
   dispara o e-mail no relé falso que já existe nos testes (`rele_falso`) e o
   comando de SMS falso (um script que grava o argumento) **em menos de 1 s**;
   silêncio: dois erros seguidos, um aviso; retorno ao normal e erro de novo,
   dois avisos.
7. `docs/SAUDE-DO-DISCO.md` (o que mede, o que não mede, os canais e a
   recusa do TLS), `docs/SEGURANCA.md` §aviso com o comando do SMS,
   `MANUAL.txt` se ele listar o bloco `alertas`. Guardas no catálogo.
   `fmt`, `clippy` zero, suíte verde. Não comita.

**Fora do escopo:** SMS por HTTP com TLS (pétrea de zero dependências);
reparo automático.

## Encontro das frentes (o que só o integrador vê)

- As duas tocam `servidor.rs` e `config.rs` em regiões diferentes: T nos
  laços de aceitação e em `Recursos`; D no vigia/painel e em `Alertas`.
- A thread da saúde do disco **entra no mapa das threads** da frente T, com o
  teto «1».
- O 503 da porta web e o alerta de disco escrevem no mesmo `acessos.log`.
