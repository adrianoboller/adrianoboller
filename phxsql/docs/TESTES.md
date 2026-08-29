# Como este projeto se prova

Três baterias, e cada uma prova o que as outras não conseguem:

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

---

## 1. A cobertura de hoje, medida

`cargo test --workspace`: **1.114 testes, 0 falhas** (1.113 `#[test]` mais um
doc-test). Por área, contando `#[test]` por arquivo e agrupando:

<!-- cobertura:inicio -->
| área | testes | % |
|---|---:|---:|
| Motor de dados (arquivos, índice, diários) | 311 | 26,4 |
| Protocolo e portões (despachar) | 173 | 14,7 |
| Núcleo (JSON, tipos, UUID, zip, paralelo) | 123 | 10,5 |
| Configuração | 76 | 6,5 |
| Criptografia e codificação | 75 | 6,4 |
| DbLink | 65 | 5,5 |
| Camada SQL (léxico, sintaxe, tradução) | 44 | 3,7 |
| Telemetria e profiler | 41 | 3,5 |
| Gatilhos e procedimentos | 38 | 3,2 |
| Jobs | 31 | 2,6 |
| Interface web (servidor HTTP) | 25 | 2,1 |
| MCP | 19 | 1,6 |
| Usuários e permissões | 19 | 1,6 |
| Console de terminal (phxsqlcmd) | 18 | 1,5 |
| Segurança de rede (blacklist, firewall) | 18 | 1,5 |
| **ODBC** | **17** | **1,4** |
| **Exportação** | **13** | **1,1** |
| **Mensagens (i18n do servidor)** | **13** | **1,1** |
| **Junções e união** | **13** | **1,1** |
| **Pivot** | **12** | **1,0** |
| **Replicação** | **11** | **0,9** |
| **Alertas e e-mail** | **8** | **0,7** |
| **Cluster** | **7** | **0,6** |
| **Monitor de máquina** | **6** | **0,5** |
| **total** | **1176** | |

Arquivos de `src` com mais de 120 linhas e **zero** `#[test]`:

| arquivo | linhas |
|---|---:|
| `phxsql-store/src/table.rs` | 2441 |
| `phxsql-store/src/ndx.rs` | 1580 |
| `phxsql-cli/src/main.rs` | 845 |
| `phxsql-server/src/main.rs` | 452 |
| `phxsql-server/src/replica.rs` | 352 |
| `phxsql-server/src/dblink/conexao.rs` | 238 |
| `phxsql-server/src/carga.rs` | 226 |
| `phxsql-cmd/src/main.rs` | 162 |
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

Como rodar, os onze casos e o que ela deliberadamente não faz:
`testes-web/LEIA-ME.md`.

O resumo do desenho:

- **Sobe o próprio servidor**, nas portas 6200/6201, num diretório temporário,
  e o derruba **pelo PID**. A senha não fica em claro em lugar nenhum: o hash
  sai do próprio `phxsqld --senha`, como no `bancada/replicacao/montar.py`.
- **Entra pela tela de login**, com o desafio-resposta de verdade. Se a página
  cair em modo demonstração, o caso falha — sem essa guarda a bateria inteira
  passaria sem tocar no motor.
- **Percorre 112 telas** clicando cada item dos nove menus e cada botão da
  barra, e reprova em qualquer erro. Esse laço sozinho vale mais que dez
  asserções bonitas: foi ele que pegou um `` ` `` a mais dentro de um template
  literal em três segundos, com a página inteira morta.
- **Um contexto de navegador por caso.** A página guarda tema, largura e
  estado da lateral no `localStorage`; com contexto compartilhado, o caso que
  recolhe a lateral fazia o próximo começar com a árvore invisível — e a falha
  aparecia no caso errado.

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
