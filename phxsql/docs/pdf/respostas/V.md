# V) teste de backups agendados e listagem log se deu tudo certo

> Corrida em 2026-09-07 16:42:47 UTC · commit `a56a165` ·
> `target/release/phxsqld` · reproduzido por
> `python3 bancada/jobs/backup-agendado.py`

## Resposta curta

**Existe e responde.** Um job de backup (`{"op":"jobs"}`/`jobs.json`) rodou
**pelo relógio de verdade** — não por `job_rodar` manual — **duas vezes
seguidas** (a primeira quase imediata, a segunda ~1 minuto depois), gravou
`backup.json` no disco a cada corrida, e a op `jobs` devolveu o **histórico**
das duas com `ok:true`. Um segundo job com destino impossível **falhou de
propósito**, e o histórico mostra a falha com o motivo, sem escondê-la. O
backup bom foi **restaurado e conferido**: mesmo número de linhas que o
original — backup não conferido não é backup.

**A armadilha que valeu a pena registrar**: um job ligado depois que o
servidor já subiu **não acorda o relógio** — ele só sobe se algum job já
estava ligado NO ARRANQUE (`docs/JOBS.md`). Este teste sobe o `phxsqld` duas
vezes sobre a mesma raiz por causa disso: a primeira grava a `loja` com dados
de verdade, a segunda já nasce com o backup ligado.

## Exemplo exercitado

### FASE 1–2: dados de verdade, depois o job já ligado no arranque

```
== FASE 1: sobe SEM job, grava a loja, desce ==
  OK    a loja tem 20 linhas gravadas ANTES do job existir  -- registros=20

== FASE 2: escreve o jobs.json com o backup LIGADO, e sobe de novo ==
```

`jobs.json` escrito antes da segunda subida:

```json
{"jobs": [{"nome": "backup_da_loja", "descricao": "copia a raiz de dados a cada minuto",
           "ligado": true, "cada_minutos": 1, "usuario": "",
           "pedido": {"op": "backup", "destino": ".../backups/agendado"}}]}
```

### FASE 3: o relógio roda sozinho, duas vezes — e (1) o backup no disco

```
== FASE 3: o RELOGIO agendado (nao `job_rodar`) faz o backup sozinho ==
  OK    o relogio dos jobs subiu (havia job ligado NO ARRANQUE)
  ... esperando a primeira corrida (quase imediata: ultimo_ms=0) ...
  OK    a primeira corrida aconteceu  -- corridas ate agora: 1
  OK    estado da ficha depois da 1a corrida  -- {"job": "backup_da_loja", "ok": true, "duracao_ms": 1, ...}
  ... esperando a SEGUNDA corrida (agendada 1 min depois da primeira, pelo relogio que confere a cada 30s) ...
  OK    uma SEGUNDA corrida aconteceu sozinha (prova que e agendado, nao so um disparo)  -- corridas ate agora: 2

  (1) o backup no disco, com o backup.json:
    backup.json                                  1090 bytes
    loja/clientes.bin                              64 bytes
    loja/clientes.log                             944 bytes
    loja/clientes.memo                             64 bytes
    loja/clientes.ndx                            8192 bytes
    loja/clientes.pag                             453 bytes
    loja/clientes.reason                           64 bytes
    loja/clientes.reg                            1952 bytes
    loja/clientes.trash                            64 bytes
  OK    o backup.json existe
    backup.json: {"phxsql": "0.18.0", "quando": "2026-09-07 16:43:47,941", "arquivos": 8, "bytes": 11797, "escopo": "raiz", "database": null, "conteudo": [{"caminho": "loja/clientes.bin", "bytes": 64, "sha256": "454aa2ad9b725d9653bf79912326c530426986c20ffa42187f0183fd0c3fe33a"}, ...]}
  OK    o manifesto lista a tabela clientes.reg dentro de loja
```

### (2) o histórico do job — a listagem/log que diz se deu certo

```
  (2) o HISTORICO do job, pela op `jobs` (campo "historico"):
    2026-09-07 16:43:47,941  ok=True     0 ms  {"destino":".../backups/agendado","arquivos":8,"bytes":11797,"ms":0}
    2026-09-07 16:42:47,940  ok=True     1 ms  {"destino":".../backups/agendado","arquivos":8,"bytes":11797,"ms":0}
  OK    o historico mostra >=2 corridas, todas com ok=true  -- 2 corridas
```

A operação que lista o histórico é a **mesma `jobs`** (alelo `job_listar`) que
lista os jobs cadastrados: o campo `"historico"` da resposta traz até
`historico` (padrão 50) corridas recentes de **todos** os jobs, cada uma com
`job`, `quando`, `ok`, `duracao_ms` e `detalhe` — não existe uma operação
`job_historico` separada.

### (3) um job que falha de propósito, e o histórico mostrando a falha

```
== FASE 4: um job de backup que FALHA de proposito (destino inexistente/impossivel) ==
  OK    o PEDIDO de rodar teve sucesso (ok=true la fora)  -- {"ok": true, "op": "job_rodar", "resultado": {"job": "backup_que_falha", "ok": false, "duracao_ms": 0, "detalhe": "[SP00...
  OK    mas o JOB em si falhou (ok=false DENTRO do resultado)  -- [SP000010] erro de E/S: Not a directory (os error 20)
  OK    a ficha do job mostra estado falhou
  OK    o historico registra a FALHA com o motivo
    historico: 2026-09-07 16:43:48,095  ok=False  [SP000010] erro de E/S: Not a directory (os error 20)

  o e-mail de alerta que ISTO tentaria mandar, se alertas.email estivesse ligado:
    aviso_email agora (desligado neste servidor): {"ligado": false, "email_ligado": false, "avisar_jobs": false, "para": [], "repetir_horas": 6}
  OK    o aviso de e-mail de jobs esta DESLIGADO neste servidor (nenhum bloco alertas.email)
```

**Nota sobre "destino inexistente":** o backup cria o diretório de destino
sozinho (`create_dir_all`), então um caminho simplesmente ausente **não
falha**. Para falhar de propósito de forma reproduzível, o destino usado foi
um caminho que passa por um **arquivo comum** (`.../isto-e-um-arquivo-nao-uma-pasta/sub/backup`)
— o sistema operacional recusa criar um diretório dentro de um arquivo
(`ENOTDIR`, "Not a directory"), e o motor propaga o erro de E/S em vez de
mascará-lo. É o `[SP000010] erro de E/S` visto acima.

Com `alertas.email.ligado:true` e `avisar_jobs:true`, `avisar_sobre_a_corrida`
(`crates/phxsql-server/src/servidor.rs`, por volta da linha 4131) dispararia,
em thread própria, um e-mail com:

```
assunto = "PhxSql: job backup_que_falha falhou"
corpo   = job, descrição, operação, usuário, agenda, quando, duração_ms e o
          erro (NUNCA senha nem hash) -- texto_do_aviso_de_falha
```

O disparo **real** desse caminho, com um SMTP falso de verdade recebendo a
mensagem, já está provado em `bancada/jobs/prova-avisos.py` (passo 2 da
bateria, ver `docs/JOBS.md`); esta sonda não duplica esse SMTP falso — só
confirma que, **sem** o bloco `alertas.email`, zero conexão SMTP sai e o
motivo da falha continua visível na tela (não escondido).

### FASE 5: restaurar e conferir — backup não conferido não é backup

```
== FASE 5: RESTAURAR o backup e conferir -- backup nao conferido nao e backup ==
  OK    `conferir_backup` diz integro (SHA-256 de cada arquivo bate)
  OK    a simulacao le o conteudo (sem escrever) e acha o database loja
  OK    restaurar_backup (modo novo) copiou os arquivos para o database novo  -- {"database": "loja_restaurada", "de": "loja", "modo": "novo", "arquivos": 8, "bytes": 11797, "tabelas": ["clientes"], "substituiu": false, "ms": 3, "ok": true}
  OK    a copia restaurada tem o MESMO numero de linhas que a original  -- original=20 restaurado=20
    3 primeiras linhas restauradas: [(1, 'cliente 1'), (2, 'cliente 2'), (3, 'cliente 3')]
```

**Restaurado e conferido: 20 = 20.**

## O que NÃO existe, e é dispensa registrada

- **Não existe uma operação `job_historico` separada.** O histórico sai do
  campo `"historico"` da própria op `jobs`/`job_listar` — a mesma chamada que
  lista os jobs cadastrados.
- **Não existe agendamento "daqui a N minutos" (um disparo único).** A agenda
  de jobs só conhece `cada_minutos` (recorrente) e `hora` ("HH:MM" diário,
  `docs/JOBS.md`). Este teste usou `cada_minutos:1`, que — por o job nunca ter
  rodado antes (`ultimo_ms==0`) — dispara quase imediatamente no arranque e
  depois a cada minuto; não há um "só uma vez, daqui a 60 segundos".
- **Job ligado depois do arranque não acorda o relógio sozinho** — é
  documentado (`docs/JOBS.md`) e confirmado aqui: só um reinício com o job já
  ligado sobe o relógio. Sem isso o job fica **agendado**/**parado**, visível
  na ficha, mas ninguém o roda até alguém mandar `job_rodar` ou reiniciar.
- **O e-mail de alerta não foi disparado de verdade nesta sonda** (o servidor
  subiu sem `alertas.email`) — dispensa deliberada, para não duplicar
  `bancada/jobs/prova-avisos.py`, que já prova esse caminho ao vivo com um
  SMTP falso.

## Como se refaz

```bash
python3 bancada/jobs/backup-agendado.py
```

`PHX_SONDA_PORTA` (padrão 6725). A corrida inteira leva pouco menos de 2
minutos (a segunda corrida do relógio é o que domina o tempo). Detalhe em
`bancada/jobs/LEIA-ME.md`.
