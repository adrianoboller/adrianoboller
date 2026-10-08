# E0: as provas que confirmam ou matam os furos lidos (709–713)

08/10/2026 · papel C (DBA sênior) · só leitura, sem `cargo` · nível: modelo forte (recuperação e
formato em disco). É a etapa E0 do `recuperacao-e-replica-desenho-unico.md` §5, escrita para o
engenheiro (B) codar **depois** que a divisão do `servidor.rs` em `servidor/servico_*.rs` for
comitada. Tudo aqui cita **função**, nunca linha.

**Nada aqui foi medido.** As funções e os ganchos citados foram lidos na árvore de trabalho de hoje.
Cada prova diz o que se vê se o furo for **real** e o que se vê se ele **não** for. As duas saídas
são resultado.

---

## 0. Regras comuns às cinco provas

1. **Onde moram.** Em `crates/phxsql-server/tests/`, com o `phxsqld` de `debug`
   (`env!("CARGO_BIN_EXE_phxsqld")`), pelo molde do `commit-inteiro-na-queda.rs`: o `subir(dir,
   vez, gancho)` escreve o `config.json`, e a porta sai do erro padrão
   (`comum::porta_do_phxsqld`). Os ganchos são `cfg(debug_assertions)`; o binário de `release`
   não os tem.
2. **SIGKILL.** `Filho(...).0.kill()` + `wait()`, que no Unix é `SIGKILL`. A página escrita por
   `write`/`pwrite` fica no cache do núcleo e **sobrevive** à morte do processo (pedido 186); é
   isso que deixa o «b» no disco no F1.
3. **Nenhum `sleep` com a trava na mão.** Quando o ponto da morte é **entre dois pedidos** (F1, F3,
   F8a), o estado já está parado: a resposta do pedido anterior chegou, e a trava está solta.
   Nesse caso **não precisa de gancho** e o teste mata de fora. O gancho só é necessário quando o
   ponto fica **dentro** de uma tomada da trava (F2, F8b, F9). As únicas esperas permitidas são
   sondagens com prazo de alguma condição, como a porta aberta, um texto no erro padrão ou a
   posição da réplica.
4. **Janela parada:** sem ela não há F1. Vai no `config.json` do processo que morre:
   `"recursos": {"durabilidade": "por_lote", "lote_operacoes": 1000000, "lote_milissegundos": 600000}`
   (o mesmo de `janela_parada` em `testes_do_panico_sob_a_trava.rs`). Com isso o
   `ligar_relogio_de_gravacao` dorme 10 min, o fecho por contagem não chega, e a marca do `COMMIT`
   fica em `marcas_pendentes` até o `SIGKILL`. Quem a apagaria é o `descarregar_sujas_com`, e
   ele não roda.
5. **Premissa conferida antes do `SIGKILL`.** Sem ela, a prova passaria por engano:
   - existe `dados/loja/transacao_*.tx` (a marca pendente);
   - `ler` devolve o valor da escrita posterior;
   - `diario` (`op_diario`, com `rowid`) dá o total `T0` e a versão `V0` do último evento da linha.

   Premissa que falha **mata a prova, não o furo**. Escreve-se que o caminho não se alcançou e
   por quê.
6. **Leitura final, pelas duas portas:**
   - pelo protocolo (`ler`/`varrer`) com o processo de pé, depois de a porta abrir, porque o
     arranque recupera antes de servir;
   - pelo disco com o processo parado: `Instancia::nova` → `abrir_database` → `abrir_qualificada`,
     depois `Table::ler(rowid)`, `Table::versao(rowid)`, `Table::eventos()` e
     `diario_com_imagem(0, total)` com `valores_da_imagem` (o molde é o `ids_por_venda` do 702).
     Por evento se olha `carimbo`, `operacao`, `rowid`, `versao`, `tx` e `origem`.
7. **Conferidor de duas faces (I2), um só.** Uma função nova em `tests/comum/mod.rs`,
   `duas_faces(t: &mut Table) -> Vec<String>`. Para cada rowid ela confere duas coisas e devolve
   a lista do que falha:
   - slot vivo ⇔ inclusão no diário sem exclusão posterior;
   - o conteúdo do slot é igual à imagem do último evento do rowid.

   F1, F2, F3 e F8 chamam a mesma função. Quatro cópias divergiriam, e a lei «do mesmo motor»
   proíbe isso.
8. **O que se faz com o resultado.**
   - Furo **real**: a prova é vermelha hoje. Ela entra com
     `#[ignore = "E0/<pedido>: vermelho medido em <data>; liga na E<n>"]` e o número vai ao
     `PENDENCIAS.md`. Teste vermelho sem `ignore` derrubaria o `portoes.sh`. Ela liga sem o
     `ignore` no commit do conserto, e a guarda vai ao `bancada/guardas/catalogo.py` com o furo
     que a motivou.
   - Furo **morto**: a prova é verde hoje. Ela entra **sem** `ignore` como guarda, e o pedido
     fecha como hipótese morta, com o número.
9. **Prova real, nos dois sentidos.** O vermelho de hoje já é o «defeito reposto». Depois do
   conserto, o B confere que a prova volta a cair com a linha do conserto revertida. Sem isso, o
   verde não vale.

---

## 1. F1 — pedido 709 (a mais grave): a marca do `COMMIT` reaplicada por cima de escrita solta

**Arquivo:** `crates/phxsql-server/tests/recuperacao-nao-volta-o-valor.rs` (F1, seus controles e o
F3, porque a montagem é a mesma).

**Montagem.** Database `loja`, tabela `clientes`:

- colunas `id Int8 obrigatoria` e `nome Texto`, com `pk_id` único primário;
- **sem chave estrangeira** e **sem `ao_alterar`**. Isso é de propósito: com FK, a solta vira
  cascata solta, ganha marca própria (`atualizar_com_a_marca`) e cai no controle C3, não no
  F1.

Há uma linha, `id=1`, `nome="0"`, gravada **solta** antes de tudo, para que o `COMMIT` só altere.

### 1.1 Roteiro (caso F1-alt, o principal)

| Passo | Ação | Conferência |
|---|---|---|
| 1 | `subir(dir, 1, None)` com a janela parada (§0.4); cria a tabela; insere `id=1,"0"` | `ok` |
| 2 | Uma ligação: `begin` → `atualizar rowid 1 {"id":1,"nome":"a"}` → `commit` | `transaction_state` = confirmada; existe **uma** `transacao_*.tx` (pendente: a passada terminou em `passada_sob_a_marca` e o caminho foi para `marcas_pendentes`). Guardar o nome do arquivo e o `tx` do COMMIT (do `diario`, ou do disco depois) |
| 3 | Outra ligação, **sem** transação: `atualizar rowid 1 {"id":1,"nome":"b"}` | `ok`; `ler` → `"b"`; a marca do passo 2 **continua lá**; `diario rowid=1` → `T0` eventos, o último é Alteração com versão `V0` |
| 4 | Anota `t_morte = agora_ms()`; `filho.0.kill()` + `wait()` | — (sem gancho: a trava está solta, §0.3) |
| 5 | `subir(dir, 2, None)` (sem gancho; a porta só abre depois de `transacao::recuperar` → `Database::recuperar_marcas` → `completar` → `aplicar_na_recuperacao`) | — |
| 6 | Pelo protocolo: `ler rowid 1`; `diario rowid=1` | ver §1.2 |
| 7 | `drop` do filho; pelo disco: `ler`, `versao`, cauda do diário de `clientes`, `duas_faces`; `dados/loja/transacao_*.tx` | ver §1.2 |

### 1.2 O que se vê

| | F1 **real** (o previsto pela leitura) | F1 **morto** |
|---|---|---|
| `ler rowid 1` | `"a"` | `"b"` |
| eventos da linha | `T0 + 1`: o último é Alteração com imagem `"a"`, `carimbo >= t_morte`, `origem = 0`, versão `V0 + 1` | `T0`, versão `V0` |
| `tx` do evento extra | registrar qual é: o do COMMIT (se `id_da_metade_que_entrou` o adotou, o diário passa a ter um `tx` que reaparece **depois** do `tx` da solta) ou um novo. Os dois são defeito; o primeiro também viola I4 | — |
| `duas_faces` | vazio (o diário e o slot concordam em `"a"`; o estrago é **semântico**, por isso a prova lê o valor e o carimbo, não só a coerência) | vazio |
| marca | saiu (`completar` deu `Aplicou` e apagou) | saiu |

**Critério de vermelho:** `ler == "a"` **e** existe evento com `carimbo >= t_morte`. As duas coisas
juntas separam o F1 de uma escrita perdida. Se só o valor desse `"a"`, sem evento novo, o «b» teria
morrido no `SIGKILL` (cache de usuário, família 498 H3), que é **outro** defeito. Quem desempata é
o controle C0.

### 1.3 Controles (no mesmo arquivo; todos têm de passar **hoje**, senão o F1 não se lê)

| | Roteiro | Esperado hoje | O que prova |
|---|---|---|---|
| **C0** | sem COMMIT: solta `"a"`, solta `"b"`, `SIGKILL`, reabre | `"b"`, `T0` igual | que o `SIGKILL` preserva a solta; sem o C0 verde, o F1 vermelho não é F1 |
| **C1** | COMMIT **insere** `id=2,"a"`; solta altera `id=2` para `"b"`; `SIGKILL`; reabre | `"b"`, `T0` igual | a inclusão está protegida (`slot_ja_consumido` → `JaEstava`); é o lado que a `TRANSACOES.md` §5.5 descreve |
| **C2** | igual ao F1-alt, mas a solta do passo 3 é **excluir_de_vez** `id=1` | registrar: `atualizar` sobre rowid inexistente → `NaoEncontrado` → a marca **fica** e o arranque a relata como impossível a cada subida | não é o F1 (o dado não volta), mas é relatório falso e marca eterna; vai ao F11/I12 se confirmar |
| **C3** | tabela **com** FK (`ao_alterar` cascata); a solta do passo 3 grava marca própria (`atualizar_com_a_marca`) | `"b"`: as duas marcas se completam por id, a do COMMIT primeiro | que o F1 só aparece quando a escrita posterior **não tem bilhete**; e é o controle do F12 enquanto o id tiver 13 dígitos |

### 1.4 As variantes da mesma reaplicação (mesmo arquivo, mesmo roteiro)

| | COMMIT | Solta posterior | Real | Morto |
|---|---|---|---|---|
| **F1-sup** | `excluir` suave `id=1` | `restaurar id=1` | a linha **some de novo**: `ExcluirSuave` é reaplicado por `excluir_suave_com_maes`, e o evento extra traz `carimbo >= t_morte` | a linha visível |
| **F1-rest** | `restaurar id=1` (excluída suave antes, solta) | `excluir` suave `id=1` | a linha **volta** | a linha oculta |

### 1.5 Variante do bidirecional (F1-bidi)

**Arquivo:** `crates/phxsql-server/tests/valor-velho-no-par-do-bidi.rs`. A montagem é a do
`venda-inteira-na-queda-do-bidi.rs`, com uma diferença: aqui o **par também puxa do central**.

**Montagem:**

- `caixa01`, dentro do processo do teste: `Servidor::novo`, `Papel::Multi`, `imagem_da_linha`,
  com `origens` → `central` em `127.0.0.1:P`.
- `central`, o filho `phxsqld`: `multi`, `id_servidor: central`, origem `caixa01`, janela parada
  e `"bind": "127.0.0.1:P"` **fixo**. A porta tem de ser a mesma nas duas vidas, porque a origem
  do `caixa01` é estática. O `P` sai de `comum::ouvinte_reservado()`, que é solto antes do
  `spawn`. Se o `bind` falhar porque um vizinho tomou a porta, o teste **recomeça com outra
  porta**; não aceita a porta errada em silêncio.
- A tabela é `clientes` com a chave primária, porque o bidi casa pela chave. Ela é criada no
  `caixa01` com `id=1,"0"`. Depois se sonda até o central mostrar `"0"`.

**Sub-variante (i), a solta no próprio central:**

1. No central: `begin` → `atualizar id=1 "a"` → `commit`. Premissa: `transacao_*.tx` existe.
2. No central, solta: `atualizar id=1 "b"`.
3. Sonda até o `caixa01` mostrar `"b"`: o par convergiu, e o «mais recente vence» do `caixa01`
   guardou o carimbo do «b». A premissa do passo 1 ainda vale, e se reconfere.
4. Anota `t_morte`, faz `SIGKILL` no central e o reabre (mesma porta, sem gancho).
5. Sonda até a posição do `caixa01` sobre o `central` alcançar `eventos()` de `clientes` no
   central depois do arranque. A posição sai do `replicacao_estado`, bloco `origens`, e o B
   confere o nome do campo. Só então se lê o valor nos dois lados.

**Sub-variante (ii), a do desenho único §3.6:** o «b» é escrito no **caixa01** e chega ao central
pelo grupo do bidi (`aplicar_grupo_bidi`). O resto é igual. **Premissa própria:** depois de o
grupo entrar, a `transacao_*.tx` do COMMIT ainda está no disco do central. Se a rodada do bidi
drenar a janela (`descarregar_sujas_com`, que apaga `marcas_pendentes`), a sub-variante (ii)
**morre por inalcançável**, e isso se escreve. A (i) continua valendo.

| | F1-bidi **real** | **morto** |
|---|---|---|
| central | `"a"`, com o evento de `carimbo >= t_morte` e `origem = 0` | `"b"` |
| caixa01 | `"a"`: o evento do arranque é o mais novo e **vence** | `"b"` |
| prova de disco (sempre se faz, mesmo se o passo 5 não convergir) | no diário do central há um evento local para `id=1` com `carimbo >` o carimbo do «b». É exatamente o que o par recebe | não há |

A prova de disco é **obrigatória**. A ponta a ponta (o `caixa01`) é o que mostra o estrago chegando
ao par. Se o passo 5 não convergir dentro do prazo, a prova **não** se dá por verde: sai vermelha
dizendo que a sonda não chegou.

---

## 2. F2 — pedido 710: inclusão do COMMIT com o slot gravado e sem o evento

**Gancho que tem de nascer:** `PHXSQL_TESTE_PARAR_NO_REG_DO_COMMIT=N`.

- Lido uma vez por `OnceLock`, como `parar_no_commit_de_teste`.
- Mora em `servidor/servico_marca_01.rs`, dentro de `aplicar_conjunto`, **antes** da N-ésima
  escrita e só quando ela é `Acao::Inserir`. Ele arma
  `phxsql_store::ndx::panico_de_teste::armar_gancho(Ponto::InserirDepoisDoContador, || sigkill_de_teste("teste: COMMIT parado entre o .reg e o diario"))`.
- O ponto já existe no `Table::inserir_com_maes_opt` e é **por thread**, a da passada: o slot e o
  contador estão no `.reg`, o `.ndx` não tem a chave, e não há evento.
- **Nome próprio, e não o `PHXSQL_TESTE_PARAR_NO_REG`.** Aquele já é lido pela réplica e pelo
  bidi, e num servidor `multi` os dois caminhos se armariam juntos.
- **Recomendação ao B (lei «do mesmo motor»):** a leitura das quatro variáveis `PHXSQL_TESTE_*`
  vira uma função só, `gancho_de_teste(nome) -> Option<u64>`. Hoje cada uma é um `std::env::var`
  repetido.

**Arquivo:** `commit-inteiro-na-queda.rs`, que já tem a venda de 7 escritas e o `subir` com gancho.

**Roteiro:**

1. Cria `vendas`, `itens` e `pagamentos`.
2. `subir(.., Some(3))` com a variável nova e `vender(porta, 1)` → a conexão cai. Espera o texto
   no erro padrão e faz `drop`.
3. `subir(.., None)` e `drop`.
4. Pelo disco: `duas_faces` em `itens`, e as inclusões por rowid.

| | F2 **real** | **morto** |
|---|---|---|
| `itens` rowid 2 (a 3.ª escrita) | slot vivo, **zero** eventos de inclusão; `eventos(itens)` = 4 com 5 slots vivos; `duas_faces` acusa o rowid 2 | um evento de inclusão para o rowid 2, com o `tx` da venda |
| controle no mesmo teste | `vendas` e `pagamentos` com uma inclusão cada; `itens` 4 e 5 completados com evento | idem |

---

## 3. F3 — pedido 711: completar uma marca de passada terminada acrescenta eventos

**Arquivo:** `recuperacao-nao-volta-o-valor.rs`. Não precisa de gancho, porque a morte é entre
pedidos.

**Roteiro:** é o do F1 **sem o passo 3**. Faz COMMIT de `id=1 "a"` com a janela parada, confere a
premissa (`transacao_*.tx`, `T0`, `V0`), `SIGKILL`, reabre e lê.

| Operação na marca | F3 **real** | **morto** |
|---|---|---|
| alteração (o caso principal) | `T0 + 1`, versão `V0 + 1`, evento com `carimbo >= t_morte` e o mesmo conteúdo | `T0`, `V0`: zero bytes e zero eventos (I9) |
| inclusão (controle) | `T0` (já protegida por `slot_ja_consumido`) | `T0` |
| exclusão suave / restaurar | registrar: o par `feito(bool)` devolve `JaEstava` se o estado já é o alvo? Medir, porque a leitura não decide | `T0` |

A consequência que o F3 carrega (réplica encadeada recebe o evento fantasma; semeadura com a marca
nasce com a posição deslocada) **não** precisa de prova própria na E0: o `T0 + 1` na origem basta.

---

## 4. F8 — pedido 712: a restauração não completa as marcas, e reconstrói o índice antes

**Arquivo:** `crates/phxsql-server/tests/marca-que-viaja-no-backup.rs`.

### F8a: a marca pendente viaja, e o arranque de destino a reaplica (o F1 pela cópia)

1. Sobe com a janela parada. Cria `clientes` com `id=1,"0"`. Faz COMMIT de `id=1 "a"`; premissa:
   `transacao_*.tx`.
2. `backup {"database":"loja","zip":true,"destino":...}`. O `op_backup` não drena a janela, então
   a marca vai junto. Premissa: o zip a contém, conferida por `phxsql_store::restaurar::conteudo`
   ou pelo `simular` do `restaurar_backup`. Se não contiver, o F8a morre por inalcançável.
3. `restaurar_backup {"origem": zip, "database": "loja2"}`.
4. **Conferência 1:** existe `dados/loja2/transacao_*.tx`?
5. Solta: `atualizar loja2 id=1 "c"`. `SIGKILL` e reabre.
6. **Conferência 2:** `loja2` `id=1`.

| | F8 **real** | **morto** |
|---|---|---|
| conferência 1 | a marca **está** no destino | não está (o palco a completou ou a conferiu antes do `rename`) |
| conferência 2 | `"a"`, com evento de `carimbo >=` instante da reabertura | `"c"` |

### F8b: a cópia fria de um servidor caído no meio da passada (I10, e a ordem do 522)

**Gancho:** já existe, `PHXSQL_TESTE_PARAR_NO_COMMIT=3`.

1. `vender(porta, 1)` → morre com `vendas` 1 e `itens` 2 no disco e a marca em voo. O diretório
   caído **não** se reabre.
2. Cópia fria **direto pelo store**, sem servidor:
   `phxsql_store::backup::executar_zip(dados, destino, "loja", "teste", agora)` +
   `finalizar_zip`. É o mesmo caminho da CLI `phxsql backup --zip`.
3. Num **segundo** servidor limpo, `restaurar_backup` do zip para `loja`.
4. Lê com o `retrato` em sanduíche do `venda-inteira-na-queda-do-bidi.rs` e procura `.tx` no
   destino.

| | F8 **real** | **morto** |
|---|---|---|
| retrato logo depois do `restaurar_backup` | `(1, 2, 0)`, a venda pela metade **visível**; o índice de `itens` reconstruído (`reconstruir_indices_marcados` no `Preparada::preparar`) só com 2 chaves | `(1, 5, 1)` |
| `.tx` no destino | presente | ausente |

---

## 5. F9 — pedido 713: erro de dado no meio do grupo da réplica apaga a marca com parte aplicada

**Gancho que tem de nascer:** `PHXSQL_TESTE_FALHAR_NO_EVENTO=<tabela>:<rowid>`.

- Mora em `servidor/servico_replicacao_01.rs`, `aplicar_grupo_da_replica`, ao lado do
  `parar_no_reg`.
- Quando o evento é dessa tabela e desse rowid, o gancho põe
  `falhou = Some(PhxError::Duplicado("teste: erro de dado injetado ..."))` (3002, classe do dado)
  **no lugar** de chamar `t.aplicar_evento`, e escreve o texto no erro padrão.
- **É permanente, de propósito.** Um erro de dado de verdade bate no mesmo evento em toda rodada.
  Um gancho «uma vez só» deixaria a rodada seguinte curar a venda, e a prova passaria por engano.
- **Por que injetado:** fora do gancho, toda escrita local na réplica anda o diário dela e cai
  antes, no rompimento de continuidade (`rompidas`), não neste braço.

**Arquivo:** `venda-inteira-na-queda-da-replica.rs`. Tem a origem dentro do processo e a réplica
como filho.

**Roteiro:**

1. A origem vende 1 venda: 7 eventos, um `tx`.
2. A réplica sobe com `FALHAR_NO_EVENTO=itens:2`; o evento falha na 3.ª posição do grupo.
3. Sonda o erro padrão até o texto injetado aparecer **duas vezes**. Isso prova duas rodadas, ou
   seja, que o estado é estável.
4. Lê o retrato e procura `transacao_*.tx` na réplica.
5. `SIGKILL`, reabre com a origem fechada (`comum::porta_fechada()`) e lê o retrato de novo.

| | F9 **real** | **morto** |
|---|---|---|
| retrato, nas duas leituras | `(1, 1, 0)` (venda e item 1 entraram, e item 2 falhou), igual antes e depois do arranque | `(0, 0, 0)` (pré-conferência recusou o grupo inteiro, E7) — ou nunca a metade |
| marca na réplica | **nenhuma** (o braço `falhou` a apagou) | — |

**Duas metades que a E0 tem de separar no registro:**

1. **A consequência** (a marca sai com a venda pela metade). É isto que o gancho prova.
2. **A alcançabilidade** (existe erro de dado real depois da conferência de continuidade?). O
   gancho **não** a prova. O candidato a medir é uma tabela `itens` pré-criada na réplica com um
   índice único a mais em `venda`, e a pergunta é se a réplica aceita a tabela pré-existente. Se
   ela recusar no esquema, a alcançabilidade morre e o F9 encolhe para «erro de E/S ou defeito do
   motor». O pedido continua na conta, porque o mesmo braço apaga a marca **com qualquer erro**,
   inclusive E/S, contra a I12. Isto é lido aqui e vai escrito no 713.

**Irmão no mesmo arquivo do bidi** (`venda-inteira-na-queda-do-bidi.rs`): o mesmo gancho no
`aplicar_itens_bidi` (`paradas`). O esperado é igual. O `ParouNoMeio` do `COMMIT` fica **fora**:
ele é cinto, e a pré-conferência do 448 o torna defeito do motor por decisão já escrita.

---

## 6. Resumo para o B

| Furo | Arquivo | Gancho | Nasce gancho? |
|---|---|---|---|
| F1 (+ C0–C3, sup, rest) | `tests/recuperacao-nao-volta-o-valor.rs` | nenhum (morte entre pedidos) | não |
| F1-bidi | `tests/valor-velho-no-par-do-bidi.rs` | nenhum; porta fixa do central | não |
| F2 | `tests/commit-inteiro-na-queda.rs` | `PHXSQL_TESTE_PARAR_NO_REG_DO_COMMIT` → `Ponto::InserirDepoisDoContador` | **sim** |
| F3 | `tests/recuperacao-nao-volta-o-valor.rs` | nenhum | não |
| F8a / F8b | `tests/marca-que-viaja-no-backup.rs` | nenhum / `PHXSQL_TESTE_PARAR_NO_COMMIT` | não |
| F9 | `tests/venda-inteira-na-queda-da-replica.rs` (+ irmão no bidi) | `PHXSQL_TESTE_FALHAR_NO_EVENTO` | **sim** |
| todos | `tests/comum/mod.rs` | `duas_faces(t)` (I2) | função nova |

Rodar: `cargo test -p phxsql-server --test <arquivo>` (o `debug` é obrigatório por causa dos
ganchos). **Formato:** a E0 não muda nada. Não há migração.

---

**Nota do integrador (08/10/2026), que prevalece sobre a «regra das provas» acima:**
prova vermelha NÃO entra com `#[ignore]`. Teste desligado é teste pulado, e a
casa não pula nem desativa teste. Prova que confirma o furo entra JUNTO com o
conserto, na mesma frente, e cai com o defeito reposto; prova que dá verde mata
a hipótese, vira guarda e fecha o pedido com o número. O número da prova
vermelha vai para a linha do pedido de qualquer jeito.
