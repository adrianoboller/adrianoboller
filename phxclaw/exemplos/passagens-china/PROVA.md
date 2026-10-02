# PROVA -- medido em 02/10/2026, neste conteiner, com o binario vivo

Binario: `target/debug/phxclaw` (ramo `phxclaw/v070-nativo`, arvore de trabalho com
alteracoes de outras frentes). Chromium: `headless_shell` 141 de `/opt/pw-browsers`,
dentro do bwrap do agente. Modelo: **nenhum** (`pgrep ollama` vazio; o fluxo nao tem passo
de modelo, entao `tokens = 0` em todas as corridas).

Estados: **VERIFICADO** = aconteceu numa corrida aqui, com o artefato citado;
**NAO VALIDADO** = nao foi exercitado aqui.

## 1. Fonte

| Fonte | Carregou | Barrada | Preco extraido | Evidencia |
| --- | --- | --- | --- | --- |
| Google Flights (`google.com/travel/flights?q=Flights from GRU to PEK on 2026-11-01 one way&curr=BRL&hl=pt-BR`) | VERIFICADO (titulo «de Sao Paulo a Pequim \| Google Voos», 10 resultados) | nao -- sem captcha, sem login | VERIFICADO | `capturas/google-flights-gru-pek.png`, `fixtures/pagina_*.txt` |
| Kayak (`kayak.com.br/flights/GRU-PEK/2026-11-10`) | carregou, mas redirecionou para `/help/bots.html` | **VERIFICADO: barrada** («O que e um bot?») | nao | `capturas/kayak-barrado-anti-bot.png` |
| API (Amadeus/Kiwi/Duffel) | NAO VALIDADO -- nao foi preciso | - | - | - |

Sondas de URL no Google (3 corridas-sonda antes do fluxo):

| `q=` | Resultado |
| --- | --- |
| `Flights from GRU to PEK` | ida e volta, datas escolhidas pelo Google (28/02-09/03/2027), «a partir de R$ 7.750» |
| `Flights from GRU to PVG on 2026-11-10 one way` | so ida, 2026-11-10, 10 resultados, min R$ 4.292 |
| `Flights from GRU to CAN between 2026-10-15 and 2026-11-30` | pagina SEM resultado (virou a fixture do caso «sem preco») |
| `Flights from GRU to PEK in November 2026 one way` | virou o dia 2026-11-01 (nao e faixa) |
| `One way flights from GRU to PEK from 2026-10-10 to 2026-12-01` | virou ida e volta 10/10-01/12, min R$ 14.411 |

Conclusao: o `q=` aceita UMA data; o fluxo consulta `hoje + DIAS_A_FRENTE` por destino.

## 2. Acomodacao deste conteiner (nao e do produto)

O egress daqui reassina o TLS com uma CA propria; o Chromium dentro do bwrap nasce com
`HOME=/work` e sem NSS, e deu `net::ERR_CERT_AUTHORITY_INVALID` na primeira sonda. A NSS
do hospedeiro (`/root/.pki/nssdb`) **nao tinha a CA** apesar de o README do proxy dizer
que sim (medido: `grep` do CN no `cert9.db` vazio; headless_shell com `HOME=/root` falhou
igual, `net_error -202`). Solucao, sem desligar verificacao nenhuma: `certutil` (pacote
`libnss3-tools`, instalado aqui) criou `/opt/pw-browsers/phx-nssdb` com as 6 CAs
`Anthropic` do `/root/.ccr/ca-bundle.crt`, e `PHXCLAW_CHROMIUM=/opt/pw-browsers/bin/chrome`
aponta para um script que copia essa NSS para `$HOME/.pki/nssdb` e `exec`uta o
`headless_shell` real. Na maquina do dono nada disso existe nem e preciso.

## 3. As tres corridas do fluxo (42 passos)

Tempos: do `evidence.jsonl` da tarefa do fluxo (carimbo de FIM de cada chamada de
ferramenta, relativo a primeira). Precos de ida, 1 adulto, economica, BRL, data de ida
2026-11-01 (= 02/10 + 30 dias) nas tres corridas.

| | Corrida 1 | Corrida 2 | Corrida 3 |
| --- | --- | --- | --- |
| Entrada | `phxclaw fluxo rodar` | `phxclaw agenda adicionar` + `phxclaw servir --canal webhook` (laco da agenda) | `phxclaw agenda disparar` (CLI) |
| Tarefa | `01a0fadd-6301-…` | `01a0fae3-1b46-…` | `01a0fae4-0c4b-…` |
| Hora (UTC) | 04:26 | 04:33 | 04:34 |
| Total do fluxo | 5,85 s | 7,24 s | 5,47 s |
| `consultar_1` PEK (open) | 1,90 s | 2,49 s | 1,85 s |
| `consultar_2` PVG | 1,59 s | 1,71 s | 1,50 s |
| `consultar_3` CAN | 1,76 s | 1,90 s | 1,45 s |
| cada `captura_N` | 0,12-0,15 s | 0,16-0,35 s | 0,11-0,24 s |
| `preparar` (python) | 0,06 s | 0,09 s | 0,06 s |
| `extrair` (python) | 0,02 s | 0,02 s | 0,01 s |
| `planilha` (xlsx 7.762 B) | 0,03 s | 0,28 s | 0,09 s |
| PEK | R$ 5.188 Ethiopian | R$ 5.188 Ethiopian | R$ 5.188 Ethiopian |
| PVG | R$ 4.292 American, Etihad | R$ 4.292 American, Etihad | R$ 4.292 American, Etihad |
| CAN | R$ 5.703 Turkish Airlines | R$ 5.703 Turkish Airlines | R$ 5.703 Turkish Airlines |
| Alerta (teto 6.000) | ALERTA nos 3 | ALERTA nos 3 | ALERTA nos 3, com «ultimo 5188; minimo historico 5188» etc. (memoria da corrida 2) |
| `avisar` | falhou: sem canal no `fluxo rodar` | **VERIFICADO**: POST assinado chegou ao receptor local 68 s depois de subir o servidor | falhou: sem canal na CLI |
| `lembrar` / `planilha` | ok / ok | ok / ok | ok / ok |
| Tokens de modelo | 0 | 0 | 0 |

Faixas (3 corridas): fluxo inteiro **5,5-7,2 s**; cada consulta ao Google **1,5-2,5 s**;
extracao **<= 0,02 s**; planilha **0,03-0,28 s**.

## 4. O que esta VERIFICADO

- Pagina carrega, preco minimo, companhia, data e moeda extraidos da pagina (3/3 corridas,
  mesmos valores nas tres, 7 minutos de intervalo).
- Agenda dispara o fluxo **uma vez por vencimento** pelo laco do servidor (corrida 2:
  `agenda: 1 tarefa(s) disparada(s)`, 1 tarefa na pasta, `last_task` anotado) e pela CLI
  (corrida 3). O teste `agenda_dispara_o_fluxo_uma_vez_e_anota_a_tarefa` prova o mesmo por
  `/v1/schedules` com intervalo adiantado no relogio.
- Aviso pelo canal: corrida 2, canal `webhook` ligado no `servir`, `channel.send` concedida;
  receptor local recebeu `{"conversa":"DONO","texto":"ALERTA passagens GRU -> China …"}` com
  `X-PhxClaw-Assinatura`. O teste `fluxo_extrai_avisa_no_webhook_grava_planilha_e_compara_com_a_memoria`
  sobe o webhook, roda o fluxo com os passos de navegador vindos das fixtures (texto real do
  Google) e confere texto, planilha (lida pelo `read_document`) e a comparacao com a memoria
  na segunda corrida.
- Memoria: corrida 3 mostrou «ultimo 5188; minimo historico 5188» vindo da corrida 2.
- Planilha `.xlsx` com cabecalho + 3 linhas (uma por destino), 7.762 bytes.
- Capturas PNG por consulta (103-107 KB).

## 5. O que esta NAO VALIDADO

- Telegram e e-mail de verdade: sem credencial aqui. O que esta provado e o `channel_send`
  pelo mesmo `Canal` que o Telegram usa, contra um webhook local.
- Janela de 60 dias inteira: so uma data por destino (secao 1).
- API de voos: nao ligada, nao ha fixture.
- `cargo test --workspace` completo: **nao rodou** -- o disco deste conteiner chegou a
  806 MB livres (piso de 800 MB) durante os portoes, e uma tentativa posterior de
  `cargo test -p phxclaw-agent -p phxclaw` foi morta pelo vigia com o disco em 100%
  (3,8 MB livres, `target/` de outras frentes). Rodaram: `cargo fmt --all`,
  `cargo clippy -p phxclaw-agent -p phxclaw --all-targets` (zero avisos), o teste novo
  `passagens_china` (3/3) e o teste existente da agenda `agenda_cria_tarefa_quando_vence`
  (binario de teste compilado depois da mudanca, 1/1).

## 6. Mudancas fora de `exemplos/` (para o integrador decidir)

- `crates/phxclaw-agent/src/api.rs`: objetivo `fluxo: ARQ.json` na agenda roda
  `fluxos::rodar` (mesmo motor da CLI); `disparar_agenda_com_handles` para quem precisa
  esperar o disparo; `fluxo_do_objetivo`. O caminho do modelo nao mudou (teste existente
  verde).
- `crates/phxclaw-agent/src/agenda.rs`: `pub use ScheduleSpec` e `adicionar_agora` (a CLI
  nao carrega chrono).
- `apps/phxclaw/src/main.rs` + `ajuda.rs`: comando `phxclaw agenda listar|adicionar|disparar`,
  que nao existia -- o pedido o cita como porta de entrada.
- `crates/phxclaw-agent/tests/passagens_china.rs`: as tres provas.
