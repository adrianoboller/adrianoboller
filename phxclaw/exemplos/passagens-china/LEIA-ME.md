# Monitor de passagens aereas para a China (SP000034)

Fluxo em DAG do PhxClaw (`fluxo.json`, o formato que `phxclaw fluxo` le) que, a cada
disparo da agenda, consulta o Google Flights, extrai o menor preco por destino, compara
com a memoria do agente, avisa pelo canal ligado e grava uma planilha. **Nao usa modelo**:
sao 42 passos, todos de ferramenta, pelo portao unico do agente (`Agent::call_tool`).
Com Ollama ou sem, o resultado e o mesmo.

O que foi medido aqui, corrida por corrida, esta em `PROVA.md`.

## Como rodar na sua maquina

Precisa: o binario `phxclaw`, Chromium (o `phxclaw` acha o do `/opt/pw-browsers` ou o de
`PHXCLAW_CHROMIUM`), `bwrap` e `python3` (o passo de extracao roda no `python_repl` do
sandbox, sem rede).

1. **Rodar uma vez, na mao** (sem canal: o passo `avisar` falha, o resto sai):

   ```sh
   phxclaw fluxo rodar exemplos/passagens-china/fluxo.json --pasta ~/.phxclaw
   ```

   A planilha fica em `<pasta>/tasks/<id>/work/saida/passagens-AAAAMMDD-HHMM.xlsx`, as
   capturas em `captura_1.png`..`captura_3.png`, o JSON completo em `saida/resultado.json`.

2. **Agendar a cada 6 h** (a mesma agenda do servidor; `agenda.json` da pasta):

   ```sh
   phxclaw agenda adicionar "passagens-china" \
     "fluxo: /caminho/absoluto/exemplos/passagens-china/fluxo.json" --cada 21600 --pasta ~/.phxclaw
   ```

   Ou pela API, com o Bearer de `<pasta>/api.token`:

   ```sh
   curl -X POST http://127.0.0.1:8787/v1/schedules -H "Authorization: Bearer $TOKEN" \
     -H 'content-type: application/json' \
     -d '{"name":"passagens-china","objective":"fluxo: /caminho/absoluto/exemplos/passagens-china/fluxo.json","every_seconds":21600}'
   ```

   O servidor le o `agenda.json` ao nascer: agendou pela CLI, (re)suba o servidor. Agendou
   pela API, ja vale. `phxclaw agenda disparar --pasta ~/.phxclaw` roda agora o que venceu.

3. **Subir o servidor com o canal do aviso e a capacidade `channel.send`** (ela fica fora
   do padrao de proposito: falar em nome do dono e efeito externo):

   ```sh
   PHXCLAW_CAPACIDADES="web.browse,fs.read,fs.write,shell.exec,memory.read,memory.write,doc.write,channel.send" \
   PHXCLAW_TELEGRAM_BOT_TOKEN=... PHXCLAW_TELEGRAM_CHATS=<seu chat id> \
     phxclaw servir --canal telegram --pasta ~/.phxclaw
   ```

   O token vai para o broker da pasta na primeira vez e nao precisa mais ficar no ambiente.
   No `fluxo.json`, troque o `"to": "DONO"` do passo `avisar` pelo seu chat id. Qualquer
   outro canal de `phxclaw canal` serve igual (e-mail, webhook, Slack...): `to` e a
   conversa permitida daquele canal.

## O que voce precisa dar

| Para | O que | Onde |
| --- | --- | --- |
| Aviso | Credencial do canal (token do bot do Telegram + chat id, ou SMTP, ou segredo do webhook) | Ambiente na primeira subida; vai ao broker cifrado da pasta. **Nunca no fluxo nem em arquivo.** |
| Capacidade | `channel.send` na lista `PHXCLAW_CAPACIDADES` (ou `agente.capacidades` no `config.json`) | Ambiente/config do servidor |
| Fonte | Nada: Google Flights responde sem chave e sem login (medido em 02/10/2026) | - |
| API de voos (opcional) | So se o Google passar a barrar. Nao esta ligada: ver «Limites» | - |

## Parametros (editar no `fluxo.json`)

Estao no comeco do codigo dos dois passos Python, marcados `edite aqui`:

- passo `preparar`: `ORIGEM` (GRU), `DESTINOS` (PEK, PVG, CAN -- exatamente tres, o fluxo
  tem tres consultas), `DIAS_A_FRENTE` (30), `JANELA_DIAS` (60), `MOEDA` (BRL);
- passo `extrair`: `TETO_BRL` (6000);
- passo `avisar`: `"to"` (a conversa do canal).

## Como o fluxo funciona

`preparar` (python) monta as tres URLs do Google Flights para a data de ida `hoje +
DIAS_A_FRENTE` -> `consultar_N` (`browser_open`) em fila, porque a sessao de navegador e
uma por tarefa -> `captura_N` (`browser_screenshot`) -> `pagina_N` grava o texto lido ->
`memoria` (`memory_search`) traz as corridas anteriores -> `extrair` (python) acha o menor
preco por destino, a companhia e a data NA PAGINA, compara com o ultimo preco e com o minimo
historico, decide `ALERTA` (preco <= teto, ou abaixo do minimo historico) e escreve um
arquivo por campo -> `avisar` (`channel_send`), `lembrar` (`memory_save`) e `planilha`
(`create_spreadsheet`, uma linha por destino) leem esses arquivos por `read_file`.

Por que tantos `read_file`: a substituicao `{{id}}` do fluxo e de texto inteiro, e a saida
do `python_repl` vem com `stdout:` na frente. Um arquivo por campo e o que deixa cada valor
chegar limpo na planilha e na mensagem, sem um segundo analisador.

## Limites conhecidos

- **O DAG nao tem passo condicional**: o aviso sai em toda corrida, com `ALERTA` ou `INFO`
  no comeco. Quem so quer o alerta filtra pelo prefixo no canal.
- **Janela de 60 dias e uma data por destino** (`hoje + DIAS_A_FRENTE`): o `q=` do Google
  Flights aceita uma data («on 2026-11-01 one way»), nao uma faixa -- «between A and B»
  devolveu pagina sem resultado e «in November 2026» virou o dia 1 (medido, `PROVA.md`).
  Varrer a janela inteira pediria o calendario de precos por clique, que nao esta aqui.
- **Kayak barra o navegador headless** (pagina «O que e um bot?», captura em `capturas/`).
  Nao ha segunda fonte por navegador; se o Google tambem barrar, o passo `consultar` e o
  lugar de trocar por uma API (Amadeus/Kiwi/Duffel) com a chave pelo broker -- nao foi
  feito porque nao foi preciso, e por isso nao ha fixture de API.
- **Sem canal ligado, `avisar` falha** e o fluxo termina com `sucesso=false`, mas planilha,
  memoria e capturas saem (os passos nao dependem do aviso).
- **Sem bwrap ou python3, `preparar` e `extrair` nao rodam.**
