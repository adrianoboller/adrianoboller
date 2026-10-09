# Como este projeto se prova

**Um comando roda tudo:**

```bash
python3 phxsql/provar.py --construir
```

Ele não refaz bateria nenhuma — cada uma tem dono, já foi provada e continua
rodando sozinha pelo comando dela. O `provar.py` as chama, cronometra, soma, e
imprime **o que passou, o que falhou e o que foi pulado, com o motivo do
pulo**. O desenho inteiro está em [§7](#7-a-bateria-única-o-comando-que-roda-tudo).

Três famílias, e cada uma prova o que as outras não conseguem:

| bateria | onde | o que prova | como roda |
|---|---|---|---|
| **backend** | `cargo test --workspace` | o motor, o protocolo e os portões | `cargo test --workspace` |
| **frontend** | `testes-web/` | a tela, contra o servidor de verdade | `node testes-web/bateria.mjs` |
| **soquete** | `bancada/*/prova-*.py` | o que depende do sistema operacional | `python3 bancada/…` |

A terceira existe porque **teste unitário não prova queda de conexão**: os dez
testes do `BULKINSERT` passavam e a reserva não era solta quando o soquete
caía. A segunda existe pelo mesmo motivo, um andar acima — **teste de motor
não prova formulário**, e foi por isso que *todo salvar e todo incluir pela
tela* ficaram quebrados por uma versão inteira com 1.106 testes verdes.

E há uma quarta, que não prova o produto e sim **as outras três**:
`bancada/guardas/` repõe cada defeito que esta casa já pagou e confere que o
teste que o motivou ainda cai. [§8](#8-as-guardas-provar-que-a-prova-pega).


---

## 1. A cobertura de hoje, medida

<!-- testes:total:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->
`cargo test --workspace`: **4.102 testes, 0 falhas** — somado dos `test result:` de uma rodada de verdade, e não digitado: quem escreve este número é `docs/dossie/numeros-do-projeto.py`, e ele **aborta se a suíte falhar**.
<!-- testes:total:fim --> Por área,
contando `#[test]` por arquivo e agrupando:

<!-- cobertura:inicio -->
| área | testes | % |
|---|---:|---:|
| Motor de dados (arquivos, índice, diários) | 923 | 22,3 |
| Protocolo e portões (despachar) | 862 | 20,8 |
| Servidor (outros) | 640 | 15,5 |
| Núcleo (JSON, tipos, UUID, zip, paralelo) | 368 | 8,9 |
| Camada SQL (léxico, sintaxe, tradução) | 263 | 6,4 |
| Configuração | 176 | 4,3 |
| Criptografia e codificação | 147 | 3,6 |
| DbLink | 146 | 3,5 |
| ODBC | 84 | 2,0 |
| Telemetria e profiler | 81 | 2,0 |
| **Replicação** | **54** | **1,3** |
| **Gatilhos e procedimentos** | **48** | **1,2** |
| **Jobs** | **42** | **1,0** |
| **Usuários e permissões** | **39** | **0,9** |
| **Mensagens (i18n do servidor)** | **37** | **0,9** |
| **Interface web (servidor HTTP)** | **31** | **0,7** |
| **Segurança de rede (blacklist, firewall)** | **27** | **0,7** |
| **Cluster** | **27** | **0,7** |
| **Console de terminal (phxsqlcmd)** | **22** | **0,5** |
| **Alertas e e-mail** | **21** | **0,5** |
| **MCP** | **21** | **0,5** |
| **Transações** | **19** | **0,5** |
| **Junções e união** | **17** | **0,4** |
| **Exportação** | **13** | **0,3** |
| **Pivot** | **12** | **0,3** |
| **CLI** | **11** | **0,3** |
| **Monitor de máquina** | **6** | **0,1** |
| **total** | **4137** | |

Arquivos de `src` com mais de 120 linhas e **zero** `#[test]`:

| arquivo | linhas |
|---|---:|
| `phxsql-store/src/table.rs` | 10122 |
| `phxsql-store/src/ndx.rs` | 2873 |
| `phxsql-server/src/servidor/servico_transacao_01.rs` | 2586 |
| `phxsql-server/src/servidor/servico_escrita_01.rs` | 2176 |
| `phxsql-server/src/servidor/servico_bidirecional_01.rs` | 2106 |
| `phxsql-server/src/servidor/servico_marca_01.rs` | 2048 |
| `phxsql-server/src/servidor/servico_consulta_01.rs` | 1943 |
| `phxsql-server/src/servidor/servico_replicacao_01.rs` | 1859 |
| `phxsql-server/src/servidor/servico_web_01.rs` | 1678 |
| `phxsql-server/src/servidor/servico_backup_01.rs` | 1664 |
| `phxsql-server/src/servidor/servico_rede_01.rs` | 1650 |
| `phxsql-ffi/src/lib.rs` | 1640 |
| `phxsql-server/src/servidor/servico_cluster_01.rs` | 1599 |
| `phxsql-server/src/servidor/servico_sql_01.rs` | 1511 |
| `phxsql-server/src/servidor/servico_esquema_01.rs` | 1435 |
| `phxsql-server/src/servidor/servico_leitura_01.rs` | 1395 |
| `phxsql-server/src/servidor/servico_telemetria_01.rs` | 1379 |
| `phxsql-server/src/servidor/servico_nucleo_01.rs` | 1332 |
| `phxsql-server/src/servidor/servico_admin_01.rs` | 1242 |
| `phxsql-server/src/servidor/servico_composicao_01.rs` | 1178 |
| `phxsql-server/src/servidor/servico_permissao_01.rs` | 1152 |
| `phxsql-core/src/cadeia.rs` | 1128 |
| `phxsql-server/src/servidor/servico_replicacao_02.rs` | 1106 |
| `phxsql-server/src/servidor/servico_config_01.rs` | 973 |
| `phxsql-server/src/servidor/servico_avisos_01.rs` | 971 |
| `phxsql-server/src/servidor/servico_jobs_01.rs` | 953 |
| `phxsql-server/src/servidor/servico_diario_01.rs` | 918 |
| `phxsql-server/src/main.rs` | 838 |
| `phxsql-server/src/servidor/servico_quorum_01.rs` | 696 |
| `phxsql-server/src/servidor/servico_dblink_01.rs` | 562 |
| `phxsql-store/src/integridade.rs` | 336 |
| `phxsql-ffi/src/punho.rs` | 303 |
| `phxsql-ffi/src/valor.rs` | 290 |
| `phxsql-core/src/p384.rs` | 271 |
| `phxsql-server/src/carga.rs` | 260 |
| `phxsql-core/src/rsa.rs` | 256 |
| `phxsql-cmd/src/main.rs` | 208 |
| `phxzip/src/erro.rs` | 167 |
| `phxsql-odbc/src/registro.rs` | 149 |
| `phxzip/src/phz.rs` | 138 |
| `phxsql-odbc/src/tipos.rs` | 135 |
| `phxsql-server/src/sinais.rs` | 133 |
<!-- cobertura:fim -->

As duas tabelas acima **não se digitam**: `python3
docs/dossie/cobertura-por-area.py` as regrava daqui mesmo, entre as marcas
`cobertura:inicio` e `cobertura:fim`. O total que o dossiê mostra sai de
`numeros-do-projeto.py`, que conta o que o `cargo test --workspace`
**reporta** — os dois números são diferentes de propósito, e o script diz por
quê.

### Quem cobre os arquivos sem `#[test]` dentro

«Sem `#[test]` dentro» não quer dizer «sem teste». A coluna abaixo é
julgamento, e por isso é escrita à mão — a lista dos arquivos, não:

| arquivo | quem cobre hoje |
|---|---|
| `store/src/table.rs` | os 11 arquivos de `store/tests/` — coberto por fora |
| `store/src/ndx.rs` | `store/tests/ndx.rs`, 27 testes — coberto por fora |
| `cli/src/main.rs` | **nada** |
| `server/src/main.rs` | `tests/mcp_stdio.rs` roda o binário; o resto, nada |
| **`server/src/replica.rs`** | **nada no `cargo test`** — só `bancada/replicacao/` |
| `server/src/dblink/conexao.rs` | `tests/dblink-postgres-no-fio.rs`, pelo fio |
| `server/src/carga.rs` | os testes de `BULKINSERT` em `servidor.rs` |
| `cmd/src/main.rs` | `cmd/tests/console.rs`, pelo soquete |

O buraco de verdade é o `replica.rs`: **1,0% dos testes** cobrem o pedido que
o dono chamou de «replicação como a do MySQL(R)», e o laço que a faz andar
não tem nenhum. A prova dele é o `bancada/replicacao/`, que **não roda no
`cargo test`** e precisa de quatro servidores no ar.

---

## 2. A bateria de frontend

Como rodar, os casos e o que ela deliberadamente não faz:
`testes-web/LEIA-ME.md`. **O número de casos não fica escrito aqui de
propósito** — ele dizia «treze» com dezenove no diretório, e número digitado à
mão envelhece calado. Quem quiser a conta que ela vale: `ls testes-web/casos/`,
ou o rodapé da própria bateria, que diz quantas execuções passaram.

O resumo do desenho:

- **Sobe o próprio servidor**, nas portas 6200/6201, num diretório temporário,
  e o derruba **pelo PID**. A senha não fica em claro em lugar nenhum: o hash
  sai do próprio `phxsqld --senha`, como no `bancada/replicacao/montar.py`.
- **Entra pela tela de login**, com o desafio-resposta de verdade. Se a página
  cair em modo demonstração, o caso falha — sem essa guarda a bateria inteira
  passaria sem tocar no motor.
- **Percorre todas as telas** dos menus e da barra, clicando item por item, e
  reprova em qualquer erro. Quantas foram sai como **nota do próprio caso** a
  cada rodada (115 na última) em vez de ficar digitado aqui. Esse laço sozinho vale mais que dez
  asserções bonitas: foi ele que pegou um `` ` `` a mais dentro de um template
  literal em três segundos, com a página inteira morta.
- **Um contexto de navegador por caso.** A página guarda tema, largura e
  estado da lateral no `localStorage`; com contexto compartilhado, o caso que
  recolhe a lateral fazia o próximo começar com a árvore invisível — e a falha
  aparecia no caso errado.
- **Espera a entrada TERMINAR, e não começar.** O `entrar()` aguarda
  `#app[data-pronto="1"]`, a marca que o `abrirApp()` põe quando a árvore está
  montada, a primeira tela pintada e as abas pinadas de volta. Esperar só por
  `#arvore .no` era esperar pelo meio da entrada, e deixava 32 ms de corrida
  entre a bateria e a página — os 32 ms que faziam o caso `telemetria`
  reprovar em 10% a 36% das rodadas. Ver §11.

### Os três canais de erro, e por que não basta o `pageerror`

O `ligarMenu` faz `Promise.resolve().then(entrada.faz).catch(e => avisar(e, true))`.
Uma tela que estoura no meio vira **recado vermelho**, não exceção — e uma
bateria que só escutasse `pageerror` passaria verde por cima dela. Por isso o
passeio olha `pageerror`, `#aviso.mal` **e** `#painel .aviso.mal`, e limpa os
dois últimos antes de cada clique.

---

## 3. O que esta rodada achou

Cada item traz o **defeito reposto** que prova o teste — a bateria falha com
ele e passa sem ele. Prova nos dois sentidos, como manda a casa.

### 3.1 A tela de LGPD nunca auditou nada

`ui/index.html`, `telaDadosPessoais`. A tela procurava um campo booleano
`pessoal` por coluna. O servidor nunca mandou esse campo: o `esquema` responde
**`dado_pessoal`**, em texto (`"nao"` / `"pessoal"` / `"sensivel"`), e existe
uma op própria — `dados_pessoais` — feita exatamente para essa varredura.

O efeito era o pior possível para o assunto: a tela dizia, para toda base,
«o esquema deste servidor ainda não traz a marca» — **«não sei» sobre um motor
que sabe**, numa tela de conformidade. E dizia isso em vermelho, o que a fazia
parecer uma limitação conhecida em vez de um defeito.

Nenhum dos 1.106 testes podia pegar: o servidor estava certo dos **dois**
lados, e quem lia errado era a página.

A tela passou a chamar a op. De quebra, a op filtra tabela a tabela pelo
direito de quem pergunta — o laço da tela refazia essa conferência por fora,
que é onde ela um dia deixa de existir.

- **Trava:** `testes-web/casos/11-lgpd.mjs`.
- **Defeito reposto:** a tela lista 0 colunas marcadas de 2 → o caso falha.

### 3.2 O pivot era a porta dos fundos para a tabela negada

`servidor.rs`, `op_pivotar`. O portão de permissão confere o campo `"tabela"`
do pedido — e o pivot tem **dois** lugares com tabela: a de fatos em `tabela`,
e a lista `juntar`, com um `"tabela"` **dentro de cada item**. O portão não
desce até ali, e a função começava com `let _ = sessao;`.

Bastava juntar a tabela negada e pedir um campo dela em `linhas`: os rótulos
das linhas do cruzamento **são** os valores dela. Medido: o usuário sem
direito sobre `folha` recebia `rotulos_linha: ["x"]` e, no rodapé,
`juncoes: [{tabela: "folha", linhas: 1}]` — o dado, o nome e a contagem.

É a **terceira** operação da mesma família. O `juntar` (`a.tabela`/`b.tabela`)
e o `unir` (uma lista) já tinham conferência própria; o `pivotar` foi
esquecido, porque nele o campo tem o nome certo — só que aninhado.

**A lição que isto acrescenta à regra da casa:** quando o portão passar a
olhar um campo novo, procure quem não tem esse campo — **e quem o tem
aninhado**, que é o disfarce mais fácil de não ver.

- **Trava:** `pivotar_nao_e_a_porta_dos_fundos` e
  `pivotar_na_tabela_permitida_continua_valendo`.

### 3.3 Mais duas portas para a lista que a árvore esconde

Da mesma varredura saíram outras duas operações que percorrem a base inteira
**sem campo `tabela`**:

- **`sequencias`** devolvia nome, coluna de sequência, contador e quantidade
  de registros de **toda** tabela — inclusive a negada. É a terceira porta
  para a lista que a árvore esconde; o `sistabelas` e o `siscolunas` já
  filtravam.
- **`posicao`** (o `SHOW MASTER STATUS` daqui) devolvia nome, eventos,
  registros, a chave única e — com `com_esquema: true` — o **esquema cru** de
  cada tabela. A conferência aqui é de `replicar`, e não de `ler`: é o direito
  que o portão aplicou à operação.

- **Travam:** `sequencias_esconde_a_tabela_negada`,
  `posicao_esconde_a_tabela_negada` e, o que mais importa,
  `sem_regra_de_tabela_posicao_e_sequencias_veem_tudo` — a réplica de sempre
  não tem regra por tabela e continua vendo tudo. Guarda nova que quebra quem
  já funcionava não é guarda, é estrago.

### 3.4 `duplicar_tabela` não conferia o destino

`servidor.rs`. O portão confere `criar` contra o campo `tabela`, que ali é a
**origem** — e a tabela que nasce tem o nome do campo `destino`. Quem podia
criar nominalmente uma tabela criava qualquer outra, duplicando a permitida.

O `copiar_tabela` ao lado já fazia essa conferência no destino dele; a
diferença era só que aqui o destino mora no mesmo database e por isso parecia
coberto.

- **Trava:** `duplicar_confere_o_direito_no_destino` e
  `duplicar_com_direito_no_destino_continua_valendo`.

### 3.5 Um teste de credencial para todas, e não um por campo

`config.rs` tinha três testes de vazamento — a senha da cifra, a do relé e a
do cluster — e nenhum deles pegaria **o campo que alguém acrescentar amanhã**.

Entrou `nenhuma_credencial_do_config_sai_pela_op_config`: dez marcas
distintas, uma em cada campo que carrega segredo, e a asserção é sobre o JSON
inteiro. Ele também confere que o caminho de **leitura** continua funcionando
— um `para_json` que esconde tudo porque não leu nada passaria sem valer.

- **Defeito reposto:** `("token", texto_de(&self.token))` no lugar de
  `"(oculto)"` → «vazou pela op `config`: token do servidor».

### 3.6 A tela de entrada em branco por 12,7 s

`ui/index.html`, cabeçalho. A folha da fonte da marca vinha do Google como
`<link rel="stylesheet">` **bloqueante**: o parser para até a resposta chegar,
e com ele o primeiro `<script>` e o `DOMContentLoaded`.

A resposta que nunca chega é o caso **normal** de um servidor de banco: rede
que **descarta** o pacote (firewall com DROP, proxy que só responde reset
depois do prazo) em vez de recusar na hora.

Medido, três rodadas de cada lado:

| rede | DOMContentLoaded |
|---|---:|
| pedido **engolido** (o caso do firewall) | 12.778 / 12.743 / 12.677 ms |
| recusa **imediata** | 124 / 115 / 101 ms |
| **depois do conserto** | 125 / 103 / 101 ms |

**12,7 s de tela branca contra 0,11 s — 116×.** O comentário do `http.rs`
dizia «servidor sem internet: a fonte não carrega, a pilha de reserva assume e
a página continua inteira». Verdade e incompleto: não dizia em quanto tempo.

`media="print"` + `onload="this.media='all'"` faz o navegador buscar sem
bloquear a pintura. O `noscript` devolve o caminho antigo para quem desligou o
script — ali a página não funciona mesmo, e a fonte bonita não custa nada.

- **Trava:** `testes-web/casos/10-primeira-pintura.mjs`, que pendura o pedido
  num buraco negro e exige a tela de entrada em menos de 3 s. Ele também exige
  que a fonte **continue sendo pedida**: um conserto que a removesse passaria
  no tempo e reprovaria a marca, que manda.

### 3.7 O CSS global, de novo — três mordidas

O projeto já tinha três remendos pontuais contra o `input{width:100%}`
(`.form-dbl .linha-chk`, `.un-item`, `table.conf .esc`), e **cada um deles só
nasceu depois de alguém abrir a tela e olhar**. A tela «Nova tabela» tinha a
quarta:

| controle | medido |
|---|---|
| checkbox `obrig.` do cadastro de campos | 57 × 13 px |
| checkbox `único` dos índices | 114 × 13 px |
| radio `primária` dos índices | 161 × 13 px |

Entrou uma regra para **toda célula** — `td/th input[type=checkbox|radio]` —
em vez do quinto remendo, para o próximo componente nascer certo.

A segunda mordida foi a caixa de marcar **separada do próprio texto**:
`.criar .chk` trocava o `display` para `flex` mas não o `flex-direction`, e
`.criar label{flex-direction:column}`, vinte linhas abaixo, vencia. «exigir
motivo escrito» e «tabela particionada» apareciam com a caixinha **em cima**
do texto, os dois jogados na borda direita. No código as duas regras estão
perto e cada uma está certa sozinha.

A terceira foi o `label{text-transform:uppercase}` por outro caminho: o
`.pino` da tela de LGPD mostrava o índice `porNome` como **PORNOME** — nome de
índice é dado, e mostrar dado numa caixa que ele não tem é a mesma mentira do
«BLUMENAU».

- **Trava:** `testes-web/casos/06-css-global.mjs`, com três medições —
  controle esticado, texto **misto** que sai em caixa alta dentro de tabela, e
  controle geometricamente separado do texto dele. A regra do misto (maiúscula
  **e** minúscula no texto de origem) é o que separa estilo de mentira: um
  rótulo escrito para ser lido em caixa alta não tem maiúscula no HTML; um
  nome de cidade, de coluna ou de índice tem.
- **Defeitos repostos:** os três, um a um, com a bateria falhando em cada.

### 3.8 O único contraste reprovado, achado varrendo

A varredura de contraste mede **todo elemento pintado com texto em cima**, em
sete telas e nos dois temas — 45 elementos por tema. Achou um:

O chip «ativas» da grade trazia `color:#10060a` fixo sobre `var(--laranja)`.
No tema claro o laranja escurece para `#c63c0a` (adaptação da marca, por
contraste), e tinta quase preta em cima dele dá **3,85:1** — abaixo dos 4,5:1.
É a mesma armadilha que o comentário das cores da ação já descrevia («fundo
laranja com texto escuro em cima ficava ilegível»), sobrevivendo no único
lugar que não usava o token `--tinta-botao`.

**E o número mostra por que se mede:** a conta de cabeça, feita antes, deu
2,65:1. O navegador disse 3,85:1. As duas reprovam, mas a errada estava
errada — e no dia em que a diferença decidir, ela decide errado.

Os números de contraste que o CSS traz escritos nos comentários **conferem**,
e agora são recalculados a cada rodada:

| par | escuro | claro |
|---|---:|---:|
| `--texto` / `--painel` | 14,47:1 | 18,45:1 |
| `--texto-2` / `--painel` | 8,63:1 | 10,18:1 |
| `--texto-3` / `--painel-2` | 5,30:1 | 5,45:1 |
| `--texto-3` / `--realce` | 4,78:1 | 4,94:1 |

---

## 4. As hipóteses que morreram medidas

Resultado válido, e é o que impede a mesma ideia de voltar sem medição.

**«O mutex serializa» — não, o parse é que custa.** Já estava no `CLAUDE.md`;
esta rodada não mexeu nele. Fica citado porque a varredura de contraste
repetiu a lição em miniatura: a conta plausível deu 2,65:1 e a medida deu
3,85:1.

**«O `pageerror` basta para provar a interface» — não basta.** Todos os
achados de tela desta rodada — a LGPD, as três mordidas do CSS, o contraste,
a tela branca — aconteceram **sem uma única exceção não capturada**. O
`ligarMenu` manda toda falha de item de menu para `avisar(..., true)`, e uma
tela que estoura no meio vira recado vermelho, não `pageerror`. O canal de
erro que uma interface usa **é escolha dela**, e um observador que só escuta o
canal do runtime observa metade. O `pageerror` pegou exatamente um defeito
nesta rodada, e foi um meu: um `` ` `` a mais dentro de um template literal.

**«O aviso vermelho no painel é sempre defeito» — não é.** A primeira versão
do passeio reprovou sete telas legítimas: «uma junção precisa de duas
tabelas», «escolha uma tabela primeiro». A resposta certa **não** foi
ensinar a bateria a ignorar recusa — foi montar o cenário certo (duas tabelas)
e refazer o que a pessoa faria (escolher a tabela de novo na árvore). Bateria
que aprende a ignorar recusa deixa de ver a recusa que importa.

**«Botão com fundo cheio é defeito» — nem sempre: pode ser o mouse.** A
primeira varredura de cores acusou o «Atualizar» da tela de Serviço de estar
preenchido em repouso. Estava preenchido porque o **ponteiro tinha ficado em
cima dele** depois do clique anterior, e `:hover` preenche. Uma falsa acusação
custou uma linha (`page.mouse.move(4,4)`) e uma lição: medida de estilo mede o
estado, e o estado inclui onde está o mouse.

**«A árvore some no tablet» — some, e a causa é o celular.** As primeiras
capturas de tablet e desktop saíram sem a árvore. A causa não é a largura: em
390px a lateral vira gaveta e se fecha sozinha depois de cada escolha, e esse
fechamento é **gravado no navegador**. A bateria media do menor para o maior,
e o celular contaminava os dois seguintes. Passou a medir do maior para o
menor. O comportamento em si está anotado abaixo — não é defeito óbvio.

---

## 5. Anotado, e não consertado

O que é grande demais para esta frente, ou é decisão do dono.

### 5.1 `duplicar_tabela` e `copiar_tabela` não conferem `ler` na origem

As duas são `Atividade::Criar`. O portão confere `criar` contra a tabela de
origem, e o destino agora também é conferido — mas **nenhuma das duas confere
`ler` na origem**, e copiar uma tabela é ler os bytes dela.

O caminho: com um cadastro do tipo
`{"*":{"criar":true,"tabelas":{"folha":{"criar":true}}}}` — «pode criar, não
pode ler a folha» —, `duplicar_tabela` origem=`folha` destino=`copia` cria uma
tabela legível com o conteúdo da negada.

**Por que não consertei:** exigir `ler` na origem muda o significado de um
`config.json` que já existe. Quem tem `criar` por **nível** também tem `ler`
(os níveis são cumulativos), então na prática quase ninguém é afetado — mas
«quase» não é o critério desta casa, e a regra é clara: guarda nova entra
pedida, não imposta. O conserto é uma linha em cada operação; falta a decisão.

### 5.2 `replica.rs` sem nenhum teste no `cargo test`

352 linhas, o laço que faz a réplica alcançar o master, e **zero** `#[test]`.
A prova hoje é `bancada/replicacao/`, que precisa de quatro servidores no ar e
não roda no portão de commit.

O que dá para cobrir sem os quatro servidores: a decisão de **quando** puxar
(streaming, `cada_minutos`, `hora`), o cálculo do atraso, e a espera crescente
quando a origem não responde. O que **não** dá, e continua sendo prova de
soquete: a origem que cai no meio de um lote.

### 5.3 A gaveta fechada no celular não reabre ao voltar para o desktop

Reprodução: abra em 1600px (árvore visível), reduza para 390px, clique numa
tabela, volte para 1600px. A árvore fica recolhida.

A causa é `fecharSeSolta()` → `alternarLateral(false)` → `guardarLateral()`: o
fechamento **automático** da gaveta é gravado como se fosse escolha da pessoa.

**Por que não consertei:** os dois lados são ruins. Não gravar faz o celular
abrir a gaveta por cima do conteúdo, com véu, a cada carregamento — que é pior
onde a tela é pequena. O caminho provável é distinguir «fechei porque escolhi
algo» de «fechei porque você mandou», e isso é desenho, não conserto.

### 5.4 A grade da aba Conteúdo mostra as colunas de sistema; a editável, não

Decisão, não defeito: a aba Conteúdo mostra a linha como ela está no `.reg`, e
a grade editável esconde `softdeleted` e `rownum` porque ali quem manda neles
é o botão de excluir e o de restaurar. O caso `grade` trava as **duas**: se um
dia alguém uniformizar, um dos lados falha e a conversa acontece antes do
commit, e não depois do relato.

### 5.5 O `phxsql-cli` (845 linhas) sem nenhum teste

O `phxsqlcmd` tem 18; o `phxsql` da linha de comando, nenhum. Fica anotado
com o tamanho: é a maior superfície sem cobertura depois do `replica.rs`.

### 5.6 ~~O `abrirAdmin` escreve na tela depois do `await`~~ — FECHADO na SP000056

**Consertado e provado nos dois sentidos.** Ver §11.

Fica aqui a parte que ensina, e não o registro: entre este item ser anotado e
ser fechado, **uma guarda entrou** — o contador `admGeracao` — com um
comentário de vinte linhas descrevendo exatamente este defeito e admitindo que
**não tinha prova real**. Ela não fechou o item, e ninguém percebeu, porque ela
cobria `abrirAdmin` contra `abrirAdmin` e a vítima do §9.8 (Configurações)
pinta por `folha()`. *Guarda sem prova real não é guarda, é intenção* — e o
aviso estava escrito no próprio comentário.

---

## 6. O que a bateria de frontend NÃO cobre, de propósito

- **A internet.** A fonte da marca é recusada na origem; deixá-la sair traria
  a rede de quem roda para dentro do resultado.
- **Desempenho.** Isso é a `bancada/`. A única medida de tempo aqui é o teto
  de 3 s da primeira pintura, e ele existe para falhar redondo, não para
  medir a máquina.
- **Replicação, cluster e DbLink pela tela.** As telas são percorridas pelo
  passeio (abrem sem erro), mas o **comportamento** delas exige um segundo
  servidor, um MySQL(R) vivo ou quatro nós — e isso já tem prova própria em
  `bancada/`.
- **Impressão e exportação de arquivo.** O `telaExportar` é aberto e medido;
  o download em si o navegador entrega ao sistema, e provar isso é provar o
  Chromium.
- **Teclado e leitor de tela por completo.** O caso `lateral` exercita
  `Ctrl+\` e as setas da pega; o resto dos atalhos e os papéis ARIA não têm
  asserção. É o buraco mais óbvio que sobra nesta bateria.

---

## 7. A bateria única: o comando que roda tudo

```bash
python3 phxsql/provar.py --construir      # compila e roda tudo
python3 phxsql/provar.py --listar         # o que existe, e o que cada parte prova
python3 phxsql/provar.py --so tela        # uma parte só
python3 phxsql/provar.py --sem jobs       # a mais demorada fica de fora
python3 phxsql/provar.py --exigir-tudo    # pular passa a contar como reprovar
```

### Por que ela existe

As baterias já estavam todas aqui. O que não estava era o **relatório**: eram
oito comandos, em três linguagens, espalhados por seis diretórios. Quem chegava
no projeto não sabia o que rodar, e ninguém sabia dizer, num só lugar, se o
projeto estava verde.

O `provar.py` **não refaz nenhuma delas** — cada uma tem dono, já foi provada e
continua rodando sozinha pelo comando dela. Ele chama, cronometra e soma.

### As partes

A lista sai do `provar.py --listar`, e não desta tabela: uma contagem
digitada aqui envelheceria calada na próxima parte que entrasse.

| parte | o que prova | portas |
|---|---|---|
| `motor` | o motor, o protocolo e os portões — `cargo test --workspace` | — |
| `guardas` | que cada teste ainda **pega** o defeito que o motivou (§8) | — |
| `pacote` | que os **dois** conferidores de pacote concordam — e que a receita antiga do manifesto, reposta, reprova com 2 divergências por arquivo | — |
| `tela` | a interface contra o servidor de verdade: 120 telas, CSS global, contraste, primeira pintura | 6950/6951 |
| `idiomas` | o caminho do idioma de ponta a ponta, e o comportamento velho | 6952/6953 |
| `ponta-a-ponta` | os seis itens do dono pelo soquete, mais a passada pela tela | 6300/6301 |
| `alter` | acrescentar coluna numa tabela com dado pelo soquete: rowid preservado, backup, e a réplica que ainda não alterou | 7150/7152 |
| `transacoes` | `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT` pelo soquete — com **SIGKILL no meio de um `COMMIT`**, e o banco reabrindo para dizer o que aconteceu | 7320 |
| `rotinas` | gatilhos e procedimentos pelo soquete, com SIGNAL, lote e reinício | 5301/5701 |
| `profiler` | a redação do Profiler por soquete: vinte pedidos torcidos, sentinela no anel e no `.txt` | 6251 |
| `profiler-custo-zero` | que o Profiler DESLIGADO custa perto de zero — TRAVADO, não só medido (achado do QA-PDCA) | 6270/6272 |
| `telemetria-desenho` | o painel de bolhas por medida: rótulo na esfera, alvo de clique, contraste | — |
| `telemetria-interacao` | clicar na bolha menor com o painel em movimento, descer de nível, voltar | — |
| `telemetria-cores` | as cores configuráveis, exercitando: escolher, salvar, conferir no painel | 6600/6601 |
| `cluster` | eleição e promoção automática com três servidores e um SMTP falso | 5310-5312, 5316 |
| `replicacao` | os quatro modos por soquete, com o comportamento velho no fim | 5330-5339 |
| `jobs` | o aviso de jobs por e-mail — e o servidor **sem** bloco de e-mail, que não manda nada | 5303/5703 |
| `profiler-disco` | o `.txt` do Profiler contra o sistema operacional: disco cheio, somente-leitura | 6253 |
| `dblink` | a sincronia de tabelas primas contra um MySQL(R) de verdade | — |
| `odbc` | a ABI do driver pelo `ctypes`, sem passar pelo unixODBC | 6954 |

Cada parte abre as portas dela — documentadas no cabeçalho de cada script — e
mata só os processos que ela mesma criou, **pelo PID**. O `provar.py` não abre
porta nenhuma.

**Quanto leva:** a rodada inteira, medida nesta máquina com o `target/` quente,
**14m35s**. As três mais caras são a `tela` (3m54s, 24 execuções em dois
temas), as `guardas` (2m46s) e os `jobs` (2m36s, que esperam de verdade a volta
do vigia de 60 s — encurtar esse relógio seria provar outro relógio). As outras
treze somam menos de cinco minutos.

### O que ele recusa, herdado

A página da interface está **embutida** no `phxsqld` (`include_str!`). Mexer em
`ui/` e não recompilar faz metade destas baterias exercitar a página anterior e
passar verde numa correção que ainda não existe. A bateria de frontend já
recusava rodar nesse caso; aqui a recusa vale para o comando inteiro — e vale
também para os **examples**, que o `cargo build --release` não recompila
sozinho, e que já custaram a esta casa uma rodada inteira de ganhos invisível.

Recusa é `exit 2`, e não `exit 1`: não rodar não é reprovar.

### O que se pula, e por quê — e por que isso aparece no relatório

**Bateria que esconde o que não rodou mente por omissão.** O relatório termina
com a lista dos pulos e o motivo de cada um, e o código de saída separa os três
estados:

| saída | quer dizer |
|---|---|
| `0` | nada falhou — pode ter pulado, e o relatório diz o quê |
| `1` | alguma parte reprovou |
| `2` | recusou rodar (binário velho ou ausente) |

E quatro vereditos por parte: **PASSOU**, **FALHOU**, **PULADA** (com o motivo)
e **RODOU** — este último só para as sondas, logo abaixo.

`--exigir-tudo` transforma pulo em reprovação, para quem quer o portão
apertado.

Os pulos possíveis, e o requisito de cada um:

| parte | pula quando | por quê |
|---|---|---|
| `dblink` | não há MySQL(R) com o banco `crm` | a prova compara com um motor de verdade; simular seria provar o simulador |
| `odbc` | falta a `libphxsql_odbc.so` | um `cargo build --release` resolve. **Esta parte era um pulo permanente até esta rodada**: o passo do meio — subir um `phxsqld` com token e usuário próprios — estava escrito só em prosa no `docs/ODBC.md`, e passo em prosa não entra em bateria. Virou `bancada/odbc/provar.py`, que é o passo do meio e nada mais: monta o servidor, chama as duas provas que já existiam, mata pelo PID |
| `profiler-disco` | não é root | monta `tmpfs` para provar disco cheio **de verdade**; fingir com um diretório `0500` não vale, porque o bit de permissão não se aplica ao uid 0 e o teste passaria por engano |
| qualquer uma com porta | a porta já está ocupada | há outras frentes na mesma máquina, e **uma bateria que acusa a vizinha de defeito é pior que uma que não roda** |
| as de navegador | o Playwright não está instalado | ele **não entra no projeto** — a regra de zero dependência vale, e um conferidor de tela não é motivo para quebrá-la |

### O que fica de fora de propósito

As **medições** — `bancada/carga/`, `bancada/profiler/custo.py`,
`bancada/replicacao/medir.py`, `bancada/telemetria/monta-bancada.py` como fim em
si. Elas não têm veredito: um número mais lento não é uma reprovação, é um
número. Misturá-las aqui faria a bateria ficar vermelha por causa da carga da
máquina, e bateria que fica vermelha por acaso ensina a ignorar vermelho.

### Prova e sonda não são a mesma coisa, e o relatório separa

Uma **prova** sabe reprovar: sai diferente de zero quando o que ela mede está
errado. Uma **sonda** imprime o que achou e sai zero **sempre** — e chamar isso
de «PASSOU» seria inventar um veredito que ninguém deu.

A `profiler-disco` (`bancada/profiler/sonda-log.py`) é sonda: ela escreve
«ACEITOU — devia ter recusado» em vez de reprovar, e nos itens que precisam de
`tmpfs` escreve «PULADO». Dar-lhe um código de saída exigiria decidir o que
conta como falha em cada um dos seis itens, e isso é desenho do Profiler, não do
orquestrador. Ela sai como **RODOU**, com o buraco declarado, e o veredito é de
quem lê o log. A `sonda-permissao.py` é do mesmo tipo e ficou fora da lista por
ser puramente exploratória; a `sonda.py`, que procura a sentinela e devolve 1
quando acha, é prova e está na lista.

Foi esta distinção que revelou os dois conferidores da telemetria que **eram
prova e se comportavam como sonda** — §9.1.

---

## 8. As guardas: provar que a prova pega

A casa exige que todo teste novo **falhe com o defeito reposto**. Isso sempre
foi feito à mão, uma vez, por quem escreveu o teste — e depois se perdia.
Ninguém conseguia dizer, hoje, quais das 1.229 asserções ainda pegariam o
defeito que as motivou.

```bash
python3 bancada/guardas/provar-guardas.py
python3 bancada/guardas/provar-guardas.py --listar
```

Dois arquivos, e a divisão entre eles é o ponto: `catalogo.py` é **só dados** —
cada defeito, o trecho de hoje, o trecho de antes, e quais testes têm de cair.
`provar-guardas.py` copia a árvore, repõe um defeito por vez numa cópia, roda
só os testes nomeados, desfaz e julga. O desenho todo está em
`bancada/guardas/LEIA-ME.md`.

### A tabela das guardas provadas

Ela **não se digita** — sai de uma rodada, como as duas tabelas de cobertura da
§1:

```bash
python3 bancada/guardas/provar-guardas.py --json /tmp/guardas.json
python3 bancada/guardas/tabela-no-testes.py /tmp/guardas.json
```

<!-- guardas:inicio -->
| guarda | o defeito reposto | testes que caem | veredito |
|---|---|---:|---|
| `profiler-recorta` | o Profiler recorta o texto do pedido em vez de analisar | 5 | ✅ provada |
| `profiler-recorta-largo` | o Profiler recorta procurando a palavra `senha` solta | 4 | ✅ provada |
| `evento-linha-sem-escape` | campo livre vai cru para o .txt e forja uma linha inteira | 1 | ✅ provada |
| `profiler-sem-portao` | o portão próprio do Profiler não existe; o leitor lê o pedido alheio | 1 | ✅ provada |
| `pivotar-sem-portao` | `pivotar` sem conferência própria: a junção vira a porta dos fundos | 1 | ✅ provada |
| `sequencias-sem-portao` | `sequencias` mostra o contador de toda tabela, inclusive a negada | 1 | ✅ provada |
| `posicao-sem-portao` | `posicao` entrega eventos e o esquema cru de toda tabela | 1 | ✅ provada |
| `duplicar-sem-destino` | `duplicar_tabela` confere a origem e não o destino | 1 | ✅ provada |
| `regra-de-tabela-imposta` | sem regra de tabela, nega: a guarda nova entra imposta e nao pedida | 1 | ✅ provada |
| `sujas-com-a-trava` | `descarregar_sujas()` chamado com a trava de dados já na mão | 1 | ✅ provada |
| `cadeia-sem-teto` | a cadeia de gatilhos sem fundo: o binário aborta com stack overflow | 1 | ✅ provada |
| `excluir-tabela-lista-curta` | `excluir_tabela` apaga SEIS extensões e a tabela já tem NOVE | 1 | ✅ provada |
| `backup-sem-sha256` | restaurar aceita o backup adulterado: só o tamanho é conferido | 1 | ✅ provada |
| `aad-fora-do-slot` | só o dado associado sai: o nonce sozinho ainda amarra o endereço | — | 🟰 redundante |
| `nonce-sem-endereco` | só o endereço sai do nonce: o AAD sozinho ainda amarra | — | 🟰 redundante |
| `endereco-fora-da-amarracao` | as DUAS fechaduras somem: dá para embaralhar as linhas cifradas | 1 | ✅ provada |
| `cache-de-chaves-nao-limpo` | trocar a senha da cifra não limpa o cache: a senha errada abre | 1 | ✅ provada |
| `coluna-externa-sozinha-em-claro` | tabela cujas únicas colunas marcadas são externas nasce em claro | 3 | ✅ provada |
| `catraca-dos-textos` | mais um texto de tela cravado, fora da fábrica de idiomas | 1 | ✅ provada |
| `trava-fora-do-ponto-unico` | uma tomada da trava de dados fora do `travar_dados()` | 1 | ✅ provada |
| `trava-sem-guarda-de-reentrancia` | a trava pedida duas vezes pela mesma thread pendura o servidor | 1 | ✅ provada |
| `exclusao-na-janela-por-padrao` | a exclusão entra na janela por padrão, sem ninguém pedir | 1 | ✅ provada |
| `exclusao-na-janela-sem-leitor` | `exclusao_na_janela` no config.json, no MANUAL e na tela — e ninguém o lê | 1 | ✅ provada |
| `reg-fecha-antes-do-trash` | a janela sincroniza o `.reg` antes do `.trash` | 1 | ✅ provada |
| `rodizio-do-profiler-ignora-o-zero` | `profiler.arquivo_mib: 0` deixa de querer dizer «sem rodízio» | 2 | ✅ provada |
| `cabecalho-do-profiler-forjado` | o cabeçalho do arquivo do Profiler aceita linha forjada | 1 | ✅ provada |
| `profiler-sem-descritor-calado` | sem descritor, com arquivo pedido, a linha some sem ser contada | 1 | ✅ provada |
| `trava-atras-da-rede` | o laço da réplica segura a trava de dados enquanto lê do soquete | 1 | ✅ provada |
| `ordem-pequena-aceita` | o segredo X25519 todo-zeros aceito como chave de sessão | 2 | ✅ provada |
| `contador-do-fio-parado` | o contador de registros do fio parado — nonce repetido | 3 | ✅ provada |
| `fio-cortado-vira-fim` | o fio cortado no meio devolvido como fim de conversa | 1 | ✅ provada |
| `cifra-do-fio-rebaixada` | a cifra do fio de volta a OPCIONAL por padrão | 1 | ✅ provada |
| `portas-http-sem-o-portao-da-cifra` | as portas HTTP atendendo em claro com a cifra exigida | 2 | ✅ provada |
| `transcricao-sem-o-cifrado` | o hash da transcrição sem o texto cifrado da mensagem 2 | 2 | ✅ provada |
| `amarra-ao-canal-ignorada` | o login amarrado ao canal conferido SEM a transcricao | 1 | ✅ provada |
| `amarra-exigida-ignorada` | o servidor exige a amarracao ao canal, mas o login nao a cobra | 1 | ✅ provada |
| `remoto-em-claro-para-quem-exige` | o abrir_remoto manda o login em claro mesmo com cifra: true | 1 | ✅ provada |
| `fio-sem-teto-de-registro` | a leitura do fio volta a ser ilimitada | 1 | ✅ provada |
| `teto-do-fio-sem-a-constante` | o `Canal::ler` de producao troca `TETO_DO_REGISTRO` por um teto quase infinito | 1 | ✅ provada |
| `teto-do-fio-sem-a-constante-no-soquete` | a mesma troca da constante por um teto quase infinito, vista pela rede | 1 | ✅ provada |
| `teto-da-linha-sem-a-constante-no-soquete` | o `teto_da_linha` do servidor troca `TETO_DO_REGISTRO` por um teto quase infinito, visto pela rede | 1 | ✅ provada |
| `pulso-do-cluster-em-claro` | o pulso da eleição saindo em claro com a cifra do cluster ligada | 1 | ✅ provada |
| `replicacao-do-cluster-em-claro` | a replicação entre os nós do cluster saindo em claro | 1 | ✅ provada |
| `alter-compacta-o-buraco` | a reescrita da coluna nova pula os slots excluídos e renumera o rowid | 1 | ✅ provada |
| `alter-sem-remapear-posicao` | a coluna nova desloca as de sistema e ninguém remapeia quem guarda posição | 2 | ✅ provada |
| `alter-espelho-para-tras` | o espelho `.bkp` fica com a largura velha depois de acrescentar coluna | 1 | ✅ provada |
| `alter-queda-no-meio` | o conjunto de volumes misturado abre e lê o volume 3 com a largura do 1 | 2 | ✅ provada |
| `ffi-panico-atravessa` | o pânico atravessa a fronteira de C em vez de virar código de erro | 2 | ✅ provada |
| `ffi-panico-nao-envenena` | o punho continua sendo usado depois de um pânico capturado | 1 | ✅ provada |
| `ffi-punho-morto-lido-antes-de-conferir` | a fronteira volta a ler a etiqueta de DENTRO do punho antes de saber se ele ainda existe | 1 | ✅ provada |
| `ffi-texto-ate-o-byte-zero` | a fronteira trunca o dado do cliente no primeiro byte zero | 2 | ✅ provada |
| `ffi-erro-global` | a mensagem de erro é global e uma thread lê o erro da outra | 1 | ✅ provada |
| `ffi-rowid-fora-e-erro` | «não há essa linha» volta de duas formas diferentes conforme o motivo | 1 | ✅ provada |
| `ffi-cursor-para-no-lote` | o cursor entrega só o primeiro lote e diz que a tabela acabou | 1 | ✅ provada |
| `texto-colado-nos-seis` | a mesma frase colada nas seis colunas de idioma | 2 | ✅ provada |
| `frase-longa-repetida` | uma frase longa repetida em três das seis colunas de idioma | 1 | ✅ provada |
| `rest-operacao-sem-documento` | operação nova no despachar que a especificação OpenAPI não documenta | 2 | ✅ provada |
| `rest-rota-fantasma` | a especificação promete uma rota que o servidor não atende | 1 | ✅ provada |
| `rest-nasce-ligado` | o webservice REST passa a escutar numa atualização, sem ninguém pedir | 1 | ✅ provada |
| `rest-corpo-manda-no-caminho` | o corpo do pedido REST troca a operação do caminho, em silêncio | 1 | ✅ provada |
| `rest-filtro-so-o-campo-tabela` | o filtro de tabelas do REST olha só o campo `tabela` — e a junção é a porta dos fundos | 1 | ✅ provada |
| `rest-fecha-sem-escoar` | a recusa por lista negra é engolida por um RST, e quem foi barrado vê «connection reset» | — | 🟰 redundante |
| `transacao-nao-empilha` | a transação escreve direto no disco em vez de empilhar | 3 | ✅ provada |
| `commit-confirma-abortada` | o COMMIT confirma uma transação que já estava em ABORT_ONLY | 1 | ✅ provada |
| `marca-antes-do-fsync` | a marca `.tx` é apagada antes de a tabela sincronizar | 1 | ✅ provada |
| `insert-sem-travar-o-fim` | duas transações que anexam preveem o mesmo rowid | 1 | ✅ provada |
| `recuperar-sem-reindexar` | a recuperação não reconstrói o `.ndx` que a queda deixou para trás | 1 | ✅ provada |
| `comum-anexa-no-fim-travado` | a escrita comum que anexa não olha o fim travado | 1 | ✅ provada |
| `dependencia-de-fora-fica-invisivel` | o filtro de dependência externa vira mudo (mede e nunca acusa) | 1 | ✅ provada |
| `sem-indice-na-filha-ignora-em-vez-de-recusar` | sem índice na filha, a exclusão da mãe ignora em vez de recusar | 1 | ✅ provada |
| `cache-paginas-nao-chega-ao-motor` | `cache_paginas` do config.json deixa de chegar ao motor | 2 | ✅ provada |
| `replica-julga-fk` | a replica volta a conferir chave estrangeira no evento que aplica | 2 | ✅ provada |
| `cascata-sem-imagem-no-diario` | a filha que a cascata abre volta a nascer sem imagem no diario | 2 | ✅ provada |
| `replica-refaz-a-cascata` | a replica volta a refazer a cascata que o source ja mandou | 1 | ✅ provada |
| `marca-de-replica-fica-acesa` | a marca de replica nao se apaga na volta do `aplicar_evento` | 1 | ✅ provada |
| `fk-nao-pergunta-se-a-mae-esta-viva` | a conferencia da chave volta a perguntar so se a mae EXISTE | 3 | ✅ provada |
| `drop-table-mata-o-pai` | o `excluir_tabela` volta a apagar a mae com filha apontando | 1 | ✅ provada |
| `before-sem-prazo-de-parede` | o corpo do gatilho BEFORE volta a rodar sem prazo, com a trava global na mão | 1 | ✅ provada |
| `declara-conferida-sobre-orfa` | a chave volta a nascer conferida sobre tabela que ja tem orfa | 1 | ✅ provada |
| `verificador-nao-pergunta-se-a-mae-esta-viva` | o verificador volta a aceitar mae excluida como mae | 1 | ✅ provada |
| `restaurar-nao-pergunta-pela-mae` | restaurar volta a ressuscitar a filha sem olhar a mae | 1 | ✅ provada |
| `bidirecional-julga-fk` | o bidirecional volta a conferir a chave do evento que aplica | 1 | ✅ provada |
| `bidirecional-julga-as-filhas` | o bidirecional volta a recusar apagar a mae que tem filha | 1 | ✅ provada |
| `recascata-sem-conferir-a-arvore` | a recuperação gravava a primeira filha e só então descobria que a neta da segunda restringe | 1 | ✅ provada |
| `auto-referencia-em-silencio` | a auto-referência sai da cascata em silêncio e orfana a subordinada | 1 | ✅ provada |
| `recado-manda-reparar-arquivo-sao` | a mãe invisível manda reparar o índice — de um arquivo intacto | 2 | ✅ provada |
| `procura-das-filhas-manda-reparar-arquivo-sao` | a procura pelas filhas manda reparar o índice — de um arquivo intacto | 1 | ✅ provada |
| `recuperacao-nao-reconstroi-a-filha` | a recuperação não reconstrói o índice da filha, e a cascata fica pela metade | 1 | ✅ provada |
| `pista-de-leitura-engole-a-trilha` | a pista de leitura aceita tabela com dado pessoal, e a trilha fica sem o registro | 1 | ✅ provada |
| `pista-de-leitura-nao-espelha` | a pista de leitura aceita tabela sem `.bkp` e o espelho deixa de nascer | 1 | ✅ provada |
| `leitura-sem-recuo-para-a-exclusiva` | a tabela que pede a ficha exclusiva vira erro em vez de recuo | 1 | ✅ provada |
| `abrir-para-ler-cria-a-lixeira` | abrir para LER cria o `.trash` que falta, sob a ficha compartilhada | 1 | ✅ provada |
| `leitura-sem-guarda-de-reentrancia` | a ficha compartilhada pedida com a exclusiva na mão pendura o servidor | 1 | ✅ provada |
| `familia-pela-grafia-crua` | a grafia do caminho divide a família do registro de `fsync`, e o volume sujo fica para trás | 1 | ✅ provada |
| `pag-gravado-com-truncagem` | o `.pag` escrito com `fs::write` aparece pela metade para quem lê de fora | 1 | ✅ provada |
| `pagina-anterior-de-um-em-um` | a página anterior anda de um em um pelo vazio entre baldes — e ali o `ler` cru RECUSA em vez de dizer «vazio» | 1 | ✅ provada |
| `fecho-em-paralelo-engole-o-erro` | o `fsync` que falha dentro do fio, e o `join` que engole o erro | 1 | ✅ provada |
| `fecho-em-paralelo-fio-que-nao-sobe` | uma tabela do fecho fica sem fio, e ninguém percebe | 1 | ✅ provada |
| `pagina-ordenada-varre-o-indice-inteiro` | a grade ordenada percorre o índice inteiro para devolver 50 linhas | 2 | ✅ provada |
| `cursor-do-pedaco-sem-o-mais-um` | o cursor da varredura em pedaços devolve de novo a linha da borda | 2 | ✅ provada |
| `perfil-grava-o-texto-da-tabela-declarada` | o perfil.txt grava em claro o pedido de uma tabela declarada em cifra.tabelas | 4 | ✅ provada |
| `perfil-decide-so-pela-lista-e-nao-pelo-reg-cifrado` | o perfil.txt decide pela lista do config e a cifra acontece pela marca de coluna | 2 | ✅ provada |
| `perfil-grava-o-erro-que-cita-o-valor` | o perfil.txt tapa o pedido e grava o erro, que cita o valor da coluna marcada | 1 | ✅ provada |
| `profiler-ligado-sem-a-raiz-dos-dados` | o Profiler liga sem a raiz de dados e volta a decidir por um campo só | 1 | ✅ provada |
| `perfil-so-olha-a-tabela-do-primeiro-nivel` | o Profiler so olha a tabela do primeiro nível e a junção vira a porta dos fundos | 1 | ✅ provada |
| `fase-da-telemetria-com-dado-do-usuario` | a fase do SQL Check passa a carregar dado do usuário, e o furo nasce calado | 1 | ✅ provada |
| `cache-de-derivadas-sobrevive-a-troca-de-senha` | o cache de chaves derivadas responde a quem não deu a senha | 1 | ✅ provada |
| `fts-reindexar-sem-o-irmao` | o reindexar reconstrói só o .ndx, e a queda trava a tabela para sempre | 1 | ✅ provada |
| `fts-reconstruir-sem-recriar` | reconstruir o índice de texto sem recriar o arquivo não é idempotente | 1 | ✅ provada |
| `fts-abrir-recusa-a-tabela` | o .fts ilegível derruba a tabela inteira, em vez de se refazer | 1 | ✅ provada |
| `fts-nasce-na-pista-de-leitura` | a pista de leitura cria o .fts, e escrever sob a ficha compartilhada é o que ela existe para impedir | 1 | ✅ provada |
| `fts-chave-truncada-nao-se-declara` | a chave truncada não se declara truncada, e a busca acha a mais | 1 | ✅ provada |
| `operacao-sem-poder-declarado` | operação catalogada sem linha de poder vira administrador em silêncio | 2 | ✅ provada |
| `sequencia-numero-cru-perde-precisao` | id acima de 2⁵³ mandado como número cru é gravado trocado, calado | 1 | ✅ provada |
| `sequencia-grande-sai-numero-mentiroso` | id acima de 2⁵³ já gravado sai do servidor como número f64 trocado | 1 | ✅ provada |
| `colisao-de-sequence-calada` | dois masters na mesma faixa perdem uma linha sem contar a ninguém | 1 | ✅ provada |
| `contador-de-sequence-atras-do-dado` | contador de Sequence atrás do dado repete número, e não havia reparo | 1 | ✅ provada |
| `regra-de-coluna-com-typo-carrega-calada` | regra de direito por coluna que cita coluna inexistente carrega calada e não protege nada | 3 | ✅ provada |
| `juncao-direita-vazia-perde-colunas` | LEFT JOIN com a direita vazia sai sem as colunas da direita, e a forma da linha muda | 2 | ✅ provada |
| `decimal-do-consultar-compara-como-texto` | Decimal no consultar.expressao compara como texto, e 9,50 passa por um filtro de acima de 10 | 3 | ✅ provada |
| `existe-fora-do-inventario-de-tabelas` | existe[].de fora de tabelas_do_pedido: quem pergunta que tabelas o consultar alcanca nao ve a de dentro do EXISTS | 1 | ✅ provada |
| `wchar-recusa-no-driver-odbc` | SQL_C_WCHAR volta a recusar no driver ODBC, que agora fala UTF-16 na borda | 2 | ✅ provada |
| `replica-insiste-na-credencial-recusada` | a réplica com credencial recusada insistia a cada `reconectar_em` e bloqueava o próprio IP — derrubando o operador junto | 1 | ✅ provada |
| `upsert-zera-a-coluna-negada` | o upsert (`inserir` com `se_existir: "atualizar"`) zerava a coluna que o usuário não altera — para quem não lê, para quem lê e não altera, pelo SQL `ON CONFLICT DO UPDATE` e em transação | 1 | ✅ provada |
| `presenca-da-coluna-negada-recusa-a-ficha` | a presença da coluna que o usuário não altera recusava a operação inteira — e a ficha, que manda a linha inteira com a coluna como `null`, não incluía nem salvava nada | 3 | ✅ provada |
| `set-do-on-conflict-ignorado` | o `SET` do `INSERT … ON CONFLICT DO UPDATE` / `ON DUPLICATE KEY UPDATE` (o campo `atualizar`) era ignorado calado, e o `VALUES` ia por cima da linha com NULL no que ele não trazia | 1 | ✅ provada |
| `filtro-do-indice-parcial-e-oraculo` | o índice parcial cujo `onde` cita a coluna negada respondia sobre ela: varrer por ele devolvia exatamente quem tem `salario > 5000` | 1 | ✅ provada |
| `juncao-materializa-antes-do-teto` | as junções `interno`/`esquerdo`/`direito`/`completo` materializavam a saída inteira antes de conferir o teto — 1000 × 1000 com a mesma chave custava +561 MiB para recusar contra um teto de 1000 | 2 | ✅ provada |
| `select-da-coluna-negada-devolve-nulo` | `SELECT salario FROM folha` por quem não lê `salario` devolvia `{"salario": null}` em toda linha, em vez de recusar | 1 | ✅ provada |
| `em-engole-o-campo-ausente` | `consultar.em` com `campo` que o sub-pedido não devolve — inclusive a coluna negada — respondia zero linhas com `ok: true` | 2 | ✅ provada |
| `literal-negativo-nao-parseia` | o literal negativo não parseava em `SET`/`VALUES` («esperava um valor e veio "-"») enquanto `WHERE a = -5` passava pela expressão | 1 | ✅ provada |
| `tabela-inexistente-vaza-o-caminho` | a tabela que não existe respondia «nenhum volume de x.reg em /tmp/…» — o caminho absoluto do disco do servidor, a todo cliente que erra uma letra | 1 | ✅ provada |
| `permissao-sem-devolver-a-vaga` | a permissão do semáforo morre sem devolver a vaga — o `fetch_sub` esquecido, com outro nome | 7 | ✅ provada |
| `permissao-de-dados-sem-raii` | a vaga da porta de dados só volta no caminho feliz — um pânico no `atender` a leva junto | 1 | ✅ provada |
| `web-sem-teto` | a porta web volta a nascer sem teto — uma thread por pedido, como até a 0.18 | 1 | ✅ provada |
| `ficha-do-fio-pulada-no-panico` | a ficha da thread na telemetria fica «viva» para sempre quando o corpo entra em pânico | 1 | ✅ provada |
| `disco-erro-de-es-sem-aviso` | o erro de E/S respondido ao cliente não avisa ninguém | 3 | ✅ provada |
| `disco-sonda-cega-ao-erro` | a sonda canário diz «passou» num diretório que o sistema operacional recusa | 1 | ✅ provada |
| `disco-silencio-furado` | todo erro de E/S manda um aviso: cem mil linhas, cem mil e-mails | 4 | ✅ provada |
| `disco-config-nao-lida` | `alertas.disco.checar_segundos` está no arquivo e ninguém o lê | 2 | ✅ provada |
| `recuperacao-deixa-a-marca-orfa` | a recuperação completa (ou descarta) a marca `.tx` e a deixa no disco | 1 | ✅ provada |
| `recuperacao-nao-completa-o-commit` | a recuperação conta e apaga a marca válida sem completar o commit | 1 | ✅ provada |
| `ndx-queda-com-cabecalho-limpo` | a marca de sujo do `.ndx` fica só em RAM e a queda deixa o índice atrasado em silêncio | 2 | ✅ provada |
| `reserva-sobrevive-a-queda-da-ligacao` | a saída da conexão não solta a reserva do BULKINSERT | 1 | ✅ provada |
| `bulkinsert-false-nao-drena-a-marca` | o `bulkinsert(false)` sincroniza a tabela e deixa a marca `.tx` do COMMIT no disco | 1 | ✅ provada |
| `fecho-sem-suja-nao-drena-a-marca` | o fecho da janela volta antes de drenar as marcas quando não há tabela suja | 2 | ✅ provada |
| `fsync-do-arquivo-limpo` | `Volumes::sincronizar` leva ao disco todo descritor aberto, sem pular o limpo | 2 | ✅ provada |
| `fsync-so-dos-escritos` | o fecho confia só no registro em RAM — e o registro nasceu vazio com o processo | 2 | ✅ provada |
| `relogio-ao-alcance-do-teste` | o estado do gerador de v7 fica ao alcance de um teste, que o escreve para trás | 1 | ✅ provada |
| `upsert-gatilho-do-ramo` | no upsert que atualiza, o BEFORE UPDATE vê a linha mesclada e o AFTER é o do ramo que ele virou | 5 | ✅ provada |
| `threads-do-so-pela-diferenca` | a prova de que o SO viu a thread subida é a diferença entre duas leituras do total do processo | 1 | ✅ provada |
| `varredura-sem-o-elo` | a varredura barata do diretorio perde a tabela alcancada por elo | 1 | ✅ provada |
| `linha-vazia-na-conferencia-de-filhas` | a linha descida para a conferencia de filhas vai vazia, e toda mae parece sem filha | 3 | ✅ provada |
| `teto-de-64-bits-satura` | número cru fora da faixa do `Int8` é GRAVADO saturado, e `1e21`, `1e30` e `1e300` viram todos o mesmo número | 2 | ✅ provada |
| `saida-do-direito-por-coluna` | a recusa do direito por coluna manda «peça as colunas por varrer» também para o `agrupar` e para o `backup` | 1 | ✅ provada |
| `check-que-se-contradiz-no-alter` | `acrescentar_coluna` aceita um `padrao` que viola o `check` declarado no MESMO comando, e todo `atualizar` da linha velha passa a recusar | 1 | ✅ provada |
| `upsert-parcial-vira-mescla` | o upsert sem o campo `atualizar` passa a MESCLAR, e a sincronia do DbLink perde a única forma de gravar NULO num destino | 1 | ✅ provada |
| `direcao-do-indice-sem-saida` | a recusa por direção do índice explica bem por que não dá, e não diz o que fazer | 1 | ✅ provada |
| `sha256-sem-somar-o-estado` | SHA-256 sem a realimentação do estado: a compressão vira permutação reversível | 4 | ✅ provada |
| `sha256-com-o-tamanho-em-little-endian` | SHA-256 com o tamanho da mensagem, no padding, em little-endian | 4 | ✅ provada |
| `hmac-com-a-chave-longa-truncada` | HMAC com a chave maior que o bloco TRUNCADA em vez de pré-hasheada | 2 | ✅ provada |
| `pbkdf2-com-o-contador-de-bloco-parado` | PBKDF2 com o contador de bloco parado: saída longa repete o primeiro bloco | 1 | ✅ provada |
| `pbkdf2-sem-o-xor-acumulado` | PBKDF2 sem o XOR acumulado: vira HMAC aplicado N vezes | 2 | ✅ provada |
| `juntar-sem-portao` | `juntar` sem conferência própria: a tabela negada entra como lado B | 1 | ✅ provada |
| `unir-sem-portao` | `unir` sem conferência própria: a tabela negada entra na LISTA | 1 | ✅ provada |
| `diferencas-sem-portao` | `diferencas` sem conferência própria: a tabela negada entra em `a` ou em `b` | 1 | ✅ provada |
| `derivado-sem-portao` | o portão some do irmão `executar_derivado`: o SQL inteiro vira a porta dos fundos | 8 | ✅ provada |
| `ficha-do-usuario-devolve-o-hash` | a ficha do usuário passa a devolver o `senha_hash` junto | 2 | ✅ provada |
| `senha-em-claro-no-cadastro` | a senha entra no config.json em texto puro: o `cifrar` sai do caminho de gravação | 2 | ✅ provada |
| `senha-velha-fica-no-arquivo` | trocar a senha não leva junto a que estava em texto puro no arquivo | 1 | ✅ provada |
| `cifra-reserializa-a-senha` | o `para_json` da cifra devolve a senha de verdade em vez de «(oculta)» | 2 | ✅ provada |
| `debug-da-cifra-mostra-a-senha` | o `Debug` da cifra imprime a senha: um `dbg!` apressado a joga no log | 1 | ✅ provada |
| `debug-do-segredo-mostra-o-valor` | o `Debug` do tipo `Segredo` imprime o valor: todo dono que o chamar vaza | 1 | ✅ provada |
| `profiler-sem-a-senha-dentro-do-sql` | o Profiler perde a senha que está DENTRO da frase SQL, e não num campo | 1 | ✅ provada |
| `comando-invalido-vira-texto-cru` | o SQL que o léxico recusa volta inteiro para o log, com a senha dentro | 1 | ✅ provada |
| `trilha-sem-o-nome-de-segredo` | a trilha LGPD deixa de olhar o NOME da coluna e só analisa o valor | 1 | ✅ provada |
| `trilha-so-olha-o-nome-da-coluna` | a trilha LGPD deixa de ANALISAR o valor e só confia no nome da coluna | 1 | ✅ provada |
| `debug-da-ligacao-mostra-a-senha` | o `Debug` da ligação de DbLink imprime a senha e o token do outro banco | 1 | ✅ provada |
| `fio-cifrado-manda-o-claro-junto` | o fio cifrado manda a linha em claro junto do registro selado | 1 | ✅ provada |
| `diario-das-diretivas-guarda-o-segredo-anterior` | o diário das diretivas grava o valor ANTERIOR do campo sigiloso em claro | 1 | ✅ provada |
| `cluster-devolve-a-credencial-na-tela` | o resumo do cluster na op `config` leva o token entre nós e o hash do replicador | 2 | ✅ provada |
| `token-do-rest-entra-pela-tela` | o token da porta REST passa a se gravar pela tela de configuração | 1 | ✅ provada |
| `cifra-do-odbc-volta-a-nascer-em-claro` | a receita do driver ODBC volta a nascer em claro, e o esquecimento vira o padrao | 5 | ✅ provada |
| `receita-odbc-devolve-a-senha` | a connection string mascarada do ODBC devolve a senha inteira | 1 | ✅ provada |
| `cifra-do-fio-reserializa-a-privada` | o `para_json` da cifra do fio devolve a chave privada em vez de «(oculta)» | 1 | ✅ provada |
| `especificacao-openapi-leva-o-token` | a especificação OpenAPI, servida sem portão, passa a carregar o token da porta | 1 | ✅ provada |
| `token-remoto-fora-da-lista-de-segredos` | o `token_remoto` sai da lista de segredos: o token do OUTRO servidor vai em claro para o `perfil.txt` e para a op `profiler` | 4 | ✅ provada |
| `job-recusa-um-nome-e-grava-os-outros` | a guarda do job volta a recusar só `token`: `senha`/`token_remoto` vão para o `jobs.json` e voltam na ficha | 1 | ✅ provada |
| `config-json-escreve-aberto-e-herda` | o `config.json` volta a nascer na permissão do `umask` e a herdar o `0644` do original | 2 | ✅ provada |
| `config-phz-troca-escreve-aberto-e-herda` | a troca de forma (`--empacotar-config`/`--desempacotar-config`) grava o arquivo novo aberto, herdando o `0644` do original | 1 | ✅ provada |
| `config-phz-abre-com-os-24-ciclos` | o `config.phz` volta a abrir com o teto de 24 ciclos: um cabecalho hostil custa 2^24 rodadas ja no arranque | 1 | ✅ provada |
| `config-phz-abre-cabecalho-de-megabytes` | o `config.phz` volta a aceitar cabecalho de megabytes: um arquivo de KiB aloca o que o cabecalho declarar | 1 | ✅ provada |
| `config-phz-dois-presentes-escolhe-calado` | com `config.json` E `config.phz` presentes, o servidor escolhe um calado e sobe | 2 | ✅ provada |
| `config-phz-terceiro-nao-e-ignorado` | o nome de um TERCEIRO numa pasta com sticky bit volta a travar o arranque | 1 | ✅ provada |
| `config-phz-terceiro-nao-avisa-no-arranque` | o servidor ignora o nome de TERCEIRO e sobe calado sobre o que descartou | 1 | ✅ provada |
| `config-phz-par-root-vira-terceiro` | o `.json` do ROOT ao lado do `.phz` do servico vira arquivo de terceiro, e o servico sobe do `.phz` VELHO | 1 | ✅ provada |
| `config-phz-par-root-e-terceiro` | o root sai do lado de confianca: numa pasta com sticky bit, o `.json` dele vira arquivo de terceiro | 2 | ✅ provada |
| `config-phz-par-sem-sticky-escolhe` | o nome de terceiro e ignorado numa pasta SEM sticky bit, onde quem o criou tambem troca o do servico | 1 | ✅ provada |
| `config-phz-par-pasta-que-so-o-dono-grava` | o nome de outro dono e ignorado numa pasta com sticky bit que SO o dono grava | 1 | ✅ provada |
| `config-phz-par-dono-da-pasta-vira-terceiro` | o nome do DONO da pasta e ignorado como se fosse de terceiro | 1 | ✅ provada |
| `config-phz-par-falha-aberto` | o ramo que falha fechado escolhe o `.json` quando nenhum lado e de confianca | 4 | ✅ provada |
| `config-phz-par-sem-euid-escolhe` | sem o uid de quem roda, o par supoe root e escolhe | 1 | ✅ provada |
| `config-phz-euid-le-o-uid-real` | o uid de quem roda sai do campo REAL do `/proc/self/status`, e nao do efetivo | 1 | ✅ provada |
| `config-phz-troca-sobre-terceiro-diz-corrida` | `--empacotar-config` com o `.phz` de um terceiro ao lado culpa uma corrida que nao houve | 1 | ✅ provada |
| `config-json-claro-vira-phz-sem-pedir` | o servidor que subiu de um `config.json` em claro passa a grava-lo empacotado sem ninguem pedir | 1 | ✅ provada |
| `config-dica-do-modelo-sobre-arquivo-presente` | o `phxsqld` que nao sobe manda gerar o modelo `> config.json` por cima do arquivo que o erro esta nomeando | 2 | ✅ provada |
| `config-phz-troca-so-depois-de-validar` | a troca de forma so roda depois de o `Config::ler` aceitar: o `.phz` com um campo torto nao sai para conserto | 1 | ✅ provada |
| `config-phz-copia-guardada-fica-aberta` | a copia em claro que a migracao guarda leva o `0644` da instalacao, com o token, para sempre | 1 | ✅ provada |
| `config-phz-migra-o-link` | a migracao de um config que e LINK move so o link e diz que guardou o original | 1 | ✅ provada |
| `config-phz-aviso-procura-a-copia-pelo-lido` | o aviso de arranque procura a copia em claro por um nome que a migracao nao usou, e cala | 1 | ✅ provada |
| `gravar-privado-temporario-e-o-proprio-config` | o irmao-por-sufixo troca a extensao, e com `--config servidor.tmp` (ou `"jobs": "agenda.log"`) o irmao e o proprio arquivo | 2 | ✅ provada |
| `config-phz-desfazer-apaga-a-unica-copia` | o desfazer da troca apaga o arquivo novo mesmo quando o velho sumiu, e diz que o velho «continua valendo» | 1 | ✅ provada |
| `config-phz-grava-o-texto-cru` | o servidor que subiu de um `config.phz` grava o texto cru dentro dele: o token volta a ler-se num editor | 1 | ✅ provada |
| `replica-lista-e-pedida-nao-imposta` | replicas_autorizadas vazia libera todos -- e so isso e' pedida, nao imposta | 1 | ✅ provada |
| `posicao-nao-encolhe-em-silencio` | tabela que nao abre some da soma do diario sem marcar `incompleta` | 1 | ✅ provada |
| `eleicao-prefere-completa` | `cluster::vencedor` volta a comparar so a posicao numerica, ignorando `incompleta` | 1 | ✅ provada |
| `replica-nao-atende-escrita` | `aplicar` pela rede deixa de exigir um papel que receba replicacao | 2 | ✅ provada |
| `spare-nao-atende-ninguem` | o papel Spare deixa de recusar toda operacao que nao esta em OPS_NO_SPARE | 1 | ✅ provada |
| `read-replica-recusa-escrita` | `ReadReplica` deixa de recusar escrita e para de apontar o primario | 1 | ✅ provada |
| `pulso-fora-da-lista-e-recusado` | `op_cluster_pulso` deixa de conferir o id contra a lista viva de nos | 1 | ✅ provada |
| `laco-preso-no-unico-secundario` | chave duplicada num índice único secundário prende o laço do bidirecional para sempre | 3 | ✅ provada |
| `par-parado-reapresentado-a-cada-rodada` | a tabela parada por conflito volta a ser puxada a cada rodada, e o grito se repete para sempre | 1 | ✅ provada |
| `dado-pessoal-no-grito-do-conflito` | o grito do conflito de unicidade publica a coluna marcada como dado pessoal | 1 | ✅ provada |
| `so-o-disco-vem-da-porta-e-nao-de-desligar-depois` | o empilhar volta a abrir pela porta de sempre e desligar a sobreposicao na linha seguinte | 1 | ✅ provada |
| `slot-de-outro-reg` | o sal deixa de ser por arquivo: o slot cifrado de um `.reg` abre no outro | 2 | ✅ provada |
| `pulso-sem-prova-de-identidade` | o pulso do cluster aceitando identidade auto-declarada | 3 | ✅ provada |
| `aperto-de-mao-sem-teto` | a leitura do aperto de mao fora do `Canal`, sem teto nenhum | 1 | ✅ provada |
| `erro-do-pulso-mapeia-quem-nao-tem-pino` | a recusa da prova do pulso dizendo quais nós ainda não têm pino | 1 | ✅ provada |
| `relogio-do-pulso-com-sono-plantado` | um atraso de 100 µs só no nó sem pino, com a frase já igual | 1 | ✅ provada |
| `pino-cego-sem-a-recusa-do-no-sem-pino` | a forja contra o pino cego entrando pelo nó sem pino | 1 | ✅ provada |
| `nonce-do-pulso-sem-regua-de-bytes` | o nonce do pulso retido do tamanho que o remetente escolheu | 2 | ✅ provada |
| `antirrepeticao-envenenada-vira-pulso-inedito` | a antirrepetição do pulso desligada, calada, por uma trava envenenada | 2 | ✅ provada |
| `trava-da-guarda-recupera-calada` | a trava envenenada da guarda recuperada sem dizer nada | 1 | ✅ provada |
| `tofu-envenenado-vira-nunca-provou` | o TOFU do pulso desligado, calado, por uma trava envenenada | 1 | ✅ provada |
| `guarda-do-pulso-inerte-e-muda` | o pulso sem prova aceito sem deixar rastro no log | 1 | ✅ provada |
| `guarda-do-pulso-inerte-aviso-por-pulso` | o aviso da guarda inerte repetido a cada pulso | 1 | ✅ provada |
| `resposta-do-pulso-sem-crivo-da-lista` | a resposta do pulso com id fantasma rebaixando o master | 2 | ✅ provada |
| `resposta-sem-prova-assinada-so-com-pino` | a resposta de sucesso a um pulso sem prova dizendo quais nós têm pino | 1 | ✅ provada |
| `resposta-sem-prova-assina-e-esconde` | a resposta a um pulso sem prova igual na forma e diferente no relógio | 1 | ✅ provada |
| `reescrita-sem-portao-na-trava` | a migração congela a tabela que o COMMIT de uma transação aberta vai abrir | 1 | ✅ provada |
| `acrescentar-coluna-sem-portao` | o acrescentar_coluna congela a tabela que o COMMIT de uma transação aberta vai abrir | 1 | ✅ provada |
| `commit-sem-rede-antes-da-marca` | o COMMIT grava a marca com uma tabela do alcance congelada | — | 🟰 redundante |
| `commit-sem-as-duas-recusas-antes-da-marca` | o COMMIT grava a marca com a tabela congelada quando a rede do 426 E a pre-conferencia do 448 somem | 1 | ✅ provada |
| `instrucao-na-vizinha-da-congelada` | a escrita ligada pela chave a uma tabela congelada entra na lista da transação | 1 | ✅ provada |
| `braco-de-erro-retrava` | a passada do COMMIT quebra depois da marca e a recuperação da hora não roda | 2 | ✅ provada |
| `completar-apaga-a-marca-impossivel` | a recuperação do COMMIT apaga a marca de uma operação que só estava congelada | 1 | ✅ provada |
| `after-no-commit-some-calado` | o AFTER disparado no COMMIT grava numa lista já descartada e some sem aviso | 1 | ✅ provada |
| `commit-zero-aplicado-vira-committed` | a passada que quebra antes de qualquer byte da lista responde COMMITTED | 1 | ✅ provada |
| `commit-meio-sem-dizer-o-que-ficou` | a chave que falha no meio da passada vira COMMITTED sem a escrita que falhou | 1 | ✅ provada |
| `de-hex-fatia-texto-por-byte` | o de_hex em pânico com hexadecimal que corta um caractere de vários bytes | 2 | ✅ provada |
| `prova-do-pulso-derruba-a-conexao` | a prova do pulso que corta um caractere derruba a conexão e mata o laço do pulso | 3 | ✅ provada |
| `copia-do-de-hex-envenena-a-trava-de-dados` | o binário que corta um caractere envenena a trava global de dados | 1 | ✅ provada |
| `percent-da-web-fatia-texto-por-byte` | o %XX da porta web em pânico com caractere de vários bytes, sem login | 1 | ✅ provada |
| `mapa-do-cluster-envenenado-vira-vazio` | o mapa de pulsos envenenado devolvido vazio: a eleição trava | 1 | ✅ provada |
| `lista-do-cluster-envenenada-volta-ao-arranque` | a lista viva de nós envenenada respondida pelo config.json do arranque | 1 | ✅ provada |
| `smtp-linha-sem-teto` | o cliente SMTP lê a linha do relé com `read_line` cru, sem teto de tamanho | 3 | ✅ provada |
| `leitura-fora-do-canal-na-web` | a porta HTTP le a linha do pedido por `read_line` cru, fora do `Canal` (o caso que fundou a lei, pedido 434) | 1 | ✅ provada |
| `leitura-fora-do-canal-na-web-pela-catraca` | a leitura crua da linha do pedido HTTP volta e a catraca `TETO_LEITURA_FORA_DO_CANAL` nao sobe | 1 | ✅ provada |
| `leitura-fora-do-canal-no-aperto-do-odbc` | o driver ODBC le a resposta do aperto por `read_line` cru, fora do `Canal` (o terceiro irmao do 312) | 1 | ✅ provada |
| `leitura-fora-do-canal-no-aperto-do-odbc-pela-catraca` | a leitura crua do aperto do ODBC volta e a catraca `TETO_LEITURA_FORA_DO_CANAL` nao sobe | 1 | ✅ provada |
| `conferidor-de-segredos-cala-por-engano` | o conferidor de segredos varre a arvore e nao acusa nada, nem a chave plantada | 1 | ✅ provada |
| `conferidor-de-temporarios-cala-por-engano` | o conferidor dos temporarios deixa de casar o padrao e diz `ok 0` com o `temp_dir` cru na arvore | 1 | ✅ provada |
| `conferidor-de-vermelhas-cala-por-engano` | o conferidor das provas vermelhas deixa de reconhecer o `#[ignore]` da vermelha e a catraca fica verde com qualquer uma solta | 1 | ✅ provada |
| `conferidor-de-inventario-ve-tudo-por-engano` | o conferidor do inventario de extensoes acha toda extensao em qualquer figura e nunca acusa a copia que perdeu uma | 1 | ✅ provada |
| `conferidor-de-grades-cala-por-engano` | o conferidor de grades deixa de ver o `<table` cru e so conta o ajudante | 2 | ✅ provada |
| `conferidor-de-botoes-cala-por-engano` | o conferidor de botoes deixa de ver o `<button` e so conta o `role=button` | 1 | ✅ provada |
| `conferidor-de-texto-cru-cala-por-engano` | o conferidor do texto cru deixa de achar o `${txt(` sem `esc` e a catraca fica verde | 2 | ✅ provada |
| `teto-decidido-antes-do-bloqueio` | o teto da linha é decidido antes de a leitura bloquear, e o usuário excluído enquanto esperava manda 1 MiB | 3 | ✅ provada |
| `teto-decidido-sem-refrescar-a-ficha` | o teto é perguntado na hora certa, mas com a ficha da sessão que nunca se refrescou | 2 | ✅ provada |
| `teto-refrescado-antes-do-bloqueio` | a ficha é refrescada antes de a leitura bloquear, e o excluído enquanto esperava continua com 128 MiB | 2 | ✅ provada |
| `linha-residente-depois-da-resposta` | a linha já respondida fica residente enquanto a conexão espera a próxima | 1 | ✅ provada |
| `hexadecimal-ecoa-o-valor` | o erro do hexadecimal inválido devolve o valor recebido inteiro | 1 | ✅ provada |
| `citar-sem-teto` | a citação do valor recebido numa mensagem de erro perde o teto, e os irmãos voltam a ecoar | 3 | ✅ provada |
| `json-recebido-ecoa-o-valor` | a recusa de tipo do `inserir` devolve o JSON recebido inteiro, pelo fio e pelo `acessos.log` | 1 | ✅ provada |
| `phxzip-bomba-do-lzma2` | o pedaço de LZMA2 que anuncia 2 MiB é decodificado inteiro antes de se saber que não cabe no teto | 1 | ✅ provada |
| `phxzip-distancia-antes-da-janela` | a distância de um casamento LZMA lida do arquivo sem conferir contra o que já saiu | 1 | ✅ provada |
| `phxzip-zip-slip` | entrada com `..` no nome extraída fora da pasta de destino (zip-slip) | 1 | ✅ provada |
| `phxzip-crc-do-cifrado` | byte trocado no dado cifrado relatado como «senha errada» porque o CRC do cifrado não foi conferido | 1 | ✅ provada |
| `phxzip-crc-do-conteudo` | senha errada devolvendo lixo como se fosse o arquivo, porque o CRC do conteúdo não foi conferido | 1 | ✅ provada |
| `phxzip-teto-do-declarado` | conteúdo acima do teto de quem chama é descompactado inteiro em vez de recusado pelo tamanho declarado | 1 | ✅ provada |
| `drop-grava-o-ndx-rasgado` | o `Drop` do `.ndx` grava a árvore rasgada por um pânico no meio da escrita e baixa o byte 52: a tabela volta limpa e errada | 6 | ✅ provada |
| `marca-do-ndx-sobe-depois-do-reg` | o byte 52 do `.ndx` só sobe na primeira página suja, depois de o `.reg` já ter gravado a linha: a queda no meio volta limpa | 3 | ✅ provada |
| `drop-do-ndx-decide-por-panicking` | o `Drop` do `.ndx` decide por `thread::panicking()`: o pânico capturado e o `Drop` depois gravam a árvore rasgada como limpa | 1 | ✅ provada |
| `drop-do-ndx-decide-por-panicking-pela-abi` | pela ABI de C, o punho envenenado por um pânico no meio da escrita, ao ser fechado, grava o índice rasgado como limpo | 1 | ✅ provada |
| `sincronizar-limpa-o-ndx-aberto-sujo` | o `sincronizar` de um `.ndx` que abriu sujo grava o byte 52 em 0 sem reconstruir: a escrita do descritor que caiu fica fora do índice | 1 | ✅ provada |
| `reindexar-sem-janela-grava-o-ndx-vazio` | um pânico no meio do `reindexar` grava o `.ndx` recém-recriado VAZIO e marcado limpo: a tabela inteira fica fora do índice | 1 | ✅ provada |
| `janela-do-ndx-interrompe-em-toda-recusa` | «tabela cheia» fecha a janela do `.ndx` como interrompida: uma recusa comum passa a exigir `reparar indice` | 1 | ✅ provada |
| `dblink-cifra-selo-ignorado` | com a chave mestra disponível, o `dblink.json` recebe a senha e o token em claro | 1 | ✅ provada |
| `dblink-cifra-envelope-sem-nome` | o envelope da ligação A colado na linha da ligação B abre, e a B apresenta a senha de outro banco | 1 | ✅ provada |
| `dblink-cifra-sem-prova` | a chave mestra errada abre o cadastro, e a ligação salva em seguida sai selada com ela | 1 | ✅ provada |
| `dblink-cifra-chave-ausente-derruba` | sem a chave mestra, o cadastro recusa abrir e o servidor inteiro não sobe | 3 | ✅ provada |
| `dblink-cifra-formato-2-sem-chave` | o `dblink.json` de hoje, sem chave declarada, ganha `formato: 2` ao ser regravado | 1 | ✅ provada |
| `dblink-cifra-chave-dentro-da-pasta` | a chave mestra num arquivo dentro da pasta do banco é aceita, e viaja na mesma cópia que o cadastro | 1 | ✅ provada |
| `dblink-cifra-rebaixa-calado` | sem a chave, a credencial nova vai em texto puro para dentro do cadastro cifrado | 3 | ✅ provada |
| `dblink-cifra-perde-envelope-trancado` | salvar outra ligação sem a chave apaga o envelope da trancada, e a credencial some para sempre | 2 | ✅ provada |
| `dblink-cifra-lista-na-chave-legada` | o formato 2 deixa a lista em `"dblink"`, e o binário anterior a lê e apaga os envelopes na primeira gravação | 1 | ✅ provada |
| `dblink-cifra-link-seguido-de-ponto-ponto` | a chave mestra em `fora/link/../chave.hex` passa pela conferência e o kernel a abre dentro da pasta do banco | 1 | ✅ provada |
| `dblink-cifra-le-caminho-diferente-do-conferido` | o diretório da chave trocado por um link depois do arranque leva a leitura para dentro da pasta do banco | 1 | ✅ provada |
| `dblink-cifra-chave-pronta-sem-subchave` | dois cadastros com a mesma chave pronta cifram com a mesma chave e repetem o par (chave, nonce) da prova | 1 | ✅ provada |
| `dblink-cifra-envelope-entrega-o-tamanho` | o envelope cifrado tem o tamanho exato da credencial, e o arquivo entrega quanto mede cada senha | 1 | ✅ provada |
| `dblink-cifra-iteracoes-sem-teto` | o `dblink.json` escolhe as iterações do PBKDF2, e `u32::MAX` segura o arranque por ~99 minutos | 1 | ✅ provada |
| `dblink-cifra-piso-do-cofre` | o cadastro aceita 10.000 iterações, e cada tentativa contra a prova sai 21 vezes mais barata que o padrão | 1 | ✅ provada |
| `dblink-cifra-declaracao-torta-some` | `cifra_do_dblink` escrita torta vira «não declarada», e o cadastro fica em claro sem recusa nenhuma | 1 | ✅ provada |
| `phxzip-ciclos-do-arquivo` | o 7zAES de um arquivo hostil pede 2^24 rodadas e o padrão deriva inteiro já no abrir | 1 | ✅ provada |
| `phxzip-derivacoes-por-abertura` | um sal diferente em cada bloco fura o cache e cobra uma derivação inteira por bloco | 1 | ✅ provada |
| `phxzip-contagem-sem-teto` | a contagem de entradas do cabeçalho comprimido dimensiona vetores pelo que o arquivo declara | 1 | ✅ provada |
| `phxzip-cabecalho-plano-sem-teto` | o cabeçalho gravado em claro é analisado inteiro mesmo acima de `Limites::cabecalho` | 1 | ✅ provada |
| `phxzip-nome-repetido-na-leitura` | duas entradas com o mesmo nome: o extrator grava a segunda por cima da primeira, calado | 1 | ✅ provada |
| `panico-sob-a-trava-sem-reparo` | um pânico com a trava global de dados na mão a envenena para sempre: toda conexão recebe «a trava suja» até reiniciar | 4 | ✅ provada |
| `trava-de-dados-recupera-sem-reparar` | a trava de dados envenenada volta a atender sem reparo nenhum (a H2 ingênua): o disco rasgado e a cópia em RAM servidos como se nada tivesse havido | 4 | ✅ provada |
| `reparo-da-trava-sem-o-piso` | o reparo da trava que falha deixa o processo de pé, servindo de estado incerto, em vez de abortar | 2 | ✅ provada |
| `reparo-da-trava-sem-as-marcas-orfas` | o reparo da trava não completa a marca em voo: o COMMIT que morreu na passada sai pela metade, com as travas da transação já soltas | 2 | ✅ provada |
| `reparo-varre-todas-as-marcas` | o reparo da trava completa marca que não é do pânico: reaplica um `atualizar` velho por cima da gravação mais nova | 2 | ✅ provada |
| `reparo-da-trava-deixa-o-residente` | o reparo da trava deixa a cópia residente de pé: a memória serve a tabela atrás do disco depois do pânico | 1 | ✅ provada |
| `fecho-drena-as-sujas-antes-do-fsync` | o fecho da janela esvazia a lista das tabelas sujas antes de sincronizar: um pânico no meio apaga a marca de commit cujo dado não foi ao disco | 1 | ✅ provada |
| `panico-em-thread-de-servico-morre-calado` | o pânico com a trava na mão numa thread de serviço é reparado e a thread morre calada: a janela de gravação para de fechar sozinha | 1 | ✅ provada |
| `reparo-no-desenrolar-de-panico-de-fora` | o `AoSair` de um pânico FORA da trava a toma no desenrolar, e o reparo roda (e pode abortar) por um pânico que nunca tocou em dado | 1 | ✅ provada |
| `reparo-ignora-a-operacao-impossivel` | a marca em voo com operação impossível fica no disco e o processo segue de pé, com as travas da transação soltas | 1 | ✅ provada |
| `reparo-apaga-a-marca-gravada-que-nao-se-rele` | a marca em voo JÁ GRAVADA que não se relê no reparo sai do disco como «não confere»: a transação confirmada fica pela metade, ou sem bilhete para o arranque | 1 | ✅ provada |
| `reparo-com-panico-engolido` | um `catch_unwind` em volta do reparo engole o pânico duplo: a trava fica fechada com o processo de pé | 1 | ✅ provada |
| `commit-sem-pre-conferencia` | o COMMIT confere a chave estrangeira so na passada, depois da marca, e grava a parte da frente | 9 | ✅ provada |
| `sobreposicao-acha-pela-chave-velha` | o buscar da sobreposicao acha pela chave velha a linha do disco que o prefixo alterou | 1 | ✅ provada |
| `mae-viva-lida-por-baixo-da-sobreposicao` | a conferencia de «mae viva» le o disco por baixo da marca pendente | 1 | ✅ provada |
| `indice-da-sobreposicao-parado` | o indice das chaves pendentes fica no retrato da primeira busca | 1 | ✅ provada |
| `passada-replaneja-a-cascata` | a passada replaneja a cascata depois da marca, e a lista valida sai pela metade | 2 | ✅ provada |
| `prefixo-copia-a-sobreposicao` | o plano da cascata abre a filha com uma COPIA da sobreposicao dela | 1 | ✅ provada |
| `sobreposicao-guarda-a-linha-crua` | a sobreposicao guarda a linha crua do empilhar, e nao a que o store vai gravar | 4 | ✅ provada |
| `nulo-colide-no-unico` | o segundo NULL num indice unico cai em DUPLICADO | 1 | ✅ provada |
| `nulo-colide-no-unico-do-commit` | o COMMIT com o segundo NULL num indice unico sai pela metade | 1 | ✅ provada |
| `fsync-recusado-repete-no-diario` | o `fsync` recusado de um volume é repetido e responde Ok: o `Volumes` devolvia a lista ao registro «para o fecho tentar de novo» | 1 | ✅ provada |
| `fsync-recusado-repete-no-indice` | o `.ndx` cujo `fsync` foi recusado responde Ok no fecho seguinte, pela porta da árvore que não presta | 1 | ✅ provada |
| `drop-baixa-o-byte-52-depois-do-fsync-recusado` | depois de um `fsync` recusado no diretório, o `.ndx` sai do `Drop` dizendo que presta — até o 522 gravando o 0, desde o 522 atestando para a reabertura | 2 | ✅ provada |
| `pagina-que-o-disco-recusou-sai-das-sujas` | a página do `.ndx` que o disco cheio recusou sai da lista de sujas antes de ser gravada, e o segundo fecho baixa o byte 52 sobre ela | 2 | ✅ provada |
| `pagina-despejada-que-o-disco-recusou-some` | a página suja despejada do cache que o disco recusou some: nem no arquivo, nem na RAM | 1 | ✅ provada |
| `servidor-segue-de-pe-depois-do-fsync-recusado` | o servidor segue de pé depois de um `fsync` recusado, gravando num disco que já se sabe que mente | 1 | ✅ provada |
| `completar-engole-a-tabela-que-nao-foi-ao-disco` | a recuperação engole o erro do `sincronizar` e apaga a marca de um commit cujo dado não foi ao disco | 1 | ✅ provada |
| `fk-antes-do-default` | a chave estrangeira confere a linha crua, e o DEFAULT sem mãe grava a filha órfã | 5 | ✅ provada |
| `fk-antes-do-default-pelo-servidor` | o DEFAULT e a calculada sem mãe gravam a órfã pelo servidor, fora e dentro da transação | 2 | ✅ provada |
| `cascata-sobre-calculada-na-declaracao` | a chave sobre coluna calculada é declarada em cascata, e a filha fica órfã quando a mãe troca de chave | 1 | ✅ provada |
| `cascata-confere-a-filha-crua` | a cascata confere a filha crua, e a mãe fica gravada quando a linha final da filha recusa | 2 | ✅ provada |
| `literal-no-erro-do-sql` | o erro de sintaxe do SQL cita o literal do pedido («e veio '123.456.789-00'») | 1 | ✅ provada |
| `literal-no-erro-da-expressao` | o erro da expressão cita o literal do pedido na janela, no «sobrou» e no «esperava» | 2 | ✅ provada |
| `texto-sem-fechar-no-acessos-log` | o `texto sem fechar` da expressão cita o pedido inteiro, e o `acessos.log` grava o dado em claro | 1 | ✅ provada |
| `senha-sobra-no-erro-do-cadastro` | a recusa do `CREATE USER` cita o que sobrou — e numa senha de aspas não dobradas o que sobra é um pedaço dela | 1 | ✅ provada |
| `senha-fora-de-aspas-simples-no-perfil` | o `sem_a_senha` tapa só o literal de aspas simples: `PASSWORD "x"`, `PASSWORD x` e `PASSWORD 123` saem em claro no `perfil.txt` | 1 | ✅ provada |
| `aspas-duplas-no-erro-de-sintaxe` | `VALUES (2, "123.456.789-00")`, o texto do jeito do MySQL, volta citado no erro e vai ao `acessos.log` | 1 | ✅ provada |
| `duracao-citada-sem-teto` | a recusa da duração cita o texto recebido inteiro: `BEGIN TRANSACTION TIMEOUT '<1 MiB>'` soma um megabyte ao `acessos.log` | 1 | ✅ provada |
| `portao-da-senha-por-espaco` | o portão da redação da senha lê palavras separadas por espaço: `/* odbc */ CREATE USER`, `ALTER ROLE … PASSWORD` e `SET PASSWORD FOR` levam a senha em claro ao `perfil.txt` e ao `jobs.json` | 1 | ✅ provada |
| `senha-depois-de-identified` | a redação só olha `PASSWORD`: `IDENTIFIED BY "x"`, a forma do MySQL e do MariaDB, sai em claro no perfil | 1 | ✅ provada |
| `parametros-irmaos-da-senha` | o Profiler tapa o `?` do `ALTER USER c PASSWORD ?` e grava o `parametros` irmão com a senha em claro | 1 | ✅ provada |
| `portao-da-senha-pelos-simbolos` | o portão da redação lê símbolos e o perfil e o job guardam bytes: a linha comentada, o `/*!…*/` e o literal que carrega a senha passam em claro | 1 | ✅ provada |
| `palavra-que-contem-a-senha` | `MASTER_PASSWORD=x` e `SOURCE_PASSWORD="x"`: só a palavra exata abre a redação, e a senha sai em claro no perfil | 2 | ✅ provada |
| `eco-do-sql-com-a-senha` | o roteiro com a senha numa linha comentada roda, e a resposta da op `sql` ecoa o texto inteiro no campo `sql` | 1 | ✅ provada |
| `jobs-json-antigo-derruba-o-arranque` | a guarda de credencial roda tambem ao LER o `jobs.json`, e o job legitimo salvo antes dela derruba o arranque | 1 | ✅ provada |
| `job-recusado-roda-mesmo-assim` | o job que voltou do disco com credencial sobe e RODA -- pela agenda, pela tela, ou religado | 1 | ✅ provada |
| `ficha-do-job-devolve-a-senha-do-disco` | o job aceito no arranque com a senha no pedido a devolve na ficha da op `jobs` | 1 | ✅ provada |
| `pbkdf2-normaliza-a-chave-a-cada-iteracao` | PBKDF2 resume a senha longa a cada iteração: o custo do login cresce com o tamanho dela | 1 | ✅ provada |
| `conferir-sem-o-teto-da-senha` | o `conferir` roda o PBKDF2 com senha acima do teto | 1 | ✅ provada |
| `fachada-do-login-com-mil-iteracoes` | o login de quem não existe paga 2.000 iterações contra as 210.000 de quem existe | 1 | ✅ provada |
| `inativo-pula-o-pbkdf2` | o login de quem está inativo responde sem PBKDF2 nenhum | 1 | ✅ provada |
| `prova-de-quem-nao-existe-sai-sem-conferir` | o login por desafio-resposta de quem não existe, ou está inativo, sai sem conferir a prova | 1 | ✅ provada |
| `login-sem-o-teto-da-senha` | o login recebe senha acima do teto e recusa como «credencial inválida» | 1 | ✅ provada |
| `criar-usuario-sem-o-teto-da-senha` | `usuario_criar`, `usuario_alterar` e `CREATE USER` derivam o hash de senha acima do teto | 1 | ✅ provada |
| `pulso-que-morre-fica-marcado` | a thread de pulso que morre em panico nao se desmarca do `pulsando` | 1 | ✅ provada |
| `pulso-em-panico-sem-recuo` | o pulso que entra em panico a cada volta vira laco de panico | 1 | ✅ provada |
| `relogio-de-jobs-morto-diz-que-esta-no-ar` | o relogio de jobs que morre continua marcado como no ar | 1 | ✅ provada |
| `amostrador-morto-diz-que-esta-no-ar` | o amostrador que morre continua marcado como no ar no retrato | 1 | ✅ provada |
| `job-corre-na-thread-de-servico` | o job em panico com a trava na mao derruba o servidor | 1 | ✅ provada |
| `backup-corre-na-thread-de-servico` | o backup agendado em panico com a trava na mao derruba o servidor | 1 | ✅ provada |
| `job-que-derrubou-roda-de-novo-no-arranque` | a corrida de job que derrubou o processo roda de novo no arranque | 1 | ✅ provada |
| `backup-que-derrubou-roda-de-novo-no-arranque` | o backup que derrubou o processo roda de novo no arranque | 1 | ✅ provada |
| `lapide-do-futuro-empurra-o-job` | a lapide de job com hora no futuro vira a ultima corrida sem teto | 1 | ✅ provada |
| `lapide-do-futuro-empurra-o-backup` | a lapide do backup com hora no futuro vira a ultima corrida sem teto | 1 | ✅ provada |
| `corrida-interrompida-nao-avisa` | a corrida de job fechada no arranque como FALHOU nao avisa por e-mail | 1 | ✅ provada |
| `backup-agendado-falha-calado` | o backup agendado que falha so escreve no erro padrao | 1 | ✅ provada |
| `dblink-ilegivel-derruba-o-motor` | o cadastro do DbLink ilegivel derruba o motor inteiro | 1 | ✅ provada |
| `dblink-que-nao-se-le-abre-vazio` | o dblink.json que existe e nao se le vira cadastro vazio | 1 | ✅ provada |
| `jobs-ilegivel-derruba-o-motor` | o cadastro de jobs ilegivel derruba o motor inteiro | 1 | ✅ provada |
| `core-leva-a-senha-do-cofre` | o core do abort leva a senha do cofre para o disco | 1 | ✅ provada |
| `catalogo-so-declara-token-nao-token-remoto` | o catálogo de `replicacao_testar` não declara `token_remoto`, o campo que a sonda lê primeiro | 1 | ✅ provada |
| `esquema-vaza-o-histograma-da-particao` | `op_esquema` publica `baldes[].registros` mesmo com a coluna da partição negada ao usuário | 2 | ✅ provada |
| `dblink-mysql-sem-teto-de-colunas` | o DbLink MySQL(R) reserva `Vec::with_capacity` do número de colunas que o PAR manda, sem teto | 1 | ✅ provada |
| `dblink-mysql-sem-teto-do-quadro-acumulado` | `ler_quadro` do DbLink MySQL(R) junta continuações de 16 MB sem teto sobre o total | 1 | ✅ provada |
| `smtp-sem-teto-de-linhas-de-continuacao` | o cliente SMTP aceita QUALQUER número de linhas de continuação (`250-...`), sem teto | 1 | ✅ provada |
| `por-login-para-no-primeiro-que-casa` | `Cadastro::por_login` é um `find`: quem não existe custa muito mais que o primeiro da lista | 1 | ✅ provada |
| `fechar-baixa-o-byte-52-sem-fsync` | o `fechar` grava o byte 52 em 0 sem `fsync`: o núcleo guarda o cabeçalho limpo e perde as páginas | 3 | ✅ provada |
| `atestado-sobrevive-a-escrita` | o atestado do processo sobrevive à escrita que não terminou: a reabertura confia na árvore de antes dela | 1 | ✅ provada |
| `atestado-pelo-caminho-e-nao-pelo-arquivo` | o atestado do processo vale para o caminho, e não para o arquivo: outro `.ndx` no mesmo lugar abre confiado | 1 | ✅ provada |
| `atestado-de-antes-da-recusa-vale-depois` | o atestado que o `fechar` deu ANTES de um `fsync` recusado no diretório continua valendo depois dele | 1 | ✅ provada |
| `fts-fora-do-fecho-da-janela` | o `.fts` fica fora do fecho da janela: nenhum `fsync` o alcança, e o byte 52 dele só desce sem `fsync` | 1 | ✅ provada |
| `reindexar-deixa-o-punho-velho-gravar` | o `reindexar` deixa o punho velho gravar páginas e cabeçalho por cima do `.ndx` recém-truncado | 1 | ✅ provada |
| `restauracao-nao-reconstroi-o-marcado` | a restauração de backup devolve a tabela com o `.ndx` marcado, e ela recusa toda escrita até alguém mandar `reindexar` | 1 | ✅ provada |
| `arranque-nao-reconstroi-o-marcado` | o arranque não reconstrói o `.ndx` que o processo anterior só fechou: a tabela sobe recusando até alguém mandar `reindexar` | 1 | ✅ provada |
| `atestado-fica-no-caminho-velho` | renomear, duplicar ou colar uma tabela escrita desde o último fecho deixa o destino recusando tudo, sem queda nenhuma | 1 | ✅ provada |
| `renomear-esquece-o-atestado` | o renomear move os arquivos e deixa o atestado no nome velho: a tabela renomeada recusa tudo | 1 | ✅ provada |
| `fechar-do-embutido-nao-sincroniza` | o embutido que fecha a tabela sem `phx_sincronizar` não a abre no processo seguinte, e a ABI não tem como reconstruí-la | 1 | ✅ provada |
| `phx-reindexar-nao-reindexa` | o `phx_reindexar` responde Ok sem reconstruir: o índice que a queda marcou continua recusando pela ABI | 1 | ✅ provada |
| `auto-referencia-pulada-no-excluir` | excluir o chefe que tem subordinado na MESMA tabela responde Ok, e o subordinado fica órfão | 1 | ✅ provada |
| `auto-referencia-pulada-no-excluir-pelo-servidor` | o chefe com subordinado sai pelo servidor, e na transação `[inserir 11->10, excluir 10]` confirma | 2 | ✅ provada |
| `auto-laco-conta-como-filha` | a linha que aponta só para si mesma é contada como filha dela, e nunca mais sai | 1 | ✅ provada |
| `renomear-pula-a-auto-referencia` | renomear a tabela que aponta para si mesma deixa a chave no nome velho, e o chefe com subordinado passa a sair | 1 | ✅ provada |
| `marca-do-disco-no-empilhar` | dentro da transação, alterar a linha excluída suave a ressuscita; fora, ela continua excluída | 1 | ✅ provada |
| `upsert-solto-ressuscita-a-excluida` | o upsert fora de transação ressuscita a linha excluída suave; o mesmo upsert dentro a mantém excluída | 1 | ✅ provada |
| `mescla-do-upsert-sobre-o-disco` | o upsert com SET dentro da transação mescla sobre a linha do disco, e a excluída na lista ressuscita | 1 | ✅ provada |
| `elo-do-empilhar-pelo-disco` | o elo que o `empilhar` planeja pelo disco sobrescreve o que a própria lista já escreveu na filha | 1 | ✅ provada |
| `tabela-que-some-segura-as-marcas` | a tabela escrita na janela e excluida ou renomeada fica nas sujas pelo nome velho e segura todas as marcas de COMMIT | 1 | ✅ provada |
| `renomear-deixa-o-registro-no-nome-velho` | o renomear deixa o registro do que deve ao disco no nome velho, e a divida fica para sempre onde ninguem sincroniza | 1 | ✅ provada |
| `familia-partida-por-grafia` | a familia do `Volumes` se parte por symlink e `..`, e familia partida perde dado | 1 | ✅ provada |
| `inserir-sem-janela-do-texto` | o inserir deixa a linha viva fora da busca de texto num panico entre o `.reg` e o `indexar_texto` | 1 | ✅ provada |
| `atualizar-sem-janela-do-texto` | o atualizar deixa o texto novo fora da busca num panico entre o `.reg` e o `.fts` | 1 | ✅ provada |
| `excluir-sem-janela-do-texto` | o excluir de vez deixa a linha viva fora da busca num panico entre o texto e o slot | 1 | ✅ provada |
| `cascata-embutida-sem-pre-conferencia` | a cascata do embutido grava a mae antes de conferir a FK da filha para OUTRA mae | 1 | ✅ provada |
| `jobs-devolve-a-coluna-negada` | o `jobs` devolve o pedido salvo inteiro, com o valor da coluna negada que alguem digitou na definicao | 1 | ✅ provada |
| `cabecalho-do-ndx-rasgado-trava-a-tabela` | o cabecalho do `.ndx` rasgado impede a tabela de abrir, e nem o arranque nem o `reindexar` o refazem | 1 | ✅ provada |
| `diretiva-sigilosa-sai-crua-no-sql` | `ALTER SERVER SET <campo sigiloso> = x` sai cru no perfil: a redacao do SQL so conhecia `PASSWORD` | 1 | ✅ provada |
| `diretiva-sigilosa-sai-crua-no-json` | o `valor` do `diretiva_gravar` e a chave do `config_gravar` com caminho sigiloso saem crus no perfil | 1 | ✅ provada |
| `sal-falso-pelo-token` | o sal falso do `desafio` sai do token que todo cliente tem, e quem o tem sabe quem nao existe | 1 | ✅ provada |
| `scram-sem-teto-de-iteracoes` | o `i=` do SCRAM que o par manda nao tem teto, e cada iteracao e CPU deste processo | 1 | ✅ provada |
| `recado-de-trava-entrega-o-login` | o recado de trava mostra o login do dono dela a quem esbarrou, que pode nem ter direito na tabela | 1 | ✅ provada |
| `smtp-ecoa-a-credencial` | o erro do SMTP traz o texto do rele, e o rele que ecoa a credencial poe o base64 da senha no log | 1 | ✅ provada |
| `arranque-reconstroi-calado` | o arranque reconstroi indice marcado e so diz no `stderr`: quem opera nao fica sabendo da queda | 1 | ✅ provada |
| `reconstruir-fts-sem-janela` | o panico no meio do `reconstruir_fts` grava o indice de texto pela metade marcado limpo | 1 | ✅ provada |
| `carimbo-da-a-volta-no-teto` | o rowstamp empurrado ao teto por evento replicado da a volta, e o filho nasce com carimbo menor que o pai | 1 | ✅ provada |
| `upsert-solto-sem-trava-da-linha` | o upsert solto altera a linha que uma transacao segura, e o COMMIT dela apaga a escrita | 1 | ✅ provada |
| `cascata-solta-sem-trava-da-filha` | a cascata solta grava a filha que uma transacao segura, por cima do X dela | 1 | ✅ provada |
| `cascata-solta-sem-pre-conferencia` | a cascata solta grava a mae antes de conferir a FK da filha para OUTRA mae, e deixa filhas orfas | 1 | ✅ provada |
| `elo-implicito-sem-trava` | o elo que só o COMMIT descobre escreve sem trava, e a leitura repetível de outra transação lê 5 e depois 6 | 1 | ✅ provada |
| `ciclo-de-commits-sem-desempate` | dois COMMITs cujos elos se barram são mandados repetir para sempre, e ninguém confirma | 2 | ✅ provada |
| `quem-cede-no-ciclo-segura-as-travas` | a transação que cede no ciclo de COMMITs volta ativa com as travas, e a mais velha continua barrada | 1 | ✅ provada |
| `aresta-velha-depois-do-savepoint` | a transação barrada volta ao SAVEPOINT e a aresta velha faz a outra ceder num ciclo que não existe mais | 1 | ✅ provada |
| `corrente-do-ciclo-atravessa-quem-nao-confirma` | a corrente do ciclo atravessa transação em ABORT_ONLY, e a outra cede por quem nunca mais vai confirmar | 1 | ✅ provada |
| `cascata-em-voo-ignorada-no-drop` | pânico entre duas filhas da cascata solta deixa as seguintes na chave velha, e a tabela delas não recusa | 2 | ✅ provada |
| `cascata-em-voo-so-no-aplicar` | pânico depois de a mãe ir ao disco e antes da primeira filha deixa as filhas na chave velha, calado | 2 | ✅ provada |
| `cascata-do-embutido-sem-marca` | a cascata do `ao_alterar` do embutido volta a rodar sem marca: a queda no meio deixa a filha na chave velha, e a abertura a cala | 2 | ✅ provada |
| `recusa-do-fsync-por-grafia` | a recusa do `fsync` casa pela GRAFIA do caminho: pelo symlink ou por `dir/../dir` o mesmo diretório sincroniza Ok e baixa o byte 52 | 2 | ✅ provada |
| `dblink-mysql-lenenc-embrulha` | o DbLink MySQL(R) entra em pânico com `0xFE` + `u64::MAX` num campo `lenenc` do par, e corta calado o campo maior que o pacote | 3 | ✅ provada |
| `dblink-pg-contagem-negativa` | o DbLink PostgreSQL(R) reserva `Vec::with_capacity` da contagem de campos `int16` do par: `-1` vira `usize::MAX` e pânico de `capacity overflow` | 1 | ✅ provada |
| `dblink-mysql-cadeia-alem-do-fim` | o aperto de mão do DbLink MySQL(R) entra em pânico com saudação curta ou troca de plugin sem NUL, antes da credencial | 2 | ✅ provada |
| `job-dispara-job` | um job cujo pedido é `job_rodar` sobe uma corrida aninhada por nível, sem teto: o job de si mesmo empilha threads até o processo cair | 1 | ✅ provada |
| `smtp-sem-prazo-total-da-conversa` | o `timeout_s` do cliente SMTP mede o silêncio e não a conversa: um relé que pingue abaixo do prazo segura a thread de aviso pelo tempo que quiser | 1 | ✅ provada |
| `fts-nasce-com-permissao-aberta` | o `.fts` nasce `644` -- legivel por todo usuario da maquina | 2 | ✅ provada |
| `conferir-fk-afirma-indice-sao-quando-marcado` | a conferencia contra a MAE afirma "esta sao" com o indice marcado | 2 | ✅ provada |
| `procura-das-filhas-afirma-indice-sao-quando-marcado` | a procura pelas filhas afirma "esta sao" com o indice marcado | 1 | ✅ provada |
| `backup-sem-fsync` | o backup responde "concluido" sem `fsync` nenhum | 2 | ✅ provada |
| `backup-fsync-derruba-o-servidor` | o `fsync` recusado no DESTINO DE UM BACKUP derruba o servidor inteiro | 1 | ✅ provada |
| `backup-recusa-envenena-a-raiz` | a recusa do `fsync` no destino do backup marca a raiz de dados, e todo COMMIT seguinte recusa | 1 | ✅ provada |
| `backup-recusa-para-o-commit` | pelo soquete: depois de um backup com `fsync` recusado no destino, o `inserir` seguinte erra | 1 | ✅ provada |
| `backup-destino-que-contem-a-raiz` | o backup em arvore aceita destino igual, acima ou (por link) dentro da raiz de dados | 1 | 🟰 redundante |
| `diff-null-na-chave-apaga-linha-irma` | o `diff` com NULL repetido no indice some com linhas do relatorio | 2 | ✅ provada |
| `recusa-de-coluna-marcada-cita-o-valor` | A recusa de conversão cita o valor curto de coluna marcada como dado pessoal | 4 | ✅ provada |
| `dblink-empurra-valor-pela-regua-de-nome` | O DbLink empurra valor de texto pela régua de NOME de objeto | 2 | ✅ provada |
| `dblink-puxar-cita-a-celula-remota` | O DbLink, ao puxar, cita na recusa a célula do outro banco | 2 | ✅ provada |
| `dblink-puxar-apara-o-texto` | O DbLink, ao puxar, apara o texto e troca o vazio por nulo | 1 | ✅ provada |
| `dblink-empurra-upsert-de-mysql-no-postgres` | O DbLink empurra para o PostgreSQL com o upsert do MySQL | 3 | ✅ provada |
| `dblink-empurra-booleano-como-numero` | O DbLink empurra o booleano como 1/0 | 1 | ✅ provada |
| `dblink-puxar-le-booleano-pela-carga-colada` | O DbLink, ao puxar, lê o booleano pela régua da carga colada | 1 | ✅ provada |
| `dblink-puxar-le-blob-cru` | O DbLink, ao puxar, lê o BLOB cru como se fosse hexadecimal | 3 | ✅ provada |
| `dblink-puxar-inventa-uuid` | O DbLink, ao puxar, troca a célula «novo» por um uuid aleatório | 1 | ✅ provada |
| `dblink-tela-mostra-blob-com-perda` | O DbLink mostra na tela o BLOB remoto pelo leitor com perda | 2 | ✅ provada |
| `dblink-colacao-bin-vira-hex` | O DbLink mostra em hexadecimal o texto de uma colação _bin | 2 | ✅ provada |
| `dblink-espelho-bin-pela-bandeira` | O espelho do DbLink cria Bin a coluna de texto em colação _bin | 1 | ✅ provada |
| `dblink-bit-lido-como-hex-decimal` | O DbLink puxa o BIT do MySQL em hexadecimal e o grava como decimal | 2 | ✅ provada |
| `faixa-do-slot-cita-coluna-marcada` | A faixa do tipo, conferida no slot, cita o número de coluna marcada | 1 | ✅ provada |
| `carga-colada-converte-sem-a-coluna` | A carga colada converte a célula sem a marca da coluna | 1 | ✅ provada |
| `upsert-converte-sem-a-coluna` | O `atualizar` do upsert converte o valor sem a marca da coluna | 1 | ✅ provada |
| `sql-vai-ao-perfil-com-o-literal` | O `sql` vai ao `perfil.txt` com o literal dentro | 3 | ✅ provada |
| `sql-vai-ao-perfil-com-o-literal-pelo-soquete` | O `INSERT` em SQL da tabela marcada vai ao `perfil.txt` com o valor, visto pelo soquete | 1 | ✅ provada |
| `erro-do-sql-normalizado-vai-ao-arquivo` | O `sql` normalizado leva ao arquivo o erro que cita o literal | 1 | ✅ provada |
| `normaliza-o-que-nao-e-sql` | O Profiler normaliza pelo NOME do campo, e a carga colada vira lixo de léxico | 1 | ✅ provada |
| `transacoes-recuperadas-sem-sanear` | O registro das transações volta do pânico sem sanear, e o COMMIT seguinte confirma o que ele não afirma | 1 | ✅ provada |
| `transacoes-envenenadas-recusam-toda-conexao` | Um pânico com as transações na mão mata toda transação de toda conexão até reiniciar | 1 | ✅ provada |
| `trava-suja-sem-nome` | O `SP000010` da trava suja sai com a MESMA frase em 85 pontos de 14 travas | 1 | ✅ provada |
| `esvaziar-lixeira-fora-do-ops-do-no` | A réplica somente-leitura não esvazia o próprio `.trash`, e a linha apagada no source fica nela para sempre | 1 | ✅ provada |
| `ops-do-no-fora-do-ops-escrita` | `esvaziar_lixeira` e `expurgar_trilha` fora do `OPS_ESCRITA`: rodam dentro de BEGIN sem voltar no ROLLBACK e passam por cima da trava de outra transação | 2 | ✅ provada |
| `normalizado-deixa-o-booleano-cru` | O `sql` normalizado deixa `TRUE`, `FALSE` e `NULL` crus no `perfil.txt` | 1 | ✅ provada |
| `sql-vai-ao-perfil-com-o-literal-na-bateria-do-497` | O `sql` vai ao `perfil.txt` com o literal, visto pela bateria do 497 nas duas portas | 1 | ✅ provada |
| `expurgar-trilha-fora-do-ops-do-no` | A réplica somente-leitura não expurga a própria trilha `.lgpd` | 2 | ✅ provada |
| `veneno-dito-uma-vez-por-trava` | O segundo pânico com as transações na mão passa calado e sem saneamento | 1 | ✅ provada |
| `commit-ignora-o-prazo` | o COMMIT depois do prazo da transação grava a lista inteira | 1 | ✅ provada |
| `old-do-before-update-pelo-disco` | dentro da transação o OLD do BEFORE UPDATE é a linha do disco, e o delta de estoque sai -4 onde é -2 | 1 | ✅ provada |
| `old-do-upsert-pelo-disco` | o upsert que vira alteração na transação dá ao BEFORE UPDATE o OLD do disco | 1 | ✅ provada |
| `old-do-before-delete-pelo-disco` | dentro da transação o BEFORE DELETE vê a linha do disco, e a nascida na transação nem dispara | 1 | ✅ provada |
| `elo-do-empilhar-sem-trava-de-linha` | o elo que o empilhar planeja não trava a linha da filha, e a escrita de outra conexão nela passa | 1 | ✅ provada |
| `elo-do-empilhar-regrava-a-linha-inteira` | o COMMIT regrava a filha inteira que o empilhar viu, e desfaz a cascata solta de outra mãe dela | 1 | ✅ provada |
| `cascata-solta-sem-marca` | a alteração solta que cascateia grava sem marca, e a queda no meio deixa filha na chave velha | 4 | ✅ provada |
| `cascata-solta-pela-marca-do-embutido` | a alteração solta que cascateia volta ao `Table::atualizar`: a marca do store não se completa no reparo da trava | 3 | ✅ provada |
| `upsert-solto-cascateia-sem-marca` | o upsert solto que vira alteração com cascata grava pelo `atualizar` de dentro dele, sem marca | 1 | ✅ provada |
| `cascata-solta-com-o-punho-de-quem-chama-sujo` | a cascata solta abre o punho da passada com o `t` de quem chama ainda sujo, e o `Drop` dele desfaz o índice da mãe | 1 | ✅ provada |
| `descida-do-punho-sem-o-fts` | a descida do punho de quem chama leva o `.ndx` e esquece o `.fts`: a busca de texto da mãe acha o nome velho | 1 | ✅ provada |
| `varredura-encerra-quem-confirma` | a varredura do prazo encerra a transação que está no COMMIT e solta as travas de quem ainda grava | 1 | ✅ provada |
| `devolver-desfaz-o-abort-only` | a lista devolvida ao fim de um COMMIT recusado desfaz o ABORT_ONLY que chegou no meio | 1 | ✅ provada |
| `subida-do-byte-52-sem-fsync` | a SUBIDA do byte 52 volta a ir só ao cache do núcleo: numa queda de energia o disco guarda o `.reg` novo sob o 0 do último fecho, e o pai com filhas se apaga calado | 2 | ✅ provada |
| `subida-do-byte-52-sincroniza-a-cada-pagina` | a subida do byte 52 sincroniza a cada página suja, e não só na passagem de 0 para 1: um `fdatasync` no laço quente de toda escrita | 1 | ✅ provada |
| `arquivo-do-banco-nasce-aberto` | os arquivos do banco voltam a nascer na permissão do `umask`: `.reg`, `.ndx`, `.log`, `.lgpd`… `644`, legíveis por todo usuário da máquina | 1 | ✅ provada |
| `diretorio-do-banco-nasce-aberto` | a raiz, o database, o palco da restauração e o destino do backup voltam a nascer `755` | 2 | ✅ provada |
| `arquivo-refeito-herda-o-modo-velho` | o arquivo que o banco REFAZ por cima de um antigo -- o `.ndx` e o `.fts` do `reindexar` -- herda o `644` dele | 1 | ✅ provada |
| `copia-do-backup-nasce-aberta` | a cópia do backup volta a nascer `644` -- até a do `.lgpd`, que nasceu `600` | 1 | ✅ provada |
| `base-antiga-sem-alerta` | a base antiga, `644` em `755`, deixa de ser apontada: o motor não aperta o que existe e ninguém avisa | 1 | ✅ provada |
| `arranque-nao-alerta-a-base-antiga` | o `phxsqld` sobe numa base `644`/`755` sem dizer nada | 1 | ✅ provada |
| `backup-atravessa-link-plantado` | o motor da permissão volta a seguir o link simbólico no último nome: um link plantado no destino do backup faz o `.reg` ser gravado NA vítima de fora, e ela vira 0600 | 3 | ✅ provada |
| `base-por-link-cala-o-alerta` | o alerta da base antiga cala quando `config.base` é um link simbólico | 1 | ✅ provada |
| `arranque-cala-o-alerta-da-base-por-link` | o `phxsqld` sobe numa base `644`/`755` alcançada por link simbólico sem dizer nada | 1 | ✅ provada |
| `ndx-novo-sobe-com-o-diretorio-vazio` | o primeiro cabeçalho durável de um `.ndx` novo leva o byte 52 em 1 e ZERO índices: a queda no meio do `reindexar` trava a tabela | 1 | ✅ provada |
| `migracao-do-separador-decide-pelo-nome` | a migracao do separador de volume le `vendas_2024.reg` como volume 2024 de `vendas` e some com a tabela | 1 | ✅ provada |
| `marca-do-separador-antes-dos-renomes` | a marca do formato de volume vai ao disco antes dos `rename`s, e a queda no meio deixa o diretorio marcado e meio migrado | 1 | ✅ provada |
| `painel-com-copia-do-analisador-de-volume` | o painel soma os bytes do `.reg` por uma copia do nome do volume e mede zero em tabela de 4 digitos ou por letra | 1 | ✅ provada |
| `carga-adiada-solta-sem-reconstruir` | o `bulkinsert(false)` da carga com o índice adiado solta a reserva com a árvore suspensa | 1 | ✅ provada |
| `suspensao-do-indice-so-na-ram` | a suspensão do `.ndx` para a carga adiada fica só na memória, e a queda no meio deixa a árvore vazia se declarando limpa | 2 | ✅ provada |
| `carga-adiada-orfa-sem-reconstruir` | a carga adiada que sai sem o `bulkinsert(false)` (conexão caída, reserva vencida) deixa o índice suspenso até o próximo arranque | 1 | ✅ provada |
| `diario-que-falha-sem-marca-do-evento-devido` | o `.log` que falha depois de a linha estar no `.reg` não deixa a marca do evento devido, e a abertura não sabe o que completar | 3 | ✅ provada |
| `abertura-nao-completa-o-evento-devido` | a abertura da tabela acha a marca do evento devido e não completa o `.log` pela linha | 3 | ✅ provada |
| `diario-que-falha-nao-derruba-o-servidor` | o servidor segue de pé depois de o `.log` falhar com a linha já no `.reg` — linha sem diário servindo | 1 | ✅ provada |
| `disco-cheio-deixa-a-sentinela-do-509` | o disco cheio que derruba pelo `.log` grava a sentinela do `fsync` recusado, e o servidor não sobe no mesmo boot | 1 | ✅ provada |
| `exclusao-de-vez-sem-conferir-o-teto-do-diario` | no teto do diário, a exclusão de vez tira a linha do `.reg` e só então o `.log` recusa | 1 | ✅ provada |
| `exclusao-de-vez-motivo-que-falha-pula-o-diario` | na exclusão de vez, o `.reason` que falha com o slot já livre devolve o erro antes do `.log` — a linha some sem evento | 1 | ✅ provada |
| `insercao-fts-que-falha-pula-o-diario` | na inclusão, o `.fts` que falha com a linha já no `.reg` devolve o erro antes do `.log` — a linha fica sem evento | 1 | ✅ provada |
| `zip-que-falha-no-rename-deixa-o-part` | o `rename` final do backup em ZIP que recusa deixa o `.part` na pasta para sempre | 1 | ✅ provada |
| `backup-em-pasta-que-falha-deixa-as-copias` | o backup em PASTA cujo manifesto recusa deixa as cópias na pasta sem `backup.json` para sempre | 1 | ✅ provada |
| `backup-reaproveitado-que-falha-deixa-o-manifesto-velho` | o backup em pasta REAPROVEITADA que falha deixa o `backup.json` velho descrevendo cópias que já mudaram | 1 | ✅ provada |
| `zip-que-falha-deixa-a-pasta-que-criou` | o backup em ZIP que falha deixa vazia a pasta que ele mesmo criou | 1 | ✅ provada |
| `backup-fsync-reabre-a-copia` | o `fsync` da cópia do backup cai num descritor REABERTO, e não no de quem escreveu | 1 | ✅ provada |
| `backup-copia-fecha-o-descritor-antes-do-fsync` | a cópia do backup fecha o descritor na escrita, sob a trava, e o inode fica livre para sair da memória antes do `fsync` | 2 | ✅ provada |
| `zip-fsync-reabre-o-part` | o `fsync` do `.part` do backup em ZIP cai num descritor REABERTO, e não no de quem escreveu | 1 | ✅ provada |
| `backup-manifesto-novo-sem-fsync-da-pasta` | o manifesto novo do backup nasce sem o `fsync` da pasta de onde o `backup.json` velho saiu | 1 | ✅ provada |
| `zip-rename-que-recusa-deixa-a-pasta` | o `rename` final do backup em ZIP que recusa deixa vazia a pasta que a corrida criou | 1 | ✅ provada |
| `cascata-dispara-after-do-elo-so-no-commit` | a mesma cascata do `ao_alterar` dispara o AFTER da filha no COMMIT e não na alteração solta | 1 | ✅ provada |
| `dblink-troca-o-host-e-herda-a-senha` | trocar o host de uma ligação do DbLink sem mandar a senha herda a guardada, e ela sai para o destino novo | 2 | ✅ provada |
| `dblink-no-fio-com-a-trava-de-dados` | `dblink_ligar` e `dblink_sincronizar` vão ao fio com a trava de dados global na mão: um par que goteja abaixo do prazo por leitura prende todo pedido de todo cliente | 1 | ✅ provada |
| `dblink-sem-prazo-total` | Os três clientes do DbLink (mysql, pg e phx) só têm prazo por LEITURA: um par que goteja um byte antes de cada prazo prende a thread do job ou da conexão para sempre | 1 | ✅ provada |
| `dblink-sem-teto-de-bytes` | O resultado do DbLink só tem teto de LINHAS: o par decide quanto pesa cada uma (até 128 MiB no MySQL, 64 MiB no PostgreSQL) e o servidor guarda gigabytes | 2 | ✅ provada |
| `dblink-max-mib-sem-leitor` | O `max_mib` da ligação do DbLink aparece no arquivo e na tela e nenhum cliente o lê: o teto de bytes fica o de fábrica, diga a ligação o que disser | 1 | ✅ provada |
| `replica-sem-prazo-total` | O laço da réplica, a sonda e o console só têm prazo por LEITURA: um par que goteja um byte antes de cada prazo prende a thread para sempre | 1 | ✅ provada |
| `porta-lida-pela-metade` | O apoio dos testes lia a porta do phxsqld antes de a linha acabar: o eprintln! sai em várias escritas, e o parse do endereço pela metade dava AddrParseError (ou a porta errada) | 2 | ✅ provada |
| `copia-da-troca-sem-fsync` | A cópia de reserva da troca no restaurar (o caminho sem rename) apagava a origem sem fsync da cópia: uma queda no meio deixava a única via de volta pela metade | 1 | ✅ provada |
| `replica-limite-sem-recuo` | O estouro do prazo total da réplica caía em `Outra`: o par que goteja era retentado no intervalo fixo, sem recuo | 2 | ✅ provada |
| `cluster-replica-sem-recuo` | O laço da réplica do CLUSTER retentava a cada pulso sem o `Ritmo`: sem recuo nem para rede nem para limite | 1 | ✅ provada |
| `odbc-sem-prazo-total` | O driver ODBC só tinha prazo por LEITURA: um servidor que goteja um byte antes de cada prazo prendia a thread do aplicativo dentro do SQLExecDirect | 1 | ✅ provada |
| `odbc-total-pela-vida-da-conexao` | O prazo total do driver ODBC contado pela vida da conexão, e não por pedido: o aplicativo que abre de manhã e consulta à tarde cairia no primeiro pedido depois do total | 1 | ✅ provada |
| `copia-de-tabela-sem-fsync` | `duplicar_tabela` e `copiar_tabela_para` respondiam «ok» com a cópia só no cache do núcleo: uma queda podia levar a tabela nova, ou deixá-la rasgada | 1 | ✅ provada |
| `copia-de-tabela-sem-fsync-da-pasta` | A cópia de tabela sincronizava os arquivos e não a pasta: o nome novo podia sumir numa queda depois do «ok» | 1 | ✅ provada |
| `colar-em-schema-novo-sem-fsync-do-database` | Colar num schema que ainda não existe criava a pasta dele sem `fsync` do database: a cópia sincronizada podia morar numa pasta que a queda leva | 1 | ✅ provada |
| `porta-anunciada-em-pedacos` | A linha «porta de dados escutando em …» saía em várias escritas: quem lia o log no meio via a porta pela metade | 1 | ✅ provada |
| `criar-tabela-sem-fsync-dos-arquivos` | `criar_tabela` respondia «criada» com o `.reg`, o `.ndx` e os outros arquivos só no cache do núcleo: numa queda a tabela podia sumir ou voltar sem o esquema | 1 | ✅ provada |
| `garantir-schema-sem-fsync-do-database` | `criar_schema` e `criar_tabela` num schema novo criavam a pasta sem `fsync` do database: o schema que o cliente ouviu criar podia sumir numa queda | 2 | ✅ provada |
| `criar-database-sem-fsync-da-base` | `criar_database` criava a pasta sem `fsync` da base: o database que o cliente ouviu criar podia sumir numa queda | 1 | ✅ provada |
| `marca-do-database-sem-fsync` | O marcador `_database.json` nascia sem `fsync`: numa queda uma colmeia voltava como database padrão, calada | 1 | ✅ provada |
| `excluir-tabela-sem-fsync-da-pasta` | `excluir_tabela` respondia «excluída» com os `unlink` só no cache do núcleo: numa queda a tabela voltava, inteira ou pela metade | 1 | ✅ provada |
| `esvaziar-lixeira-sem-fsync-da-pasta` | `esvaziar_lixeira` apagava os volumes do `.trash` sem `fsync` da pasta: numa queda o dado apagado de vez voltava, com o `.reason` dizendo que saiu | 1 | ✅ provada |
| `expurgo-da-trilha-sem-fsync-da-pasta` | A fase 3 do expurgo da trilha apagava os volumes do `.lgpd` sem `fsync` da pasta: numa queda o volume vencido voltava, com o rastro selado dizendo que saiu | 1 | ✅ provada |
| `levar-ao-disco-esquece-o-que-saiu` | O `levar_ao_disco` sincronizava a pasta do que nasceu e esquecia a do que saiu: as três exclusões respondiam antes do disco | 1 | ✅ provada |
| `backup-atravessa-link-na-pasta-do-meio` | o backup volta a criar e atravessar as pastas do destino pelo NOME: um link numa pasta do meio (`copias/loja -> dados/rh`) grava a cópia por cima da tabela viva de outro database | 1 | ✅ provada |
| `backup-escreve-no-arquivo-de-outro-dono` | o backup volta a truncar e reescrever o arquivo regular de OUTRO dono (ou com link físico) que já está no destino: quem plantou fica dono da cópia do banco, e no ZIP o `.part` plantado vira o `.zip` final | 1 | ✅ provada |
| `fifo-trocada-na-janela-para-o-backup` | o motor da permissão volta a abrir pelo nome seguindo link e esperando leitor: trocar o nome por um link para FIFO entre o `lstat` e o `open` para o backup com a trava de dados na mão | 1 | ✅ provada |
| `copia-reaberta-pelo-nome-no-fsync` | a cópia além do teto de descritores volta a reabrir pelo NOME para o `fsync`: trocada por um link, o `fsync` cai noutro arquivo e o manifesto diz «pronto» sobre a cópia que nunca sincronizou | 1 | ✅ provada |
| `fsync-da-pasta-do-backup-pelo-nome` | o `fsync` da pasta do backup reabre pelo NOME fora da trava: trocada por um link, sincroniza a pasta do outro lado e a nossa nunca | 1 | ✅ provada |
| `faxina-do-backup-remove-pasta-pelo-nome` | a faxina do backup que falhou remove a pasta criada pelo NOME real: um link numa pasta do meio faz apagar a pasta vazia de outro | 2 | ✅ provada |
| `faxina-do-backup-sem-conferir-o-inode` | a faxina do backup remove pelo descritor da mãe mas não confere o inode: a pasta vazia de outro que entrou no nome da nossa sai | 1 | ✅ provada |
| `estado-do-cluster-sem-troca-duravel` | O estado do cluster gravava por `write` no lugar, sem `fsync`: o arquivo perdido ou vazio numa queda fazia o master rebaixado voltar mandando | 2 | ✅ provada |
| `estado-do-cluster-ilegivel-vira-config` | O estado do cluster presente e ilegível valia como ausente: o `source` rebaixado com o arquivo truncado subia master na época 0, aceitando escrita | 1 | ✅ provada |
| `promover-libera-antes-de-gravar` | O `promover` liberava a escrita ANTES de gravar o papel: a gravação que falhava deixava um master escrevendo que o disco não conhecia | 1 | ✅ provada |
| `posicao-bidi-antes-do-dado` | A posição do bidirecional ia ao disco a cada lote, antes do `fsync` do dado: numa queda, os eventos entre o dado perdido e a posição gravada nunca mais eram pedidos | 1 | ✅ provada |
| `posicao-bidi-sem-troca-duravel` | A posição do bidirecional gravava por `write` no lugar: mesmo depois do dado, a queda podia devolver o arquivo antigo ou nenhum | 1 | ✅ provada |
| `cadastro-regravado-sem-fsync` | `gatilhos.json`, `procedimentos.json` e `visoes.json` eram regravados no lugar e sem `fsync`: a queda no meio deixava JSON pela metade, e o arranque caía | 1 | ✅ provada |
| `cadastro-apagado-sem-fsync-da-pasta` | o último gatilho, procedimento ou visão que saía apagava o arquivo sem `fsync` da pasta: numa queda o excluído voltava | 1 | ✅ provada |
| `gatilho-orfao-na-queda-do-excluir-tabela` | `excluir_tabela` levava ao disco o sumiço da tabela ANTES do `gatilhos.json`: a queda entre os dois deixava o gatilho de uma tabela que não existe mais | 1 | ✅ provada |
| `erro-no-meio-da-exclusao-sem-fsync` | o erro no meio do `excluir_tabela` esquecia os nomes que já tinham saído sem `fsync` da pasta: numa queda a tabela voltava pela metade | 1 | ✅ provada |
| `prova-do-gravar-privado-dentro-do-processo` | a prova do `gravar_privado` rodava no mesmo processo de um `Servidor::novo`: o gancho do 509 virava a recusa armada em SIGABRT, e ela só passava pela ordem alfabética | 1 | ✅ provada |
| `pular-engole-a-posicao` | o `replicacao_pular` respondia «pulou» por uma posição que não foi ao disco: um reinício devolvia o par ao evento descartado | 1 | ✅ provada |
| `registrar-engole-a-epoca-espelhada` | o `registrar` do cluster engolia a falha de gravar a época espelhada: o pulso respondia como se ela estivesse no disco | 1 | ✅ provada |
| `blacklist-regravada-no-lugar` | o `blacklist.json` era regravado no lugar e sem `fsync`: a queda no meio deixava JSON pela metade, e o arranque o recusa | 1 | ✅ provada |
| `esvaziar-esquece-no-erro` | o erro no meio do `esvaziar_lixeira` esquecia os volumes do `.trash` que já tinham saído sem `fsync` da pasta | 1 | ✅ provada |
| `expurgo-esquece-no-erro` | o erro no meio da fase 3 do expurgo da trilha esquecia os volumes do `.lgpd` que já tinham saído sem `fsync` da pasta | 1 | ✅ provada |
| `trilha-pagina-por-contagem` | A exportação da trilha paginava só por `pular`: um expurgo entre duas páginas fazia o auditor pular registro vivo sem aviso | 1 | ✅ provada |
| `rowid-revela-coluna-negada` | Com a coluna que particiona negada pelo direito, a primeira letra (ou o período) de cada linha saía pelo rowid, pelos baldes, pelo `slots` e pelo catálogo | 2 | ✅ provada |
| `conta-cita-numero-de-coluna-marcada` | A recusa da expressão citava número e booleano, e a conta que parte de coluna marcada e cai em coluna sem marca saía com o valor | 1 | ✅ provada |
| `externo-selado-gravado-como-anexo` | A réplica decidia pelo PRÓPRIO cofre se o externo marcado da imagem vinha selado: sem cofre gravava o texto cifrado como o anexo, calada; com a mesma senha acusava adulteração que não houve | 2 | ✅ provada |
| `replicar-manda-o-externo-selado` | O `replicar` mandava ao fio o externo marcado selado com a chave do `.reg` da origem: nenhuma réplica o abria, nem com a mesma senha, porque o sal é por arquivo | 1 | ✅ provada |
| `visoes-entrega-o-literal` | A op `visoes` pede só `ler` e devolvia o SQL da visão verbatim: o literal do `WHERE` e o comentário saíam para quem tinha a coluna negada | 1 | ✅ provada |
| `congelamento-sensivel-a-caixa` | a chave do congelamento distinguia caixa: em NTFS e APFS o `inserir` em `"Clientes"` gravava no volume vivo durante a FASE A | 2 | ✅ provada |
| `excluir-tabela-fura-o-congelamento` | `excluir_tabela` apagava os arquivos de uma tabela em reescrita: mexe no disco SEM abrir a tabela, e o portão do congelamento mora na abertura | 1 | ✅ provada |
| `renomear-tabela-fura-o-congelamento` | `renomear_tabela` movia os arquivos de uma tabela em reescrita, o irmão do `excluir_tabela` | 1 | ✅ provada |
| `conflito-do-retrato-publica-o-caminho` | a recusa da FASE B publicava ao cliente o caminho absoluto da raiz de dados do servidor | 1 | ✅ provada |
| `rodizio-do-acessos-nasce-desligado` | o `acessos.log` nascia sem rodízio: um anônimo escrevia 266 B de log por 2 B recebidos, sem teto | 1 | ✅ provada |
| `pulso-torto-uma-linha-por-envio` | cada pulso torto escrevia uma linha no stderr, que é o journal: quem tem a credencial do cluster afogava o «REBAIXANDO» no limite de taxa | 1 | ✅ provada |
| `pulso-com-o-id-deste-no-uma-linha-por-envio` | o pulso com o id DESTE nó escrevia uma linha no stderr por envio — o irmão do B2 no `op_cluster_pulso` | 1 | ✅ provada |
| `web-acima-do-teto-sem-rastro` | as três portas HTTP recusavam o pedido acima do teto sem linha no `acessos.log` — o irmão do 216 na web | 1 | ✅ provada |
| `operacao-anonima-fora-do-inventario` | o inventário das operações anônimas dizia «seis» quando eram dezesseis | 1 | ✅ provada |
| `politica-do-diario-fora-da-abertura` | a tabela aberta pelo `Database` volta a nascer sem a politica do diario: a recuperacao grava o COMMIT completado sem imagem | 2 | ✅ provada |
| `exclusao-fora-da-politica-do-diario` | a politica do diario volta a ligar so a imagem da linha: a exclusao fisica sai sem imagem, e no multi o par para | 3 | ✅ provada |
| `exclusao-replicada-sem-conferir-o-carimbo` | a exclusao replicada volta a apagar o rowid sem perguntar de quem e a linha: a de outra origem some com `Ok` | 1 | ✅ provada |
| `chave-anulavel-como-identidade-do-bidirecional` | o bidirecional volta a aceitar indice unico sobre coluna que aceita nulo como identidade: a linha de chave nula de um no apaga a do outro | 1 | ✅ provada |
| `apoio-engole-a-falha-do-bind` | o apoio dos testes subia o servidor com `let _ = escutar()` e esperava a porta ATENDER: com a porta tomada por um vizinho do mesmo binário, o teste conversava com o servidor do vizinho («database loja já existe») | 1 | ✅ provada |
| `tarefa-pela-listagem-do-proc` | o teste das threads do SO procurava a thread listando `/proc/self/task`: a listagem pula a thread viva quando a tarefa listada logo antes dela morre | 1 | ✅ provada |
| `contador-do-congelamento-relativo` | o teste do contador do congelamento exigia `antes + 2`: o vizinho congelado na leitura de `antes` que soltava no meio derrubava o teste sem defeito nenhum | 1 | ✅ provada |
| `drop-do-congelamento-esquece-o-contador` | o `Drop` do congelamento tirava a tabela do registro e esquecia o contador: o portão barato ficava caro para sempre, e o teste antigo não via | 2 | ✅ provada |
| `arbitro-engole-o-rebaixar` | o árbitro do cluster engolia a falha de gravar o rebaixamento (`let _ = estado.rebaixar(...)`): o nó voltava mandando num reinício, sem pista nenhuma | 2 | ✅ provada |
| `replica-atras-de-proxy-passa-pela-lista` | atrás do proxy declarado, `replicas_autorizadas` comparava o IP do PROXY e autorizava todo cliente que chegava por ele | 1 | ✅ provada |
| `trilha-em-claro-depois-do-cofre` | o ativo do `.lgpd` nascido em claro continuava recebendo registro em claro depois de o cofre ligar | 2 | ✅ provada |
| `trilha-ativo-vazio-em-claro` | com o cofre ligado, o ativo VAZIO do `.lgpd` em claro (o que um rodízio sem cofre deixa) recebia o primeiro registro em claro | 1 | ✅ provada |
| `pulso-deixa-de-provar-calado` | o nó que deixava de assinar o pulso para um par que já recebera prova dele não dizia nada (`campos_da_prova` com `.ok()?`) | 1 | ✅ provada |
| `marca-dagua-da-particao-negada` | com a coluna que particiona negada pelo direito, `verificar`, `migrar_esquema`, `acrescentar_coluna` e `memoria_carregar` devolviam a marca d'agua da tabela | 1 | ✅ provada |
| `recuperacao-do-embutido-sem-politica` | o embutido que replica completa a marca da queda com a politica do diario PADRAO, e o evento recuperado sai sem imagem | 2 | ✅ provada |
| `recuperacao-do-schema-sem-politica` | a recuperacao das marcas abre a pasta de cada schema como um `Database` novo, com a politica do diario padrao: o COMMIT completado ali sai sem imagem | 1 | ✅ provada |
| `check-novo-contra-a-linha-velha` | `acrescentar_coluna` com CHECK que linhas que ja existem violam e aceito, e a tabela fica com duas verdades | 2 | ✅ provada |
| `backup-copia-sob-a-exclusiva` | O backup copiava com a ficha EXCLUSIVA da trava de dados: a leitura parava a cópia inteira (100 GB = 50 a 64 min sem ler nada) | 1 | ✅ provada |
| `backup-sem-portao-do-retrato` | A cópia do backup com a ficha COMPARTILHADA e sem o portão do retrato: o primeiro escritor na fila do `RwLock` fazia toda leitura nova esperar a cópia inteira | 1 | ✅ provada |
| `faixa-do-config-nao-lida` | o `inicio` da faixa da `Sequence` não tinha porta de produção: todo servidor numerava na faixa 0 e vinte caixas com passo 20 colidiam 100% | 3 | ✅ provada |
| `faixa-sai-da-classe` | o contador da `Sequence` com faixa saía da própria classe na primeira inserção (`v + 1`), e a abertura seguinte recusava a tabela como se fosse de outro nó | 1 | ✅ provada |
| `faixa-nao-declarada-tranca-a-leitura` | a CLI e a FFI sem a faixa declarada recusavam ABRIR a tabela que outro nó numerou: nem `info`, nem `listar`, nem `verificar` por ferramenta oficial | 2 | ✅ provada |
| `faixa-nao-declarada-numera-na-zero` | com a leitura liberada, o processo sem faixa declarada numerava a tabela de outro nó na faixa 0 -- a colisão que a faixa existe para impedir, calada | 1 | ✅ provada |
| `faixa-da-cli-nao-chega-ao-motor` | a `--inicio-da-sequencia` da CLI era lida e não chegava ao motor: a ferramenta gravava como quem não declarou | 1 | ✅ provada |
| `faixa-da-ffi-nao-chega-ao-motor` | a `phx_definir_inicio_da_sequencia` devolvia PHX_OK sem declarar nada: o aplicativo achava que numerava na faixa dele | 1 | ✅ provada |
| `vetor-do-pulso-ignorado` | a posição POR TABELA do pulso não chegava ao painel: a soma escondia o nó em dia na tabela grande e cego na pequena | 1 | ✅ provada |
| `faixa-sem-saida` | a tabela gravada pelo contador `v + 1` não abria (a faixa recusa) e o remédio exigia abrir: ficava sem saída | 1 | ✅ provada |
| `reconciliar-fora-da-faixa` | o `reparar` reconciliava a `Sequence` com `maior + 1` cru: numa tabela com faixa o valor caía fora dela e o reparo virava erro | 2 | ✅ provada |
| `escopo-do-begin-sem-login` | só com o token, sem login, um `begin` com `scope` e `lock_mode:EXCLUSIVE` travava qualquer tabela, e a recusa «está no SCOPE e não existe» enumerava o catálogo | 1 | ✅ provada |
| `escopo-do-begin-sem-direito` | o `SCOPE` do `begin` travava tabela sem conferir o direito de quem pedia (`declarar_escopo` sem `pode_em`): o leitor de outra base travava `rh.salarios` | 1 | ✅ provada |
| `prazo-da-transacao-sem-teto` | o `timeout_ms` do `begin` não tinha teto: 10^12 ms abria uma transação de 31 anos | 1 | ✅ provada |
| `datarow-curta-do-postgres` | a `DataRow` do PostgreSQL com menos campos que a `RowDescription` passava pelo leitor e entrava em pânico na sincronia, com a trava de dados na mão | 1 | ✅ provada |
| `linha-remota-curta-na-sincronia` | `linha_remota_para_negocio` indexava a linha do par pela posição do cabeçalho (`remota[de]`): linha curta de qualquer motor era pânico, não recusa | 1 | ✅ provada |
| `dblink-ligar-grava-copia-velha` | o `dblink_ligar` gravava no fim a cópia da ligação lida antes da rede: a excluída no meio voltava com a senha antiga, e a troca de senha feita no meio era desfeita | 1 | ✅ provada |
| `troca-de-chave-vira-linha-nova` | No bidirecional, a alteração que troca a chave virava inserção nova do outro lado e a linha antiga ficava: a imagem só dizia o «depois» | 1 | ✅ provada |
| `fio-cifrado-perde-o-antes` | A imagem aberta para o fio numa tabela cifrada era remontada só até os externos, e a troca de chave perdia o «antes» só ali | 1 | ✅ provada |
| `composta-casa-pela-primeira-coluna` | A chave composta do bidirecional casando só pela primeira coluna: (1,2) e (1,3) caem na identidade de (1,1) | 1 | ✅ provada |
| `numero-de-origem-conferido-so-no-par` | O número de origem do bidirecional conferido só contra o próprio: dois caixas com o mesmo número entre si não eram vistos, e o central suprimia os eventos de um ao servir o outro | 1 | ✅ provada |
| `numero-de-origem-atribuido-ignorado` | O `numero_servidor` lido do config e ignorado na conta do número de origem: o caixa inocente continua no hash que colide | 1 | ✅ provada |
| `replica-renumera-o-buraco-do-source` | A réplica fiel e o PITR geravam o `rownum` deles: o buraco histórico do source (1,2,4) virava 1,2,3 na cópia, para sempre | 2 | ✅ provada |
| `bidirecional-honra-o-rownum-do-outro` | O bidirecional honrando o `rownum` do outro servidor: as duas fontes de numeração colidem no mesmo `.reg` | 1 | ✅ provada |
| `imagem-com-sobra-ignorada` | O decodificador da imagem ignorava calado os bytes que sobravam depois dos externos: um campo novo passaria despercebido por todo binário anterior | 1 | ✅ provada |
| `registro-de-numeros-ilegivel-vira-vazio` | O `replicacao-numeros.json` ilegível lido como vazio: a colisão que ele existe para recusar passaria e iria para dentro dos `.log` | 2 | ✅ provada |
| `numero-aceito-antes-do-disco` | O par novo de número de origem entrava na memória antes de o registro ir ao disco: com o disco recusando, a chamada seguinte o aceitava sem nunca ter gravado | 1 | ✅ provada |
| `manifesto-velho-apagado-pelo-nome` | o backup apaga o manifesto velho pelo NOME do destino: um link trocado no meio da corrida apaga o backup.json de OUTRO backup | 1 | ✅ provada |
| `destino-do-backup-conferido-so-pelo-nome` | o destino do backup é conferido pelo NOME e aberto depois pelo descritor: a troca de um link no meio põe as cópias dentro do database vivo | 1 | ✅ provada |
| `chave-sem-urandom-pela-mistura` | sem /dev/urandom (Windows), a chave efêmera do TLS e do Noise e a do autoassinado saem de SHA-256 de relógio, PID e endereço | 2 | ✅ provada |
| `dblink-phx-analisa-antes-de-pesar` | O teto de bytes do DbLink não valia para o motor `phxsql`: a linha de até 128 MiB do `Canal` virava árvore `Json` antes de ser pesada, e o `max_mib` só limitava a cópia | 1 | ✅ provada |
| `pg-autenticado-sem-scram` | O cliente PostgreSQL do DbLink aceitava `AuthenticationOk` sem SCRAM, com senha na ligação: quem respondesse no endereço dizia «pode entrar» sem conhecer a senha | 1 | ✅ provada |
| `replica-sem-cofre-grava-externo-marcado-em-claro` | A réplica SEM cofre gravava a coluna externa marcada em claro no disco: o 344 trocou o selado (lixo) pelo dado aberto, sem a palavra do dono | 1 | ✅ provada |
| `restauracao-recusa-como-replica-sem-cofre` | A restauração do PRÓPRIO diário passaria pela recusa da réplica sem cofre: o servidor sem cofre deixaria de restaurar toda tabela com anexo marcado, sem proteger um byte | 1 | ✅ provada |
| `replica-sem-cofre-grava-inline-marcado-em-claro` | A réplica SEM cofre recusava só a coluna EXTERNA marcada: a INLINE chegava aberta na imagem e pousava em claro no `.reg` | 2 | ✅ provada |
| `bidirecional-sem-cofre-inserir-marcado-em-claro` | O bidirecional sem cofre passava pelo `inserir_replicado` com o dado marcado de OUTRO servidor: a recusa do 613 morava só no `aplicar_evento` | 1 | ✅ provada |
| `bidirecional-sem-cofre-atualizar-marcado-em-claro` | O bidirecional sem cofre passava pelo `atualizar_replicado` com o dado marcado de OUTRO servidor: a recusa do 613 morava só no `aplicar_evento` | 1 | ✅ provada |
| `bidirecional-sem-cofre-excluir_de_vez-marcado-em-claro` | O bidirecional sem cofre passava pelo `excluir_de_vez_replicado` com o dado marcado de OUTRO servidor: a recusa do 613 morava só no `aplicar_evento` | 1 | ✅ provada |
| `linhagem-nao-cunhada-na-declaracao` | a tabela declarada nasce sem linhagem: duas origens com historias diferentes ficam indistinguiveis e o carimbo empatado de dois servidores recem-nascidos apaga a linha errada | 1 | ✅ provada |
| `alter-perde-a-linhagem` | acrescentar coluna devolve o esquema sem linhagem: depois do primeiro ALTER a replica deixa de conferir a historia da tabela | 1 | ✅ provada |
| `copia-leva-a-linhagem-da-origem` | a copia de tabela (duplicar e colar) leva a linhagem da origem byte a byte: duas tabelas de historias diferentes passam pela conferencia como a mesma | 1 | ✅ provada |
| `replica-fiel-sem-conferir-a-linhagem` | a replica fiel abre a tabela daqui sem conferir a linhagem do source: tabela de outra historia recebe os eventos no rowid de outra linha | 1 | ✅ provada |
| `aplicar-sem-conferir-a-linhagem` | o `aplicar` ignora a linhagem que veio no pedido: a exclusao de uma caixa recem-nascida apaga a linha de outra com o carimbo empatado | 1 | ✅ provada |
| `teto-de-colunas-sem-o-bit-do-selo` | o esquema aceita ate 65.535 colunas: a coluna 32.768 externa e lida na imagem como a 0, selada | 1 | ✅ provada |
| `evento-pre-344-ao-fio-sem-abrir` | o evento do diario gravado antes do 344 (externo selado, sem o bit) sai para o fio como veio: a replica grava o cifrado como se fosse o anexo | 1 | ✅ provada |
| `portao-da-carga-le-um-campo-so` | O portão da carga (Portão 4) lia só `"tabela"`: a tabela reservada pelo `BULKINSERT` se lia como o lado B de um `juntar` | 2 | ✅ provada |
| `bidi-absorve-o-diario-sob-a-exclusiva` | A primeira rodada do bidirecional depois do arranque absorvia o diário local inteiro com a trava exclusiva na mão | 1 | ✅ provada |
| `bidi-absorve-o-diario-sob-a-exclusiva-pelo-soquete` | A primeira rodada do bidirecional depois do arranque absorvia o diário local inteiro com a trava exclusiva na mão — a prova pelo soquete, com o escritor de cliente gravando | 1 | ✅ provada |
| `pre-absorcao-fura-a-fila-do-escritor` | A absorção do bidirecional retomava a trava de leitura entre as fatias antes de o escritor acordado entrar, e o escritor esperava dezenas de fatias | 1 | ✅ provada |
| `leitor-que-cede-volta-na-hora` | O leitor que cede a vez voltava sem esperar o escritor da fila pegar a ficha exclusiva | 1 | ✅ provada |
| `fatia-com-o-prazo-vencido-nao-anda` | A fatia da absorção que chegava com o prazo já vencido saía sem lote nenhum, e a pré-absorção entregava o resto à trava exclusiva | 1 | ✅ provada |
| `diario-sob-a-compartilhada-recusa-a-cauda` | A leitura do diário sob a ficha compartilhada recusava a tabela escrita desde o último fecho da janela, e a absorção do bidirecional voltava inteira para a exclusiva | 1 | ✅ provada |
| `bidi-rodada-seguinte-sem-a-marca-do-diario` | Cada rodada do bidirecional com um evento local novo caminhava o diário desde o começo do volume para lê-lo | 1 | ✅ provada |
| `posicao-do-cluster-conta-tabela-que-nao-replica` | A posição somada do cluster contava tabela que não é replicada, e o nó com dado local ganhava a eleição | 2 | ✅ provada |
| `ledger-marcado-recebido-calado` | A réplica criava a cadeia de ledger com coluna marcada sem gritar nem contar | 1 | ✅ provada |
| `censo-do-ledger-le-a-forma-e-nao-a-marca` | O censo do ledger achava a cadeia pela forma e não lia o byte de marca: a cadeia marcada saía limpa | 1 | ✅ provada |
| `indice-da-chave-nao-nasce-no-criar-tabela` | A chave conferida nascia no criar_tabela sem o índice da filha, e a mãe perdia todo excluir | 1 | ✅ provada |
| `indice-da-chave-nao-nasce-no-declarar-fk` | A chave declarada numa filha que já existe não ganhava o índice, e a mãe perdia todo excluir | 1 | ✅ provada |
| `fts-orfao-reaproveitado-na-redeclaracao` | Redeclarar o índice de texto reaproveitava o .fts órfão, e a busca achava menos que a varredura | 1 | ✅ provada |
| `fts-orfao-na-lista-vazia` | Redeclarar o índice de texto como lista vazia deixava o .fts órfão no disco | 1 | ✅ provada |
| `fts-montado-pela-declaracao-velha` | A redeclaração do índice de texto montava o .fts novo pela declaração velha | 1 | ✅ provada |
| `mapa-de-toques-sem-teto` | O mapa de toques do bidirecional crescia uma entrada por chave distinta, sem teto, o processo inteiro | 1 | ✅ provada |
| `toque-esquecido-decide-as-cegas` | Chave esquecida pelo teto decidia «vence» abaixo do piso, e a escrita velha de lá apagava a nova daqui calada | 1 | ✅ provada |
| `master-conta-tabela-negada-ao-cluster` | O master somava na posição do cluster a tabela que o usuário do cluster não pode replicar | 1 | ✅ provada |
| `replica-grava-filha-sem-mae-calada` | A réplica gravava a filha sem a mãe e nada contava: o invariante «só existe filho se o pai existir» caía calado | 2 | ✅ provada |
| `bidi-grava-filha-sem-mae-calada` | O bidirecional gravava a filha sem a mãe calado, enquanto a réplica fiel já contava | 1 | ✅ provada |
| `escrita-local-na-replica-calada` | A réplica aceitava escrita local calada, e a ruptura que ela causava culpava o source | 1 | ✅ provada |
| `novo-orfao-sobrevive-ao-excluir-tabela` | Excluir e renomear a tabela deixavam para trás os *.novo de uma reescrita interrompida | 3 | ✅ provada |
| `fts-ao-lado-sobrevive-a-abertura` | O .fts.novo de uma redeclaração morta ficava no disco até a próxima redeclaração | 1 | ✅ provada |
| `redeclarar-texto-com-so-criar` | Redeclarar o índice de texto pedia só criar, e copia o .reg inteiro como o acrescentar_coluna | 1 | ✅ provada |
| `criacao-sem-reserva-605` | A tabela recém-criada atendia um terceiro antes do fsync da pasta de quem a criou | 1 | ✅ provada |
| `abrir-nao-espera-a-tabela-que-nasce-605` | Abrir uma tabela não esperava a que ainda nascia — só o campo «tabela» do servidor esperava | 1 | ✅ provada |
| `copia-nasce-sem-reserva-605` | A cópia de tabela, irmã da criação, nascia sem reserva e atendia um terceiro antes do fsync | 1 | ✅ provada |
| `terceiro-espera-dentro-da-trava-605` | Quem achava a tabela nascendo esperava com a trava global na mão e parava o servidor inteiro | 1 | ✅ provada |
| `marca-do-diario-de-outra-vida-620` | A marca do diário de outra vida da tabela era aceita e a varredura pulava os eventos da vida nova | 1 | ✅ provada |
| `mapa-de-toques-de-outra-vida-620` | O mapa de toques do bidirecional não zerava com a tabela recriada ou restaurada: o remoto mais velho sobrescrevia a escrita local nova | 3 | ✅ provada |
| `replica-culpa-o-source-pela-contagem-626` | A réplica fiel culpava o source («apagada e recriada») pelo ramo da contagem mesmo quando a causa era escrita local | 1 | ✅ provada |
| `copia-leva-volumes-de-duas-versoes` | A cópia de tabela no meio de uma troca decidida levava volumes de duas versões | 3 | ✅ provada |
| `sobra-da-fase-a-fica-sem-dono` | Os *.novo do .reg de uma fase A morta ficavam no disco enquanto a tabela vivesse | 2 | ✅ provada |
| `sobra-sem-paginacao-nao-se-varre` | A tabela sem paginação não tinha os *.novo varridos, e a sobra dela ficava | 1 | ✅ provada |
| `novo-com-dono-apagado-pelo-vizinho` | A abertura gravável apagaria o *.novo de uma troca ainda viva | 1 | ✅ provada |
| `fase-b-troca-meio-conjunto` | A fase B trocava o conjunto pela metade quando um *.novo tinha sumido | 1 | ✅ provada |
| `calculada-acrescentada-nula-na-linha-velha` | `acrescentar_coluna` com `calculada` deixa a linha velha NULA, e `SUM` conta metade da tabela sem dizer | 1 | ✅ provada |
| `calculada-le-o-envelope-do-externo-selado` | a calculada acrescentada que fala de um `.memo` selado calcula sobre o ENVELOPE cifrado, e nao sobre o texto | 1 | ✅ provada |
| `busca-reversa-rele-as-irmas-a-cada-exclusao` | a busca reversa da integridade relia o `.reg` de cada irma a cada exclusao, mesmo sem nada ter mudado | 1 | ✅ provada |
| `carimbo-da-irma-sem-os-tempos` | o carimbo que valida o esquema lembrado de uma irma ignora `mtime`/`ctime`, e a chave declarada no lugar passa despercebida: o pai com filha sai | 1 | ✅ provada |
| `carimbo-recente-lembrado` | o esquema da irma se lembra com carimbo RECENTE, e duas mudancas no mesmo tique grosso do nucleo deixam o mesmo carimbo | 1 | ✅ provada |
| `calculada-copia-a-marcada-em-claro` | a calculada que cita coluna marcada nasce SEM marca, e o preenchimento grava o texto do cofre em claro no `.reg` | 1 | ✅ provada |
| `calculada-cita-coluna-negada-na-declaracao` | `acrescentar_coluna` com calculada (ou CHECK) que cita coluna negada ao usuario e aceito, e a coluna negada passa a ser lida por outro nome | 1 | ✅ provada |
| `calculada-derivada-de-negada-se-le` | a calculada que o dono declarou sobre coluna negada sai na leitura de quem nao le a coluna | 1 | ✅ provada |
| `recusa-da-calculada-marcada-diz-a-linha` | a recusa da calculada sobre coluna marcada nomeia a linha velha, e vira oraculo por rowid sobre o dado pessoal | 1 | ✅ provada |
| `espera-de-fora-so-le-o-campo-tabela-629` | A espera da tabela que nasce, fora da trava, lia só o campo «tabela» e mandava o juntar esperar com a trava global na mão | 1 | ✅ provada |
| `espera-de-dentro-sem-prazo-629` | A espera da tabela que nasce DENTRO da trava global não tinha prazo: um fsync lento de pasta parava o servidor inteiro | 1 | ✅ provada |
| `irma-que-nao-abre-some-do-excluir` | na busca reversa do `excluir`, a irma cujo `.reg` nao abre fica de fora, e a mae com filha ilegivel morre | 1 | ✅ provada |
| `irma-que-nao-abre-some-do-excluir-tabela` | o `excluir_tabela` (e o renomear) pula a irma que nao abre, e a tabela mae some com a filha ilegivel apontando para ela | 1 | ✅ provada |
| `irma-que-nao-abre-some-do-ao-alterar` | a cascata do `ao_alterar` pula a irma que nao abre, e a mae muda a chave deixando a filha ilegivel apontando para a chave velha | 1 | ✅ provada |
| `irma-em-troca-vira-recusa-eterna` | a busca reversa abre a irma sem curar, e a irma em troca interrompida passa a trancar toda exclusao do diretorio | 1 | ✅ provada |
| `escrita-local-contada-antes-do-portao-3` | a escrita local na replica fiel se conta no portao 2b, antes da permissao e da abertura da tabela: memoria sem teto e diagnostico envenenado | 1 | ✅ provada |
| `escrita-local-pelo-sql-nao-conta` | o `executar_derivado` chama o direito por coluna sem a conta da escrita local, e o `INSERT` pelo SQL numa replica fiel volta a ser calado | 1 | ✅ provada |
| `regravar-esquema-troca-volume-a-volume-632` | a regravacao de esquema de uma fase so escreve e troca volume a volume, e a queda no meio do *.novo do volume 2 destroi o volume | 1 | ✅ provada |
| `troca-decidida-renomeia-novo-incompleto-632` | a abertura termina a troca decidida com um *.novo que nao tem todos os slots do volume velho | 1 | ✅ provada |
| `ordem-de-digitacao-reaproveita-slot` | o `.reg` reaproveita o slot da linha excluída, e a linha nova entra no meio da ordem de digitação | 1 | ✅ provada |
| `ao-excluir-aceita-cascata` | a declaração da chave aceita `ao_excluir` em cascata, e o pai com filhos passa a poder morrer | 1 | ✅ provada |
| `chave-declarada-nasce-sem-conferir` | a chave declarada sem `verificar` volta a nascer sem conferir, e o órfão entra calado | 1 | ✅ provada |
| `chave-sem-saida-para-nao-conferir` | o `verificar: false` escrito deixa de valer, e quem escolheu não conferir perde a opção junto com o padrão | 1 | ✅ provada |
| `carimbo-por-tabela-empata-pai-e-filha` | o `rowstamp` sai de um contador por tabela, e o pai e a filha nascem com o mesmo carimbo | 1 | ✅ provada |
| `versao-imposta-ao-cliente-antigo` | a guarda de conflito passa a exigir `versao`, e todo cliente antigo para de gravar | 1 | ✅ provada |
| `quinta-operacao-na-ficha-compartilhada` | o `ler` entra na pista de leitura sem a varredura de escrita escondida, e a catraca da ficha compartilhada tem de acusar | 1 | ✅ provada |
| `operacao-cancelavel-fora-da-lista` | uma operação com ponto de cancelamento fica fora de `OPS_CANCELAVEIS`, e a tela mostra o botão desabilitado | 1 | ✅ provada |
| `indice-da-chave-imposto-a-quem-nao-confere` | a chave com `verificar: false` ganha índice na filha, e a guarda nova passa a ser imposta | 1 | ✅ provada |
| `retrato-da-fase-a-nao-ve-volume-que-nasce-427` | o retrato da FASE A fotografa so os volumes que existem, e o volume que nasce no meio dela fica na geometria velha | 2 | ✅ provada |
| `retrato-da-fase-a-sem-selo-634` | o retrato da FASE A guarda o `mtime` real, e a atualizacao no mesmo tique passa e e desfeita pela troca | 3 | ✅ provada |
| `selo-do-retrato-nao-devolve-o-mtime-634` | a troca abortada deixa o volume com o `mtime` de 1980 | 1 | ✅ provada |
| `segundo-gravador-sem-trava-de-instancia-635` | dois processos abrem a mesma pasta para gravar e um sobrescreve os contadores do outro | 3 | ✅ provada |
| `raiz-ociosa-solta-a-trava-de-instancia-635` | o servidor ocioso, entre dois pedidos, deixa a CLI gravar a pasta que ele serve | 1 | ✅ provada |
| `quorum-espera-sem-degradar` | o commit com quórum esperando o prazo inteiro a cada gravação, sem o modo degradado: uma réplica caída para o servidor 10 s por commit | 1 | ✅ provada |
| `quorum-espera-fora-da-trava` | a espera do quórum depois de soltar a trava de dados: a linha fica visível antes de qualquer réplica ter confirmado | 1 | ✅ provada |
| `quorum-ack-pede-a-trava` | o `replicar_aguardar` tomando a trava de dados do master: a réplica espera o commit que espera por ela, e todo commit estoura o prazo | 1 | ✅ provada |
| `quorum-ack-antes-do-fsync` | a réplica confirmando o lote do quórum sem levá-lo ao disco: o «ok» vira «recebi», a garantia que o Cassandra chama de QUORUM | 1 | ✅ provada |
| `quorum-de-epoca-velha` | a réplica aceitando lote de master com época menor que a que ela conhece: o master rebaixado continuaria obtendo confirmação | 1 | ✅ provada |
| `quorum-sem-fsync-local` | o master esperando o quórum sem ter sincronizado a própria gravação: volta como master atrás das réplicas que confirmaram | 1 | ✅ provada |
| `quorum-escritor-sem-espera` | a anotação das tabelas tocadas fora do ponto único onde o diário cresce: a família de escrita esquecida responde «gravei» sem quórum | 2 | ✅ provada |
| `quorum-conta-o-master` | o `quorum_minimo` contando o master: `quorum_minimo:2` com três nós fecharia com uma réplica só | 1 | ✅ provada |
| `quorum-espera-quem-nao-existe` | o commit com quórum esperando o prazo inteiro sem nenhuma réplica no canal: o arranque do master para o servidor 10 s por nada | 1 | ✅ provada |
| `quorum-volta-sem-recuo` | o degradado voltando ao síncrono no primeiro ack, sem o recuo: uma réplica que pisca para o servidor inteiro a cada pulso | 1 | ✅ provada |
| `faixa-imprecisa-no-int8` | número cru entre 2⁵³ e o teto do `Int8`/`UInt8` era gravado como o VIZINHO, calado — `9007199254740993` virava `9007199254740992` | 1 | ✅ provada |
| `sequencia-nomeada-proximo-sem-durar` | o `proximo` da sequência nomeada devolvia o número ANTES de durá-lo: reabrir repetia o que já tinha saído | 2 | ✅ provada |
| `tabela-com-nome-de-sequencia` | `criar_tabela` aceitava o nome de uma sequência nomeada que já existe: dois objetos com um nome só | 1 | ✅ provada |
| `backup-fase-1-sob-a-trava` | A fase 1 do backup (a cópia inteira) sob a ficha de leitura: a escrita esperava a cópia inteira, como no passo 1 | 1 | ✅ provada |
| `backup-sem-fase-2` | O backup em duas passadas SEM a fase 2: a cópia sai da fase 1, e a escrita feita durante ela não está no backup -- e o `conferir` aprova o retrato errado | 1 | ✅ provada |
| `backup-fase-2-nao-acerta` | A fase 2 do armazém devolvendo um acerto vazio sem conferir nada: o alterado, o novo e o sumido entre as fases ficam como estavam na fase 1 | 4 | ✅ provada |
| `backup-fase-2-sem-racy` | A fase 2 decidindo só pelo `stat`: o arquivo escrito no mesmo tique do relógio (mtime e tamanho iguais) não é recopiado | 1 | ✅ provada |
| `backup-fase-2-sem-eventos` | A fase 2 sem a rede dos eventos: a tabela que andou com o relógio recuado (stat igual) não é recopiada | 2 | ✅ provada |
| `manutencao-durante-o-retrato` | `congelar` sem perguntar pelo retrato: a reescrita inteira de uma tabela entra no meio da fase 1 do backup | 1 | ✅ provada |
| `zip-pasta-nova-sem-fsync-da-mae` | o backup em ZIP responde «concluido» sem o `fsync` da mae de cada pasta que criou | 1 | ✅ provada |
| `arvore-pasta-nova-sem-fsync-da-mae` | o backup em arvore responde «concluido» sem o `fsync` da mae de cada pasta que criou | 1 | ✅ provada |
| `odbc-dml-anuncia-colunas-do-esquema` | um DELETE pelo driver ODBC anunciava as colunas do esquema da tabela citada | 2 | ✅ provada |
| `odbc-colattribute-recusa-unsigned` | `SQLColAttribute(SQL_DESC_UNSIGNED)` recusava com HYC00 e derrubava o primeiro SELECT do pyodbc | 1 | ✅ provada |
| `odbc-getfunctions-esconde-o-par-de-diagnostico` | o driver ODBC deixava de anunciar `SQLGetDiagField` na lista de funcoes | 1 | ✅ provada |
| `sql-call-nao-resolve-interrogacao-do-odbc` | o `CALL` da op `sql` recusava o `?` do ODBC, e o parametro de SAIDA nunca funcionou ponta a ponta | 1 | ✅ provada |
| `servidor-call-nao-passa-parametros-a-rotina` | a op `sql` chamava a rotina SEM os `parametros` do pedido | 1 | ✅ provada |
| `zip-sem-a-guarda-de-espaco-da-arvore-temporaria` | o zip em duas passadas copia a arvore inteira para um disco que nao a comporta e o backup MORRE em vez de cair na passada unica | 1 | ✅ provada |
| `contador-do-source-nao-adotado` | a replica abria a tabela sem adotar o contador da `Sequence` do source: promovida atrasada, reemitia o numero que o master ja tinha entregue | 1 | ✅ provada |
| `posicao-sem-o-contador-da-sequencia` | o `posicao` do source nao dizia onde a `Sequence` estava: a replica nao tinha de onde adotar o contador | 1 | ✅ provada |
| `lote-do-quorum-sem-o-contador-da-sequencia` | o lote do quorum nao levava o contador da `Sequence`: o caminho irmao do pull esquecia o que o pull sabe | 1 | ✅ provada |
| `adocao-do-contador-nao-anda` | `adotar_sequencia_do_source` devolvia sem mover o contador: a adocao existia no fio e nao no disco | 2 | ✅ provada |
| `migracao-da-cifra-sela-com-o-material-velho` | Criptografar selava os slots com o material VELHO (em claro) e a tabela saia 'cifrada' com o segredo legivel | 1 | ✅ provada |
| `migracao-da-cifra-reaproveita-o-sal` | cada Criptografar tinha de sortear sal NOVO; reaproveitar o anterior repete chave e nonce | 1 | ✅ provada |
| `migracao-da-cifra-ressuscita-o-slot-livre` | a migracao reescrevia o slot excluido como ATIVO: a linha apagada voltava | 1 | ✅ provada |
| `migracao-da-cifra-sem-conferir-o-retrato` | a FASE B da migracao renomeava o retrato por cima de uma escrita confirmada no meio | 1 | ✅ provada |
| `migracao-da-cifra-deixa-o-memo-marcado-em-claro` | Criptografar aceitava tabela com coluna Memo/Bin marcada e deixava o conteudo legivel no .memo/.bin | 1 | ✅ provada |
| `migracao-da-cifra-sem-nada-a-cifrar` | Criptografar de tabela sem coluna inline marcada reescrevia a tabela para a v5 sem proteger nada | 1 | ✅ provada |
| `geometria-do-volume-sem-a-versao` | a decisao da troca sem a versao na geometria deixa o *.novo da migracao indistinguivel do volume velho | 1 | ✅ provada |
| `migracao-da-cifra-pelo-sql-sem-portao` | ALTER TABLE ... ENCRYPT pelo SQL passava pela permissao da op `sql` (ler) e cifrava a tabela | 1 | ✅ provada |
| `migracao-da-cifra-sem-pergunta-de-transacao` | a migracao congelava a tabela debaixo de uma transacao viva e o COMMIT dela saia pela metade | 1 | ✅ provada |
| `escrita-local-contada-depois-da-escrita-630` | a escrita local na replica se conta DEPOIS de gravar: na janela entre uma coisa e outra a rodada da replica nomeia as duas causas e a recusa fica guardada por posicao | 1 | ✅ provada |
| `gancho-nunca-chamado` | o carteiro da saúde não chama o gancho do operador | 3 | ✅ provada |
| `gancho-sem-portao-ligado` | o gancho executa mesmo com `alertas.gancho.ligado` falso | 1 | ✅ provada |
| `gancho-por-shell` | o comando do gancho passa por `sh -c`: `;` e `$()` viram execução | 1 | ✅ provada |
| `gancho-ambiente-herdado` | o filho do gancho herda o ambiente do servidor | 2 | ✅ provada |
| `gancho-sem-kill-no-prazo` | o gancho que passa de `timeout_s` continua vivo | 4 | ✅ provada |
| `gancho-zumbi` | o gancho morto por prazo vira zumbi (kill sem wait) | 2 | ✅ provada |
| `gancho-saida-do-filho-vaza` | o stdout/stderr do gancho cai no stderr do servidor | 1 | ✅ provada |
| `gancho-editavel-pela-api` | um campo de `alertas.gancho` entra no CAMPOS_EDITAVEIS | 2 | ✅ provada |
| `gancho-nao-valida-no-arranque` | `comando[0]` relativo, inexistente ou não executável passa no arranque | 1 | ✅ provada |
| `gancho-config-nao-lida` | `alertas.gancho.timeout_s` está no arquivo e ninguém o lê | 1 | ✅ provada |
| `firewall-sob-o-mutex-da-lista-negra` | o comando de firewall roda com a lista negra na mao (pedido 638) | 1 | ✅ provada |
| `firewall-output-sem-prazo-e-com-stderr` | o firewall volta a `Command::output()`: sem prazo, ambiente herdado, stderr no erro (pedido 638) | 1 | ✅ provada |
| `gancho-reserva-sem-raii` | a reserva da execucao unica do gancho nao e solta por `Drop` (pedido 640) | 1 | ✅ provada |
| `gancho-programa-gravavel-pelo-grupo` | o programa do gancho 0775 (gravavel pelo grupo) passa na conferencia (pedido 639) | 1 | ✅ provada |
| `gancho-programa-em-diretorio-gravavel` | o programa do gancho em diretorio 0777 sem sticky passa na conferencia (pedido 639) | 1 | ✅ provada |
| `gancho-programa-por-link-simbolico` | o link do programa do gancho nao e seguido: julga-se o modo do proprio link (pedido 639) | 1 | ✅ provada |
| `gancho-programa-de-outro-dono` | o programa do gancho pertence a outro usuario e passa (pedido 639) | 1 | ✅ provada |
| `gancho-diretorio-de-outro-dono` | o diretorio do programa do gancho pertence a outro usuario e passa (pedido 639) | 1 | ✅ provada |
| `gancho-linha-so-troca-crlf` | a linha do SMS e do stdin do gancho so troca CR e LF (pedido 643) | 1 | ✅ provada |
| `gancho-filho-direto-com-a-trava` | o filho do gancho nasce direto do servidor, com a trava de instancia na mao (pedido 759) | 1 | ✅ provada |
| `json-texto-sem-escapar-a-aspa` | o escritor de JSON deixa a aspa do valor sem escapar: texto vira campo (pedido 249, B1a) | 1 | ✅ provada |
| `io-do-caminho-pedido-avisa-o-disco` | o Io de um caminho digitado pelo usuario dispara o aviso de saude do disco (pedido 641) | 1 | ✅ provada |
| `profiler-caminho-pedido-como-io` | o `arquivo` do profiler que nao abre volta como erro de E/S (pedido 641) | 1 | ✅ provada |
| `encerrar-sessao-adivinha-web-pela-forma` | o encerrar_sessao decide web x conexao pela forma do id e ignora o `tipo` do pedido (pedido 644) | 1 | ✅ provada |
| `ping-crava-a-porta-5000` | o ping devolve a porta 5000 de fabrica em vez da que o servidor escuta (pedido 645) | 1 | ✅ provada |
| `conferidor-nao-ve-porta-cravada` | o conferidor de numero cravado em texto de tela deixa de acusar «porta NNNN» (pedido 645) | 1 | ✅ provada |
| `ack-do-quorum-sem-alcance` | o ack do quorum vale para tabela que a sessao nao alcanca (pedido 649) | 1 | ✅ provada |
| `ficha-do-quorum-ultimo-a-chegar` | a ficha do cubo do quorum e gravada por qualquer credencial `Replicar` (pedido 649) | 1 | ✅ provada |
| `sequencia-do-source-sem-teto` | o contador de sequencia que o source anuncia e adotado sem teto (pedido 650) | 2 | ✅ provada |
| `na-faixa-da-a-volta` | `na_faixa` soma sem saturar perto de `u64::MAX` (pedido 650) | 1 | ✅ provada |
| `noise-entra-sem-registro` | quem chega pelo Noise entra sem a linha de log da transicao para o TLS (pedido 652) | 1 | ✅ provada |
| `noise-sem-silencio-por-par` | o aviso do Noise sai uma linha por conexao em vez de uma por par (pedido 652) | 1 | ✅ provada |
| `eleicao-sem-teto-de-atraso` | a eleicao promove a replica atrasada alem do `atraso_maximo_na_eleicao` (pedido 313) | 1 | ✅ provada |
| `arranque-recusado-calado` | o arranque recusado pela sentinela do 509 nao avisa o operador (pedido 573) | 1 | ✅ provada |
| `veneno-permanente-recusa` | a trava de dados envenenada recusa mesmo com o reparo terminado (pedido 653) | 1 | ✅ provada |
| `backup-fsync-do-grosso-na-fase-1` | O `fsync` do grosso da cópia de volta à fase 1 do backup, com o escritor andando (pedido 646) | 1 | ✅ provada |
| `backup-manifesto-nasce-na-fase-2` | O manifesto do backup gravado já na fase 2, antes do `concluir`: uma queda ali deixa um destino que o `op_backups` lista e o `restaurar` aceita (pedido 646) | 1 | ✅ provada |
| `backup-concluir-manifesto-antes-do-fsync` | O `concluir` grava o manifesto ANTES do `fsync` das cópias das duas fases (pedido 646, condição C2 do 524) | 1 | ✅ provada |
| `trava-de-instancia-segue-link` | A trava de instância aberta com `create(true).write(true)`: segue o `.phxsql.trava` plantado como link e TRUNCA o alvo (pedido 648) | 3 | ✅ provada |
| `trava-de-instancia-aceita-link-fisico` | O motor do arquivo do banco deixa de contar os nomes do inode: a trava escreve o pid num `.phxsql.trava` que é link físico de outro arquivo (pedido 648) | 1 | ✅ provada |
| `zip-retrato-part-aproveitado` | A árvore temporária do zip (`.retrato.part`) aproveitada se já existe: o link plantado leva a cópia para fora e o intruso entra no zip (pedido 651) | 2 | ✅ provada |
| `restaurar-aceita-trava-no-manifesto` | A restauração aceita um manifesto que lista `.phxsql.trava`, nome que o backup nunca grava (pedido 651) | 1 | ✅ provada |
| `novo-da-fase-a-reusa-o-inode` | O `*.novo` da FASE A trunca e reusa o inode do nome: o `descriptografar` escreve o texto claro numa isca plantada como link físico (pedido 661) | 1 | ✅ provada |
| `fase-b-nao-confere-o-novo` | A FASE B renomeia o `*.novo` sem conferir que é o que a FASE A escreveu: o trocado entre as fases vira o `.reg` (pedido 661) | 1 | ✅ provada |
| `fase-b-nao-confere-o-novo-na-janela` | O `*.novo` trocado na janela sem trava do servidor (`rodar_gancho_da_janela`) é publicado pela FASE B (pedido 661) | 1 | ✅ provada |
| `colattribute-tamanho-escrito-sem-01004` | `SQLColAttribute` devolve os bytes escritos e não o total, e trunca sem `01004`: a pergunta com NULL volta 0 (pedido 663) | 1 | ✅ provada |
| `tamanho-smallint-negativo` | O tamanho de texto acima de 32.767 bytes vira negativo no `SQLGetDiagRec`/`SQLGetDiagField` (pedido 663) | 1 | ✅ provada |
| `contador-da-sequencia-fora-do-cabecalho` | O contador do auto number sai do cabeçalho de `gravar_contadores`: a queda que perde o cabeçalho repete número (pedido 664) | 1 | ✅ provada |
| `seq-avanca-antes-de-gravar` | O `.seq` avança `geracao`/`proximo` antes do `fdatasync`: duas gravações que falham rasgam os dois slots (pedido 665) | 1 | ✅ provada |
| `tls-cliente-pino-ignorado` | O cliente TLS conecta com pino e não confere o SPKI do servidor contra ele (pedido 572, T6b-1) | 1 | ✅ provada |
| `tls-cliente-certificate-verify-sem-conferir` | O cliente TLS não confere a assinatura do `CertificateVerify` contra a chave do certificado (pedido 572, T6b-1) | 2 | ✅ provada |
| `tls-cliente-finished-do-servidor-sem-conferir` | O cliente TLS não confere o `Finished` do servidor (pedido 572, T6b-1) | 1 | ✅ provada |
| `tls-cliente-hrr-sem-eco-do-cookie` | O cliente TLS não ecoa o `cookie` do `HelloRetryRequest` no segundo `ClientHello` (pedido 572, T6b-1) | 1 | ✅ provada |
| `senha-em-claro-de-fora-do-loopback` | O `op_login` aceita `senha`/`senha_b64` de fora do loopback por fio sem cifra (pedido 667) | 1 | ✅ provada |
| `senha-em-claro-pelo-login-remoto-da-web` | O login da web que vai para OUTRO servidor leva a senha em claro de fora do loopback sem passar pelo portao (pedido 667) | 1 | ✅ provada |
| `backup-manifesto-antes-do-fsync-com-faxina-total` | O `concluir` grava o manifesto ANTES do `fsync` e a faxina o apaga em TODO erro: a prova que só olhava depois da faxina passava (pedido 670) | 2 | ✅ provada |
| `contador-da-sequencia-em-escrita-separada` | O contador do auto number vai ao disco num `pwrite` SEPARADO do `slot_count`: a queda entre os dois repete número (pedido 671) | 1 | ✅ provada |
| `fase-b-aceita-novo-de-outro-inode` | A FASE B deixa de comparar o inode do `*.novo`: o arquivo plantado com o mesmo tamanho e a mesma data vira o `.reg` (pedido 672) | 1 | ✅ provada |
| `fase-b-aceita-link-fisico-no-novo` | A FASE B deixa de contar os nomes do `*.novo`: o link físico pendurado entre as fases vira um segundo nome da tabela em claro (pedido 672) | 1 | ✅ provada |
| `fase-b-aceita-novo-que-cresceu` | A FASE B deixa de comparar o tamanho do `*.novo`: o que cresceu entre as fases, com a data reposta, é publicado (pedido 672) | 1 | ✅ provada |
| `fase-b-aceita-novo-escrito-por-fora` | A FASE B deixa de comparar a data do `*.novo`: a escrita pelo nome entre as fases, no mesmo tamanho, é publicada (pedido 672) | 1 | ✅ provada |
| `fase-b-segue-com-o-novo-que-nao-se-le` | O `conferir_novos` segue em frente quando o `lstat` do `*.novo` falha: o `.novo` do espelho apagado entre as fases deixa o `.bkp` velho atrás do `.reg` novo, com Ok (pedido 672) | 1 | ✅ provada |
| `replica-aplica-o-que-chegou-sem-esperar-a-transacao` | A réplica volta a aplicar o que chegou, lote a lote e tabela a tabela: com o fio caído no meio do envio o central mostra a venda pela metade (pedido 676) | 6 | ✅ provada |
| `tomada-da-trava-sem-unidade-do-diario` | A tomada da trava de escrita deixa de abrir a unidade do diário: cada evento de um COMMIT ganha id próprio e a réplica aplica a venda em pedaços (pedido 676) | 2 | ✅ provada |
| `crc-do-evento-sem-o-id-de-transacao` | O CRC do evento da versão 4 do `.log` deixa de cobrir o id de transação: um `tx` trocado no disco passa no `verificar` (pedido 676) | 1 | ✅ provada |
| `grupo-sem-a-vez-das-maes` | O grupo da réplica volta a aplicar as tabelas na ordem da chegada: a filha do mesmo commit entra antes da mãe e é contada órfã sem nunca ter sido visível sem ela (pedido 676) | 1 | ✅ provada |
| `diario-sem-piso-do-disco-para-o-id` | A abertura do `.log` deixa de semear o id de transação pelo disco: com o relógio recuado entre dois arranques o diário recebe id menor que o da vida anterior (pedido 684) | 1 | ✅ provada |
| `cura-sem-o-id-da-cauda` | A cura do `.log` deixa de contar o id dos eventos da cauda: o evento gravado depois do último `sincronizar` some do piso, e o id novo sai menor que ele (pedido 684) | 1 | ✅ provada |
| `cabecalho-do-log-sem-o-maior-id` | O cabeçalho da versão 4 do `.log` deixa de gravar o maior id de transação: a abertura não tem piso sem caminhar o volume inteiro (pedido 684) | 1 | ✅ provada |
| `commit-misto-sem-contar` | A tomada que grava em volume sem id (2/3) e em volume com id (4) volta a passar calada: a réplica recebe o commit partido e ninguém conta (pedido 684) | 1 | ✅ provada |
| `commit-acima-do-teto-aceito` | O COMMIT acima do teto da transação volta a ser aceito na origem: a réplica o recebe em pedaços (pedido 685) | 1 | ✅ provada |
| `custo-da-transacao-sem-a-imagem` | A conta da origem esquece a imagem da linha: aceita a transação que a réplica mede acima do teto, e ela chega em pedaços (pedido 685) | 1 | ✅ provada |
| `escrita-local-na-base-recebida-por-replica` | A base que o nó recebe por réplica volta a aceitar escrita local: o caixa cadastra no database do central (pedido 677) | 3 | ✅ provada |
| `espelho-imposto-a-quem-nao-pediu` | A guarda do 677 volta a valer sem `"espelho": true`: config antigo passa a recusar a escrita local que fazia | 1 | ✅ provada |
| `quorum-aplica-lote-a-lote` | O lote do quórum volta a ser aplicado tabela a tabela: o leitor da réplica vê a venda pela metade (pedido 681) | 1 | ✅ provada |
| `quorum-entrega-parte-o-commit` | A entrega do quórum volta a cortar no meio de um commit: a réplica recebe uma tabela da venda sem a outra (pedido 681) | 1 | ✅ provada |
| `bidi-alcanca-tabela-a-tabela` | O bidirecional volta a alcançar tabela a tabela: o par vê os itens sem a venda quando o fio cai (pedido 681) | 3 | ✅ provada |
| `carga-acima-do-teto-aceita` | A carga fora de transação acima do teto volta a ser aceita: a réplica a recebe em pedaços (pedido 686) | 1 | ✅ provada |
| `grupo-da-replica-sem-marca` | O grupo da réplica deixa de gravar a marca `.tx`: o SIGKILL no meio dele reabre a réplica com a venda pela metade (pedido 682) | 1 | ✅ provada |
| `replica-reaplica-inclusao-sem-olhar-o-reg` | A recuperação do grupo da réplica volta a conferir só o diário: o SIGKILL entre o `.reg` e o evento reabre com a linha duplicada (pedido 699) | 1 | ✅ provada |
| `evento-no-diario-sem-a-linha-apaga-a-marca` | O evento que está no diário conta como aplicado sem o `.reg` confirmar: a marca do grupo sai com a linha ausente (pedido 699) | 1 | ✅ provada |
| `grupo-do-bidi-sem-marca` | O grupo do bidirecional deixa de gravar a marca: o SIGKILL no meio dele reabre com a venda pela metade (pedido 698) | 1 | ✅ provada |
| `marca-do-bidi-sem-completar-no-arranque` | A marca do grupo do bidirecional é gravada e o arranque não a completa: a venda reabre pela metade (pedido 698) | 1 | ✅ provada |
| `bidi-grava-alteracao-por-cima-da-inclusao-orfa` | O arranque do bidirecional grava uma ALTERAÇÃO pela chave de um rowid cuja inclusão a queda deixou fora do diário (pedido 700) | 1 | ✅ provada |
| `bidi-completa-o-grupo-com-outro-id` | O arranque completa o grupo do bidirecional com um id de transação novo: a réplica encadeada recebe a venda em dois pedaços (pedido 701 b) | 1 | ✅ provada |
| `reparo-completa-pelo-rowid-a-marca-do-bidi` | O reparo da trava completa pelo rowid a marca em voo do grupo do bidirecional, que casa pela chave (pedido 700) | 1 | ✅ provada |
| `erro-no-meio-do-grupo-tira-a-marca-da-lista` | O `?` no meio do grupo da réplica devolve o erro com a marca fora da lista da rodada: ela fica no disco até o próximo arranque (pedido 701 d) | 1 | ✅ provada |
| `alteracao-ja-aplicada-sem-olhar-o-conteudo` | A recuperação da marca da réplica dá a alteração por aplicada só porque a linha existe: a versão velha no `.reg` faz a marca sair (pedido 701 a) | 1 | ✅ provada |
| `marca-completada-fora-da-unidade` | O arranque completa a marca da réplica fora de uma unidade de transação: cada evento ganha um id e o grupo chega em pedaços à réplica encadeada (pedido 701 b) | 1 | ✅ provada |
| `exclusao-sem-evento-recusa-na-marca` | A queda entre o slot liberado e o evento da exclusão: a recuperação da marca recusa a cada arranque em vez de completar o evento da lixeira (pedido 701 c) | 1 | ✅ provada |
| `commit-completado-no-arranque-com-outro-id` | A marca do COMMIT completada no arranque dá ao resto um id de transação novo: a réplica encadeada recebe a venda em dois pedaços (pedido 702) | 1 | ✅ provada |
| `metade-adotada-na-marca-inteira` | A recuperação adota o id do grupo de uma marca que já está INTEIRA no diário: a regravação redundante se pendura num grupo que a réplica já fechou (pedido 702) | 1 | ✅ provada |
| `metade-adotada-depois-de-outro-commit` | A recuperação adota o id antigo com um commit DEPOIS na cauda de uma tabela da marca: o diário sai fora da ordem dos ids (pedido 702) | 1 | ✅ provada |
| `braco-da-uniao-perde-o-database-dele` | O braço do `unir` deixa de ler o próprio `database`: a visão da loja lê o mesmo caixa N vezes (pedido 679) | 1 | ✅ provada |
| `uniao-cala-o-braco-cortado-no-teto` | O `unir` responde `truncado: false` com um braço parado no teto de linhas: a loja aparece inteira sem estar (pedido 679) | 1 | ✅ provada |
| `sinal-mata-sem-fechar-a-janela` | O `phxsqld` morre pelo padrão do núcleo no SIGTERM/SIGINT: a janela não vai ao disco e o `.ndx` fica «para trás numa queda» (pedido 687) | 3 | ✅ provada |
| `parada-afirma-sem-levar-ao-disco` | A parada em ordem sai com código 0 sem ter sincronizado as tabelas sujas (pedido 687) | 3 | ✅ provada |
| `ponte-mcp-sai-sem-fechar-a-janela` | A ponte MCP termina no fim da entrada sem fechar a janela: o `.ndx` do que ela gravou fica marcado (irmão do 687) | 1 | ✅ provada |
| `recusa-manda-comando-que-nao-existe` | A recusa do índice marcado manda rodar «`reparar indice`», comando que não existe em porta nenhuma (pedido 688) | 1 | ✅ provada |
| `alter-com-regra-sem-aviso` | `acrescentar_coluna` com `check` ou `calculada` numa tabela com linha é aceito SEM AVISO, e a linha velha fica fora da regra | — | 🪦 aposentada (01/10/2026) |
| `cifra-do-fio-imposta` | a cifra do fio EXIGIDA por padrão, quebrando todo cliente velho | — | 🪦 aposentada (18/09/2026) |

**861 das 863 guardas do catálogo: 2 aposentadas, 854 provadas, 5 redundantes** — 41983 s de mutação, medido de 2026-09-16 15:25 a 2026-10-09 13:21, em 8 datas (2026-09-16: 1, 2026-09-30: 27, 2026-10-01: 93, 2026-10-02: 93, 2026-10-06: 36, 2026-10-07: 220, 2026-10-08: 309, 2026-10-09: 82).

> **Esta rodada NÃO julgou 4 das 863 entradas do catálogo.** Elas não estão provadas nem reprovadas — a rodada não chegou nelas, e ler a tabela acima como inventário do catálogo a lê 4 entradas curta. Para julgá-las é preciso uma corrida do `provar-guardas.py` que as alcance.

- `ndx-sobre-coluna-marcada-em-claro` — o `.ndx` sobre coluna marcada guarda o valor em claro com o cofre ligado
- `ndx-trunca-antes-de-conferir-a-capacidade` — o `.ndx` vivo é truncado antes de a capacidade da página selada ser conferida
- `declaracao-aceita-chave-que-nao-cabe-selada` — criar índice ou marcar coluna aceita chave que não cabe na página selada
- `arvore-em-claro-sob-o-cofre-sem-aviso` — a árvore sobre coluna marcada fica em claro com o cofre ligado e o arranque cala

As guardas que esta corrida ainda cita, hoje aposentadas:

- `alter-com-regra-sem-aviso` (01/10/2026) — o aviso que ela repunha saiu do produto. Ele dizia que a `calculada` acrescentada ficava NULA na linha velha; desde o pedido 245 O2b (parecer do papel C) a linha velha e PREENCHIDA na reescrita, e o aviso passou a mentir. O CHECK ja tinha deixado de ser aviso no O2a. Nasceu no lugar dela a `calculada-acrescentada-nula-na-linha-velha`, que repoe o defeito que o aviso so descrevia; a resposta sem `avisos` esta conferida no `acrescentar_calculada_preenche_a_linha_velha`.
- `cifra-do-fio-imposta` (18/09/2026) — o defeito que ela repunha -- `cifra_fio.exigir: true` de fabrica -- virou o PRODUTO, por ordem do dono (*a comunicacao deve obrigatoriamente ser cifrada*, pedido 370). Guarda cujo defeito deixou de existir nao tem o que repor. Ela nao foi remendada para o numero fechar: nasceu no lugar dela a `cifra-do-fio-rebaixada`, que repoe o defeito CONTRARIO (a cifra voltar a ser opcional) e cuja prova e o mesmo teste, tambem trocado de lado (`o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo`). O que a petrea *guarda nova entra pedida* continua protegendo ficou com o escape escrito, e ele esta no `seguem` da nova.

As notas que a rodada deixou:

- `cadeia-sem-teto` — o binario abortou, que e como esta guarda pega
- `aad-fora-do-slot` — confirmado: tirar so o AAD nao e sentido por teste nenhum, porque o `nonce_de_pedaco` carrega o ROWID. Medido em 03/09/2026, e nao deduzido: tirando o AAD e SO o rowid do nonce -- volume e contador ficando --, o teste CAI. Volume e versao nao entram nesta conta porque o teste copia o slot INTEIRO, e os dois slots moram no mesmo volume com a mesma versao
- `nonce-sem-endereco` — confirmado: tirar so o endereco do nonce tambem passa despercebido, porque o AAD carrega o ROWID. Medido em 03/09/2026: tirando o endereco do nonce e SO o rowid do AAD -- volume e versao ficando --, o teste CAI
- `ffi-panico-atravessa` — o binario abortou, que e como esta guarda pega
- `rest-fecha-sem-escoar` — confirmado: nenhum teste de unidade sente isto, e nao poderia -- o RST e do sistema operacional, e so aparece com um soquete de verdade. Quem pega e o passo 13 de `bancada/rest/provar.py`, e esta entrada existe para dizer, com o numero da rodada, que a cobertura mora la e nao aqui
- `recuperar-sem-reindexar` — o binario abortou, que e como esta guarda pega
- `commit-sem-rede-antes-da-marca` — confirmado (pedido 720): sem a rede do 426, quem recusa o COMMIT antes da marca e a pre-conferencia do 448 -- EM_MIGRACAO 4006 sem a frase da rede, 0 de 2 linhas gravadas. Tirando as duas camadas, o teste CAI com 1 de 2 linhas no disco e o arranque aplicando a outra (`commit-sem-as-duas-recusas-antes-da-marca`)
- `backup-destino-que-contem-a-raiz` — medido em 02/10/2026 (frente do 513 passo 2), e nao deduzido: desde o pedido 611 (S7, `conferir_destino_aberto`, commit 940e0e31 de 01/10 06:57) a mesma pergunta e feita ao DESCRITOR da pasta aberta, antes da primeira copia -- entao repor so' a conferencia de texto no `conferir_destino` nao e sentido por teste nenhum: os quatro destinos do teste recusam no descritor. A guarda que pega o par e `destino-do-backup-conferido-so-pelo-nome`. A ultima PROVADA desta entrada e de 01/10 02:26, ANTES do 611.
- `prova-do-gravar-privado-dentro-do-processo` — o binario abortou, que e como esta guarda pega
<!-- guardas:fim -->

### As duas metades, e a terceira que ninguém pede

1. **passa com o conserto** — a árvore limpa roda inteira, primeiro. Se não
   estiver verde, nada ali prova nada e o executor para. Sem isso, um teste já
   vermelho apareceria como guarda provada.
2. **falha com o defeito** — a lista `caem`, teste a teste.
3. **e os que têm de continuar passando** — a lista `seguem`. Sem ela, uma troca
   que quebrasse o arquivo inteiro pareceria uma guarda excelente.

### Os cinco vereditos

| veredito | o que quer dizer |
|---|---|
| **PROVADA** | todos os `caem` caíram e todos os `seguem` continuaram de pé |
| **REDUNDANTE** | a entrada declarou `espera: "nada muda"` e nada mudou — a guarda existe **duas vezes** no código, e tirar uma só não é sentida por teste nenhum. É resultado medido, e não falha |
| **NAO PEGOU** | um `caem` continuou passando: **é um teste que passa por engano** |
| **ESTRAGOU** | um `seguem` caiu junto: a troca quebrou mais do que o defeito de origem quebrava, então ela não prova a guarda |
| **QUEBRADA** | o trecho não está mais no arquivo, aparece duas vezes, ou o código trocado não compila |

Sai `0` quando todas ficaram provadas ou redundantes, `1` quando alguma não
ficou.

---

## 9. O que esta rodada achou, e o que ela mediu e jogou fora

Cada item traz o **defeito reposto** ou o **número**. Prova nos dois sentidos,
como manda a casa — e a hipótese que morre fica escrita, porque a recusa com o
número é o que impede a mesma ideia de voltar sem medição.

### 9.1 Dois conferidores saíam ZERO com «FALHAS» na tela

`bancada/telemetria/conferir-desenho.mjs` e `conferir-interacao.mjs` mediam
certo e imprimiam certo — e **saíam com código 0 sempre**, imprimissem
`FALHAS (n)` ou linhas `FALHA …` ou não. Lidos por gente, acusavam. Chamados
por uma bateria que soma códigos de saída, mentiam verde.

Ninguém tinha notado porque, até esta rodada, **ninguém os chamava por
programa**: eram dois comandos que uma pessoa rodava e lia. O buraco só existe
a partir do dia em que aparece o orquestrador — e é o mesmo formato do teste
que passa por engano, um andar acima: **conferidor que não sabe reprovar não
confere nada quando ninguém está olhando.**

- **Conserto:** três linhas em cada um, `process.exitCode = … ? 1 : 0`. O do
  desenho também passou a reprovar quando o contraste medido fica abaixo de
  4,5:1 — número que ele já calculava e só imprimia.
- **Prova real, nos dois sentidos.** O defeito de origem é a ausência da linha.
  A falha foi forçada de dois jeitos, para cobrir os dois canais do conferidor —
  uma medida geométrica reprovada (`falhas.push(...)`) e o piso de contraste
  baixado de 4,5 para 30, que nenhum par de cores atinge. Medido:

  | o conferidor | com o `exitCode` | sem ele (o defeito) |
  |---|---|---|
  | limpo | `0` | `0` |
  | com uma falha de geometria forçada | **`1`**, e imprime `FALHAS (1):` | `0`, e imprime `FALHAS (1):` |
  | com o piso de contraste impossível | **`1`** | `0` |

  A linha do meio é a que dói de ler: o conferidor **escreve a reprovação na
  tela** e diz ao chamador que passou. Sem a linha, a bateria única dá `PASSOU`
  numa parte que acabou de imprimir que reprovou.

### 9.2 O AAD do slot cifrado é a **segunda** fechadura, e a ficha dizia que era a única

A ficha de `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` dizia, em texto:

> Provado com o defeito reposto: tirando o `aad` do `montar_slot` e do
> `abrir_slot`, este teste passa a ler a linha trocada e falha no
> `assert!(erro)`.

Medido, com o defeito reposto de verdade pelo `bancada/guardas/`: **não passa a
ler nada.** O teste continua verde.

O motivo, achado seguindo o código **depois** da medição: o endereço está
amarrado duas vezes. O `aad_do_slot` leva `(volume, rowid, versao)`, e o
`cofre::nonce_de_pedaco(rowid, volume, versao, tempero)` leva os mesmos três —
e nonce diferente já dá texto cifrado e etiqueta diferentes. Medido nas três
combinações:

| o que sai | `trocar_o_corpo…` |
|---|---|
| só o AAD | **passa** (verde) |
| só o endereço do nonce | **passa** (verde) |
| os dois | **cai** |

A garantia que o teste nomeia continua de pé; o que estava errado era a
atribuição dela a uma peça só. É o corolário do `CLAUDE.md` em miniatura:
**diagnóstico plausível não é diagnóstico medido, e o errado sobrevive melhor
quando o conserto funcionou por outro motivo** — aqui o conserto (o AAD) foi
escrito e funcionou, só que a proteção já vinha do nonce.

- **Consertado:** a ficha do teste e a do `aad_do_slot` agora dizem a verdade
  medida, e apontam para as três entradas do catálogo.
- **Não consertado, de propósito:** o AAD **fica**. Ele é defesa em
  profundidade — no dia em que o nonce virar sorteado e guardado no slot, ele
  passa a ser a única coisa entre o arquivo e o embaralhamento. Tirar redundância
  de cripto porque «hoje não é sentida» é o caminho para o dia em que ela era.
- **Travado:** `aad-fora-do-slot` e `nonce-sem-endereco` **afirmam** a
  redundância (`espera: "nada muda"`). No dia em que uma das duas deixar de
  cobrir, elas viram `NAO PEGOU` e o relatório avisa.

### 9.3 «Os seis torcidos caem com qualquer recorte» — depende de qual recorte

O comentário do `profiler.rs` diz que os seis casos torcidos «todos falham se
alguém trocar a análise por um `find` e um corte». A primeira entrada do
catálogo acreditou nele e listou sete testes. Medido: **caem cinco.**

Dois sobreviveram, e cada um por um motivo diferente e legítimo:

- `aspas_escapadas_dentro_de_um_valor_nao_confundem` guarda o recorte errando
  para o **outro** lado — tapando o que não era segredo. O recorte que exige o
  `":"` colado não erra assim, porque dentro de um valor o texto chega escapado
  e o par nunca fica colado. Quem o derruba é um recorte mais largo, que procura
  a palavra `senha` solta — e ele virou a segunda entrada,
  `profiler-recorta-largo`.
- `quebra_de_linha_no_pedido_nao_forja_linha_no_arquivo` **nem passa pelo
  `redigir`**: o pedido dele é `{}` e as quebras estão na `op`, no usuário e no
  banco. Quem o guarda é o `de_uma_linha`, e ele ganhou entrada própria,
  `evento-linha-sem-escape`.

Nenhum dos dois testes está errado. Errada estava a conta de sete — e ela era
minha. **A lição:** «este teste pega aquele defeito» é uma afirmação como outra
qualquer, e vale o que vale uma afirmação não medida. O catálogo existe para
transformar cada uma delas numa asserção que roda.

### 9.4 A regra do binário velho apareceu **dentro** da ferramenta que a caça

A primeira versão do executor copiava a árvore com `shutil.copytree`, que usa
`copy2` e **preserva a data**. O efeito, medido: a rodada anterior compilava o
`target/` da cópia a partir do fonte mutado; a seguinte devolvia o fonte limpo
com a data velha; e o cargo, que decide por data, achava o artefato mais novo
que o fonte e não recompilava — a «árvore limpa» rodava o binário **com o
defeito ainda dentro**, e o executor acusava a árvore limpa de estar vermelha.

Quem pegou foi a conferência da árvore limpa, que existe exatamente para isso.
Hoje a cópia é **por conteúdo**, com a data de agora no que mudou, e os arquivos
que o catálogo sabe mutar levam `utime` a cada invocação — custa uma
recompilação dos dois pacotes por rodada, e é o preço de a ferramenta não ser
enganada pelo que ela existe para pegar.

### 9.5 A cópia da árvore não pode morar no `/tmp`

`restaurar.rs` tem um teste que exige que o palco da restauração **não** caia em
`std::env::temp_dir()`, e ele mede isso contra o diretório de trabalho. Com a
cópia em `/tmp/phx-guardas`, o próprio diretório de trabalho é temporário e o
teste reprova sem haver defeito nenhum. A cópia mudou para `~/.cache`.

Não é defeito do teste — é um requisito dele que não estava escrito em lugar
nenhum, e que só aparece quando alguém roda a árvore de outro lugar.

### 9.6 `crates/` sozinho não compila

O `lib.rs` do servidor faz
`include_str!("../../../exemplos/Config_exemplo_01.json")`. A primeira cópia
levou só `crates/`, `Cargo.toml` e `Cargo.lock`, e o compilador disse
exatamente qual arquivo faltava. Fica anotado porque qualquer ferramenta que
copie a árvore vai tropeçar no mesmo lugar.

### 9.7 A prova da replicação estava reprovando, e ninguém sabia

`bancada/replicacao/modos.py`, estágio (g) — *read replica: leitura ok,
escrita recusada apontando o primário*. Na primeira corrida da bateria única
ele **falhou**:

```
esperado: … recusada com ESCRITA_NA_REPLICA (4003) apontando 127.0.0.1:5338
medido:   escrita: REDIRECIONA 4003 -> 'REDIRECIONA 127.0.0.1:5338 (g-source)
          -- este servidor e uma replica de leitura; escreva no primario'
```

Não é defeito do servidor: é a prova que ficou para trás. O commit
*«Integra os quatro modos de replicação: um redirecionamento, não dois»*
(`378c0f7`) fundiu `EscritaNaReplica` e `Redireciona` num erro só — os dois
sempre tiveram o código 4003 e sempre quiseram dizer a mesma coisa a quem
chama, *«você escreveu no nó errado, vá para aquele»*. Aquele commit atualizou
o **teste unitário** do papel, com o motivo escrito, e não atualizou o
`modos.py`: ele não estava em portão nenhum, então ninguém o rodava, então
ninguém viu.

**É o achado que justifica a bateria única sozinho.** Uma prova que não está em
nenhum portão não é uma prova — é um arquivo. Ela pode estar vermelha desde
sempre e o projeto continua se dizendo verde.

O conserto aceita os **dois** nomes, e não só o novo: o `replica.rs:142` lê os
dois do fio de propósito, para uma réplica de hoje entender um source antigo, e
uma prova que exigisse só o nome novo passaria a mentir contra exatamente o
servidor que o código promete atender. As garantias que o estágio prova —
código 4003, o endereço do primário no texto, leitura continuar passando —
seguem idênticas. A rodada seguinte da bateria devolveu a parte `replicacao`
verde, em 1m24s.

### 9.8 A tela mente sobre si mesma: título de Configurações, corpo do Painel

A parte `telemetria-cores` reprovou na bateria única — «esperava 4 campos de
cor, achei 0» — e a caça começou pelo suspeito errado (uma intermitência da
prova). Instrumentando a página, o mecanismo apareceu inteiro. **É a única
parte vermelha da rodada final** (13 passaram, 1 reprovou, 1 pulada, 1 sonda,
14m35s).

**Medido**, com uma sonda que fotografa `#painel` a cada 250 ms e intercepta
quem escreve nele:

```
t+2250ms  cmp=4  html=31092  titulo="Configurações gerais do servidor"
t+2500ms  cmp=0  html=13818  titulo="Configurações gerais do servidor"
ESCRITA   tam=11673  pilha: at abrirAdmin (…)
FIM       titulo="Configurações gerais do servidor"
          subtitulo="o que está valendo agora · edita e grava no config.json"
          cmp=0  primeiros=["kpis","cartas"]
```

O corpo é o **Painel** (`kpis`, `cartas`); o título e o subtítulo são os de
**Configurações**. A tela diz uma coisa e mostra outra.

**A causa, no `ui/index.html`, `abrirAdmin`:**

```js
$("#titulo").textContent = txt("tela.painel", "Painel");
p.innerHTML = await vPainel();      // ← escreve DEPOIS do await, sem perguntar
```

Entre o `await` e a escrita cabe qualquer navegação. Quem entra e clica em
Configurações **antes de o Painel terminar de carregar** vê a tela certa
aparecer e ser substituída pelo Painel dois segundos depois, com o cabeçalho de
Configurações por cima. O `vPainel()` consulta os monitores da máquina, então a
janela **cresce com a carga** — e é por isso que a prova das cores passava
quando foi escrita e reprova hoje, com quatro frentes na mesma máquina.

O padrão certo já existe neste mesmo arquivo, três linhas acima de outro
`innerHTML`: *«O diálogo pode ter sido fechado enquanto a sonda falava»* →
`if (!vivo || !document.body.contains(alvo)) return;`. E o `CLAUDE.md` já
registra a família: *«todo laço que já perguntava ‹ainda estou na tela?› parar
sozinho»*. O `abrirAdmin` não pergunta.

**Não consertado aqui, e o motivo:** `ui/index.html` é a tela, e há frentes
mexendo nela nesta mesma rodada. O conserto é uma linha — guardar qual tela foi
aberta antes do `await` e desistir se mudou —, mas é decisão de quem manda no
Centro de Controle, não do orquestrador de baterias. Fica registrado com a
reprodução exata, e a parte `telemetria-cores` fica **vermelha na bateria**, que
é o comportamento certo: bateria verde com defeito na tela é a mesma mentira,
um andar acima.

**A lição que isto acrescenta:** *escrita depois de `await` é escrita numa tela
que talvez não seja mais a sua.* E o corolário sobre a prova: uma prova de tela
que reprova três vezes seguidas não é flaky por decreto — foi o terceiro
resultado igual que fez a caça sair do «deve ser a máquina» e ir para o
`MutationObserver`.

### 9.9 Hipótese que morreu medida: «rodar tudo a cada mutação custaria horas»

Foi a premissa do desenho: rodar só o binário de teste que cada entrada nomeia.
Escrevi «rodar tudo custaria horas» antes de medir. **Medido**, na mesma
máquina, na cópia da árvore e com o `target/` quente — cada linha é uma
mutação, que é sempre uma recompilação do pacote mexido:

| o que se roda por mutação | tempo | 18 mutações |
|---|---:|---:|
| `cargo test -p phxsql-server --lib` (o binário nomeado) | **8,1 s** | ~2 min |
| `cargo test --workspace --no-fail-fast` (tudo, 46 binários) | **49,2 s** | ~15 min |

A soma real está na tabela da §8 — o executor cronometra cada mutação e o
gerador a escreve —, e ela fica **abaixo** da estimativa da primeira linha
porque um terço das entradas mexe em `phxsql-store`, que compila mais rápido, e
uma delas aborta em 4 s.

**«Horas» estava errado por uma ordem de grandeza: são 15 minutos.** O desenho
continua certo, e o motivo mudou de lugar — não é inviabilidade, é caber
**dentro** da bateria única (14m35s inteira) em vez de dobrá-la. É a mesma
correção que a casa já fez com o mutex: o número não muda a decisão, muda a
frase que a explica, e a frase errada é a que sobrevive.


### 9.10 Duas guardas que o provador (263) achou NAO PEGOU, as duas pelo mesmo motivo

`fechar-do-embutido-nao-sincroniza` e `elo-do-empilhar-pelo-disco` passavam com
o defeito reposto (02/10, 12:45 e 13:08). **Saída honesta nas duas: (i), o teste
estava fraco** -- o defeito segue possível, e o que o escondia era **outro
conserto, posterior, que cobre o veredito final**:

| guarda | hipóteses | o que a medição disse |
|---|---|---|
| fechar do embutido | (a) confere o veredito, não o dano; (b) o defeito sumiu do caminho; (c) outro ponto repete o `fsync` | (a) **viva**, (b) **viva em parte**: com o `fechar` reposto como só-o-`Drop`, byte 52 do `.ndx` = **1** no disco (medido) e, no processo novo, `precisa_reconstruir` = **falso**: o `phx_base_abrir` passou a rodar `recuperar_marcas` (563), que reconstrói o índice marcado. A sonda abre de qualquer jeito. (c) morta: não há segundo `sincronizar` no caminho |
| elo do `empilhar` | (a), (b) o COMMIT refaz o elo, (c) outro caminho de empilhar | (b) **viva**: `pre_conferir_a_lista` chama `refazer_o_elo` (537) sobre a linha atual, então o resultado **depois do COMMIT** sai certo mesmo com o plano do `empilhar` só pelo disco. (c) morta: os três pontos passam por `planejar_cascata_empilhada` |

O padrão repete a lição desta casa: **o teste media o veredito que um conserto
POSTERIOR passou a garantir, e não o dano que o defeito faz**. O dano que sobra
é outro, e é o que o teste passou a medir: (1) o byte 52 em **0** no disco logo
depois do `phx_tabela_fechar` -- o processo seguinte não paga uma reconstrução
O(n) nem depende do que o núcleo ainda não levou ao disco; (2) a **leitura de
dentro da transação** depois do `UPDATE` da mãe -- o elo planejado só pelo disco
já está na lista, e a transação lê `id 10` onde a lista escreveu `11`
(`(10, 6, false)` contra `(11, 6, false)`).

Prova real: ver `docs/cognicao/cognicao_guarda-que-mede-o-veredito-que-outro-conserto-passou-a-garantir_20261006_2100.md`.


## 10. O que a rodada das transações achou na própria bateria

Três achados que não vieram do código novo: vieram de rodar a bateria e
desconfiar do resultado dela.

### 10.1 Prazo medido em relógio de parede é corrida, e a corrida disparou

Dois testes das transações abriam com `TIMEOUT 1ms`, faziam uma inserção,
dormiam 30 ms e exigiam que a operação seguinte recebesse o erro do prazo. A
lógica está certa e o caminho exercitado é o de produção. **O teste, não.**

Numa rodada com a bateria inteira em paralelo, `o_prazo_estourado_reverte_e_solta_as_travas`
reprovou — e reprovou na linha **errada**:

```
called `Result::unwrap()` on an `Err` value: TransacaoAbortada(
  "a transacao 1788109415658 passou do TIMEOUT de 1 ms e foi revertida; ...")
   at ./src/servidor.rs:21492   <- a PRIMEIRA insercao, a que tem de passar
```

Com a máquina carregada, o milissegundo acabou **antes** de a primeira inserção
chegar. Nada estava quebrado; o teste é que mediu o relógio da máquina em vez
de medir o servidor.

O conserto não é dormir mais — é não dormir. Um ajudante move o relógio da
transação:

```rust
fn vencer_agora(s: &Servidor, ligacao: u64) {
    let mut t = s.transacoes.lock().unwrap();
    t.de_mut(ligacao).unwrap().expira_ms = crate::agora_ms() - 1;
}
```

A transação abre com `TIMEOUT 10s` (folga de sobra para a primeira operação), e
o vencimento passa a ser um fato, não uma aposta. O caminho provado é o mesmo —
a varredura vê a vencida, o gestor a encerra, o dono recebe o erro com o número
—, e os 26 testes de transação caíram de segundos para **0,54 s** porque os
dois `sleep` saíram. Três rodadas de `cargo test --workspace` seguidas, verdes.

**A lição é a irmã da que já estava escrita sobre teste que passa por engano:**
teste que *reprova* por engano custa quase o mesmo, porque gasta a confiança na
bateria inteira — e o primeiro impulso, diante dele, é olhar o código que está
certo.

### 10.2 A cópia das guardas é compartilhada, e duas rodadas se estragam

A rodada completa das 42 guardas saiu com **36 provadas, 1 redundante, 1 não
pegou e 4 quebradas**. Quatro dos cinco problemas eram mentira, e os quatro
tinham cara de entrada envelhecida.

O que denunciou foi olhar a cópia depois: `~/.cache/phx-guardas/crates/phxsql-server/src/servidor.rs`
ainda tinha um `// DEFEITO REPOSTO` plantado dentro. O caminho da cópia é fixo —
de propósito, porque é o que guarda o `target/` quente —, e **duas invocações ao
mesmo tempo mexem nos mesmos arquivos**. O `LEIA-ME.md` das guardas já avisava
disso e mandava passar `--arvore`; a regra dependia de alguém lembrar.

Hoje o executor **tranca** a cópia com um `flock` num arquivo ao lado do
diretório, e a segunda rodada espera a primeira em vez de a estragar. Provado
segurando a tranca de fora e chamando o executor:

```
outra rodada esta usando /root/.cache/phx-guardas -- esperando a vez
                 esperou 27 s pela vez
  alter-espelho-para-tras      PROVADA                  1.0 s  1/1 cairam
```

O `flock` foi escolhido porque o núcleo o solta sozinho quando o processo morre,
**inclusive num `SIGKILL`** — que é o único jeito de o `atexit` do executor não
rodar. Tranca pendurada por rodada morta é impossível, e isso importa numa
ferramenta cujo trabalho é justamente matar processos por prazo.

### 10.3 Duas entradas do catálogo tinham envelhecido de verdade

Descontada a contaminação da §10.2, sobraram duas quebradas legítimas:
`aad-fora-do-slot` e `endereco-fora-da-amarracao`, ambas em
`crates/phxsql-store/src/reg.rs`. O `trecho` que elas procuravam não existia
mais **na árvore de verdade** — não era cópia trocada.

A causa é inocente: a cifra do slot virou função livre, e o `rustfmt` recolheu
a chamada para uma linha só.

```rust
// o que o catalogo procurava        // o que o codigo virou
let selado = self                    let selado = material.selar(
    .material                            &nonce, &aad_do_slot(volume, rowid, versao), &claro);
    .selar(&nonce, ...);
```

Com os trechos atualizados, as duas voltaram a dar o veredito que declaram —
`aad-fora-do-slot` **REDUNDANTE** (a entrada afirma que tirar só o AAD não é
sentido por teste nenhum, e não é mesmo) e `endereco-fora-da-amarracao`
**PROVADA**, 1/1 caiu. A amarração do slot cifrado ao endereço voltou a estar
provada, e ficou **duas refações sem estar** — que é o tempo em que ninguém
percebeu, porque a quebrada aparecia no relatório como texto e não como número
que desce.

---

## 11. SP000056 — a bateria confiável: o intermitente medido, e o módulo que não tinha defeito

O caso `telemetria` reprovava «em ~metade das rodadas, trocando de tema entre
elas», e enquanto isso o portão da bateria **não distinguia regressão de
ruído**. A decisão do dono era reescrever o gestor de threads, que é o módulo
onde a falha aparecia. **Medido antes de reescrever, ele não tinha defeito
nenhum** — e essa é a metade que mais interessa deste capítulo.

### 11.1 A taxa, antes: 4 de 40 isoladas, 5 de 14 com a máquina carregada

Nada de «~metade»: o número. Duas medições, e a diferença entre elas é a
informação.

| condição | reprovações |
|---|---|
| caso sozinho, 40 execuções seguidas num processo | **4** (10%) |
| bateria completa `--caso telemetria`, 7 rodadas × 2 temas | **5 de 14** (36%) |

A segunda rodou com outra frente compilando ao lado. **A carga não é ruído: ela
é o que abre a janela**, e é por isso que o mesmo caso dava 10% e 36% no mesmo
dia. Quem chamou isso de *flake* estava medindo a máquina sem saber.

### 11.2 A falha não era «timeout»: era um elemento que não existia

O `clicarOuExplicar`, que a própria SP000056 tinha entregado antes, disse o
que a frase do Playwright nunca diria:

```
nao consegui clicar em .tlm-threads summary — e o estado no instante da falha:
{ "achou": false }
```

Não coberto por outro, não invisível, não desabilitado: **ausente**. E ausente
era impossível de explicar lendo o código, porque `#tlmThreads` (que a asserção
anterior tinha acabado de achar) e `.tlm-threads summary` saem do **mesmo**
template literal.

### 11.3 O mecanismo, com um `MutationObserver` no lugar de um palpite

```
NASCEU .tlm   em #painel
SUMIU  .tlm   em #painel      ← 37 ms depois (104 ms na outra reprovação)
#painel  = <div class="kpis">…bancos…registros…      ← o corpo do Painel
#titulo  = "Telemetria"                              ← o título de outra tela
```

`montarArvore()` terminava disparando o clique no nó Painel, e esse clique
rodava `Promise.resolve(abrirAdmin("painel"))` **que ninguém segurava**.
`abrirApp()` devolvia, `#arvore .no` aparecia — o sinal por onde o `entrar()`
da bateria dizia «entrei» —, e o `abrirAdmin` ainda estava no `await
vPainel()`. Ao voltar, escrevia `p.innerHTML` por cima de quem tivesse chegado
no meio-tempo. O `#titulo` não voltava atrás porque `abrirAdmin` o escreve
**antes** do `await` e o corpo **depois**.

**A janela, medida em 12 logins:** 32 ms de mediana (min 29, máx 35) entre a
árvore aparecer e o Painel pintar. A viagem do `page.evaluate` seguinte cai
dentro ou fora dela conforme o humor da máquina. Era isso, e nada mais, que
decidia o veredito — e o tema alternava porque o tema é só quem estava na vez.

### 11.4 O achado que dói: a guarda existia e cobria só quem a escreveu

O contador `admGeracao` já estava lá, com um comentário de vinte linhas
descrevendo este defeito por extenso — «título de uma tela e corpo da outra» —
e uma admissão rara:

> **ATENCAO, e isto e desconforto honesto: esta guarda NAO tem prova real.** A
> sonda que escrevi passa com a guarda E passa com o defeito reposto […] o
> pedido continua ABERTO no PENDENCIAS.

Ela não tinha prova real **porque não cobria o caso que descrevia**. O contador
era privado do `abrirAdmin`: defendia `abrirAdmin` de `abrirAdmin` e de mais
ninguém. Toda tela que pinta por `folha()` — telemetria, profiler, backup e as
**Configurações**, que é a vítima do §9.8 — passava por fora.

O §9.8 e o §5.6 ficaram abertos meses depois de uma guarda ter entrado
justamente para fechá-los. *Guarda sem prova real não é guarda, é intenção.*

### 11.5 O conserto: a posse é do PAINEL, e não de quem pinta

```js
let painelGeracao = 0;
function tomarPainel()      { return ++painelGeracao; }
function aindaNoPainel(v)   { return v === painelGeracao; }
```

- **`folha()` toma a posse.** Uma linha, e as ~50 telas que passam por ela
  ficam cobertas. Espalhar a conferência por cinquenta funções é o que o
  `CLAUDE.md` já proíbe: *a que alguém esquecer vira a porta dos fundos*.
- **`abrirAdmin()` e `desenharAba()` conferem** depois de cada `await`, antes
  de escrever. As cinco abas da tabela tinham o mesmo buraco.
- **`montarArvore()` espera a primeira tela pintar** em vez de disparar um
  clique e ir embora, e `abrirApp()` marca `#app[data-pronto="1"]` quando
  termina de verdade — árvore montada, primeira tela no ar, abas pinadas de
  volta.
- **`entrar()` espera essa marca.** Ninguém clica no menu 30 ms depois de a
  tela abrir; o teste deixou de medir uma corrida que a pessoa não corre.

### 11.6 A prova real, e por que a de antes não provava

`testes-web/casos/18-tela-atropelada.mjs`. A sonda antiga tentava vencer o
relógio e por isso passava dos dois lados. Esta **não torce por timing**:
segura a resposta da op `painel` no fio (`page.route`) até a segunda tela estar
pintada, e só então solta. A corrida deixa de ser sorteio e vira ordem fixa —
que é o único jeito de um caso de bateria provar uma corrida sem virar ele
próprio um intermitente.

Com o `tomarPainel()` do `folha` comentado, **reprova nos dois temas**:

```
FALHOU tela-atropelada  o Painel atrasado escreveu por cima da tela que a
                        pessoa pediu depois dele
                        (titulo="Telemetria", kpis do Painel no corpo=true)
```

Ela cobre as **duas** vítimas e a metade contrária, que é a que impede a guarda
de virar «nunca pinta nada»: pedido **depois**, o Painel assume a tela
normalmente.

E a segunda vítima foi **medida, não deduzida** — «as Configurações também
pintam por `folha()`, logo a mesma linha as cobre» é raciocínio, e raciocínio
não é medição. Com o defeito reposto e a primeira metade neutralizada para a
segunda chegar a rodar, o §9.8 sai idêntico ao que ele registrou meses atrás:

```
titulo="Configurações gerais do servidor"   kpis do Painel no corpo=true
```

Ou seja: **o §9.8 continuava vivo** depois de a guarda que o citava ter
entrado.

**Como repor o defeito, para quem quiser conferir sozinho.** O catálogo de
guardas (§8) só sabe repor defeito em Rust — ele roda `cargo test` —, e esta é
de tela. A receita cabe em três linhas, e fica escrita por isso:

```bash
# em ui/index.html, dentro de folha(), comente a linha `tomarPainel();`
cargo build --release -p phxsql-server --bin phxsqld
node testes-web/bateria.mjs --caso tela-atropelada --porta 6520
```

### 11.7 A taxa, depois

| medição | resultado |
|---|---|
| caso sozinho, 60 execuções seguidas | **0 falhas** |
| bateria `--caso telemetria`, 12 rodadas × 2 temas | **0 de 24** |
| bateria completa, 18 casos × 2 temas | **36/36**, repetida |

Se a taxa de 10% tivesse continuado, ver 60 execuções limpas teria 0,18% de
chance. Isso não é a prova — a prova é o §11.6; é o que sobra depois dela.

### 11.8 O que NÃO foi feito, e por quê

**O gestor de threads da telemetria não foi reescrito.** A sprint mandava
reescrevê-lo, e a medição diz que ele nunca aparece na falha: o painel vivo
sobre `phx-grid` nasce preguiçoso (por causa da largura zero dentro de
`display:none`), sobrevive à volta do relógio, e as asserções que provam as
duas coisas passam em 60 de 60. Reescrevê-lo teria custado uma frente e
comprado zero, e teria trocado um módulo provado por um módulo novo.

É o mesmo padrão do pedido 113: alvo certo, causa errada. *Medir a premissa do
item vem antes de implementar o item — inclusive quando o item é nosso.*

**Continua aberto:** uma tela que faz `await api(...)` e **só então** chama
`folha()` — o profiler é uma — pinta por cima de quem chegou no meio-tempo.
Título e corpo saem coerentes, então não é a mesma mentira do §9.8; é a tela
que você pediu chegando atrasada e ganhando de quem você pediu depois. Sem
guarda e sem prova real.

---

## 12. As pétreas sem guarda — o que ganhou guarda nesta rodada

O `docs/QA-PDCA.md` (seção "As pétreas sem guarda") levantou cinco pétreas do
`CLAUDE.md` sem prova real. A narrativa completa — o porquê de cada escolha,
o que não deu certo no caminho, a saída de cada reprovação — mora lá; aqui só
o inventário do que passou a existir.

| pétrea | onde a guarda mora | como se prova |
|---|---|---|
| Zero dependências externas | `crates/phxsql-server/src/conferidor_dependencias.rs` (novo) | `cargo test -p phxsql-server --lib conferidor_dependencias`; catálogo `dependencia-de-fora-fica-invisivel` |
| Merge de conflito por coluna (`dialogoConflito`) | `testes-web/casos/19-conflito.mjs` (novo) | `node testes-web/bateria.mjs --caso conflito` |
| Índice na filha da chave conferida | `crates/phxsql-store/tests/chave-estrangeira.rs` (dois testes novos) | `cargo test -p phxsql-store --test chave-estrangeira`; catálogo `sem-indice-na-filha-ignora-em-vez-de-recusar` |
| `recursos.cache_paginas` chega ao motor | `crates/phxsql-server/tests/cache-paginas-pelo-config.rs` (novo) | `cargo test -p phxsql-server --test cache-paginas-pelo-config`; catálogo `cache-paginas-nao-chega-ao-motor` |
| "Instrumentação desligada custa zero" | `bancada/profiler/custo.py` (`falhou_desligado_custa_zero`, nova 25ª parte `profiler-custo-zero` em `provar.py`) | `python3 bancada/profiler/custo.py --autoteste` (a lógica, em segundos) e a bateria completa (a medição real, ~minutos) |

As três primeiras entraram no catálogo de mutação (`bancada/guardas/`), e o
catálogo completo — **60 entradas naquele dia** — rodou inteiro depois das três
novas: **56 provadas, 4 redundantes, 0 não pegaram, 0 estragaram, 0 quebradas**
(`bancada/guardas/provar-guardas.py`). Este parágrafo é **história**: o número
de hoje está na tabela da §8, que sai do `--json` da última corrida completa e
não desta prosa. As duas últimas
pétreas não cabem no catálogo por natureza — o executor só sabe repor um
trecho de código Rust e rodar `cargo test`, e uma é JavaScript de tela sem
`cargo test` que a alcance, a outra é um script Python cuja prova real
mexeria em `servidor.rs` três vezes só para medir. As duas provam-se nos
dois sentidos do mesmo jeito, só que fora do catálogo — ver `docs/QA-PDCA.md`
para a saída de cada reprovação.

**O achado no caminho**: o `COPIAR` de `bancada/guardas/provar-guardas.py`
nunca incluía `docs/`, e um teste de `error.rs` que lê `docs/ROTEIRO-1.0.md`
em tempo de execução fazia a árvore limpa reprovar antes de qualquer defeito
ser reposto — não a cada rodada, só em quem tentasse o catálogo completo.
Consertado (`docs/cognicao/cognicao_alcance-da-copia-do-executor-de-guardas_20260903_0246.md`).

---

## 13. Os BOTÕES: quantos são, e quantos a bateria clica

Ordem do dono, 05/09/2026: *«bateria de testes de todos os botões»*. Para
cumprir isso é preciso primeiro **saber quantos são**, e esse número nunca
tinha sido medido.

### 13.1 O número cruo estava errado, e errado para baixo

A varredura ingênua (`grep -c '<button' ui/*.html ui/*.js`) diz **277**. O
conferidor diz **298**, e a diferença tem três causas, cada uma medida:

| causa | quantos | por quê |
|---|---|---|
| o subdiretório `ui/grid/` | **+19** | um `*.js` no diretório não desce até `grid/phx-grid.js`, que é onde mora a grade que **toda** tela usa. É o mesmo buraco que já deixou o `multitela.js` invisível para a catraca de idiomas por 1.474 linhas |
| `role="button"` | **+2** | o pino e o `×` da tira de abas são `<span role="button">`, e não `<button>` — um `<button>` dentro de outro não existe em HTML. Para quem usa teclado e leitor de tela eles **são** botões |
| a etiqueta de várias linhas | 0 hoje | o `id` desta base costuma vir **depois** do `class`, e o `class` costuma carregar `${…}` com uma seta (`x => y`) dentro. Um leitor que fecha a etiqueta no primeiro `>` perde o `id` e o botão vira «sem chave» calado |

O número não fica digitado em lugar nenhum:
`cargo run --example botoes-sem-prova -p phxsql-server`.

### 13.2 A chave: por `id` ou `data-*`, nunca pela frase

O botão se identifica pelo **gancho** com que a bateria o alcança, nesta ordem:
`#id` → `[data-x="v"]` → `.classe`. O **texto nunca entra**: ele passa pelos
seis idiomas da `FABRICA_TELA`, e quem casa por frase quebra calado no dia em
que alguém melhorar a redação — ou quando a tela abre em alemão. É a mesma lei
que o conferidor de textos já aplica.

E a classe só vale como chave quando **o próprio código a usa para achar o
elemento** (`querySelector`, `closest`, `matches`, `classList.contains`). Sem
esse crivo, `class="botao"` daria por provado todo botão do sistema no dia em
que alguém clicasse um. A lista sai do código, não de uma lista digitada: no
dia em que uma classe nova virar gancho, ela entra sozinha.

Medido: **219** botões têm `id` literal, **67** têm `data-*`, **11** têm classe
que é gancho, e **1** não tem identificador nenhum — o gêmeo desligado do
`#tlmEncerrar`, que nasce `disabled` e nunca recebe clique.

### 13.3 O cruzamento vem do CLIQUE, não do fonte dos casos

A pergunta «quais botões a bateria exercita» **não se responde lendo os
casos**, e o número prova: a leitura estática dos seletores escritos em
`testes-web/` dizia **48**; a gravação do que o navegador realmente recebeu
disse **28**. Vinte deles eram seletores *mencionados* — um
`waitForSelector('#btSalvar')` nomeia sem exercitar.

Então a evidência vem de um ouvinte de captura instalado no navegador, e o
arquivo `testes-web/botoes-exercitados.txt` é **gerado** pela corrida inteira
da bateria. Corrida parcial (`--caso`, `--tema`) **não** reescreve o arquivo:
evidência parcial é pior que evidência faltando.

### 13.4 O placar do dia — e o de hoje, 17/09/2026

| | 05/09 (rodada desta seção) | 17/09/2026 |
|---|---:|---:|
| botões da tela | 298 | **321** |
| clicados pela bateria | 85 (28 antes daquela rodada) | **182** |
| dispensados com motivo | 3 | **22** |
| **sem prova** | **211** | **119** |

`TETO_BOTAO_SEM_PROVA = 119` (era 211 nesta rodada, 194 em 07/09/2026, e caiu
para 119 em 17/09/2026 — 62 botões pelo clique, 13 por dispensa nova, cada
dispensa dizendo o que a tira), em
`crates/phxsql-server/src/conferidor_botoes.rs`. **Só desce.** O §13.9 conta a
rodada de 17/09/2026, e o §13.10 a de 02/10/2026 (119 → **90**).

### 13.5 O que exercitar achou — e o que ler o código não acharia

**O `.phx-th-agg` trocava a própria letra e mais nada.** O botão que alterna o
agregador da coluna (SUM → AVG → COUNT → MIN → MAX) mudava `c.agregador` e o
texto do próprio botão, e **não repintava**: o cabeçalho passava a dizer AVG e
o total geral continuava mostrando a SOMA, até alguém virar a página por outro
motivo. Rótulo que contradiz o número embaixo dele é **mentira sobre o dado** —
a mesma lei do «Blumenau» que aparecia «BLUMENAU».

E o irmão já fazia certo, que é por que a falta nunca apareceu: o «total por
grupo» (`[data-rodape]`) mexe no **mesmo rodapé** e chama `carrega()` na linha
seguinte. *Conserto entra no caminho que o motivou, e o caminho irmão fica* —
aqui foi o contrário, o conserto entrou no irmão e o caminho que faltava
esperou.

O passo que o pegou não conferia o estado: conferiu o **efeito**, lendo o total
geral antes e depois. Um passo que só olhasse o texto do botão passaria verde.

**Prova real, com os dois defeitos repostos:**

| reposição | quem acusa | a frase |
|---|---|---|
| tirar o `carrega()` do `.phx-th-agg` | `botoes-da-grade` | `o agregador foi de SUM para AVG e o total geral nao mudou («total geral2.016R$ 293.770,502.016») -- rotulo sem efeito` |
| `#pgDepois` passa a fazer o que o `#pgInicio` faz | `botoes-do-conteudo` | `a pagina nao virou: a primeira linha continua rowid 1` |

Nos dois casos o **`botoes-da-tira` continuou verde**: é a delimitação que
importa — o lote acusa a tela dele, e não a bateria inteira.

### 13.5.1 O que a própria gravação ensinou sobre a bateria

Duas coisas que só apareceram usando o gravador, e as duas viraram guarda:

- **O acumulador não pode morar na página.** Ele nasceu como um `Set` em
  `window`, e o caso `multitela` dá um `page.reload()` no meio: o `Set` nascia
  vazio de novo e os cliques anteriores sumiam — entre eles o
  `[data-jan="acoplar"]`, que aquele caso clica há rodadas. A evidência dizia
  «nunca clicado» de um botão provado, e a catraca teria mandado escrever um
  caso que já existe. Hoje vai por `exposeBinding`, que sobrevive à navegação.
- **«Corrida inteira» não é «corrida que chegou ao fim».** Numa das corridas
  de conferência o `phxsqld` caiu no meio: os 41 casos seguintes reprovaram
  com `ERR_CONNECTION_REFUSED` e a gravação aconteceu do mesmo jeito — o
  arquivo perdeu 110 ganchos. Hoje a evidência só se reescreve numa corrida
  cheia **e verde**, e a bateria **para no ato** da morte do servidor,
  nomeando o caso e mostrando a saída dele. É a mesma lição do portão de
  sintaxe deste diretório: uma linha nomeando a causa vale as 41 reprovações.

**A queda em si fica NOMEADA e não explicada.** Ela aconteceu duas vezes
seguidas — depois do `passeio` numa corrida, depois do `multitela` na outra —
e **não se reproduziu na terceira**, que passou 43/43 e regravou a evidência
byte a byte igual. Havia duas outras frentes compilando na mesma máquina e o
disco em 94%, então o palpite fácil é pressão de recurso; palpite não é
medição, e nesta rodada não houve máquina livre para medir. O que ficou é o
instrumento: a próxima queda diz o caso e mostra a saída do servidor, em vez
de sumir dentro de 41 reprovações iguais.

### 13.6 As dispensas, uma a uma

Nada entra por ser chato. «Derruba o serviço» sozinho não basta — o caso do
pedido 40 já derruba a porta de dados e a levanta pela web.

| botão | por quê |
|---|---|
| o gêmeo `disabled` do `#tlmEncerrar` | nasce desligado com o `title` dizendo por quê; botão que nasce `disabled` não recebe clique nenhum, e é por isso que ele nunca teve `id` |
| `#btSair` | derruba a sessão. **Tem prova** — o caso `entrada` sai e volta —, e está dispensado pelo mesmo motivo que o `passeio` o tira do laço: clicado no meio de uma varredura, o resto dela não teria onde acontecer |
| `[data-acao="devolver"]` e `[data-acao="pinar-janela"]` | só existem dentro de uma janela do sistema destacada (`W.destacada`), e essa janela depende da permissão `window-management`, que o Playwright 1.56 não sabe conceder — a mesma limitação que o caso `monitores` já carrega escrita |

### 13.7 O que ficou de fora, nomeado (situação em 05/09/2026 — ver §13.9 para 17/09)

Três lotes entraram inteiros: **a grade** (18 botões), **o conteúdo editável,
a ficha e a lixeira** (20) e **a tira de abas com a janela solta** (7). Os
maiores lotes que ficaram, medidos:

| lote | quantos | por quê ficou |
|---|---|---|
| `assistenteReplicacao` | 19 | o assistente de réplica pede **outro servidor**; a bancada de replicação já sobe quatro, e o caminho é ela e não o navegador |
| `assistenteDbLink` | 14 | mesmo motivo: o passo 2 em diante fala com um servidor remoto |
| o diagrama ER (`telaDiagramaER`, `cartaoTabelaER`, `cartaoNovaTabelaER`, `cartaoDeclararFk`) | 16 | é o lote seguinte na fila, e é exercitável nesta máquina |
| `gerirTabelas` / `gerirTabela` / `desenharNovaTabela` | 11 | idem |
| a tela da Claude (`claude.js`) | 12 | precisa de chave de API, que não existe nesta máquina |

Meia cobertura só é pior que nada quando finge ser inteira: o
`--example botoes-sem-prova` lista os que faltam a cada rodada, por tela, do
maior lote para o menor. **O diagrama ER e a gestão de tabelas fecharam em
07/09/2026** (eram 15, não 16+11 — `telaDiagramaER` 4, `cartaoTabelaER` 6,
`gerirTabelas` 5), e **os dois assistentes e o resto fecharam quase todo em
17/09/2026** — ver §13.9.

### 13.8 Um achado que não é meu para consertar: o buraco do pedido 170

Procurando botões que fazem `await api(...)` e **só então** pintam — a forma
que o pedido 170 deixou aberta e sem guarda —, a varredura acha **pelo menos
30 funções** de `ui/index.html` com essa forma. O caso `tela-atropelada` cobre
**duas**: as vítimas conhecidas (telemetria e Configurações).

Três coisas para quem for pegar isto, e a terceira é a que importa:

- **A régua é declaradamente incompleta**, e para menos. Ela casa
  `await api(`, e por isso **não** vê `await Promise.all([api(…), api(…)])` —
  que é exatamente a forma da `verConfigServidor`, uma das duas vítimas
  conhecidas. Trinta é o **piso**, não a conta.
- **A guarda não existe no código.** Não há marca de geração de pintura
  (`est.pintura`, `geracao`, «ainda sou eu») em `ui/index.html`: o caso 18
  prova o comportamento das duas telas que ganharam conserto próprio, e não um
  mecanismo que valha para as outras.
- **Isto não vira lote de botões.** Um caso por tela repetiria trinta vezes a
  mesma prova de uma corrida que só se reproduz segurando a resposta no fio.
  O que resolve é a marca de geração — e essa é decisão de arquitetura da
  tela, não de quem escreve teste.

### 13.9 A fila fechou mais em 17/09/2026 (commit `6319396`), e a frase «pedem outro servidor» morreu medida

Quatro casos novos, um por tema: `30-botoes-do-assistente-de-replicacao.mjs`,
`31-botoes-do-dblink.mjs`, `32-botoes-do-pivot.mjs`,
`33-botoes-dos-idiomas-e-do-backup.mjs`. Clicados de 120 para **182**,
dispensados com motivo de 9 para **22**, sem prova de 194 para **119** — 62
pelo clique e 13 pela dispensa, cada dispensa nomeando o que a tira. Bateria
**63 de 63** casos nos dois temas, evidência de 228 para **276** ganchos
(§13.3).

**A hipótese herdada da §13.7 — que os dois assistentes «pedem outro
servidor» — morreu medida.** O assistente de replicação **sonda a si mesmo**:
a porta de dados da bateria é um `phxsqld` de verdade do outro lado do
soquete, e `replicacao_testar` é só mais um cliente do protocolo. **12 dos
19** botões do assistente saem disso, inclusive o de testar a conexão; o
DbLink cede **8 dos 14** pelo mesmo caminho (a tela vazia e o ramo «Não
conectou»). O que trava os restantes está medido, não suposto:

| resta | quantos | o que trava |
|---|---:|---|
| `assistenteReplicacao` (passo 4 em diante) | 7 | a sonda só avança com o servidor de destino declarando `replicacao.id_servidor` e `replicacao.imagem_da_linha`; o servidor da bateria não declara nenhum dos dois |
| `assistenteDbLink` (passo 3 em diante) | 6 | exige um MySQL(R)/MariaDB(R) de verdade respondendo, e não há um destes nesta máquina — subir um traria dependência externa para dentro da bateria |

**A frente RECUSOU destravar** ligando `imagem_da_linha` no servidor da
bateria: `ligar_imagem_no_diario` vale para **toda** tabela de **todo** caso, e
ligá-lo trocaria a cobertura de sete botões por uma mudança de gravação
debaixo dos outros 32 casos existentes — o mesmo tipo de escolha que
`docs/CLAUDE.md` nomeia como «recusa registrada, não esquecimento».

**Exercitar achou quatro defeitos que ler o código não acharia** — os quatro
do commit `6319396`, cada um com o teste que o acusa:

| defeito | onde | o que quebrava | acusado por |
|---|---|---|---|
| dois botões mortos | tela vazia de DbLink (a primeira que quem não tem ligação nenhuma vê) | um `return` deixava as duas linhas de `onclick` seguintes inalcançáveis — tela sem saída nenhuma | `botoes-do-dblink`, nomeando `#btDef` |
| variável sombreando função | copiar como CSV do pivô | `const txt` cobria a `txt()` da fábrica de idiomas; a cópia acontecia e os três recados morriam em «txt is not a function» — o conferidor de textos não vê, porque a chave *está* na fábrica | `botoes-do-pivot` |
| exceção sem dono | `backupAgora` / `conferirBackup` | o pedido saía sem o `destino` obrigatório, a exceção subia sem tratamento, e a folha ficava em «rodando…»/«conferindo…» **para sempre** — sem cópia e sem erro | `botoes-dos-idiomas-e-do-backup` |
| ficha com campo errado | as fichas de resultado de backup e de conferência | pediam `segundos`/`conferidos`/`diferentes`/`faltando`, e a resposta traz `ms`/`arquivos`/`bytes`/`divergencias` — a ficha mostrava travessão onde havia número medido no `<pre>` logo abaixo | `botoes-dos-idiomas-e-do-backup` |

**Nomeado e NÃO consertado, porque é de outra frente**: no passo 3 do
assistente de replicação a tela escreve a versão e os milissegundos que o
`sondar_origem` não devolve — `docs/PENDENCIAS.md` #311.

**O que falta, 119, com a cauda agora parelha**: `cartaoNovaTabelaER` **4**,
`desenharNovaTabela` **4**, `editarJob` **4**, `telemetria.js` **4**, todos
exercitáveis nesta máquina, mais os **7** e **6** que restam dos dois
assistentes (tabela acima) e **12** em `ui/claude.js`, que pedem chave de API.
**Pergunta em aberto**: os 12 do `claude.js` resolvem por interceptar a rota
(há precedente em `testes-web/claude-interceptar.mjs`) ou por dispensa
registrada — decisão de quem manda na catraca (papel G), não decidida aqui.

### 13.10 A cauda fechou em 02/10/2026 (pedido 190), e a pergunta do `claude.js` foi respondida: interceptar a rota

Quatro casos novos, um por tema: `34-botoes-de-nova-tabela.mjs`,
`35-botoes-do-job.mjs`, `36-botoes-da-telemetria.mjs`,
`37-botoes-da-claude.mjs`. Clicados de 182 para **211**, sem prova de 119 para
**90**, `TETO_BOTAO_SEM_PROVA = 90` no mesmo commit, bateria **71 de 71** (era
63 de 63) e evidência de 276 para **313 ganchos**.

**A pergunta em aberto da §13.9 — interceptar a rota ou dispensar? —
respondeu-se interceptando, e nenhuma dispensa nova entrou.** O `claude.js` fala
com `https://api.anthropic.com/v1/messages` direto do navegador, e o
`page.route` do Playwright responde por essa URL com o fluxo SSE do contrato.
Sem chave real, sem rede, e com o `phxsqld` de verdade por trás (a chave é
fabricada e só existe no caso). Diferente da `claude-bateria.mjs` — servidor
falso, ponta a ponta, rodada à parte —, este caso roda **dentro** da bateria e
por isso é o que alimenta `botoes-exercitados.txt`. Os 12 botões saem dele, e o
caso confere o que a bateria à parte não conferia como botão: a chave repousa na
aba e nunca no `localStorage`, o painel «o que vai subir» mascara a chave, a
chave chega à «Anthropic» e **nunca** a um pedido ao `phxsqld`.

| caso | botões | o que confere (o efeito, nunca o estado) |
|---|---|---|
| `botoes-de-nova-tabela` | `cartaoNovaTabelaER` 4, `desenharNovaTabela` 4 | o nome sobrevive ao «+ campo»; o Cancelar não cria; a recusa de nome vazio fica no cartão; a tabela criada tem o esquema pedido (`esquema`); o rascunho do «Cadastro completo…» chega à tela cheia; o «Voltar» não cria |
| `botoes-do-job` | `editarJob` 4 | a ficha nova não oferece Rodar/Excluir; JSON inválido não grava; o job nasce desligado; «Rodar agora» soma UMA corrida ao histórico; recusar o `confirm` não exclui |
| `botoes-da-telemetria` | `telemetria.js` 4 | a pausa deixa o relógio **mudo** (zero pedidos em 5 s); o Agora pede com a tela pausada; Ligar/Desligar muda a coleta **no servidor** e devolve como achou; a legenda some e volta com `aria-expanded` |
| `botoes-da-claude` | `claude.js` 12 | salvar/testar (verde e 401)/remover; receitas; ver o envio; perguntar sem executar; executar traz as linhas do motor; criar → dicionário, ER, desfazer só a rodada |

**Prova real nos dois sentidos, sem recompilar:** `testes-web/prova-real-botoes.mjs`
serve a página por um proxy reverso que repõe **13 defeitos** (botão morto,
`confirm` ignorado, chave inteira no painel, pausa que não para o relógio…) e
exige que cada caso reprove; o controle sem defeito exige que passe. Medido: 4
controles verdes e 13 de 13 defeitos pegos. A primeira rodada pegou 12 de 13 —
**o «Atualizar agora» morto passava**, porque o relógio de 2 s caía na janela de
espera (cognição `cognicao_botao-de-relogio-testado-com-o-relogio-andando-passa-por-engano_20261002_0300.md`);
o caso passou a pausar antes de clicar.

**Exercitar achou dois defeitos que ler o código não acharia:**

| defeito | onde | o que quebrava | acusado por |
|---|---|---|---|
| tela em branco | «Cadastro completo…» do cartão | o rascunho nascia sem `indices_texto` e `desenharNovaTabela` estourava em `r.indices_texto.map` — a pessoa saía do cartão e caía numa folha vazia | `botoes-de-nova-tabela` (`PAGEERR … reading 'map'`) |
| caixa de marcar fora do centro | cartão de nova tabela e `table.montar` | o `td{vertical-align:top}` global punha a caixa e o rádio ~7 px acima do centro dos campos ao lado; achado olhando a captura | conferido na captura (`--capturas`) |

**Quatro textos cravados saíram da tela da telemetria** (`Retomar`, o aviso de
pausa e os dois avisos de coleta ligada/desligada) e entraram pela fábrica, nos
seis idiomas: `tela.tl_retomar`, `tela.tl_pausado_congela`, `tela.tl_coleta_on`
(e o `tela.tl_coleta_off`, que já existia). O caso lê o rótulo esperado **da
fábrica**, não da frase.

**Nomeado e NÃO consertado:** (1) o «Criar» do cartão termina com
`await montarArvore(); telaDiagramaER(db)`, então o diagrama pode pintar por
cima de uma tela aberta logo depois — a pintura tardia do pedido 170, que o
caso contorna esperando o diagrama ter a tabela; (2) restam 3 botões de
`desenharCartao` (`#tlmEncerrar`, `#tlmDerrubar`, `#tlmEstacao`) e o
`[data-nivel]` da trilha na telemetria, que pedem uma conexão viva para
encerrar/derrubar e ficam na fila.

**O que falta, 90** (medido por `--example botoes-sem-prova`): os 7 e 6 dos dois
assistentes (§13.9) e a cauda de 1 a 3 botões por tela em `index.html`.

### 13.11 A pintura tardia (pedido 636): o diagrama cobria a tela que a pessoa já tinha aberto

**O defeito, reproduzido no navegador.** O «Criar» do cartão de nova tabela
terminava com `await montarArvore(); telaDiagramaER(db)`. Com a resposta de
`criar_tabela` segura no fio (`page.route`), a pessoa abre a Telemetria; ao
soltar, o diagrama ER pintava **por cima** dela — título «Diagrama ER», corpo do
diagrama, e a Telemetria sumida. A bateria inteira reprovou duas vezes assim
(o caso 34 isolado, nunca) e o 34 passou a esperar `ER.esquemas` ter a tabela:
**a espera escondia o defeito sem consertá-lo**, e foi retirada.

**A causa não era o «Criar».** A guarda do pedido 170 (`tomarPainel` /
`aindaNoPainel`) já existia, mas só `abrirAdmin`, `desenharAba` e a *tomada* em
`folha()` a usavam. A `folha()` que chega depois de um `await` é indistinguível
de uma tela nova pedida agora, então **a conferência não pode morar no motor**:
tem de estar em quem pinta depois do `await`. Medido no fonte: **83 chamadas a
`folha()` vinham depois de um `await`, em 59 funções**; 3 funções eram falso
positivo (comentário e um clique), e as demais — mais as quatro telas que
escrevem direto no `#painel` (grade de conteúdo, lixeira, motivos, mensagens) —
passaram a conferir a posse, **pelo mesmo contador**, em quatro formas fixas
(documentadas junto de `vezDoPainel()` no `index.html`). Quem chama de fora da
tela (o «Criar», a chave declarada, o excluir de chave, o acrescentar coluna)
guarda `vezDoPainel()` **antes** de ir ao servidor e a passa a
`telaDiagramaER(db, vez)`.

**O caso `38-pintura-tardia.mjs`** não torce por timing: segura a op no fio até
a segunda tela estar pintada (como o 18) e só então solta. Três cenas — o
«Criar», o «Redesenhar» e seis irmãs (duas fases, uma fase, escrita direta) — e
um veredito sobre o **par** título/corpo, medido depois de o dano ter tido
chance de acontecer.

| corrida | resultado |
|---|---|
| caso 38 sobre o `index.html` sem o conserto | **FALHOU** nos dois temas: `esperava "Telemetria", achei "Diagrama ER"` |
| caso 38 com o conserto | **2 de 2** (escuro e claro) |
| caso 34 (sem a espera), 5 corridas isoladas | **10 de 10** casos verdes |
| `prova-real-botoes.mjs --so pintura` | controle verde e **8 de 8** defeitos pegos (RED) |

Os oito defeitos tiram **uma** conferência cada: o «Criar», o corpo final do
diagrama, `verSequencias`, `verSessoes`, o ramo «nenhuma ligação» do
`telaDbLink`, a grade de conteúdo, `verQuemSou` e `telaMensagens`. O total da
prova real passou de 13 para **21 defeitos** e de 4 para 5 controles. **O
primeiro `telaDbLink` PASSOU:** o patch tirava a guarda do ramo «com ligações»,
e a base do caso não tem ligação nenhuma — o caminho exercitado era o outro. Foi
a prova real desta prova que o achou; o patch passou a mirar o ramo vivo.

**O que não se alcança, nomeado:** a conferência é por convenção em cada tela
nova — uma tela que pinte depois de um `await` e esqueça a posse volta a
cobrir a seguinte. Um conferidor estático (função com `await` antes de
`folha(` sem `aindaNoPainel`) cabe como catraca do papel G; não entrou aqui
porque o catálogo de guardas prova testes Rust, e a prova desta frente é de
navegador. Hoje `testes-web/varrer-pintura-tardia.py` (varredura de texto, não prova a
tela) acha **0** funções nessa forma fora os 3 falsos positivos. Um relógio de atualização (sessões, cluster…) que dispare
enquanto a pessoa espera outra tela passa a tomar a posse dela: o clique
pendente é descartado e a pessoa clica de novo — janela de milissegundos a cada
3 s, contra a tela errada por cima.

### 13.12 A fila de botões fechou: 90 → 0 (pedido 190, 02/10/2026) — e o que o zero não diz

Oito casos novos, `39` a `46`. Clicados de 211 para **296**, dispensados com
motivo de 22 para **27**, **sem prova de 90 para 0**, e a catraca
`TETO_BOTAO_SEM_PROVA` desceu de 90 para **0** no mesmo commit. Bateria
**89 de 89** nos dois temas (era 71 de 71) e evidência de 313 para **437
ganchos**.

**O zero é por CHAVE, e não por sítio.** Os 90 do relatório eram **80 chaves**
(`#btVoltarGer` mora em 5 sítios, `[data-t]` em 3…), e a evidência grava o
gancho, não a tela: clicar um sítio dá por provado todo botão que divide a
chave. O caso 39 clica os **dois** `#btVoltarGer` de partições (arquivo único e
paginada), os dois `#btVoltaMot` (a lista e a recusa a quem não administra) e o
`#btSemear` nas duas formas — o conferidor não cobraria nenhum. **Os sítios que
dividem chave com outro e que esta máquina não alcança, ditos sem esconder:**
o `#btVoltarGer` de «Reparar tabela» (a bateria não liga o espelho `.bkp`, o
servidor recusa e o resultado nunca abre — o caso confere a recusa) e o
`[data-t]` da lista de tabelas do DbLink (pede um MySQL/MariaDB de fora).
Cognição: `cognicao_chave-de-botao-compartilhada-esconde-o-sitio-nao-exercitado_20261002_0650.md`.

| caso | o que exercita | o que confere (o efeito, nunca o estado) |
|---|---|---|
| `39-botoes-da-gestao-de-tabela` | gestão de tabela e de banco: partições, configuração, importar, exportar, soma, copiar/colar, motivos, lixeira, SysTables/SysColumns | o banco restaurado/copiado/importado **existe e tem as linhas**; reparar índice recusado no `confirm` não abre o resultado; quem não administra recebe a recusa **e** a saída |
| `40-botoes-de-sessoes-servico-e-profiler` | sessões, serviço, profiler, com um **segundo cliente de verdade** (`conexaoViva`) | o soquete cai **visto de fora**; recusar o `confirm` não derruba; a porta parada recusa cliente novo e **não** derruba o conectado; a sessão web do outro navegador cai no login e a de quem clicou, não |
| `41-botoes-da-telemetria-viva` | `#tlmEstacao`, `[data-nivel]`, `#tlmDerrubar`, `#tlmEncerrar` | derrubar fecha o soquete; **encerrar** acerta uma operação viva (carga de 1,6 milhão de linhas) e a resposta da carga **concorda** com o que a tela disse (`marcada` ⇒ cancelada) |
| `42-botoes-de-lgpd-e-bloqueios` | dado pessoal, trilha, bloqueios | a varredura nova acha a coluna que nasceu depois; o IP bloqueado de verdade (`127.0.0.2`) sai da lista **e volta a ser atendido** |
| `43-botoes-de-configuracao` | gerais do servidor e editor de menu | `config` do servidor devolve o que a tela salvou, e a cor volta ao de fábrica |
| `44-botoes-de-restaurar-backup` | restaurar com outro nome e **por cima** | o botão nasce travado, o nome pela metade não libera, a porta no ar recusa, e a linha entrada depois da cópia **some** |
| `45-botoes-do-modelo-e-do-conflito` | cartão de declarar chave (arrasto com o mouse), diálogo de conflito | a chave aparece no esquema, a filha sem mãe é **recusada**, «Descartar o meu» não grava por cima do outro |
| `46-botoes-de-telas-avulsas` | entrada, «?», junção (7 formatos), união, consulta, sequência, mensagens, jobs, multitela, célula JSON | as **contagens** dos sete formatos de Venn (3/4/4/5/1/1/2), o contador no servidor, a senha **fora** do navegador |

**Prova real nos dois sentidos** (`prova-real-botoes.mjs`, sem recompilar):
os 21 defeitos da rodada anterior viraram **67**, com **46 novos** — botão morto, `confirm` ignorado, handler sem
efeito, a senha gravada junto da conexão, a caixa da carga fora do
`.form-dbl`, o `.then(irAba)` atropelando a tela seguinte, o «ao excluir» com as
quatro opções, o texto «declarada, não imposta» de volta. Medido: **13
controles verdes e 67 de 67 defeitos pegos** — mas **quatro PASSARAM na primeira
corrida**, e cada um mostrou um caso que aprovava por engano:

| defeito que passou | por quê | o conserto |
|---|---|---|
| «Varrer de novo» morto | o caso mexia no seletor «Alcance», e o `onchange` dele já varre sozinho | o caso não toca no seletor |
| `.then(irAba)` reposto | a corrida era sorteio: se a Estrutura terminava antes da outra tela pedir, não havia atropelo | o `esquema` fica **seguro no fio** até a outra tela pintar (molde do caso 38) |
| «declarada, não imposta» de volta | o patch trocava o texto de **reserva** do `txt()`, que só vale quando a chave some da fábrica — patch inerte | o patch troca também a chave |
| «Guardar conexão» guarda a senha | a segunda camada (`limparConexao`) descarta o campo sozinha | o patch quebra **as duas camadas**; o caso prova que a senha não chega ao navegador |

Um quinto patch (`declarar_fk`) **gritou** em vez de passar: o trecho aparecia
duas vezes. A reposição que confere a ocorrência única fez o que foi escrita
para fazer.

**Exercitar achou oito defeitos que ler o código não acharia — consertados:**

| defeito | onde | o que quebrava |
|---|---|---|
| **mentira de tela contra a lei** | cartão «Declarar chave» e nota do diagrama ER | diziam «declarada, **não imposta**» e «uma filha órfã ainda entra» — o contrário da decisão do dono («chave declarada NASCE conferida»), na mesma tela do aviso «já conferida na gravação». Reescritos e **entraram pela fábrica** (6 chaves de frase, `tela.fk_card_*` e `tela.er_nota_fk_*`; catraca `textos-fora-da-fabrica` 871 → **863**) |
| opção que o servidor recusa | «ao excluir a linha-mãe» | oferecia `cascata`, `anular` e `nada`; o servidor recusa as três na declaração. Passou a oferecer só `restringir` |
| campo errado | ficha «versão» do «Sobre» (`?` da barra) | lia `ping.versao`; o servidor responde `ping.phxsql`. Saía sempre «—» |
| CSS fora do componente | caixa «A carga» da importação | o rótulo estava fora do `.form-dbl`: a caixa media 166 px e a dica saía em **caixa alta** |
| pintura tardia (irmã do 636) | nome da tabela em «Dado pessoal» | `abrirTabela(...).then(() => irAba(...))` tomava o painel **de novo** e atropelava a tela pedida depois; agora `est.aba` vai antes |
| select vazio | «Copiar e colar» e «Alcance» do dado pessoal | `est.bancos` é a foto da árvore: banco criado por fora não estava no seletor, e o Colar seguia com destino `""`. O banco de onde se chegou entra sempre |
| teste que floca | `botoes-dos-idiomas-e-do-backup` | esperava a carga de ~1.900 mensagens por 600 ms; passou a esperar o **efeito** |
| teste que floca | `botoes-de-configuracao` (meu) | o «gravado» do aviso anterior valia pelo novo; o aviso é limpo antes de cada salvar |

**Consertado depois (pedido 644, 02/10/2026):** `op_encerrar_sessao` decidia
«sessão web × número de conexão» por **o id ter alguma letra**, e 2,3 %
((10/16)⁸) dos ids web saem só com algarismos. Agora o **pedido diz** qual é:
`"tipo": "web"` ou `"tipo": "conexao"`, e a tela manda o campo nos três botões
«Encerrar» (sessões, sessões web, derrubar da telemetria). Compatibilidade, por
que *guarda nova entra pedida, não imposta*: sem o campo, número JSON segue
sendo conexão e texto com letra segue sendo sessão web (conexão nunca tem
letra — não há palpite); **texto só de algarismos é recusado dizendo por quê**,
nunca adivinhado — era o único caso em que o velho derrubava a conexão errada.
Prova pelo soquete em `servidor::testes_encerrar_sessao_644` (id web «1» ×
conexão 1; **RED medido** com a heurística reposta: a conexão 1 caía). O caso 40
não sorteia mais sessão até o id ter letra: espia o pedido e confere `tipo`.

**Números cravados na tela (pedido 645, 02/10/2026):** o Profiler dizia «pela
porta 5000» e o Sobre «5 arquivos por tabela». Agora o `ping` traz
`porta_dados` (a porta que o servidor escuta *agora*, a mesma função do
`/saude`) e `arquivos_por_tabela` (a lista única de
`Database::extensoes_de_uma_tabela`, 11 tipos), e a tela só os lê. O
conferidor `numeros_cravados` (catraca nova `TETO_NUMERO_CRAVADO_EM_TELA` = 6,
medida; **não** sobe nenhuma outra) acusa pela forma «porta NNNN» e «N arquivos»
(plural); `TETO_ROTULOS_E_CRASE` caiu de 863 para 861. Ficam 6 achados do mesmo
molde, fora do pedido: `nt_sete_arquivos`, `g_prompt_duplicar_nome` («cinco
arquivos»), `sb_bio_adriano`, e o «7000 by default» do swagger. A tabela com
`.memo` e `.fts` tem a contagem conferida no caso 46 (subtítulo da estrutura =
`esquema.arquivos` do servidor); a porta, no caso 40 (a bateria sobe em 6200,
nunca em 5000).

**Duas dispensas novas, com motivo e número (5 botões):** `#btAnt`, `#btProx`,
`#btEstr` e `#btVoltaEstr` (a tela de uma tabela de um banco de fora, que só
abre com `dblink_ler` respondendo — não há MySQL/MariaDB nesta máquina) e
`#btAcompRep` (só nasce com uma **origem de replicação**; é clicado de verdade
em `testes-web/religar-na-tela.mjs`, e o `#acFim` do diálogo tem clique
próprio no caso 46).

**Ajustes na bateria:** a evidência não grava mais cliques em **SVG** (as
bolhas da telemetria são `<g role="button">` montadas por `createElementNS` —
o conferidor lê modelo de HTML, e gravá-las produzia «chave morta» com um id
que muda a cada conexão). `apoio.mjs` ganhou `conexaoViva` e `bloquearIp`.

**Não alcançado, nomeado:** o `#btVoltarGer` do reparo e o `[data-t]` do DbLink
(acima); a bateria não prova a janela `window-management` (já registrada em
`monitores`: o `#mtAlinhar` é provado com a API dublada); e os textos
cravados que ainda dizem «porta 5000» na telemetria e «5 arquivos por tabela»
no «Sobre» são números digitados à mão — fora desta frente.

## 14. Os portões de MEDIDOR, e por que eles ficam fora do catálogo de guardas

Nesta rodada nasceu uma camada de prova que este documento ainda não descrevia:
**portão dentro de um medidor**, provado com o defeito reposto.

O catálogo de `bancada/guardas/` repõe defeito em **Rust** — apaga uma linha do
fonte, roda o teste, exige que ele caia. O medidor comparativo
(`bancada/comparativo/medir.py`) é Python, e os defeitos dele não são de
código-fonte do motor: são do **instrumento**. Repô-los pelo catálogo exigiria
uma segunda máquina de reposição, e máquina de prova que ninguém consegue
manter deixa de rodar.

A saída foi um interruptor de defeito no próprio medidor:

```bash
PHX_CMP_DEFEITO=envelope python3 bancada/comparativo/medir.py   # tem de PARAR
python3 bancada/comparativo/prova-dos-portoes.py                # roda os quatro
```

| defeito reposto | o portão que dispara |
|---|---|
| `indice-velho` | `MESA NAO POSTA` — o índice volta ao formato recusado, e a tabela não nasce |
| `envelope` | `LEITOR QUEBRADO` — as sondas voltam a ler o envelope em vez do `resultado` |
| `catalogo-vazio` | `SONDA QUEBRADA` — lista vazia não é ausência |
| *(nenhum)* | o medidor vai até o fim e grava o `resultados.json` |

**A última linha é a que faz a prova valer.** Sem ela, um medidor que parasse
sempre passaria nos três primeiros — é a mesma armadilha do teste que passa por
engano, pelo outro lado.

### 14.1 O que esta camada achou, e o que ela não alcança

Achou cinco defeitos no próprio instrumento, e o pior deles produzia uma frase
**verdadeira**: «o campo `padrao` foi aceito e IGNORADO» — o motor de fato o
engole, e a medição que dizia isso estava lendo uma tabela que nunca nasceu.
*O errado sobrevive melhor quando o conserto funcionou por outro motivo.*

E derrubou um diagnóstico meu na hora de provar: o quarto interruptor ia ser
«pedir o `catalogo` sem `database`», porque foi assim que expliquei o zero.
Reposto, o medidor **passou**. A causa era outra, e sem a prova real o portão
teria ficado guardando a causa imaginada — com o comentário afirmando-a.

**O buraco que fica, declarado:** o `prova-dos-portoes.py` **não** roda dentro
da bateria única. Quem mexer no medidor tem de chamá-lo à mão, e nada avisa se
esquecer. `docs/cognicao/cognicao_o-controle-precisa-provar-o-LEITOR_20260907_0310.md`.

## 15. O que a bateria deixava em disco — pedido 150

### 15.1 O número, medido antes de mexer

Retrato do `/tmp` antes e depois de uma corrida, para que o número seja a
**diferença** e não o acumulado (o `/tmp` desta máquina já tinha 27.519
entradas de rodadas anteriores):

| Bateria | Antes do conserto | Depois |
|---|---:|---:|
| `phxsql-server` + `phxsql-cmd` + `phxsql-cli` | **265** diretórios/corrida | **0** |
| `phxsql-store` (já «convertido» numa rodada anterior) | **21** diretórios/corrida | **0** |
| `cargo test --workspace` inteiro | — | **0**, com 1.669 testes e zero falhas |

### 15.2 Por que o padrão velho não limpava

O ajudante era sempre este:

```rust
fn dir_temp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("phx-x-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);   // limpa na ENTRADA
    std::fs::create_dir_all(&d).unwrap();
    d                                       // e nada na saída
}
```

O `remove_dir_all` da entrada existe para o próximo achar limpo — não para
limpar o anterior. E um `rm` no fim do corpo do teste também não resolveria:
**teste de asserção falha no meio, não no fim**. O guarda com `Drop` resolve
justamente esse caso, porque o Rust roda o `Drop` também durante o
desenrolamento de um *panic*.

### 15.3 A armadilha da conversão, que custou 53 testes vermelhos

`DirTemp` apaga no `Drop`, então um **temporário** morre no fim da instrução:

```rust
let s = servidor(&dir_temp("vazio"));   // o diretório some ANTES do 1º pedido
```

São 38 sítios assim no `servidor.rs`, mais 2 embrulhos que criavam o guarda
dentro deles e devolviam só o resultado. O compilador não acusa nenhum: o
empréstimo é válido e o `Drop` roda depois do uso. Quem acusa é o teste, com
`database b nao existe em /tmp/…`.

O conserto é ligar o guarda a uma variável:

```rust
let guarda = dir_temp("vazio");
let s = servidor(&guarda);
```

### 15.4 A catraca, e por que ela não é zelo excessivo

Porque o defeito **já tinha voltado sozinho**: o `phxsql-store` fora
convertido inteiro numa rodada anterior, com prova real e «zero diretório
sobrando» escrito no pedido, e a frente do `.fts` repôs três sítios no mesmo
dia — 21 diretórios por corrida, sem nada avisar.

`TETO_TEMP_DIR_SOLTO = 0`, em
`crates/phxsql-server/src/conferidor_temporarios.rs`:

```bash
cargo run --example temporarios-sem-guarda -p phxsql-server
cargo run --example temporarios-sem-guarda -p phxsql-server -- --isentos
```

Ela varre `crates/*/src` e `crates/*/tests` **do disco** (lista digitada
envelhece calado), pula linha de comentário, e isenta por arquivo **com a
quantidade esperada** — 17 isenções hoje, cada uma com o motivo escrito: os
cinco guardas, os seis usos do `mensagens.rs` que só montam caminho para ler,
o `versao.rs` que usa o `/tmp` como diretório de trabalho de um processo
filho, e os três do `restaurar.rs`, que são código de produção.

**Prova real nos dois sentidos:** com um `std::env::temp_dir()` reposto no
`transacao.rs`, a catraca fica vermelha nomeando arquivo e linha; sem ele,
verde. E o casador tem controle próprio
(`o_conferidor_acusa_quando_o_defeito_volta`), porque um casador que parasse
de reconhecer o padrão continuaria imprimindo «nenhuma solta».

### 15.5 O que ficou de fora, e é decisão

Os `examples/`. Um exemplo é medidor chamado à mão ou pela bancada, não
bateria, e vários guardam o que criaram justamente para se olhar depois. São
**48 chamadas em 42 arquivos** — contadas, e **não** medidas em disco: medir
exigiria rodar cada exemplo, e alguns levam minutos. Está no pedido 209, com
o que falta medir escrito. *Dispensa registrada é decisão; dispensa
silenciosa é esquecimento.*

## 16. As provas VERMELHAS — e o buraco que a revisão de 07/09/2026 achou

### 16.1 O que é uma prova vermelha

Quando o defeito é real e o conserto é decisão do dono, esta casa entrega a
**guarda vermelha**: escreve-se o teste que falha com o defeito de pé, mede-se
o estrago, e marca-se

```rust
#[ignore = "VERMELHA de proposito: prova um vazamento que ainda nao foi consertado"]
```

O defeito fica **provado** enquanto espera a decisão, em vez de virar uma frase
num documento. É a mesma lei da prova real nos dois sentidos, com o segundo
sentido adiado.

### 16.2 O buraco

Havia duas na árvore, e **nenhuma no `docs/PENDENCIAS.md`**:

| Guarda | Onde | O que prova |
|---|---|---|
| `coluna_externa_marcada_sozinha_nao_pode_ir_em_claro` | `phxsql-store/tests/cifra-dos-dados.rs` | tabela cujas únicas colunas marcadas são `Memo`/`Bin` nasce **em claro** com o cofre ligado |
| `tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio` | `phxsql-server/src/servidor.rs` | `posicao_do_diario` engole o erro de abrir e encolhe o número que a **eleição** compara |

**Uma prova vermelha desligada é a forma mais educada de esquecer um defeito.**
A bateria fica verde, o `cargo test` diz «0 falharam», e o defeito não aparece
em lugar nenhum que alguém leia. O `#[ignore]` que deveria ser um lembrete
funciona como um silenciador.

Viraram os pedidos **210** (o vazamento) e **211** (a posição encolhida).

### 16.3 A catraca

`TETO_VERMELHA_SEM_PEDIDO = 0`, em
`crates/phxsql-server/src/conferidor_vermelhas.rs`:

```bash
cargo run --example vermelhas-sem-pedido -p phxsql-server
```

Varre `crates/*/src` e `crates/*/tests` **do disco**, acha cada
`#[ignore = "VERMELHA de proposito…`, pega o nome da função logo abaixo e exige
que ele apareça no `PENDENCIAS.md`.

**Não conta os `#[ignore]` comuns, e isso é decisão.** O vetor de 1.000.000 de
iterações do X25519 (RFC 7748 §5.2) leva minutos, e o corpo traçado da sonda do
fecho só roda reexecutado por `fecho_da_janela_sincroniza_o_reg_de_verdade` —
os dois são ignorados por **custo**, não por defeito. Misturá-los encheria a
catraca de ruído e a faria parar de significar «há defeito conhecido aqui».

**Prova real nos dois sentidos**: com um nome de função inventado a catraca fica
vermelha nomeando arquivo, linha e função; com um nome que está mesmo no
`PENDENCIAS.md`, verde. E ela tem controle próprio
(`o_casador_enxerga_uma_vermelha_sintetica`), porque um casador que parasse de
reconhecer a marca continuaria imprimindo «0 sem pedido» — o zero que não prova
nada, que é a mesma armadilha do pedido 150. Esse controle **mudou no 211**: até
então ele exigia achar uma vermelha viva na árvore, e falhou por **sucesso** no
dia em que o projeto consertou a última; agora prova o casador contra um fonte
sintético, e vale com a árvore vazia (que é o estado de hoje) ou cheia.

## 17. As 26 perguntas do dono, exercitadas — 07/09/2026

O dono mandou três perguntas abertas e vinte e seis itens (A–Z) e pediu um PDF
com o exemplo exercitado de cada um. O trabalho saiu em **oito frentes
paralelas**, cada uma com faixa de portas própria e o contrato de resposta do
`docs/pdf/LEIA-ME.md`; o PDF é gerado de `docs/pdf/respostas/*.md` pelo
`docs/pdf/gerar.py` (Chromium, zero dependência), e a seção 36 do dossiê lê
**os mesmos arquivos**. Tudo abaixo foi medido nesta rodada, com o comando que
refaz cada número no fim da resposta correspondente.

### 17.1 As baterias novas, e o placar de cada uma

| bancada | item | o que prova | placar |
|---|---|---|---|
| `bancada/sql-exemplos/exercitar.py` | A E F G H | cada comando do `docs/SQL.md` mandado ao motor | **56 comandos: 28 aceitos, 28 recusados**, todos os recusados com motivo documentado |
| `bancada/gaps-sql/sondar.py` | B C D O P Q | cada gap listado, com a recusa colada; manuais oficiais lidos e citados | **136 comandos** (PostgreSQL 49, MariaDB 27, MySQL 23, SQLite 19, Cassandra 18); sprints antigos: MariaDB 2 fecharam/11 sobreviveram, Cassandra 1/4 |
| `bancada/cluster/escalonar.py` | J | acrescentar um nó ao cluster vivo | a quente **não funciona**; reiniciar só os antigos: master **0,367 s** fora, sem eleição |
| `bancada/conexoes/nativo.py`, `multilink.py` | K L | nativo com desafio-resposta em Python puro; hub com dois DbLinks | ODBC 73/0 · FFI 40/0 · dblink 44/0 · multilink 10/10 |
| `bancada/diretivas/provar.py` | M | três diretivas e as três portas dos fundos (`juntar`/`unir`/`pivotar`) | **46 afirmações, 0 falhas** |
| `bancada/proibidos/provar.py` | X | comando e base proibidos → recusa, blacklist e **e-mail** (SMTP falso em soquete) | **21 afirmações, 0 falhas** |
| `bancada/seguranca/porta.py` | W | 16 casos pela porta TCP | **13 passou, 0 falhou, 3 achado** |
| `bancada/seguranca/injecao.py` | Y | 12 injeções clássicas + jato de 2 s | **6 passou, 0 falhou, 2 achado** |
| `bancada/particao-por-faixa/sonda.py` | S | 1.000.000 de linhas em dez volumes | vol 1 × vol 10: **0,4882 × 0,3417 ms** — não cresce |
| `bancada/transacoes/travada.py` | U | os três jeitos de cancelar | B esperou **401,6 ms** → `4005`; A abortada → `6002`; saldo 100 nos três |
| `bancada/jobs/backup-agendado.py` | V | backup pelo relógio, histórico, falha, restauração | 2 corridas `ok:true`, 1 falha registrada, 20 = 20 linhas restauradas |
| `bancada/sequencias/sonda.py` | 0.2 | declarar, numerar, listar, ajustar | contador no byte **36**; o 92 é o do `rownum` |

### 17.2 O que as baterias acharam — e virou pedido, não frase

- **214** — `replicas_autorizadas` nasce vazia, e vazia libera todos: só com o
  token, `replicar` devolveu a linha inteira e `aplicar` gravou em
  `somente_leitura`.
- **215** — injeção de SQL não bloqueia ninguém: **311.250 tentativas por
  minuto**, `blacklist.json` vazio. O tradutor analisa, então nada executa —
  mas nada é contado.
- **216** — linha acima de 128 MiB não deixa rastro no `acessos.log`.
- **217** — escalonar o cluster a quente não funciona; o caminho que funciona
  custa 0,367 s no master.
- **218** — o bloco `cluster` não aparece na resposta de `config`.
- **219** — três recusas do SQL dizendo a coisa errada, uma delas vazando erro
  cru do sistema operacional.
- **220 / 221** — `comandos_proibidos` é global, não por banco; não há
  operação que crie usuário.
- **222** — `esquema.volumes` vem vazio na partição por quantidade.
- **223** — `SELECT … WHERE id = 2` recusa quando a chave é `Sequence` e passa
  quando é `Int8`: o alargamento de tipo não alcançou o irmão.
- **224** — o catálogo documenta valores que o motor recusa (`unir distinto`,
  o exemplo do `pivotar`).
- **213** — o inventário de arquivos de uma tabela mora em três lugares à mão,
  e o `.fts` faltou nos três (corrigidos; a guarda é o pedido).

### 17.3 O que só a prova real revelou

Três vezes o motor fez **diferente** do que o documento dizia ou do que a
frente esperava, e as três estão na resposta correspondente com o número:

- `telemetria_encerrar` devolve **`ociosa`** para uma transação parada entre
  pedidos — não há laço cancelável ali; quem resolve é `encerrar_sessao` (U).
- um job ligado **depois do arranque** não acorda o relógio (V, e já estava
  no `JOBS.md` — a bancada sobe o servidor duas vezes por isso).
- o `INSERT` dentro de transação é **aceito** na partição por quantidade e
  **recusado** na por letra — e os dois estão certos, pelo mesmo motivo: o
  rowid alvo é previsível num caso e não no outro (S, R).

### 17.4 O encontro das frentes, provado

F5 mudou o servidor (o e-mail dentro de `violacao_grave`) e F6 mediu a
segurança **antes** dessa mudança. As duas baterias de F6 foram rodadas de novo
contra o binário com o código de F5: **13/0/3 e 6/0/2, idênticas**. A suíte
inteira, com os dois testes novos: **1.674 testes, 0 falhas, 0 avisos, 0
diretório deixado em `/tmp`**.


## 18. A prova que o vizinho cancelava — pedido 261

O teste `telemetria::testes::as_threads_do_so_se_medem_e_nunca_sao_menos_que_as_registradas`
provava que o sistema operacional enxerga a thread recém-subida pela
**diferença entre duas leituras** do `Threads:` do `/proc/self/status`:

```rust
let so = threads_do_so().unwrap();      // antes
// ... sobe a thread e espera ela avisar que chegou ...
let agora = threads_do_so().unwrap();   // depois
assert!(agora > so, "a thread subida nao apareceu no SO: {so} -> {agora}");
```

Esse contador é do **processo inteiro** e o `libtest` roda os testes em
paralelo: a thread de outro teste que morre entre as duas leituras come o `+1`
da nossa. Caiu uma vez com a suíte do workspace rodando, e duas frentes o viram
cair na mesma tarde sem nenhuma delas ter tocado em telemetria.

### 18.1 Não é «flake»: é o número

Um amostrador lendo `/proc/<pid>/status` durante a suíte `--lib` do
`phxsql-server` (48,4 s, 2.366.687 leituras, pico de 29 threads):

| grandeza | medida |
|---|---|
| quedas entre amostras consecutivas | **651** |
| janelas de 0,5 ms com queda ≥ 1 | **0,72%** |
| janelas de 2 ms com queda ≥ 1 | **2,32%** |

E a asserção antiga, passando a imprimir o que media — inclusive quantas
tarefas do processo se chamavam `presa`, por `/proc/self/task/*/comm` —, em 600
corridas do filtro `telemetria::` (três laços em paralelo):

```
8 vermelhos (1,33%), TODOS assim:
MEDIDA so=6 agora=6 vao_us=220 presa_no_so=1 fios_vivos=1
```

**`presa_no_so=1`**: a thread subida já estava na lista de tarefas do núcleo no
instante da segunda leitura. O motor tinha feito o que o teste cobrava; o que
faltava na conta veio do vizinho. Os vizinhos eram os do próprio módulo
(`a_thread_que_termina_deixa_de_ser_viva` e
`a_thread_que_entra_em_panico_tambem_deixa_de_ser_viva`) — não é preciso a
suíte inteira para o defeito aparecer.

### 18.2 O conserto: nomear a nossa, e do total só cobrar piso

Não existe «ler uma vez só» para uma **diferença** — diferença precisa de dois
pontos no tempo, e o segundo é justamente o que o vizinho mexe. Então o teste
mudou de grandeza:

- a prova de que o SO viu a thread é o **nome** dela em
  `/proc/self/task/*/comm` (`presa-do-teste`), que nenhum vizinho altera;
- do total do processo só se cobra **piso** (`agora >= fios_vivos`, e
  `so >= 5` com as quatro vizinhas vivas): quem nasce ao lado só aumenta, e
  piso é a única comparação que ele não estraga.

### 18.3 A montagem também é armadilha: leque, não unidade

O teste monta **quatro** vizinhas que morrem antes da medida — é o vizinho do
defeito acontecendo sem depender de sorte, e é o que torna a reposição
determinista. Uma só não serviria, e isso está **medido**: numa corrida da
suíte inteira com o defeito reposto o par foi `27 -> 25` (queda de 2 onde o
leque previa 3), porque um vizinho **nasceu** no meio. Com uma vizinha só,
esse nascimento zeraria a diferença e o `agora > so` **passaria** com o defeito
reposto — guarda verde por engano.

A montagem se confere sozinha: se as quatro não sumirem da lista de tarefas em
5 s, o teste diz que a montagem não reproduz o vizinho que morre, em vez de
virar uma guarda fraca em silêncio.

### 18.4 A prova real, nos dois sentidos

| corrida | com o defeito reposto (`agora > so`) | com o conserto |
|---|---|---|
| filtro `telemetria::` | **40 vermelhos em 40** | **0 em 600** |
| suíte `--lib` inteira, sob carga (load 13,4 / 14,6) | **6 vermelhos em 6** | **6 verdes em 6** |

Nas seis corridas da suíte inteira com o defeito reposto, os outros **1.090**
testes passaram: a reposição derruba o teste certo, e só ele. A guarda é
`threads-do-so-pela-diferenca`, no `bancada/guardas/catalogo.py`.

Uma ressalva que é dela e não do teste desta frente: nas seis corridas com o
conserto, **uma** teve um vermelho de outro teste —
`panico_dentro_do_atender_devolve_a_vaga_da_porta_de_dados`, que exige que os
três pânicos aconteçam e sob carga alta só viu dois. Está medido e aberto como
pedido **267**, e não se confunde com este: o teste desta frente passou nas
seis. **Resolvido na mesma data, na §19** — e a causa era outra: a conexão
entrava e era recusada por falta de vaga, não por pânico que não aconteceu.

E o teste ficou **mais forte**, não só mais estável: ele agora prova que o nome
do fio chega ao sistema operacional — o que faz o `top -H` servir para alguma
coisa —, coisa que a diferença nunca provou.

## 19. A vaga volta DEPOIS de o cliente ver o fim da conexão — pedido 267

O teste `servidor::testes_das_threads::panico_dentro_do_atender_devolve_a_vaga_da_porta_de_dados`
prova a permissão RAII da porta de dados: três conexões entram em pânico dentro
do `atender` com **teto de duas vagas**, e a quarta tem de ser atendida. A
versão antiga disparava os três `ping` em fila e só no fim cobrava o total:

```rust
assert_eq!(s.panicos_de_teste.load(...), 0,
           "nem todos os panicos aconteceram -- a porta fechou antes");
```

Caiu **1 vez em 6** corridas da suíte `--lib` inteira sob carga alta (load
14,6), medido pela frente F261. O log trazia **dois** pânicos onde o cenário
arma três.

### 19.1 A pergunta que vinha antes do conserto

«O pânico não aconteceu» e «a conexão não foi aceita» são causas **diferentes**,
e aquele `assert_eq!` não as separa: o `ping` cuja conexão nem chega a ser
atendida devolve `None` do mesmo jeito que o que caiu no pânico. Separá-las
custou dois contadores só de teste dentro do laço de aceitação **de produção**
(`#[cfg(test)] aceitas_de_teste` e `sem_vaga_de_teste`, em `servidor.rs`) — um
conta toda conexão que o `accept` devolveu, o outro toda recusa por falta de
vaga.

### 19.2 O número: a conexão ENTROU, e foi recusada por falta de vaga

A sonda `sonda_267_corrida_dos_tres_panicos` (`#[ignore]`, por custo) repete o
cenário N vezes e conta por `ping`. Rodada **dentro da suíte `--lib` inteira**,
com três suítes de carga ao lado (load 13–15), em 16/09/2026:

| forma do teste | rodadas | rodadas com pânico faltando | contadores nas que faltaram |
|---|---:|---:|---|
| antiga (três `ping` em fila) | 500 | **20 (4,0%)** | `aceitas=3 sem_vaga=1` nas **20** |
| nova (espera a vaga entre uma e outra) | 500 | **0** | `sem_vaga=0` |

`pings_nao_aceitos=0` nas mil rodadas: **a terceira conexão sempre entrou**. O
que ela levou foi a recusa por falta de vaga — que é o comportamento certo
acima do teto, o `max_connections` dos três motores maduros. O teste reprovava
o motor por um defeito que não existe.

O mecanismo é uma janela que a versão antiga ignorava: o cliente vê o fim da
conexão quando o **soquete** morre no desenrolar do pânico, e a vaga só volta
quando a **thread** acaba. Medida com espera ocupada (um `sleep` de 5 ms
mediria o próprio `sleep`):

| janela entre «o cliente viu o fim» e «a vaga voltou» | p50 | p90 | p99 | máximo |
|---|---:|---:|---:|---:|
| sob carga, dentro da suíte | 1–2 µs | 4,1–6,9 ms | 7,6–22,5 ms | **36,7 ms** |

Com teto 2, o terceiro `ping` que chega dentro das janelas dos dois anteriores
não acha vaga.

### 19.3 A montagem é armadilha, de novo: carga de fora não é o ambiente

A sonda rodada **sozinha**, com a máquina carregada por fora (load 10–12), deu
**0 em 340 rodadas**. A mesma sonda, com a mesma carga externa, mas dentro da
suíte `--lib` inteira (`--include-ignored --test-threads=4`), deu 20 em 500. O
vizinho que importa é o que disputa a CPU **dentro do processo** — é a lição
§18.3 por outro caminho: reproduzir é montar o vizinho, não apertar a máquina.

### 19.4 O conserto é no TESTE, e isso se diz

Não há defeito no motor: nenhum cliente pode saber que a vaga voltou só porque
o soquete dele fechou, e a recusa imediata acima do teto é comportamento
declarado (e tem teste próprio,
`a_porta_de_dados_continua_recusando_na_hora_acima_do_teto`). No molde do
pedido 261, **a grandeza medida mudou, e não o número**: em vez de um total
conferido no fim, cada conexão prova a sua — panicou **e** devolveu a vaga — e
a próxima só parte com a vaga de volta. As três continuam entrando com teto de
duas, que é o que prova o reaproveitamento. E a mensagem da falha passou a
nomear as duas causas, pelos contadores, em vez de confundi-las.

### 19.5 A prova real, nos dois sentidos

| corrida | com o defeito do motor reposto (`ManuallyDrop` sobre a `Permissao`) | com o conserto |
|---|---|---|
| teste sozinho | **10 vermelhos em 10** | — |
| provador de guardas (`--so permissao-de-dados-sem-raii`) | **PROVADA**, 1/1 caíram, o `seguem` verde | árvore limpa verde, 1.092 testes |
| sonda, 500 rodadas dentro da suíte sob carga | 20 vermelhas de 500 (forma antiga) | **0 de 500** |
| suíte `--lib` inteira sob carga (load 13,9–15,9) | — | **6 verdes em 6** |

Os dez vermelhos caem sempre na **primeira** conexão, com a mensagem
`a vaga da conexao 0 nao voltou depois do panico: 1 em uso` — o `ManuallyDrop`
pula a devolução já na primeira, e a nova forma o pega uma volta antes do que a
antiga pegava.
