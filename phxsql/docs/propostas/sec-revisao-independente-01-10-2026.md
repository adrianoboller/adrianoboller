# Revisão SEC independente — mudanças de 30/09 00:00 a 01/10/2026

Papel SEC, revisor adversário, não autor. Pedido de origem: parecer externo
(pedido 338) cobrou revisão independente; não há pedido próprio. Só leitura e
provas de leitura; nenhum código, `PENDENCIAS.md` ou dossiê alterado.

**Escopo lido:** `git log --since="2026-09-30 00:00" --first-parent` (79
commits), diffs contra o 1º pai dos que tocam `crates/`: DbLink 545, 546, 556/557,
578, 583/584, 590, 592, 470, 547; backup 552, 555, 568–570, 576, 577, 579, 593,
554; cluster/réplica 534/535, 580, 585, 587, 416/517/564, 344/359, 599; tela/HTTP
444/445, 284/436; `phxsql-core/src/prazo.rs`. Mais o que esses diffs alcançam
(portão, transação, `pg/`, `util.rs`).

**Provas vivas:** `target/debug/phxsqld` de 01/10 04:45 (árvore `5556d145`),
servidor com cadastro em `scratchpad/sec-revisao/p1`, `cifra_fio.exigir:false`
só para falar em claro — o caminho de autorização não depende da cifra. Pares
falsos MySQL e PostgreSQL em Python no mesmo rascunho (`mysql_falso.py`,
`pg_falso.py`). Roteiros: `prova_begin.py`, `prova_ligar.py`, `prova_pg.py`,
`prova_segredo.py`.

## Placar

| Sev. | ATIVO (entra na conta) | ⏸ (depois da versão) |
|---|---|---|
| Alto | 1 (S1) | 0 |
| Médio | 4 (S2, S3, S4, S5) | 1 (S9) |
| Baixo | 3 (S6, S7, S8) | 2 (S10, S11) |

Hipóteses que morreram estão no fim. Elas impedem a mesma suspeita de voltar.

---

## ATIVO

### S1 — ALTO — `begin` com `SCOPE` trava tabela exclusiva SEM login e SEM direito na tabela, com prazo escolhido pelo cliente

**Onde:** `servidor.rs:11742` (o portão do login só barra `da_operacao(op).is_some()`);
`usuarios.rs:100` + o teste do 445 `as_operacoes_anonimas_sao_estas_dezesseis`
(`begin`, `start_transaction`, `begin_transaction` estão entre as 16 anônimas);
`servidor.rs:12021` (portão 3 só confere tabela quando há atividade **e** usuário);
`servidor.rs:15927-15979` `declarar_escopo` toma `Trava::Exclusiva` por tabela
declarada sem nenhum `pode_em`; `servidor.rs:30369-30372` `duracao_ms` aceita
`timeout_ms` sem teto.

**O que a frente não viu:** o 445 contou as dezesseis anônimas e fez delas uma
única pergunta: «cabe nos 64 KiB do `TETO_DO_APERTO`?». A outra pergunta ficou
sem resposta: «alguma delas toma trava?». Seis tomam. É a pétrea do portão
único, um nível abaixo. O campo que o portão lê é `"tabela"`, e o `SCOPE`
nomeia tabela num campo (`scope`/`escopo`) que ele não olha.

**Cenário (medido, `prova_begin.py`):** quem tem só o token da porta, sem
login, manda
`{"op":"begin","database":"rh","scope":["salarios"],"lock_mode":"EXCLUSIVE","timeout_ms":1000000000000}`
e recebe `ok:true`, com `expira_em_s: 1000000000` (31 anos). A partir daí:
- o **supervisor** recebe `inserir rh.salarios`: `[SP000006] a tabela rh/salarios inteira esta travada (X) pela transacao … (ligacao 2)`,
  e isso dura enquanto a conexão anônima mandar um `ping` a cada `timeout_s`;
- o usuário `leitor`, com direito **só** na base `Z`, também passa direto pelo
  portão: chega ao conflito de trava, e nenhuma recusa de permissão aparece;
- **oráculo de catálogo sem login**: `scope:["nao_existe"]` responde
  `rh.nao_existe esta no SCOPE e nao existe`, e a tabela que existe responde
  `ok`. Isso enumera bases e tabelas.

**Conserto mínimo, pelo motor que já existe:** (a) `begin` **com escopo** exige
sessão autenticada. Basta levar `lista_do_escopo(p)` não vazia para o mesmo
`ainda_anonima` da linha 11742. Sem escopo nada muda, e a regra «guarda nova
entra pedida» fica preservada. (b) `declarar_escopo` confere
`u.pode_em(database, nome, Atividade::Ler)` em **cada** declarada **antes** de
abrir o database. É a conferência própria que `juntar`/`unir`/`pivotar` já
pagam pelo mesmo motivo, e a recusa não diz se a tabela existe. (c)
`duracao_ms` limita `timeout`/`timeout_ms` a `recursos.transacao_prazo_min`.
Pedir menos continua valendo; pedir mais vira o teto do `config.json`.

**Teste adverso:** servidor com cadastro → conexão sem login →
`begin`+`scope`+`EXCLUSIVE` tem de recusar com `erro.faca_login`. Usuário sem
direito em `rh` → recusa de permissão, nunca `SP000006`. `timeout_ms` acima do
teto → `expira_em_s` igual ao do `config.json`. E o comportamento velho:
`begin` sem escopo continua passando.

### S2 — MÉDIO — `DataRow` curta de um par PostgreSQL entra em pânico sob a trava global (`dblink_sincronizar`)

**Onde:** `pg/mod.rs:454-457` `ler_linha` usa o nº de campos **da própria
`DataRow`**, sem conferir com a `RowDescription`.
`dblink/sincronia.rs:404` indexa `&remota[de]`, com `de` vindo do mapa das
colunas da `RowDescription`. A chamada está em `servidor.rs:27051`, **depois**
de `travar_dados()` (27012). O MySQL lê por `colunas.len()` e o phx monta pelas
colunas, então só o PostgreSQL deixa a linha sair mais curta que o cabeçalho.

**O que a frente não viu:** o 544 fechou o irmão. A contagem negativa ou
gigante virava `capacity overflow` «no meio do `dblink_sincronizar`, com a
trava de dados na mão», e o comentário diz isso. A contagem **menor que o
cabeçalho** é a mesma pergunta («quantos campos?») e ficou sem resposta.

**Cenário (medido, `prova_pg.py` + `pg_falso.py`):** o par responde
`T`(id, nome) e depois `D` com **1** campo. `dblink_sincronizar` derruba a
conexão sem resposta, e o `srv.log` mostra
`thread 'dados-…' panicked at crates/phxsql-server/src/dblink/sincronia.rs:404:28 … panic_bounds_check`
seguido de `PHXSQL Reparo da trava de dados -- panico 1 com a trava na mao (pedido 451)`.
O reparo do 451 segurou (0 ms, o `inserir` seguinte respondeu). Mesmo assim o
par escolhe quando o servidor entra em pânico com a trava global na mão, e o
job repete isso a cada rodada.

**Conserto mínimo:** em `ler_linha` do PostgreSQL, recusar quando
`!colunas.is_empty() && n != colunas.len()` («DataRow com {n} campos, o
cabeçalho disse {m}»), no mesmo lugar do `quantos_campos`. Como segunda tranca,
`linha_remota_para_negocio` troca `remota[de]` por `remota.get(de)` e recusa.

**Teste adverso:** o par falso de `pg/mod.rs` (já existe em
`testes_do_teto_de_bytes`) manda `D` com 1 campo depois de `T` com 2:
`consultar` tem de devolver `Err`, e nenhum pânico pode acontecer.

### S3 — MÉDIO — `dblink_ligar` grava a cópia VELHA da ligação: ressuscita ligação excluída com a credencial e desfaz trocas feitas no meio

**Onde:** `servidor.rs:26838` `self.ligar(p)` clona a `Definicao` **antes**
da rede. `servidor.rs:26921-26922` faz `r.salvar(d)` com essa cópia **depois**
da rede. `dblink/mod.rs:1757-1766` `salvar` é *upsert* por nome e não confere
versão nem existência.

**O que a frente não viu:** o 545 tirou a trava de dados de cima da rede e
mediu o ganho. A janela entre a leitura do cadastro e a gravação dele sempre
existiu, porque a trava do cadastro nunca cobriu a rede. Com o prazo total do
578 (10 s × 60 de fábrica), essa janela chega a 10 min.

**Cenário (medido, `prova_ligar.py` + `mysql_falso.py` com 4 s de atraso):**
o admin A dispara `dblink_ligar erp`. Enquanto o par demora, o admin B faz
`dblink_excluir erp`, que responde `ok, restam 0`, e a lista fica `[]`. Quando
o `ligar` termina, a lista volta a ter `erp` com `"senha":"(oculta)"`, e
`dblink.json` traz de novo a senha antiga. Pelo mesmo caminho, uma troca de
senha, de host, de pino, de `cifra` ou de `somente_leitura:true` feita durante
o `ligar` é desfeita sem aviso. É revogação que não vale.

**Conserto mínimo:** no fim do `ligar`, sob `self.dblink.tomar`, reler
`r.achar(&d.nome)`. Se não existe, recusar. Se existe, aplicar **só** as
`sincronias` novas sobre a definição **atual**, com o mesmo
`com_as_sincronias_de` ao contrário, e não gravar a cópia inteira. É o padrão
«merge marca quem MEXEU» da casa.

**Teste adverso:** o par falso MySQL do 545 com gotejo; `dblink_excluir` no
meio. No fim `dblink` tem de listar zero ligações e o `dblink.json` não pode
conter a senha.

### S4 — MÉDIO — o teto de bytes do 546 não cobre o motor `phxsql`: a linha de 128 MiB é analisada inteira antes do `Acumulador`

**Onde:** `dblink/phx.rs:139-145` → `replica.rs:250-262`. `Canal::ler` lê até
`TETO_DO_REGISTRO` (128 MiB, `fio.rs:495`/`587`) e `Json::analisar` monta a
árvore **inteira** antes de `resultado_do_sql` → `Acumulador`.

**Aritmética (estimada, NÃO medida):** `size_of::<Json>() = 32 B`, medido com
`rustc` sobre a mesma `enum` (`tam.rs`). Cada `0,` (2 B) vira 32 B mais a sobra
do `Vec`: de 16× a 32×. Uma linha de 128 MiB vira **~2–4 GiB transitórios**.
Esse teto não cai com `max_mib: 1`, porque o `max_mib` só limita a cópia em
`Linha`. O comentário em `phx.rs:231` diz isso, mas a doc do 546 promete «num
motor só para os três clientes».

**Cenário:** o PhxSql remoto, hostil ou comprometido e cadastrado como DbLink,
responde ao `sql` com `{"ok":true,"resultado":{"linhas":[0,0,0,…]}}` de 128 MiB.
O mesmo `Cliente` atende réplica, pulso e console.

**Conserto mínimo:** dar ao `Cliente` o teto do Canal por ligação.
`Canal::ler_ate(leitor, d.teto_de_bytes())` já existe (`fio.rs:590`), e o
motor `phx` passa a usá-lo em vez de `ler`. Assim o teto vale **antes** da
análise, e não só da cópia. **Medir a premissa antes** é a primeira tarefa: um
teste com a linha de 64 MiB e `VmHWM` antes e depois.

### S5 — MÉDIO — 344: réplica SEM cofre passa a gravar a coluna externa marcada em claro no disco (antes guardava o selado)

**Onde:** `table.rs:7028-7031`. O externo que chega aberto (`selado=false`)
grava `bytes.clone()` sem conferir se a coluna é marcada nem se este `.reg` tem
cofre. `SEGURANCA.md:2170` declara o preço: «réplica sem cofre passa a guardar
o externo marcado em claro».

**Por que é ativo:** é o gap «coluna externa marcada sozinha vaza em claro»,
desta vez no disco da réplica. Antes do 344 a réplica gravava o selado: lixo,
mas não o dado. Depois grava o dado. O preço foi declarado pela frente, e
nenhuma decisão do dono foi citada. O 342 levou ao dono a metade «fio», e ela
virou guarda **imposta**. A metade «repouso na réplica» não foi.

**Conserto mínimo (saída conservadora):** em `aplicar_evento`, quando o
esquema tem coluna marcada (`Table::tem_dado_pessoal`, o mesmo motor do 342) e
`!self.reg.cifrada()`, recusar nomeando a tabela, com a saída escrita «ligue o
cofre nesta réplica». Não quebra nada que funcionava, porque a réplica sem
cofre gravava lixo antes. **Vai à mesa se o dono preferir aceitar o preço**,
e aí a decisão fica escrita.

**Teste adverso:** origem com cofre e coluna Memo marcada; réplica sem cofre;
`aplicar`. O `.reg`/`.mmo` da réplica não pode conter o texto (grep do
segredo), e a resposta tem de ser a recusa.

### S6 — BAIXO — `invalidar_manifesto_velho` apaga pelo NOME, o irmão que o 593 deixou

**Onde:** `backup.rs:929` → `1016-1023`, `destino.join(MANIFESTO)` +
`remove_file`. O manifesto novo nasce e sincroniza pela `Pasta`
(`no_destino`, 1207). O velho sai pelo caminho, **depois** de a âncora já
estar aberta (917).

**Cenário:** o destino fica numa pasta onde outros escrevem. Esse é o modelo
de ameaça que o próprio 569 escreve. Entre o `Pasta::abrir` (917) e o
`invalidar` (929), troca-se `destino` por um link para outro backup. O
`remove_file` segue o link intermediário e apaga o `backup.json` **do outro**,
e o `restaurar` passa a recusar aquele backup inteiro.

**Conserto mínimo:** `std::fs::remove_file(no_destino(copias, MANIFESTO))`,
com o `lstat` pela mesma âncora. É uma linha, e é o motor do 593.

### S7 — BAIXO — `conferir_destino` confere o NOME; a escrita segue o descritor aberto depois

**Onde:** `backup.rs:886` (confere grafia e `canonicalize`) e `backup.rs:917`
(`Pasta::abrir(destino)` segue link por desenho).

**Cenário:** com a pasta-mãe do destino gravável por outro usuário do SO,
trocar `destino` por link para `base/loja` entre 886 e 917 faz as cópias
`rh/salarios.reg` caírem em `base/loja/rh/`. Isso é o **schema `rh` dentro do
database `loja`**, visível para quem só tem direito em `loja`. Acesso cruzado
por corrida.

**Conserto mínimo:** depois do `Pasta::abrir`, conferir o descritor e não o
nome: `readlink /proc/self/fd/N` passa pelo mesmo `conferir_destino`, ou se
compara dev/inode com o da raiz e dos ancestrais dela. A `Pasta` já guarda o
fd.

### S8 — BAIXO — o cliente PostgreSQL aceita `AuthenticationOk` sem SCRAM, e a autenticação mútua prometida não vale

**Onde:** `pg/mod.rs:205`. `0 => Ok(true)` em qualquer momento, inclusive
como **primeira** resposta. O comentário do `conferir_servidor`
(`scram.rs:116-120`) diz que sem a conferência «qualquer um … poderia dizer
"pode entrar" sem conhecer a senha». O `R 0` direto faz exatamente isso.

**Medido:** o `pg_falso.py` nunca pede senha e manda `R 0`. O DbLink
conectou, sincronizou e gravou. Com `sentido: empurrar`, as linhas locais vão
para quem responder no endereço.

**Conserto mínimo:** ligação com senha configurada recusa `R 0` que não venha
**depois** de um SCRAM concluído com `conferir_servidor`. Isso é o
`require_auth=scram-sha-256` do libpq 16. O libpq de fábrica aceita, então o
choque com o padrão está dito: o que decide é a pétrea «saída mais
conservadora», e a recusa nomeia o `pg_hba.conf`.

---

## ⏸ (fora da conta, visíveis)

- **S9 — MÉDIO — o TOFU do pulso vive só na memória, e `exigir_prova_do_pulso`
  nasce `false`** (`cluster.rs:943-955`, `config.rs:599`). Depois de reiniciar
  o receptor, um pulso forjado SEM prova de um par que antes assinava volta a
  passar. O 436 só pôs o aviso do lado de quem deixa de assinar. Saída: gravar
  o `provaram` junto do estado do cluster (o 534 já leva esse estado ao disco).
- **S10 — BAIXO — SCRAM sem piso de iterações** (`scram.rs:174-182`). O 547 pôs
  teto e não pôs piso. Um intermediário manda `i=1` e sal próprio, recebe a
  `ClientProof` e faz dicionário offline a custo de 1 HMAC. A RFC 7677 pede
  ≥ 4096 do servidor, e o libpq aceita qualquer valor. Como a regra da casa é a
  convergência, fica ⏸. Some quando S8 e o TLS chegarem ao DbLink.
- **S11 — BAIXO — tempo de trava proporcional ao tamanho.**
  `dblink_sincronizar` faz `t.varrer()` da tabela local inteira sob a trava
  global (`servidor.rs:27055`), e o 545 tirou só a parte de rede. O backup lê
  cada arquivo inteiro em RAM (`backup.rs:932`). Tabela grande prende o
  servidor, ou estoura a memória, sem atacante nenhum.
- **Chave morta:** `direito_coluna::colunas_do_onde` (`direito_coluna.rs:567`)
  não tem chamador. Quem decide é `recusar_pergunta_sobre_coluna_negada`, que
  cobre `onde`, `ordenar`, `colunas`, `expressao`, `tendo`, `indice` e o
  filtro do índice parcial. Uma função pública que parece guarda e não guarda
  nada é a armadilha que esta casa já nomeou.

## Hipóteses que morreram (com a prova)

- **`desafio.segredo` (30/09, pedido 528) vazaria pelo `acessos.log`**, porque
  «segredo» não está em `nome_sigiloso` nem em `SEGREDOS`. Morreu na medição
  (`prova_segredo.py`): `config_gravar`, `diretiva_gravar` e `ALTER SERVER SET`
  recusam o campo («não se grava pela tela»), e `grep -rl abab… p1/` voltou
  vazio. Fica a lacuna da lista (palavra em português), sem caminho vivo.
- **Injeção pelo empurrão (556/583):** `Motor::texto` usa `_utf8mb4 X'…'` no
  MySQL e `E'…'` com `\\`/`''` no PostgreSQL, e o cliente fixa
  `client_encoding=UTF8` (`pg/mod.rs:159`). O nome passa por `nome_seguro` e
  por `citar`. Não achei fuga.
- **Prazo do core (`prazo.rs`):** `checked_add`/`saturating_*` em todo
  instante, e o `expect` só roda com `ate` presente, que implica `total`. Sem
  pânico alcançável.
- **284 pela porta Swagger:** `proxy_desta_porta_http("swagger")` cai no
  `rest`, e a porta Swagger só serve a página. Não despacha operação.
- **Oráculo de coluna negada (543/558):** os campos que perguntam estão
  cobertos (lista acima). As «vinte perguntas» não passaram por nenhum.
- **Recusa que cita dado (557):** `recusa_sem_valor` em todos os ramos, e a
  unicidade do `store` não cita a chave (`table.rs:4785`, `ndx.rs:1852`).

## O que NÃO foi feito

Não rodei a suíte, só os binários já compilados e os roteiros acima. Não li
534/535, 599 e 416/517/564 a fundo além do que tocava autenticação. Esses são
integridade, não SEC, e ficam para o DBA. S4 é aritmética, não medida.
