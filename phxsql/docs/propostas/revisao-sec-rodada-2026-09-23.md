# Revisao SEC (adversaria) da rodada de 23/09/2026 — `8a1de60`..`82d62c4`

**Papel:** SEC, revisor adversario. **Nao sou o autor da mudanca.**
**Escopo:** os 14 commits de `252e572..82d62c4` (pedidos 402, 394+418, 405, 406,
407, 411, 422).
**Modo:** so leitura. Nao consertei, nao editei codigo, nao comitei.
**Limite da corrida:** disco em ~2,5 GB — **nao houve `cargo build` nem `cargo
test`**. Tudo abaixo e leitura de fonte, mais **uma** medicao de sistema de
arquivos (ALTO-1/MEDIO-1, item «mtime»). O que nao foi medido esta dito.

**Numeros de linha:** ancorados no **commit `82d62c4`**
(`git show 82d62c4:<arquivo>`), e nao na arvore de trabalho — ver «Arquivos que
se mexeram debaixo de mim», no fim.

---

## Veredito em uma linha

**Nao achei ALTO de exposicao de dado, de token nem de acesso cruzado.** Os
tres portoes que a rodada tocou (`consultar` com `por`/`agregados`/`tendo`,
`migrar_esquema`, as recusas de origem do 406) **passaram** na conferencia, e
o §«Conferido e sem achado» diz por onde passaram.

**Achei um ALTO de INTEGRIDADE**, e ele e do 422: a janela nova da FASE A deixa
um `COMMIT` sair pela metade, dizendo ao cliente um erro marcado
`repetir: true`.

---

## ALTO-1 — o `COMMIT` que congela no meio sai PELA METADE, manda repetir, e a recuperacao do caminho de erro e codigo MORTO

**Arquivos e linhas (@`82d62c4`)**

| o que | onde |
|---|---|
| a marca de commit vai ao disco ANTES da passada | `crates/phxsql-server/src/servidor.rs:15422` (`op_commit`), marca gravada em `15479` |
| a passada aborta no primeiro `?` | `servidor.rs:15550` (`aplicar_conjunto`), abertura da tabela em ~`15571` |
| a recuperacao do caminho de erro | `servidor.rs:15518` — `if let Ok(t) = self.travar_dados() {` |
| `drop(trava)` existe **so** no braco `Ok` | `servidor.rs:15506` |
| o portao 5 (travas de transacao) roda ANTES do congelamento | `servidor.rs:10388` (`barrado_por_travas`), e `servidor.rs:16375` (`op_migrar_esquema` so pede a trava global depois) |
| o congelamento | `servidor.rs:16455` e `16250`; `crates/phxsql-store/src/table.rs:1264` (`abrir_com`) e `:1278` (a consulta) |
| `EmMigracao` nasce com `adianta_repetir() == true` | `crates/phxsql-core/src/error.rs` (`adianta_repetir`) |

### O que um adversario (ou um azar) consegue

Nao e preciso malicia: bastam **duas conexoes e um administrador migrando**.

1. Conexao B: `BEGIN`. **Ainda sem trava nenhuma** — `travas` so nasce quando
   uma escrita e empilhada.
2. Conexao A: `migrar_esquema` com `confirmar`. O **portao 5**
   (`barrado_por_travas`) confere as travas de transacao **antes** de
   `travar_dados()`. Como B ainda nao empilhou nada, A passa. A **entra na
   fila** da trava global.
3. Conexao B **ganha a trava antes de A** e empilha uma escrita em `pedidos` e
   outra em `clientes`. Agora B tem trava de linha nas duas — **tarde demais**:
   A ja passou pelo portao 5, que so olhou o que existia no instante da
   conferencia.
4. A pega a trava, congela `clientes`, **solta a trava** e roda a FASE A —
   0,69-1,03 µs/slot, ou **2,76 s a 2 M de slots e ~13,8 s a 10 M**, pelos
   numeros do proprio parecer do papel C.
5. Conexao B: `COMMIT`.
   - a **marca vai ao disco e e sincronizada** (`op_commit`, antes da passada)
     — ponto de compromisso: para o banco, **a transacao esta confirmada**;
   - `aplicar_conjunto` grava `pedidos` **de verdade**;
   - abre `clientes` → `Table::abrir_com(escrever=true)` →
     `congelamento::conferir` → **`EmMigracao`** → `?` → a passada aborta.

**O estrago, em tres camadas:**

- **(a) Atomicidade quebrada.** `pedidos` tem a linha, `clientes` nao. Um
  leitor ve a transacao pela metade.
- **(b) O cliente e mandado REPETIR.** `EmMigracao.adianta_repetir()` e
  `true` (`error.rs`), e o erro sai com `repetir: true`. Cliente que obedece
  ao proprio protocolo refaz a transacao inteira → **a linha de `pedidos`
  entra duas vezes**. O carimbo `repetir` foi escrito para a recusa
  **antes** da escrita; aqui ele chega **depois** de metade dela.
- **(c) A rede de seguranca do caminho de erro nao existe.** O braco `Err`
  (`servidor.rs:15518`) chama `self.travar_dados()` **com o `trava` do topo
  ainda vivo** — `drop(trava)` so aparece no braco `Ok`, em `15506`. A guarda
  de reentrancia (`COM_A_TRAVA`, `servidor.rs:1631`) devolve
  `Err(trava_reentrante())`, e o `if let Ok(…)` **engole**. Ou seja:
  `crate::transacao::recuperar` **nunca roda ali**, apesar do comentario logo
  acima afirmar que «a tentativa acontece aqui com o MESMO codigo que a
  recuperacao do arranque usa». A marca orfa fica no disco e so e completada
  no **proximo arranque do servidor** — quando a transacao que o cliente viu
  FALHAR aparece aplicada.

> (c) e **anterior a esta rodada** — `op_commit` nao foi tocado no intervalo
> revisado. O que a rodada fez foi **tornar o braco alcancavel por operacao
> normal**: antes do 422 a migracao segurava a trava global do inicio ao fim,
> e nenhum `COMMIT` conseguia se intercalar com ela.

### O tamanho exato da janela — e aqui eu corrijo a mim mesmo

Escrevi primeiro que a janela era «o tempo que a migracao espera na fila da
trava». **Esta errado**, e o erro importa: quem chega a fila DEPOIS da migracao
espera por ela, e ai o `empilhar` de B ja bate no congelamento e recebe 4006
**antes** de empilhar — sem escrita empilhada, sem commit rasgado.

A janela de verdade e um TOCTOU: o portao 5 e conferido **fora** da trava
(`servidor.rs:10388`) e `op_migrar_esquema` so entra na fila depois
(`:16375`). O que precisa acontecer e **B ganhar a trava antes de A** —
inclusive um `empilhar` que ja estivesse em voo no instante da conferencia.
Em CPU e uma janela curta; num servidor com pedidos permanentemente na fila
ela nao e exotica.

**Nao medi a probabilidade** — medi que o caminho existe. E o roteiro do teste
abaixo nao depende dela: ele forca a ordem.

### Teste adverso que demonstraria (nos DOIS sentidos)

Por **soquete**, nao unitario — e o que depende do agendador e do disco se
prova contra eles:

1. tabela `clientes` com ~2 M de slots (FASE A medida em 2,76 s) e `pedidos`
   pequena;
2. conexao B: `BEGIN`;
3. conexao A: `migrar_esquema` com `confirmar`, em fio proprio;
4. B espera ~100 ms (a FASE A ja correndo), empilha `inserir` em `pedidos` e
   `inserir` em `clientes` — **os dois tem de ser aceitos**: se o segundo ja
   recusar com 4006, o roteiro nao provou nada e o passo 3 tem de comecar
   depois do passo 4 (e ai o portao 5 e que tem de barrar);
5. B: `COMMIT`.

**Asserções:**
- o erro de B traz `codigo: 4006` **e** `repetir: true`;
- **`pedidos` tem a linha e `clientes` nao** — e a prova da metade;
- sobra um `*.tx` no diretorio do database **depois** do erro (prova de (c):
  a recuperacao nao rodou);
- reiniciando o servidor, `clientes` **ganha** a linha — a transacao que
  falhou volta do tumulo.

**Com o defeito reposto / consertado:** a asserção «`pedidos` tem e `clientes`
nao» e a que troca de lado. Conferir o veredito do `COMMIT` **nao** basta —
esta casa ja pagou por uma prova que conferia o veredito depois do dano.

**Nao medido.** Nao rodei: sem `cargo`.

---

## MEDIO-1 — a revalidacao do retrato nao ve o volume que NASCE durante a FASE A

**Onde:** `crates/phxsql-store/src/reg.rs:2680` (`retratar`), `:2620`
(`conferir_retrato`), `:2671` (`retratar_um`).

`retratar` fotografa **os volumes que existiam** quando a FASE A comecou
(`primeiros`). Uma insercao que vire o volume — `tabela_002.reg` nascendo
durante a FASE A — **nao esta no retrato e nao esta em `trocas`**:
`conferir_retrato` passa, a FASE B renomeia os volumes 1..n para o esquema
novo, e o volume n+1 fica com a geometria VELHA.

**Consequencia, medida por leitura (nao por execucao):** `reg.rs`
(`conferir_volumes_uniformes`) pega o conjunto misto na proxima abertura e
**recusa** com `Corrompido`, nomeando o volume. Entao o resultado **nao** e
corrupcao silenciosa: e uma tabela que **para de abrir** ate alguem restaurar
o arquivo a mao, mais as linhas do volume novo fora da migracao. Fail-stop, o
lado certo — mas o cinto que o `conferir_retrato` promete («se um caminho novo
escapar do congelamento amanha, a FASE B recusa») **tem esse furo**, e ele e
justamente o caso mais destrutivo.

**So alcancavel se o congelamento for burlado** (ver MEDIO-2 e MEDIO-4) — e e
para isso que o cinto existe.

**Teste adverso:** tabela paginada, `congelamento::conferir` neutralizado a
mao (o proprio `porta-do-psch-v10.rs:688` ja faz isso para provar o contrario),
inserir durante a FASE A ate virar o volume; asserir que `conferir_retrato`
devolve `Err`. Hoje devolve `Ok`.

### Sobre «tamanho e `mtime` bastam?» — **MEDIDO**

Medi a premissa antes de acusar. 2.000 escritas **em lugar** (`pwrite`, tamanho
inalterado) num arquivo de 4 KiB, ext4, o mesmo sistema de arquivos do
repositorio (`/dev/vda ext4`), lendo `st_mtime_ns` antes e depois de cada uma:

- **`mtime` + tamanho inalterados: 0 de 2.000 (0,0%)**
- menor delta de `mtime` observado: **9.234 ns**

Ou seja: **no ext4 desta casa a escrita que «mantem tamanho e `mtime`» nao
existe** — a hipotese morreu medida, e isso e resultado tao valido quanto
achado. **Nao medido:** NTFS, exFAT/FAT (granularidade de 2 s), HFS+ e NFS
(1 s), onde o par tamanho+`mtime` pode ficar cego por um intervalo grande — e
o produto compila cruzado para Windows de proposito.

---

## MEDIO-2 — a chave do congelamento e comparacao de bytes de `PathBuf`: num sistema de arquivos que ignora caixa, um arquivo tem DUAS chaves

**Onde:** `crates/phxsql-store/src/congelamento.rs:90` (`BTreeMap<PathBuf,…>`),
`:134` (`congelar` monta `diretorio.join(nome)`), `:158` (`conferir` monta a
mesma coisa); `crates/phxsql-store/src/table.rs:1264` (`abrir_com`);
`crates/phxsql-store/src/catalogo.rs` (`validar_nome`).

`validar_nome` recusa `/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, caracteres de
controle, `.` e `..` — entao **«`/` no nome» esta fechado** e travessia de
caminho nao entra. O que ela **nao** faz e normalizar **caixa**. A chave e
comparada por `Ord` de `PathBuf` (componente a componente, byte a byte).

- **No Linux (medido pela leitura do codigo e pelo modelo do FS):** `clientes`
  e `Clientes` sao **arquivos diferentes**; duas chaves para dois arquivos, e
  nao ha furo.
- **No NTFS e no APFS/HFS+ (NAO medido):** sao o **mesmo** arquivo e **duas
  chaves**. `congelar(…, "clientes")`, e um `inserir` pedindo
  `"tabela":"Clientes"` abre o **mesmo `.reg`** com `QUANTAS>0`, `c.get` dando
  `None` → **escreve no volume vivo durante a FASE A**. Quem segura o estrago
  a partir dai e so o retrato (MEDIO-1).

**Teste adverso:** no Windows, congelar `clientes` e mandar
`{"op":"inserir","tabela":"Clientes",…}`; asserir 4006. Hoje, pela leitura,
grava.

**Nao medido:** nao ha Windows nem macOS nesta maquina.

---

## MEDIO-3 — o erro da revalidacao publica o CAMINHO ABSOLUTO do servidor ao cliente

**Onde:** `crates/phxsql-store/src/reg.rs:2629` —
`r.caminho.display()` dentro de `PhxError::Conflito`, devolvido cru por
`servidor.rs:16269` e `:16473` (`return Err(e)`), e o texto chega ao cliente
por `mensagens::decompor`.

O cliente recebe algo como
`/var/lib/phxsql/dados/vendas/clientes.reg mudou enquanto a reescrita montava…`.
**E a mesma classe que esta casa ja fechou uma vez, com o motivo escrito**, em
`crates/phxsql-store/src/catalogo.rs` (`tabela_que_nao_existe`): «*o erro cru
… publica o caminho absoluto do servidor a todo cliente que erra uma letra*».

**Alcance:** quem recebe precisa de `administrar` em **uma** tabela — e aprende
a raiz de dados do servidor, que o `config.json` nao expoe pelo protocolo.
Baixo impacto isolado; e reconhecimento para as etapas seguintes.

**Teste adverso:** provocar o `Conflito` (MEDIO-4 e o caminho mais barato:
`excluir_tabela` durante a FASE A) e asserir que a mensagem do protocolo **nao
contem** `config.base`. Hoje contem.

---

## MEDIO-4 — «o ponto por onde todos passam, sem excecao» nao e verdade: as operacoes de ARQUIVO nao passam pelo congelamento

**Onde:** o cabecalho de `crates/phxsql-store/src/congelamento.rs` (linhas
34-38) afirma que `Table::abrir_com` e «*o ponto por onde todos passam, sem
excecao*». Medido, nao e:

- `servidor.rs:16627` `op_excluir_tabela` → `:16638`
  `db.excluir_tabela(tabela)` — **apaga os 11 arquivos sem abrir a tabela**;
- `servidor.rs:16705` `op_renomear_tabela` → `:16723`
  `db.renomear_tabela(…)` — idem;
- `servidor.rs:19721` `op_restaurar_backup` — grava um database inteiro (nao
  conferi linha a linha; **nao medido**).

As tres pegam a trava global — que a FASE A **soltou**. Entao, com a migracao
em voo, `excluir_tabela` apaga o `.reg` que a FASE A esta lendo.

**Consequencia hoje, verificada:** quem pega e o **retrato** —
`retratar_um` de um arquivo que sumiu devolve `bytes: 0, modificado: None`,
diferente da foto, e a FASE B **aborta** com `Conflito` e `descartar()` limpa
os `*.novo`. Nao ha ressurreicao: conferi
`RegFile::trocas_por_terminar` — ela so termina troca quando a geometria do
`*.novo` bate com `(slot_size, data_offset, esquema_crc)` do volume 1 **e** a
tabela e paginada, entao um `*.novo` orfao nao volta por cima de uma tabela
recriada com o mesmo nome.

**O achado e a FRASE, nao o estrago de hoje.** A lei desta casa diz que lista
incompleta protege menos «*no dia em que alguem usar a lista como
inventario*» — e o cabecalho do modulo e exatamente um inventario, escrito hoje,
ja curto. Quem dividir `servidor.rs` amanha e ler aquela frase vai concluir que
o congelamento cobre a escrita toda.

**Teste adverso:** congelar e chamar `excluir_tabela` na mesma tabela; asserir
`EmMigracao` (4006). Hoje passa e o estrago so aparece na FASE B, como
`Conflito`.

---

## BAIXO-1 — o inventario do pedido 420 («10 de 141») ja nasceu curto: `migrar_esquema` e a decima-primeira

O pedido **420** (`PENDENCIAS.md:444`, recontado em `a671f2d`) lista as dez
operacoes que conferem `pode_em` sobre alvo que **nao** e o campo `"tabela"`:
`juntar`, `unir`, `pivotar`, `diferencas`, `copiar_tabela`, `duplicar_tabela`,
`renomear_tabela`, `dblink_ligar`, `dblink_sincronizar`, `posicao`.

**Medido:** `migrar_esquema_varredura` (`servidor.rs:16508`) confere
`pode_administrar_tabela` (`:16573`) sobre nome vindo de `db.todas_as_tabelas()`
— alvo que **nao** e o campo `"tabela"` — e **nao esta na lista**. Ela entrou
em `55ab903`, que e **anterior** ao `a671f2d` da recontagem. E ha pelo menos
mais cinco sitios do mesmo naipe, com `pode_ver_tabela` em laco:
`servidor.rs:11470`, `:11585`, `:11662`, `:11747`, `:15968`.

O **codigo esta certo** — a varredura paga a conferencia propria, antes de
contar (conferi: o `continue` vem antes dos contadores `pendentes`,
`no_formato_atual` e `total_a_reescrever`, entao o relatorio **nao** vaza nome
nem tamanho de tabela que o chamador nao administra). **Curto e o inventario**,
e ele envelheceu no mesmo dia em que foi medido.

---

## BAIXO-2 — a recusa 4006 conta a quem esbarrou o que OUTRO administrador esta fazendo

**Onde:** `congelamento.rs:164` — a recusa interpola o `motivo` de quem
congelou: `"acrescentando a coluna salario_novo"` (`servidor.rs:16253`) ou
`"migracao para o PSCH v10 (2 passada(s))"` (`servidor.rs:16458`).

Quem so tem `inserir` naquela tabela aprende o **nome da coluna** que um
administrador esta acrescentando — informacao de uma operacao que ele nao pode
chamar nem ler. E deliberado e util («*quem esbarra precisa saber O QUE segura
a tabela*»); registro para que a decisao fique escrita, e nao esquecida.

---

## BAIXO-3 — `conferidor_segredos::raiz()` pode sair do projeto

**Onde:** `crates/phxsql-server/src/conferidor_segredos.rs:181`.

`raiz()` sobe do `CARGO_MANIFEST_DIR` **ate achar `.git`**, sem teto. Num
ambiente em que o repositorio esteja aninhado em outro repositorio — ou em que
`$HOME` seja versionado —, a varredura anda **fora do projeto**, abre todo
arquivo de 64..4096 bytes (`:292`) e imprime os **caminhos** dos que casam o
crivo na saida do teste. Um `~/.ssh/id_rsa` apareceria pelo nome.

**A metade boa, conferida:** **nao vaza conteudo**. `Solto` (`:139`) so tem
`caminho`, `bytes` e `motivo`; `Motivo` (`:117`) e enum fechado com
`dizer()` de texto fixo; ligacao simbolica nao e seguida (`:271`); e
`o_relatorio_nao_vaza_um_unico_byte` (`:415`) trava o laco. **A frente disse
que nao ha campo de conteudo, e confere.**

E ele **nao e alcancavel pelo protocolo**: nao e operacao do catalogo, so roda
no `mod testes` e no `examples/segredos-soltos.rs`. Nao serve para inferir
arquivo do servidor por pedido de rede.

---

## BAIXO-4 — o conserto do 402 e de UM teste; o PADRAO continua relativo

**Onde:** `crates/phxsql-server/src/config.rs:1842` —
`arquivo: PathBuf::from("chave-do-fio.hex")`, resolvido por
`caminho_da_chave(config_em)`; com `Config` montado em memoria, `config_em` e
`None` e a chave cai no **cwd**. O conserto (`tests/laco-do-unico-secundario.rs`,
+10 linhas) poe `c.cifra_fio.arquivo = base.join(…)` **naquele teste**.
Qualquer outro `Config` em memoria que chegue ao aperto repete o pedido 402, e
a guarda — como o proprio modulo documenta — so ve na **corrida seguinte**.

**O que esta certo, conferido:** a chave nasce com `create_new` + `mode(0o600)`
(`config.rs:1993`, `abrir_privado`), permissao posta na **criacao** e nao
depois; e `chave_de_hex` **nunca** ecoa o conteudo no erro — so o `de_onde`
(nome do campo, da variavel ou do caminho). **Nenhum byte de chave aparece em
mensagem.**

---

## Conferido e SEM achado (diz-se com a mesma clareza)

**1. `por`, `agregados` e `tendo` nao nomeiam tabela — confirmado, nao aceito.**
`agregar_a_composicao` (`servidor.rs:12853`) le so nome de **coluna**;
`tendo_do_pedido` analisa por `phxsql_core::expressao::Expressao`, que nao tem
subconsulta nem `FROM` — o que `tendo` alcanca sao os nomes de `por` e os
apelidos dos agregados, e nome desconhecido **recusa nomeando**.
`tabelas_do_pedido` (`direito_coluna.rs`) desce por `de`, `juntar[].de`,
`escalar[].de`, `em[].de` e `existe[].de`, e nenhum campo novo carrega
sub-pedido. **O portao nao mudou porque nao ha campo novo para ele olhar.**

**2. O direito por COLUNA fecha sozinho sobre o agregado — confirmado.**
`linhas_do_sub_pedido` (`servidor.rs`) monta o `modelo` a partir das **chaves da
primeira linha devolvida**, ou seja **depois da peneira**; e para `varrer`/
`buscar` o declarado vem de `modelo_da_tabela`, que tira as
`colunas_sem_leitura`. Logo `por: ["salario"]` e
`agregados: [{coluna:"salario"}]` caem em `resolver_ou_recusar`. O caminho
obvio de contorno — usar `agrupar` como sub-pedido — esta fechado por
`direito_coluna.rs:149`: `agrupar` e `group_by` sao `PorColuna::Recusa`.
A trilha do «vinte perguntas»: `onde`, `ordenar`, `colunas`, `expressao`,
`tendo` e `indice` (inclusive o **filtro** do indice parcial) ja recusam em
`recusar_pergunta_sobre_coluna_negada` (`servidor.rs:7080`) para as operacoes
`Le`, que sao justamente as que o `consultar` chama por baixo.

**3. Visao que agrega (`planejar_sobre`) nao escala direito.** O plano de
dentro volta pelo `executar_derivado` **com a sessao de quem chamou** — a visao
nao carrega o direito de quem a criou. `SELECT COUNT(*) FROM v_c` de quem nao
le a tabela base para no mesmo lugar de sempre.

**4. `migrar_esquema` — o portao esta certo.** `Atividade::Administrar`
(`usuarios.rs:318`), dentro de `OPS_ESCRITA` (`servidor.rs:125`, logo tambem
coberto pelo portao 5 das travas e pelo `somente_leitura`), classificado
`PorColuna::Nenhum` com o motivo escrito (`direito_coluna.rs:234`), e a
**varredura** paga conferencia propria **antes** de contar. A vista previa de
UMA tabela passa pelo portao geral sobre o campo `tabela`. **Nao vaza tabela
que o chamador nao administra.**

**5. O 406 nao abriu porta.** As tres recusas so ligam com
`puxa_de_varias_origens()` (`config.rs`), e com `cluster` **nenhuma** liga —
mas `subir_replicacao` (`servidor.rs:2535`) **retorna cedo** com cluster, entao
nao existe laco lendo `replicacao.origens` ali; `replicacao_ligar`
(`servidor.rs:24279`) so marca `religar_pedido` num laco **que ja
existe**. Estreitar a guarda antiga nao abriu caminho de dado. A recusa de nome
repetido fecha um furo real: `"nome"` e opcional, o padrao e `"origem"`, e duas
omissoes desligavam a guarda dinamica.

**6. Congelamento e queda do processo: nao tranca para sempre.** O registro e
`static` em RAM (`congelamento.rs:90/95`) — morre com o processo, e nao ha
marca em disco para sobreviver. A posse solta no `Drop` (`:115`), inclusive
no caminho de `?`. O unico caso de trava permanente e **mutex envenenado**, e
o modulo escolhe esse lado de proposito, com o motivo escrito (`:122`).
Registro so para dizer que **foi conferido**: nao e achado.

**7. Nenhum segredo real foi impresso nesta revisao.**

### Um detalhe de ordem que passou, mas por motivo FRAGIL

`congelar` faz `c.insert(…)` e **so depois** `QUANTAS.fetch_add(1)`; `conferir`
sai cedo com `QUANTAS == 0` **sem pegar o mutex**. Isso seria uma janela — se
`congelar` nao acontecesse **com a trava global de dados na mao** e
`abrir_com(escrever=true)` nao acontecesse sempre sob a mesma trava. Hoje
funciona por causa da trava global, **nao** por causa da ordem dos dois
atomicos. Quem congelar de outro ponto amanha herda a corrida sem nenhum teste
acusando. **Nao medido** (corrida de poucas instrucoes; nao instrumentei).

---

## Arquivos que se mexeram debaixo de mim

Como pedido, digo qual e quando. Durante a revisao (janela ~19:40-19:58 de
23/09/2026):

- **`crates/phxsql-server/src/servidor.rs`** — no inicio da revisao era o
  **unico** arquivo modificado na arvore; as 19:58 estava **+615/-25** contra
  `82d62c4`, com hunks novos em ~`563`, `12608`, `12649`, `13327`, `21558` e um
  bloco de testes `mod testes_agregado_na_composicao` no fim. Os numeros de
  linha desta revisao **mudaram entre duas leituras minhas** — por isso todos
  foram reancorados em `git show 82d62c4:`.
- Tambem apareceram modificados no mesmo intervalo: `phxsql-core/src/fio.rs`,
  `phxsql-odbc/src/conexao.rs`, `phxsql-server/src/{consultar,diferencas,
  idiomas,juncao,lib,pivot,replica}.rs`.

**Nenhum achado acima foi lido na versao nao comitada** — todos saem do commit.

---

## O que eu NAO alcancei nesta corrida

- `op_restaurar_backup` (`servidor.rs:19721`) — nao li linha a linha; entra no
  MEDIO-4 como suspeita, nao como medida.
- O comportamento em NTFS/APFS (MEDIO-2) e a granularidade de `mtime` fora do
  ext4 (MEDIO-1) — **nao medidos**, sem a plataforma aqui.
- Nada foi executado: sem `cargo`, por causa do disco. **Todo achado acima e
  leitura, menos a medicao de `mtime`, que esta marcada como tal.**
