# A saúde do disco onde o banco grava (pedido 249)

Pedido do dono, 16/09/2026: *«Monitor de status da saúde do disco onde o banco
de dados está sendo gravado; em caso de log de erro, aviso ⚠️ imediato por
e-mail e SMS.»*

Este documento diz o que a sonda faz, o que cada evento significa, como o
aviso chega, o que se configura, **o que não se detecta**, o que foi provado
com o defeito reposto, a revisão de segurança e as perguntas que ficaram para
o dono. Os números foram medidos em 16/09/2026; a hora está ao lado de cada
um.

## 1. O que existia, medido antes de escrever

| o que | existia? | onde |
|---|---|---|
| vigia de **espaço** (`df -k` a cada `alertas.checar_minutos`, silêncio `repetir_horas` por caminho) | sim | `ligar_vigia_de_disco`/`conferir_disco` |
| e-mail por relé SMTP, sem TLS | sim | `email::enviar`, `alertas.email` |
| **SMS** | não — zero ocorrências de `sms` no código | — |
| gancho no erro de E/S (`PhxError::Io`, 5001, `SP000010`) | não — virava resposta e linha no `acessos.log` | `error.rs:152` |
| sonda de escrita / montagem só-leitura | não — um EROFS passava calado até a próxima gravação | — |
| `fsync` do fecho recusado | virava só uma chave de volta na lista de sujas, sem noticiar ninguém (desde o 509 o processo aborta na recusa, e o aviso por e-mail dessa recusa deixou de sair — ver abaixo) | `descarregar_sujas_com` |
| cliente HTTP de saída em texto puro | não — os únicos `GET`/`POST` escritos num `TcpStream` são de **testes** contra o próprio servidor (`servidor.rs:42072`, `:42198`) | — |

A última linha decide o meio do SMS (§4).

## 2. As três peças

### 2.1 A sonda canário — `crates/phxsql-server/src/saude_do_disco.rs`

Uma thread própria (`sonda-disco`, pelo `telemetria.subir`, com finalidade
declarada e teto 1 no `mapa-das-threads.py`; sobe com a sonda ligada **ou**
com o e-mail ligado, porque é também o carteiro dos avisos — §2.3) roda a
cada `alertas.disco.checar_segundos` (padrão **300**, 5 min --
DECISÃO DO DONO, 17/09/2026 05:35, §8.2):

1. abre `<base>/.saude-do-disco` para escrita (cria/trunca);
2. escreve 64 bytes com PID, instante e o número da passada — **conteúdo
   diferente a cada passada**, para que um arquivo velho de outra passada ou
   de outro processo no mesmo `base` não passe por conferência;
3. `fsync` (`sync_all`): sem ele a sonda mediria o cache do núcleo, que aceita
   escrita num disco que já morreu;
4. relê e confere byte a byte;
5. apaga;
6. mede a duração de tudo.

Cada passo que falha é um evento nomeado pelo **passo e pelo errno**
(`abrir: EROFS (30) Read-only file system`), porque «o disco recusou» sem
dizer em que passo manda o operador olhar o lugar errado: `abrir` com `EROFS`
é montagem; `fsync` com `EIO` é o dispositivo.

**Por que a classificação olha o `kind` E o número do errno.** Medido em
16/09/2026 07:00 UTC com a `std` da `rustc 1.94.1`:

| errno | `ErrorKind` que a `std` devolve | tipo aqui |
|---|---|---|
| 30 `EROFS` | `ReadOnlyFilesystem` | `so_leitura` |
| 28 `ENOSPC` | `StorageFull` | `sem_espaco` |
| 122 `EDQUOT` | (`QuotaExceeded`) | `sem_espaco` |
| 5 `EIO` | **`Uncategorized`** | `entrada_saida` |
| 21 `EISDIR` | `IsADirectory` | `entrada_saida` |
| 20 `ENOTDIR` | `NotADirectory` | `entrada_saida` |

Um erro construído no código (`io::Error::new(kind, ..)`) chega **sem**
`raw_os_error`; um erro do núcleo chega com os dois; e o `EIO` só se nomeia
pelo número. Olhar só um dos lados deixaria metade dos casos como «genérico».

### 2.2 O gancho no erro de E/S — `Servidor::anotar`

Todo erro respondido — porta de dados, web, REST, jobs, fio — passa por
**um** sumidouro: `Servidor::anotar`, que escreve o `acessos.log` e recebe o
`Acesso` com o `codigo`. O gancho mora ali, e em lugar nenhum mais: em
quarenta operações, a que alguém esquecesse seria o furo.

O portão é uma comparação de inteiro (`acesso.codigo == CODIGO_DE_ES`, 5001)
e vem **antes** de qualquer trabalho — o caminho de sucesso, e o de todo
outro erro, não paga nada. Há teste que compara a constante com
`PhxError::Io(..).codigo()`, para o número não derivar calado.

O tipo do evento sai do **sufixo** `(os error N)` que a `std` põe em todo
erro do núcleo — analisado, nunca a frase.

**O irmão que não passa pelo `anotar`:** o fecho da janela de durabilidade
(`descarregar_sujas_com`) não responde a cliente nenhum, então um `EIO` no
`fsync` de uma tabela nunca chegava a lugar algum — a chave voltava para as
sujas e ponto. O `fecho_recusado` levava o erro para a mesma saúde, com
origem `fecho` e a base/tabela da chave. **Desde o pedido 509 (24/09/2026) isso
não acontece mais para o `fsync` recusado:** o servidor ABORTA no instante da
recusa (`sincronia::ao_recusar`), antes de o fecho chegar ao `fecho_recusado`, e
portanto **não sai evento nem e-mail** dessa recusa. O aviso volta com o
arranque do 509 (a sentinela que o arranque no mesmo boot acha; parecer do
papel C 509+512, C3) — até lá, quem vê a queda é o supervisor e o `stderr`.
E o **próprio `acessos.log`** que não
aceita escrita também é evento (origem `acessos.log`): esse erro ninguém
recebe como resposta.

**O que também conta, sem ter sido desenhado para isso:** qualquer op que
devolva `Io` — inclusive `ler` e `varrer`. Um disco que falha na leitura é o
mesmo disco.

### 2.3 O aviso imediato, com silêncio por tipo

O primeiro evento de cada tipo (`so_leitura`, `sem_espaco`, `entrada_saida`,
`conferencia`) dispara **na hora**, sem esperar relógio: o `anotar` chama a
saúde, a saúde consulta o silêncio (`jobs::pode_avisar`, o mesmo do vigia de
espaço e dos jobs) e, se deixar, **entrega o evento a uma fila** e acorda o
carteiro por `Condvar`. O carteiro é a própria thread `sonda-disco`, e é a
**única** que chama `email::enviar` — fora de qualquer trava do servidor.

Por que fila, e não o envio ali mesmo numa thread nova: quem registra o evento
pode estar com a **trava global de dados na mão** — o `fecho_recusado` está,
dentro de `descarregar_sujas_com` —, e a lei da casa (frente 38, «a trava
presa atrás da leitura de rede») proíbe alcançar rede sob a trava: um SMTP
lento congelaria o servidor inteiro no pior momento, o do disco doente. A
primeira versão desta frente subia uma thread `aviso-disco` de dentro da
seção crítica e a catraca `rede-ou-espera` do `mapa-da-trava.py` foi de 0
para **1 (teto 0)**: o mapa é estático e vê o `enviar` dentro do fechamento.
O desenho de agora deixa sob a trava só um `push` num `Mutex` curto e um
`notify_one` — e o mapa volta a 0. A fila tem teto (32): sem carteiro (sonda
e e-mail desligados) o mais velho sai, e o painel guarda o último de qualquer
jeito.

O seguinte do **mesmo tipo** dentro de `alertas.disco.repetir_minutos`
(padrão **30**) cala. É o que impede um erro por linha numa carga de cem mil
linhas de virar cem mil e-mails: **um** aviso, e depois silêncio por chave.
Todos contam no painel, calados ou não.

Quando o silêncio zera:

- a sonda voltando a **passar** zera o silêncio de `so_leitura`,
  `sem_espaco`, `conferencia` e `lento` — são o que o canário prova (a
  montagem aceita escrita, há espaço, o dado volta igual);
- o de `entrada_saida` **só zera pelo relógio**: o canário passar num arquivo
  não prova que o `.reg` de outra tabela voltou a aceitar escrita. O preço,
  dito: um `EIO` que continua a cada linha reavisa a cada 30 min, não a cada
  sonda boa.

`lento` (sonda que passou acima de `lento_ms`, padrão **1000**) é aviso de
**painel**, nunca de canal — disco que morre costuma ficar lento antes de
falhar, e é bom ver; mas mandar e-mail por latência seria a enxurrada de
volta com outro nome. Zero desliga.

## 3. O que cada evento significa

| tipo | quem gera | o que quer dizer | o que fazer |
|---|---|---|---|
| `so_leitura` | sonda (`abrir`/`escrever`), qualquer op | o núcleo remontou a partição só-leitura — quase sempre por erro de E/S anterior (`dmesg` diz) | parar de gravar, olhar `dmesg`, `fsck`, trocar o disco |
| `sem_espaco` | sonda, qualquer op | `ENOSPC` ou `EDQUOT`: sem bloco, sem inode ou sem cota | o vigia de espaço já devia ter avisado; liberar espaço |
| `entrada_saida` | sonda, qualquer op, `fecho`, `acessos.log` | `EIO` e o resto: setor ruim, cabo, controladora, ou o caminho errado (`EISDIR`, `ENOTDIR`, `ENOENT`) | o texto traz o errno; `EIO` é hardware, os outros são operação |
| `conferencia` | sonda (`reler`) | o dado voltou diferente do escrito — o disco mentindo | é o pior dos quatro: backup agora, disco fora |
| `lento` | sonda | passou, mas acima de `lento_ms` | olhar SMART por fora, carga da máquina, `iostat` |

Estado do painel: `erro` quando a última sonda falhou **ou** o último evento
de erro veio **depois** da última sonda boa (um `EIO` de três dias atrás, com
mil sondas boas desde então, não pinta de vermelho); `aviso` quando a última
sonda passou lenta; `ok`; `nao_medido` antes da primeira sonda.

## 4. Como o aviso chega

### 4.1 E-mail

Pelo `alertas.email` que já existe — o mesmo relé do disco apertado, dos jobs e
da segurança. Colado do relé falso da bateria, 16/09/2026 07:04 UTC:

```
From: phxsql@exemplo.com
To: admin@exemplo.com
Subject: PhxSql: saude do disco -- erro de E/S (inserir)

O PhxSql detectou um problema no disco onde o banco grava.

  servidor   vm
  base       /tmp/phxsrv-ut-9879-saude-sms-0
  tipo       erro de E/S
  origem     inserir (b/t)
  quando     2026-09-16 07:04:30,946
  erro       [SP000010] erro de E/S: Is a directory (os error 21)

  erros de E/S desde o arranque: 1
  ultima sonda: ainda nao rodou

Enquanto o problema continuar, o proximo aviso deste tipo sai em ate 30 min.
Servidor PhxSql 0.18.0
```

O corpo **nunca** leva o pedido: um `inserir` recusado carrega a linha que se
tentou gravar, e há teste que reprova se o corpo contiver `valores`.

### 4.2 SMS — e-mail-para-SMS da operadora

**O meio é decisão do dono, e o que entrou é o que funciona hoje sem crate
nenhuma.** As opções, medidas:

| meio | pede | entrou? |
|---|---|---|
| **e-mail-para-SMS** da operadora (`numero@gateway`) | o relé SMTP que já existe | **sim** — inteiro |
| gateway **HTTPS** (Twilio, Zenvia, AWS SNS, …) | TLS → crate | **não** — pétrea de zero dependências; pergunta ao dono (§8) |
| gateway **HTTP** em texto puro | um cliente HTTP de saída | **não** — não existe nenhum na casa (§1), e nenhum gateway sério aceita texto puro |
| **programa externo** (`comando: [...]`, sem shell) | `std::process::Command` | **não entrou nesta rodada** — está no contrato da rodada como opção, mas o brief da frente pediu o e-mail-para-SMS e deixou o resto como pergunta; executar comando vindo do `config.json` é ponto de segurança que merece o «sim» do dono antes (§8) |

Como sai: o mesmo `Email` com o `para` trocado por `numeros[i]@gateway_email`.
Colado do relé, na mesma corrida:

```
To: +5541999990000@sms.exemplo
Subject: PhxSql

PhxSql vm: disco erro de E/S em inserir (b/t) 07:04 UTC
```

Uma linha, **até 160 caracteres** (56 nesta), sem o caminho do disco e sem
segredo: SMS atravessa a operadora em claro, e `/home/fulano/dados` é mais do
que a operadora precisa saber. Há teste que reprova se o SMS contiver o
caminho do `base`, uma quebra de linha ou mais de 160 caracteres.

**Exige `alertas.email.ligado`** — sem relé não há por onde sair, e a recusa
vem no arranque, não na primeira falha de disco.

### 4.3 O painel e a op

- `op_painel` ganhou o bloco `saude_do_disco` (estado, `checar_segundos`,
  `repetir_minutos`, `lento_ms`, `sondas`, `medido_em`, `canario_ms`,
  `canario_falha`, `erros_es`, `ultimo_evento`, `avisos {email, sms,
  ultimo_email, ultimo_sms, ultima_falha}`).
- A op nova **`saude_disco`** (só leitura, `Atividade::Ler` como o `painel`,
  `ferramenta_mcp: true`) devolve o mesmo bloco.
- O cartão **Saúde do disco** fica no Painel, ao lado do espaço em disco
  (`saudeDiscoHtml`, `#saudeDisco`), com pino de estado por **forma e cor**
  (● saudável, ▲ lento, ✕ com erro, ○ não medido) e todo texto pela
  `FABRICA_TELA` nos seis idiomas (`tela.sd_*`, 24 chaves). Catraca
  `TETO_ROTULOS_E_CRASE` em 1.049, inalterada — nada cravado entrou.

## 5. O que se configura

```json
"alertas": {
  "email": { "ligado": true, "servidor": "127.0.0.1", "porta": 25,
             "de": "phxsql@empresa.com.br", "para": ["dba@empresa.com.br"] },
  "disco": { "ligado": true, "checar_segundos": 300, "repetir_minutos": 30, "lento_ms": 1000 },
  "sms":   { "ligado": true, "numeros": ["+5541999990000"], "gateway_email": "sms.operadora.com.br" }
}
```

| campo | padrão | leitor | o que faz |
|---|---|---|---|
| `disco.ligado` | **true** | `Disco::de_json` → `ligar_sonda_de_disco` | sobe (ou não) a thread da sonda |
| `disco.checar_segundos` | 300 (mínimo 1) | `SaudeDoDisco::checar_segundos` | intervalo da sonda |
| `disco.repetir_minutos` | 30 | `SaudeDoDisco::silencio_ms` | silêncio entre avisos do mesmo tipo |
| `disco.lento_ms` | 1000 (0 desliga) | `sondar`/`estado` | acima disto, `aviso` no painel |
| `sms.ligado` | false | `avisar_saude_do_disco` | manda o SMS junto do e-mail |
| `sms.numeros` | `[]` | `Sms::enderecos` | dígitos com `+` opcional; viram a parte local do endereço |
| `sms.gateway_email` | `""` | `Sms::enderecos` | domínio da operadora, sem arroba |

**Por que a sonda nasce ligada, ao contrário do vigia de espaço.** O vigia
manda e-mail, e por isso nasce desligado — aviso que ninguém pediu é caixa
de entrada cheia. A sonda só **mede** (64 bytes a cada 5 min) e pinta o painel;
o aviso continua dependendo de `email.ligado`. Sem ela ligada, o cartão diria
«não medido» para todo mundo que nunca abriu o `config.json`, e um monitor
que nasce cego não monitora nada. É a mesma decisão da telemetria, que nasce
coletando. Quem não quer a escrita periódica escreve `"disco": {"ligado":
false}`. (Pergunta ao dono em §8.)

Cada campo tem leitor e teste (`alertas_disco_e_sms_sao_lidos_do_arquivo`,
`sem_o_bloco_a_sonda_nasce_ligada_e_o_sms_desligado`,
`o_sms_recusa_no_arranque_o_que_nao_entregaria`), e as seções
`alertas.disco`/`alertas.sms` entraram em `SECOES_CONHECIDAS` — o verificador
de campo estranho não acusa um exemplo que está certo.

### 4.4 O backup agendado que falha (pedido 510)

O backup agendado que falhava só escrevia no erro padrão, e o dono descobria
que não tinha cópia no dia de restaurar. Agora a falha **pega carona no
carteiro**: `SaudeDoDisco::falha_do_backup` passa pelo silêncio (chave própria,
`backup`, com o mesmo `repetir_minutos`) e entrega à fila; a thread
`sonda-disco` manda o e-mail («o backup agendado FALHOU», com o destino, o erro
do sistema e quando é a próxima tentativa) e o SMS. O primeiro backup que dá
certo zera o silêncio, como nos jobs. A corrida que o processo não terminou
(pedido 502) avisa pelo mesmo caminho, no arranque seguinte.

Ele **não** entra no `ultimo_evento` nem pinta o painel, e os envios dele não
contam em `avisos`: a carta é do disco onde o banco grava, e um destino sem
espaço pintaria de vermelho um disco que está bem — e esconderia o erro de E/S
de verdade que viesse antes dele. Sem `alertas.email.ligado`, o aviso continua
sendo só o erro padrão.

Medido pelo soquete, com o rele falso (`servidor::testes_do_relogio_e_do_backup`):

| a falha, do sistema operacional | sem o aviso | com o aviso |
|---|---|---|
| destino dentro de um arquivo comum (`ENOTDIR`, os error 20) | nenhum e-mail em 10 s | um e-mail, com o destino e `os error 20` |
| destino num `tmpfs` de 64 KiB, base de 256 KiB (`ENOSPC`, os error 28) — teste `#[ignore]`, precisa de `mount` | — | «backup agendado FALHOU: [SP000010] erro de E/S: No space left on device (os error 28)» e o e-mail entregue |

«Destino sem permissão» não serve de prova neste contêiner: ele roda como
root, e root atravessa a permissão.

O que **não** mudou: o backup que falha só tenta de novo na próxima hora da
agenda. Tentar antes é decisão de política que o pedido não pediu.

## 6. O que NÃO se detecta (dito)

- **SMART** do dispositivo — não se chama `smartctl`; um disco com setores
  realocados e ainda respondendo aparece como `ok`.
- **Montagem só-leitura entre duas sondas** — o EROFS aparece na próxima
  sonda (até `checar_segundos`) ou na próxima gravação de verdade, o que vier
  primeiro. Não se lê `/proc/mounts`.
- **Disco de outro caminho** — só o `base`. O destino do backup e os
  `alertas.caminhos` continuam só no vigia de espaço.
- **Corrupção silenciosa de dado já gravado** — o canário prova a escrita de
  agora, não os `.reg` de ontem; para isso existe `verificar`/`checksum`.
- **Latência do dispositivo fora da própria escrita** — `canario_ms` é o
  tempo da sonda inteira (abrir + escrever + `fsync` + reler + apagar), num
  arquivo pequeno; não é `iostat`.
- **Erros dentro das threads de réplica e de cluster** que não passem por
  `anotar` — não foram mapeados nesta rodada; se algum `Io` morre num
  `eprintln!` por lá, ele ainda não conta aqui.
- **O sítio do fecho com o defeito reposto** — o método `fecho_recusado` é
  provado por teste direto; a **chamada** dele dentro de
  `descarregar_sujas_com` não tem prova com defeito reposto, porque não há
  como fazer um `fsync` falhar sem injeção de falha no sistema de arquivos.
  Buraco dito, não escondido.

## 7. Provas reais, nos dois sentidos

Medido em 16/09/2026 07:40 UTC, `cargo test -p phxsql-server --lib`: os 12
testes de `saude_do_disco::testes`, os 6 de
`servidor::testes_da_saude_do_disco` e os 3 novos de `config::testes_recursos`
passam; os testes de idiomas (13) e a catraca de rótulos passam. O total da
suíte está no relatório da frente, com a saída do `cargo test --workspace`.

**Contra o sistema operacional, não contra dublê**
(`a_sonda_falha_contra_o_sistema_operacional`): esta bateria roda como
**root** (`id -u` = 0), e `chmod 0555` não segura root — o teste **diz isso**
em vez de fingir que provou, e prova por dois caminhos reais: um **arquivo no
lugar do diretório** (`abrir: ENOTDIR (20)`) e um **caminho inexistente**
(`abrir: ENOENT (2)`). Sem root, o terceiro caminho (`0o555` →
`EACCES (13)`) também vale, e o teste o exercita.

**Pelo soquete, não pelo `despachar`**
(`erro_de_es_numa_gravacao_avisa_na_hora_e_uma_vez_so`): a tabela `b/t` tem o
`.reg` trocado por um diretório — o `inserir` seguinte recebe um `Io` de
verdade (`EISDIR (21)`); o pedido entra pela porta de dados de produção
(`aceitar_ate_mandarem_parar`), porque é o `atender` quem chama o `anotar`, e
o gancho mora no `anotar`. Um `despachar` direto, como nos outros testes,
provaria a recusa e **pularia o gancho** — foi o primeiro erro desta frente,
e a cognição do dia registra. O relé falso recebe **um** e-mail em menos de
5 s (medido: a corrida inteira do teste leva 0,75 s), **nenhum** no segundo
erro dentro da janela, e o painel conta 2.

**O SMS** (`o_sms_sai_pelo_gateway_da_operadora_numa_linha_sem_caminho`):
segunda mensagem no mesmo relé, `To: +5541999990000@sms.exemplo`, uma linha,
56 caracteres, sem o caminho do `base`, e nada além das duas.

**O comportamento velho** (`sem_email_ligado_o_erro_conta_e_nao_avisa`): sem
e-mail ligado o erro continua sendo respondido e contado, e nada sai.

**No navegador** (`testes-web/casos/29-saude-do-disco.mjs`, nos dois temas):
o cartão saudável com as quatro linhas e o pino ●; um diretório no lugar do
canário → `abrir: EISDIR (21)`, estado `erro`, pino ✕; removido → `ok` na
passada seguinte. O que se lê é o `data-estado`, nunca a frase.

**Guardas no catálogo** (`bancada/guardas/catalogo.py`), cada uma provada
vermelha com `provar-guardas.py --so <id>` — o resultado da corrida está no
relatório da frente e em `ultima-corrida.json`:

| id | defeito reposto | quem cai |
|---|---|---|
| `disco-erro-de-es-sem-aviso` | o gancho do `anotar` compara com um código que nunca chega | os três testes do servidor |
| `disco-sonda-cega-ao-erro` | o `abrir` que falha é engolido | `a_sonda_falha_contra_o_sistema_operacional` |
| `disco-silencio-furado` | todo evento avisa | o teste do silêncio e o do e-mail único |
| `disco-config-nao-lida` | `checar_segundos` devolve o padrão | `alertas_disco_e_sms_sao_lidos_do_arquivo` |

## 8. Perguntas ao dono

1. **O meio do SMS.** Entrou o e-mail-para-SMS da operadora, que funciona com
   o relé que já existe. Se a operadora de vocês não oferece esse gateway,
   as saídas são: (a) um **programa externo** configurado (`comando:
   ["/usr/bin/curl", ...]`, sem shell, argumento a argumento — zero crate,
   mas executa o que estiver no `config.json`, e isso é decisão de
   segurança); (b) um gateway **HTTPS** — pede TLS, TLS pede crate, e a
   pétrea diz que isso é pergunta, não decisão de frente. Qual dos dois, ou
   nenhum?
2. **O intervalo da sonda: 60 s ou 5 min? RESOLVIDO, DECISÃO DO DONO,
   17/09/2026 05:35** (`docs/PENDENCIAS.md` #249): **5 minutos**. Disco
   somente-leitura ou cheio não é evento de segundo, e 288 escritas por dia
   no diretório do banco em vez de 1.440. `Disco::default().checar_segundos
   == 300` (`config.rs`).
3. **A sonda nasce ligada? RESOLVIDO, DECISÃO DO DONO, 17/09/2026 05:35**
   (`docs/PENDENCIAS.md` #249): **sim, só medindo** — o aviso continua
   dependendo do e-mail já configurado, então ligada não incomoda quem não
   pediu, e guarda que nasce desligada é guarda que ninguém lembra de ligar
   no dia em que precisa. `Disco::default().ligado == true` (`config.rs`).
4. **O SMS vale só para a saúde do disco.** Disco apertado, job que falhou e
   IP bloqueado continuam só por e-mail. Estender é um `if sms.ligado` em
   cada um dos três avisos — quer?

## 9. Revisão SEC (chapéu trocado, 16/09/2026)

Está em `docs/SEGURANCA.md` §3, subseção *«O aviso de saúde do disco, por
e-mail e SMS»*. Em resumo: o e-mail leva host e caminho do `base` (a mesma
exposição do aviso de disco apertado, para um relé interno por construção);
o SMS **não** leva caminho; o corpo nunca leva o pedido; a senha do SMTP
continua privada e fora de toda resposta; o canário tem nome, tamanho e
conteúdo fixos pelo servidor e não entra em resposta nenhuma; o painel pede
`ler` e o texto do erro e a base/tabela só saem para quem administra; os
números do SMS e o gateway passam pelo mesmo crivo de injeção de cabeçalho do
`de`/`para`.
