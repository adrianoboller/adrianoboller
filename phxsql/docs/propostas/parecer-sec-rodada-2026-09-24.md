# Parecer SEC: rodada de 24/09/2026 (522, lote de integridade, fáceis D)

Revisor adversário de segurança (papel SEC), só leitura. Árvore viva
`phxsql/` no `a494f33`; fáceis D pelo diff da frente
(`scratchpad/faceisD.patch`, já aplicado na árvore viva, sem commit). Nada foi
compilado: as provas são de leitura, com arquivo e linha. Cada achado traz o
teste adverso que o demonstraria.

| Item | Veredito |
|---|---|
| 522 — `phx_reindexar` e `ATESTADOS` (`b5fc11c`) | **LIBERA** |
| Lote de integridade 490/491/492/515/516 (`a494f33`) | **LIBERA** |
| Fáceis A: o parecer anterior muda? | **Não muda**. O 544 cumpre a condição dele |
| Segredo em log, erro ou `Debug` nos commits de hoje | **Nada achado** |
| Fáceis D: 544, 530, 463 | **LIBERA** os três, com irmãos pré-existentes que viram pedido |

Nenhum item bloqueia. Nenhum expõe valor de linha, valor de chave, senha ou
token.

---

## 1. Pedido 522: LIBERA

### `phx_reindexar` (`crates/phxsql-ffi/src/lib.rs:817`)

- **Punho validado.** Passa por `com(p, ETIQ_TABELA, ..)` (`punho.rs`):
  - nulo, punho morto ou punho vivo de outro tipo são recusados pelo registro
    `VIVOS`, antes de a memória ser lida;
  - o teste `phx_reindexar_com_punho_errado_devolve_codigo` cobre o nulo e o
    punho de base passado como tabela.
- **Escrita em `qtd`.** Só no sucesso, pelo `saida()` (`lib.rs:185`), que
  aceita nulo. Não há outra escrita.
- **Pânico.** O `catch_unwind` do `com` o captura e envenena o punho. Nenhum
  pânico atravessa a ABI por esta função.
- **Sem privilégio a cruzar.** O embutido já tem o diretório inteiro nas mãos.
  O `.fts` refeito continua selado quando indexa coluna marcada:
  `reconstruir_fts` → `FtsFile::recriar(.., texto_sobre_coluna_marcada)`.
  Falta de chave é recusa, e não refação em claro (`table.rs`, vala do
  `Autorizacao`).
- **`phx_tabela_fechar` (`lib.rs:704`).** Tira o punho do registro antes do
  `antes`. Devolve o erro do `sincronizar` e mesmo assim libera. Um segundo
  `fechar`, ou o uso depois, cai no registro (`PHX_ERRO_PONTEIRO`), sem
  tocar em memória morta.

### `ATESTADOS` (`crates/phxsql-store/src/ndx.rs:229`)

**A chave pode ser forjada por symlink? Não abre porta nova.**

- A chave é o caminho léxico e o CRC-32 do cabeçalho.
  - Symlink, `..` e hardlink dão OUTRA chave. O atestado se perde e a tabela
    reconstrói, que é o lado seguro. O 523 (fáceis D, `volume::chaves_reais`)
    mede o mesmo: a chave léxica só erra desse lado aqui.
  - Para confiar num `.ndx` que não fechou, alguém teria de pôr no caminho
    atestado um arquivo com o mesmo CRC.
- O CRC não é MAC. Mas quem escreve no diretório de dados já pode gravar o
  byte 52 em 0, com o CRC certo, sem atestado nenhum. O atestado não amplia o
  que esse atacante já tinha.
- A restauração do motor reconstrói no palco, com outra chave, antes da troca
  (`restaurar.rs:587`).

**O registro cresce sem teto? Só por um furo pequeno.** É o R1 abaixo.

- `sincronizar`, `criar` (o truncamento), a primeira escrita e o
  `renomear` tiram a entrada.
- O `excluir_tabela` (`catalogo.rs:933-965`) apaga os arquivos e **não** tira
  o atestado.
- Cada tabela apagada enquanto atestada deixa uma entrada órfã até o fim do
  processo. São ~100 a 200 B por entrada, **estimados pelo tipo e não
  medidos**. Só cresce com DDL: criar num nome novo, gravar e apagar.

### Informativos (sem pedido)

- **I1.** `liberar_depois_de` (`punho.rs:290`): o `drop(caixa)` fica fora do
  `catch_unwind`.
  - Um pânico no `Drop` da `Table` atravessaria o `extern "C"`.
  - No rustc 1.94.1 pinado isso aborta o processo. Com o `rust-version =
    "1.75"` declarado no `Cargo.toml:18` é comportamento indefinido.
  - Já existia antes do 522.
- **I2 (para o DBA).** O `phx_reindexar` trunca o `.ndx`. Um SEGUNDO punho
  aberto na mesma tabela, com páginas sujas no cache, as despejaria no
  arquivo novo ao fechar.
  - O contrato 5 do `phxsql.h` não promete dois punhos na mesma tabela.
  - Dois escritores na mesma tabela já corrompiam sem o reindexar.

---

## 2. Lote de integridade (`a494f33`): LIBERA

**Vazam valor de linha ou valor de chave? Não.** As três mensagens novas
levam só nome de tabela, nome de chave, nome de coluna e rowid:

- `table.rs` `conferir_filhas_com`, a recusa do 491;
- `catalogo.rs`, o renomear da auto-referência;
- `servidor.rs:17152-17190`, o `elo_barrado` do 516.

**Existência de linha para quem não tem direito? O oráculo é o do RESTRICT, e
é convergente.**

- A recusa diz «esta linha tem filhas em F».
- O PostgreSQL diz «is still referenced from table F», e o MySQL diz
  `a foreign key constraint fails (db.F ...)`. Os dois fazem isso sem
  conferir direito na filha.
- O 491 só estende a recusa à PRÓPRIA tabela, onde quem pede já está
  excluindo.

**O `TRANSACAO_ABORTADA` do ciclo diz o id da outra transação? Sim, e diz mais.**

- O `recado_da_barrada` (`transacao.rs:747-803`) põe no texto o id, o LOGIN
  do dono, a ligação e o instante de abertura.
- Isso já saía em todo `LOCK TIMEOUT` antes deste lote. O 516 levou a mesma
  frase ao COMMIT, onde o elo é uma linha da tabela filha, que pode ser
  negada a quem pede.
- Não é capacidade: `kill`/`encerrar_sessao` e `transacoes` exigem
  `administrar` (`servidor.rs:367`, `usuarios.rs:260`).
- A divergência é nossa: o PostgreSQL mostra o pid, o MySQL não mostra nada.
  Vai para o R2, e não bloqueia.

**Forçar a vítima a ceder?** Só com um ciclo real em que o atacante é o mais
velho. É a resolução de impasse de qualquer motor. Segurar trava já derrubava
a vítima por `LOCK TIMEOUT`.

---

## 3. Parecer dos fáceis A: não muda

- O 522 e o lote de integridade não tocam 369, 443 nem 529.
- O 544 (fáceis D, abaixo) fecha os dois residuais do 443 que aquele parecer
  pôs como condição: o lenenc no `mysql.rs` e a contagem de campos no
  `pg/mod.rs`.

---

## 4. Segredo em log, erro ou `Debug` nos commits de hoje: nada achado

Varredura das linhas `+` de `git log -p --since='2026-09-24 00:00' -- '*.rs'`
(117 commits). O filtro foi `format!`/`eprintln!`/`{:?}` perto de senha,
token, chave, hash, cpf e `dado_pessoal`. O que sobrou fora de teste:

- `jobs.rs:276`: o `achado` é o NOME do campo («o campo "senha"»), e não o
  valor;
- `dblink/mod.rs`: a credencial é citada pelo campo, pelo nome da ligação e
  pelo tamanho em bytes;
- `MaterialEscrito` (`dblink/mod.rs:1074`) ganhou `Debug` com sal, iterações
  e prova:
  - é material público de conferência, e já está no arquivo;
  - nada o formata;
  - a `CifraDoCadastro`, que guarda a chave, não tem `Debug`;
- o eco do relé SMTP, que é o D4 abaixo.

---

## 5. Fáceis D

### 544: parser do DbLink (MySQL e PostgreSQL): LIBERA

**O conserto:**

- `fatia_lenenc` faz `checked_add` e confere o fim contra o tamanho do
  pacote;
- `cadeia_ate_nulo` nunca deixa o `*i` passar do fim;
- `quantos_campos` recusa contagem negativa e contagem acima de
  `TETO_DE_COLUNAS`.

**Sobrou pânico? Nenhum, pela leitura dos irmãos.** Os pontos de índice
lidos um a um:

- MySQL:
  - `ler_saudacao`, com `get`;
  - troca de plugin, `r[i..]` com `i <= len`;
  - `erro_do_servidor`, com `get`;
  - `ler_coluna`, com `get` e `unwrap_or`;
  - `le_lenenc`, com `get`.
- PostgreSQL:
  - `Leitor::bytes`, com `checked_add`;
  - `Leitor::cadeia`, com `pos <= len`;
  - `autenticar` e `esperar_autenticacao`, com `corpo[4..]` depois de
    `get(..4)`;
  - `erro_do_servidor`, com `i < len`;
  - `cadeias_nulas`.

**Nit, sem condição.** Um lenenc com o cabeçalho cortado (`0xFC` e um byte
só) volta `None` e vira campo VAZIO calado (`unwrap_or(0)`). É o «cortado
calado» que o 544 quis matar. Só um par malformado o produz, e esse par já
manda o valor que quiser.

**Sobrou reserva guiada pelo par? Sim, em três eixos que o 544 não prometia.**
Os três já existiam, e ficam D1 a D3.

### 530: job não dispara job: LIBERA

**Contornos testados pela leitura, nenhum passa:**

- **Entrada única.** O único caminho até `job_rodar` é
  `executar` → `op_job_rodar` (`servidor.rs:11631`).
- **Operações aninhadas.** Lote, gatilho e rotina rodam na thread de quem
  chama, e ali a família é `corrida`.
- **Procedimento e gatilho.** Não há SQL nem rotina que chame `job_rodar`: 0
  ocorrências em `phxsql-sql` e em `rotinas.rs`.
- **REST.** O job não tem operação de HTTP de saída.
- **Laço por rede.** O DbLink `phxsql` só manda operações fixas (`sql`,
  `ping`, `esquema`, `varrer`...), e não há volta a `job_rodar` por ele.
- **Forjar a família.** `familia_desta_thread` é `thread_local` posto só por
  `subir`/`rodar_em_filha`, e não vem da entrada.

**Residual.** A guarda vale enquanto toda operação aninhada numa corrida rodar
na thread da corrida. Uma operação futura que despache para uma thread de
outra família reabre o furo calada.

### 463: prazo total do SMTP: LIBERA

- O `ComPrazo` embrulha leitor e escritor desde depois do `connect` até o
  `QUIT`. Cobre o `AUTH LOGIN` (3 passos contados em `passos_da_conversa`) e o
  corpo do `DATA`.
- O prazo é por syscall, então a linha pingada byte a byte também para.

**E o TLS/STARTTLS? Não existe neste cliente.** O `email.rs:9-20` documenta o
limite: sem crate, sem TLS, e o `AUTH LOGIN` vai em claro, o que é limite
declarado e anterior. Não há o que cobrir.

**Condição para o futuro.** Quem acrescentar TLS o põe POR CIMA do
`ComPrazo`, e não do `TcpStream` cru. Senão o aperto de mão escapa do prazo.

**Residuais, já existentes antes:**

- O `connect` e o DNS ficam fora do total: N endereços × `timeout_s`, e um
  `getaddrinfo` sem prazo. Baixo.
- O eco do relé é o D4.

---

## Pedidos propostos (passam pelo juiz PhxJev)

| # | Estado proposto | Achado | Cenário (entrada → efeito) | Teste adverso |
|---|---|---|---|---|
| **D1** | ☐ **média** | A rede do DbLink é lida **com a trava de dados na mão**, e só com prazo de silêncio. `op_dblink_sincronizar` trava em `servidor.rs:25011` e consulta em `:25061`; `op_dblink_ligar` trava em `:24922` e consulta em `:24944`. O `consultar` lê até o fim: o MySQL continua depois do corte (`mysql.rs:336`) e o PostgreSQL até o `Z` (`pg/mod.rs:312`) | Um par comprometido, ou alguém no meio do fio em claro, pinga uma linha (ou um `N`) a um passo do `timeout`, para sempre. A trava global não sai, e o servidor inteiro para de responder | Par MySQL falso que responde 1 coluna e manda uma linha a cada 100 ms sem EOF, com `max_linhas` 1. Durante o `dblink_sincronizar`, um `ping` de outra conexão tem de responder em tempo limitado |
| **D2** | ☐ **média** | Memória do resultado guiada pelo par: até `teto` linhas × 128 MiB por linha no MySQL (`TETO_DO_REGISTRO` por quadro) ou × 64 MiB no PostgreSQL, sem teto de bytes no total. Também sob a trava | Par que manda `max_linhas` linhas de 100 MiB: o processo morre por falta de memória | Par falso com linhas de 100 MiB. O `consultar` tem de recusar com `LimiteExcedido` num teto total antes de o RSS passar dele |
| **D3** | ☐ **baixa** | O `i=` do SCRAM não tem teto (`pg/scram.rs:155-163`): vale qualquer `u32` | Quem está no meio ecoa o nosso nonce e manda `i=4294967295`. São ~1,38 µs por iteração, DERIVADOS de 290,3 ms/210.000 (`SEGURANCA.md` §11.4) e não remedidos: **~1 h 39 min de uma CPU por tentativa**, fora da trava | Par PostgreSQL falso com `i=4294967295`. A abertura tem de recusar antes do PBKDF2. O teto quem decide é o papel J: a libpq não tem teto, e a nossa divergência se justifica pelo fio em claro |
| **R1** | ⏸ baixa | O `excluir_tabela` não tira o atestado do 522 | Laço de criar `t_i`, gravar 1 linha e apagar: +1 entrada órfã por volta, até o processo cair | 10.000 voltas, e o registro volta ao tamanho de antes (precisa de um contador só de teste) |
| **R2** | ⏸ baixa | O recado de trava diz o LOGIN do dono a quem não tem direito na tabela travada. Já existia no `LOCK TIMEOUT`, e o 516 o levou ao COMMIT | Cadastro com a tabela `filha` negada ao usuário A. B trava uma linha da filha. A altera a chave da mãe e manda COMMIT, e lê o login de B e o rowid | A resposta a A não contém o login de B. O nome da tabela filha pode aparecer, porque é convergente nos três |
| **D4** | ⏸ baixa | O texto do relé é ecoado no erro (`email.rs:400,404`) | Relé malicioso, ou alguém no meio, responde ao `AUTH` com o base64 da senha, e ela vai ao log e à resposta do `email_testar`. Quem está no fio já a tem: o que se amplia é quem lê o log | Relé falso que ecoa o base64 recebido em `535`. O erro não pode conter a senha nem o base64 dela. A redação, ANALISANDO: nos passos do `AUTH`, só o código |

**D1 e D2 saem do mesmo motor do 463.** O prazo total do `ComPrazo` e um
teto de bytes do resultado são a mesma pergunta que o `email.rs` acabou de
responder: quanto tempo e quanta memória este lado dá a quem ainda não provou
nada. Resolver no mesmo lugar, e não com um segundo prazo ao lado, é a lei
«função e comando vêm do mesmo motor».

**D1 é o mais sério da rodada.** Não é exposição de dado. É o servidor
inteiro parado por um par de rede, e a regra do dono põe isso na conta
(travamento).

---

## 6. Fáceis C (464 e 365, os itens de segurança)

Lido no worktree `agent-a466115b9d5f16228`, com `git diff HEAD` (HEAD
`8e5a545`). Os números de linha abaixo são desse worktree.

| Item | Veredito |
|---|---|
| 464, `Column::recusa_de_valor` como motor único | **LIBERA**. O motor está certo; três irmãos pré-existentes viram pedido (C1 a C3) |
| 365, o `sql` normalizado no `perfil.txt` | **LIBERA COM CONDIÇÃO**: `TRUE`/`FALSE` ainda saem crus |
| A garantia do 497 depois das 16 expectativas trocadas | **Continua** |

### 464: LIBERA

**O motor redige analisando.**

- A frase do conversor é jogada fora inteira. A nova sai só do esquema: o
  nome da coluna, o tipo declarado (`Str(14)` é largura do esquema, e não do
  valor) e o grau.
- A variante é a mesma, então o código que o cliente trata não muda.
- `Corrompido` e `Io` passam intactos, e é certo que passem: nenhum conversor
  os usa com o valor. Conferi os três: `json_para_valor`, `valor_de_texto` e
  `escrever_inline`.

**A mensagem vaza tamanho ou formato que reidentifique? Não.**

- Não traz o comprimento nem pedaço do valor.
- Os três motivos (serve ao tipo, cabe no tipo, foi recusado) dão no máximo
  1,5 bit sobre o valor, a quem o digitou.
- O `Duplicado`, a FK e o CHECK não citam valor: conferi as seis mensagens de
  unicidade e a da FK, e o CHECK cita a expressão do esquema.

**Irmãos que ficaram fora.** Os três já existiam antes da frente. Nenhum passa
pela lista do `SEGURANCA.md` §28.

- **C1: o DbLink PUXAR** (`dblink/sincronia.rs:286`).
  - `linha_remota_para_negocio` chama `valor_de_texto(t, ty)?` sem a porta.
  - A célula remota que não serve à coluna local marcada sai citada:
    `esperado Data, recebido "999.888.777-66"`, até 48 bytes.
  - Vai à resposta do `dblink_sincronizar`, ao `acessos.log`, ao Profiler e
    ao `jobs.log` quando roda por job.
  - O dado é de um titular do OUTRO banco, que ninguém digitou aqui.
  - O conserto é uma linha:
    `.map_err(|e| esquema.colunas()[*pos].recusa_de_valor(e))`.
- **C2: o DbLink EMPURRAR** (`sincronia.rs:362` → `dblink::literal` →
  `nome_seguro`, `dblink/mod.rs:1829-1856`).
  - Todo valor `Str`/`Memo` passa pela régua de IDENTIFICADOR.
  - Uma aspa simples («D'Ávila», «Rua Sant'Ana»), aspa dupla, contrabarra ou
    quebra de linha (qualquer `Memo` de várias linhas) recusa com
    `nome com caractere que nao vale em identificador: {n:?}`.
  - A recusa traz o valor INTEIRO.
  - Acima de 128 bytes é `nome longo demais: {n:?}`, também inteiro: não
    passa pelo `citar`, então um `Memo` de 1 MiB vai inteiro ao log. É irmão
    do 464 e do 453 ao mesmo tempo.
  - Para o DBA: o mesmo caminho faz `trim()` no valor antes de mandar, e
    «Ana » chega lá como «Ana», calado.
- **C3: a avaliação de expressão.**
  - `expressao::Valor::descricao` (`expressao.rs:152-161`) tapa o texto
    desde o 497, mas cita número e booleano: «o numero 12345678901».
  - As recusas de avaliação saem pelo `?` antes do `coagir`, e ali a porta
    não alcança: padrão, calculada, índice por expressão, CHECK
    (`table.rs:4157/4346/4355/4366`), `consultar.expressao`.
  - A porta olha só a coluna de DESTINO. Uma calculada NÃO marcada que parte
    de uma coluna marcada tem o `coagir` citando o número derivado:
    `faixa Int1 = renda / 1000` mostra «o numero 500».
  - É o menor dos três: exige expressão que falha por tipo.
  - O comentário em `table.rs:4356` («a conta pode partir de coluna
    marcada») promete mais do que o código cobre.
- **Informativo:** `servidor.rs:18264`, o `coagir` do padrão no
  `acrescentar_coluna`, também fica sem a porta. É literal de DDL digitado
  pelo administrador, e não dado de titular.

### 365: LIBERA COM CONDIÇÃO

O que o integrador perguntou, conferido no léxico (`lexico.rs`) e no
`redigir` (`usuario.rs`):

| Forma | Sai no `perfil.txt` |
|---|---|
| comentário `--`, `/* */`, `/*!...*/` | some: o léxico o descarta |
| `#`, `@`, crase, `$` | `<comando invalido, N bytes>`: o léxico recusa o caractere |
| identificador entre aspas duplas com dado | `"***"` (cai no `descrever`) |
| `X'3939'` | `X ?` |
| `0x3939`, `1e5` | `<comando invalido, N bytes>`: número colado recusa |
| `DATE '2001-02-03'` | `DATE ?` |
| `SET PASSWORD FOR c = 'x'`, `IDENTIFIED BY x` sem aspas | `SET PASSWORD ?`, `IDENTIFIED ?` (regra da senha) |
| `parametros` irmãos | `"***"` |
| erro do evento | o tamanho |
| **`TRUE` / `FALSE` (e `NULL`)** | **CRU**. São `Token::Palavra { citado: false }` e caem no braço que copia a palavra (`usuario.rs:352-355`) |

**Condição.**

- **O cenário:** `INSERT INTO pacientes (id, hiv) VALUES (?, TRUE)` vai ao
  arquivo como `VALUES ( ? , TRUE )`.
- **Por que conta:** num booleano marcado, o dado é o próprio booleano, e a
  categoria sensível da LGPD é exatamente essa. A linha vem com login, IP e
  instante ao lado.
- **Contra a promessa:** o §13.14 promete «todo literal».
- **Contra a convergência:** o `pg_stat_statements` troca `true` e `NULL`
  por `$n` (são `Const`). O digest do MySQL, o papel J confere.
- **O conserto:** no modo `Literais`, `TRUE`/`FALSE` (sem caixa, não citados)
  viram `?`. `NULL` também, de preferência. É o mesmo braço, sem léxico novo.
- **O teste:** o `o_normalizado_troca_todo_literal_e_guarda_o_comando` ganha
  `TRUE` na lista de literais que não podem ficar.

**Os pontos que conferi e passam:**

- **Quem decide é o `op`.** O `e_pedido_sql` casa `op` com `trim` e sem
  caixa, e com qualquer `op` repetido (`any`). É mais largo que o despacho,
  que lê a primeira chave exata: sobra redação, nunca falta.
- **O `sql` aninhado.** Em job, lote ou transação ele é alcançado, porque o
  `limpar_com` desce objeto e lista. O `pedido` do job é sempre objeto
  (`jobs.rs:238-246`).
- **As duas portas que capturam** (`servidor.rs:9930` web e `:10356` dados)
  passam pelo mesmo `analisar_pedido`.
- **As exclusões** (`criar_visao`, `dblink_consultar`) são coerentes com o
  modelo do 356. O arquivo já leva o valor de tabela NÃO sigilosa por
  desenho, e nenhuma das duas nomeia tabela local sigilosa.

### A garantia do 497 continua

- As 16 expectativas trocadas são a PROVA de redação (`novas.contains(prova)`).
  A GARANTIA é a ausência de `SEGREDO123` em toda linha nova do
  `perfil.txt`. Ela continua em todos os casos com `perfil: Some`, pelas duas
  portas.
- A prova nova é mais forte que o `'***'`. Em `PASSWORD SEGREDO123` e
  `IDENTIFIED BY SEGREDO123` sem aspas não há literal: só a regra da senha
  produz o `?`. Então a frase ainda prova que a regra da senha rodou dentro do
  modo `Literais`.
- O anel continua no modo `Senha`, sem mudança
  (`o_sem_a_senha_continua_como_era`).

**Recomendação, sem condição:** os casos `op:"sql"` com `perfil: None` desse
arquivo (literal sem fechar, literal solto, `VALUES 'SEGREDO123'`) agora
também saem limpos no arquivo. Passá-los a `Some(..)` trava o 365 na bateria
do 497 (a catraca desce).

### Pedidos propostos (fáceis C)

| # | Estado proposto | Achado | Teste adverso |
|---|---|---|---|
| **C0** | condição do 365, no mesmo lote | `TRUE`/`FALSE` (e `NULL`) crus no `sql` normalizado | `normalizado("INSERT INTO p (id, hiv) VALUES (7, TRUE)")` não contém `TRUE` |
| **C1** | ☐ baixa | O DbLink puxar cita a célula remota na recusa de coluna marcada | Par MySQL falso devolve `"999.888.777-66"` para uma coluna local `Date` marcada. O erro do `dblink_sincronizar` não contém `999` |
| **C2** | ☐ média | O DbLink empurrar cita o valor local INTEIRO e sem teto (`{n:?}` do `nome_seguro`) | Linha local com `Memo` marcado de 200 KiB e uma quebra de linha, sentido empurrar. O erro não contém o texto, e tem menos de 1 KiB |
| **C3** | ⏸ baixa | Recusa de avaliação de expressão cita número/booleano de coluna marcada de ORIGEM | Calculada não marcada `faixa Int1 = renda / 1000`, com `renda` marcada = 500000. O erro não contém `500` |
