# X) bateria de teste com comandos definidos como proibidos para um banco x que devem ser negados pelo servidor avisando o administrador por e-mail

> Corrida em 2026-09-07T16:38:47Z UTC · commit `a56a165` ·
> `phxsqld 0.18.0 (a56a16563355-sujo) x86_64-unknown-linux-gnu` ·
> reproduzido por `python3 bancada/proibidos/provar.py` · **21 afirmações, 0 falhas**

## Resposta curta

As três metades do pedido existem, e a terceira **passou a existir hoje**. A
recusa e o bloqueio já eram: `seguranca.comandos_proibidos` e
`seguranca.bases_proibidas` recusam com `[SP000025] ACESSO_NEGADO` e põem o IP
na blacklist na primeira tentativa. O **aviso ao administrador não existia** —
`violacao_grave` só chamava `eprintln!`, e nenhum dos cinco `email::enviar` do
`servidor.rs` era ela. Esta bateria mediu a ausência primeiro, o aviso entrou
depois (`alertas.email.avisar_seguranca`, opt-in), e a bateria agora prova a
mensagem contra um relé SMTP de verdade: **1 e-mail** no comando proibido,
**1** na base proibida, **0** no comando permitido e **0** com o aviso não
pedido.

## Exemplo exercitado

A bateria sobe três `phxsqld` (portas 6500, 6501, 6502) e um **SMTP falso**
(6510) — o relé não existe nesta máquina, e sem relé não há como provar que a
mensagem saiu. São três servidores porque cada um **se auto-bloqueia**:
127.0.0.1 não está na whitelist, então o comando proibido barra o próprio
medidor, que é exatamente o comportamento que se quer provar.

A política dos três:

```json
"seguranca": {
  "comandos_proibidos": ["excluir_tabela", "reindexar"],
  "bases_proibidas": ["financeiro"],
  "bloqueio_minutos": 60
},
"alertas": {
  "ligado": false,
  "email": { "ligado": true, "avisar_jobs": true, "avisar_seguranca": true,
             "servidor": "127.0.0.1", "porta": 6510,
             "de": "phxsql@bancada", "para": ["admin@bancada"], "timeout_s": 5 }
}
```

### 0. O instrumento antes do veredito

Um SMTP falso que não recebe nada não prova ausência de e-mail: prova que o
SMTP falso não presta. Então a bateria dispara primeiro um aviso que o motor
**já** sabia mandar — o de job que falhou:

```
  [OK  ] job_salvar 'quebra' gravado
    resposta job_rodar: {"job": "quebra", "ok": true, "duracao_ms": 0, "detalhe": "[SP000018] nao encontrado: database naoexiste nao existe em /tmp/phx-f5-…/a/dados", "resposta": null, "op": "job_rodar", "ms": 0}
  [OK  ] o SMTP falso recebeu o e-mail de job que falhou
    assunto: PhxSql: job quebra falhou
```

Só a partir daqui a caixa vale como medida.

### 1. Controle positivo: o comando PERMITIDO não bloqueia nem avisa

```
  [OK  ] criar_database x (permitido) passou
  [OK  ] bancos (permitido) passou
  [OK  ] criar_tabela x.folha (permitido) passou
  [OK  ] nenhum e-mail novo depois dos permitidos
  [OK  ] nenhum IP bloqueado depois dos permitidos
```

### 2. O comando proibido: recusa, código, blacklist e e-mail

Pedido e resposta, cruas:

```json
{"token":"t","op":"excluir_tabela","database":"x","tabela":"folha","confirmar":"folha"}
{"ok": false, "op": "excluir_tabela", "erro": "[SP000025] acesso negado: operacao excluir_tabela esta proibida neste servidor; o IP foi bloqueado", "codigo": 4001, "nome": "ACESSO_NEGADO", "classe": "acesso", "sprint": "SP000025", "repetir": false, "ms": 0}
```

O que ficou no `blacklist.json`, em disco:

```json
{"ip": "127.0.0.1", "desde": "2026-09-07 16:38:48,598", "desde_ms": 1788799128598, "ate": "2026-09-07 17:38:48,598", "ate_ms": 1788802728598, "motivo": "comando proibido pela politica", "comando": "excluir_tabela", "tentativas": 1, "firewall": false}
```

E a mensagem que chegou ao relé — assunto e corpo, colados como vieram:

```
    assunto: PhxSql: IP 127.0.0.1 bloqueado (comando proibido pela politica)

      O PhxSql bloqueou um endereço por violação grave.

        endereço    127.0.0.1
        motivo      comando proibido pela politica
        operação    excluir_tabela
        tentativas  1
        desde       2026-09-07 16:38:48,598
        até         2026-09-07 17:38:48,598
        firewall    sem regra (não configurado, ou a regra falhou)

      A operação foi recusada e o endereço está na blacklist.
      Para soltar: phxsqld --desbloquear 127.0.0.1
      Servidor PhxSql 0.18.0
```

O que o servidor gravou no próprio erro padrão, na mesma corrida:

```
      aviso de jobs por e-mail: falha e parado | avisa admin@bancada
      aviso de job enviado: 250 OK: fila 0001
      BLOQUEADO 127.0.0.1 ate 2026-09-07 17:38:48,598 -- comando proibido pela politica (excluir_tabela)
      aviso de seguranca enviado: 250 OK: fila 0001
```

### 3. Depois do bloqueio, a mesma máquina não entra mais

Uma conexão **nova** do IP bloqueado é recusada antes de qualquer operação:

```json
{"ok": false, "op": "conexao", "erro": "[SP000025] acesso negado: bloqueado desde 2026-09-07 16:38:48,598 ate 2026-09-07 17:38:48,598 por comando proibido pela politica (excluir_tabela)", "codigo": 4001, "nome": "ACESSO_NEGADO"}
```

### 4. A base proibida — «o banco x» do pedido

No segundo servidor, porque o primeiro já se trancou:

```json
{"token":"t","op":"varrer","database":"financeiro","tabela":"qualquer"}
{"ok": false, "op": "varrer", "erro": "[SP000025] acesso negado: a base financeiro esta proibida neste servidor; o IP foi bloqueado", "codigo": 4001, "nome": "ACESSO_NEGADO", "classe": "acesso", "sprint": "SP000025", "repetir": false, "ms": 0}
```

```
    assunto: PhxSql: IP 127.0.0.1 bloqueado (base proibida pela politica)

      O PhxSql bloqueou um endereço por violação grave.

        endereço    127.0.0.1
        motivo      base proibida pela politica
        operação    varrer
        tentativas  1
        …
```

### 5. Guarda pedida: sem `avisar_seguranca`, o bloqueio acontece calado

Terceiro servidor, com `avisar_seguranca` **falso** — que é o padrão:

```
  [OK  ] reindexar (proibido) foi recusado tambem aqui
  [OK  ] o IP foi bloqueado do mesmo jeito
  [OK  ] nenhum e-mail saiu, porque ninguem pediu
```

### 6. Os números da corrida

| o que | quanto |
|---|---|
| e-mails no comando proibido | **1** |
| e-mails na base proibida | **1** |
| e-mails no comando permitido (controle) | **0** |
| e-mails sem `avisar_seguranca` (guarda pedida) | **0** |
| bloqueios no comando permitido (controle) | **0** |
| e-mails no total (com o do job, que é o controle do instrumento) | **3** |
| afirmações / falhas | **21 / 0** |

## O que NÃO existe, e é dispensa registrada

- **O aviso por e-mail NÃO existia até esta corrida — este era o achado.** O
  `Servidor::violacao_grave` (`crates/phxsql-server/src/servidor.rs`) fazia
  apenas isto quando o IP era bloqueado:

  ```rust
  if let crate::blacklist::Grave::Bloqueado(b, aviso) = &resultado {
      eprintln!("BLOQUEADO {ip} ate {} -- {} ({})", b.ate(), b.motivo, b.comando);
      if let Some(a) = aviso { eprintln!("AVISO: {a}"); }
  }
  ```

  Os cinco `crate::email::enviar` do arquivo eram promoção de master, cluster
  degradado, job que falhou, job parado e disco apertado — **nenhum** era a
  violação grave. Numa corrida anterior desta mesma bateria, hoje às 16:24 UTC,
  a linha saiu assim: `>>> e-mails novos depois do comando proibido: 0`.
  O aviso entrou depois disso, no mesmo ponto onde a blacklist é acionada.
- **`avisar_seguranca` nasce falso, e isso é decisão.** Guarda nova entra
  pedida, não imposta: quem configurou o relé só para o disco apertado não pode
  começar a receber aviso de segurança por causa de uma versão nova. É o mesmo
  desenho do `avisar_jobs`, que existe pelo mesmo motivo. O preço é o esperado:
  quem atualizar e não ligar o campo continua sem ser avisado.
- **O aviso tem silêncio por IP, e ele custa alguma coisa.**
  `Blacklist::bloquear` **substitui** o bloqueio a cada violação, então uma
  conexão já aberta que insista no comando proibido geraria um e-mail por
  tentativa — quem ataca escolheria quantas mensagens o administrador recebe. A
  chave é o IP e o silêncio é o `alertas.repetir_horas` (padrão 6 h), o mesmo do
  disco e dos jobs. Consequência registrada: **uma segunda investida do mesmo IP
  dentro da janela não manda segundo e-mail**; ela continua na blacklist e no
  `acessos.log`.
- **Não há TLS no cliente SMTP.** A `std` não traz TLS e o projeto tem zero
  dependências, então o `email.rs` fala em **texto claro**: serve para um relé
  interno que você controla, não para entregar direto num provedor público. Está
  escrito no topo do próprio `email.rs`, e vale igual para este aviso.
- **A violação leve não avisa por e-mail.** Token inválido, IP fora de
  `replicas_autorizadas` e afins passam por `violacao_leve`, que conta e só
  bloqueia depois do limite — e o e-mail sai do bloqueio, não da tentativa. Isso
  é de propósito: um varredor de portas mandaria uma mensagem por segundo.
- **`comandos_proibidos` é global, não por banco.** O pedido fala em «comandos
  proibidos para um banco x»; o motor tem as duas listas **separadas** — a de
  comandos vale para o servidor inteiro, e a de bases proíbe o banco inteiro.
  Proibir `reindexar` **só** em `financeiro` não existe hoje: ou se proíbe o
  comando em todo lugar, ou se proíbe o banco para todo comando. O caminho para
  o efeito por banco é o outro: tirar a atividade do usuário naquela base
  (resposta **M**), que é permissão e não política — e a diferença importa,
  porque a política vale **até para o root**.

## Como se refaz

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/proibidos/provar.py
```

Sobe três `phxsqld` (6500, 6501, 6502) e o SMTP falso (6510) em 127.0.0.1,
escreve tudo em `/tmp/phx-f5-<pid>` e derruba por PID no fim. Os números vão
para `bancada/proibidos/resultados.json`.

A guarda em código, para quem for mexer:
`comando_proibido_avisa_o_administrador_por_email` e
`sem_avisar_seguranca_o_bloqueio_acontece_calado`, em
`crates/phxsql-server/src/servidor.rs` — o primeiro sobe um relé SMTP de
verdade num soquete e **falha** se a chamada `avisar_violacao_por_email` sair
do `violacao_grave` (provado repondo o defeito); o segundo falha se o aviso
deixar de ser pedido e passar a ser imposto.
