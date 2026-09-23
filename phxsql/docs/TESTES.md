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
`cargo test --workspace`: **2.710 testes, 0 falhas** — somado dos `test result:` de uma rodada de verdade, e não digitado: quem escreve este número é `docs/dossie/numeros-do-projeto.py`, e ele **aborta se a suíte falhar**.
<!-- testes:total:fim --> Por área,
contando `#[test]` por arquivo e agrupando:

<!-- cobertura:inicio -->
| área | testes | % |
|---|---:|---:|
| Protocolo e portões (despachar) | 598 | 22,1 |
| Motor de dados (arquivos, índice, diários) | 550 | 20,3 |
| Núcleo (JSON, tipos, UUID, zip, paralelo) | 261 | 9,6 |
| Camada SQL (léxico, sintaxe, tradução) | 238 | 8,8 |
| Servidor (outros) | 209 | 7,7 |
| Configuração | 141 | 5,2 |
| Criptografia e codificação | 128 | 4,7 |
| DbLink | 94 | 3,5 |
| Telemetria e profiler | 73 | 2,7 |
| ODBC | 66 | 2,4 |
| Gatilhos e procedimentos | 45 | 1,7 |
| **Jobs** | **33** | **1,2** |
| **Usuários e permissões** | **33** | **1,2** |
| **Mensagens (i18n do servidor)** | **32** | **1,2** |
| **Interface web (servidor HTTP)** | **29** | **1,1** |
| **Replicação** | **24** | **0,9** |
| **Transações** | **23** | **0,8** |
| **Segurança de rede (blacklist, firewall)** | **21** | **0,8** |
| **MCP** | **21** | **0,8** |
| **Console de terminal (phxsqlcmd)** | **20** | **0,7** |
| **Junções e união** | **17** | **0,6** |
| **Exportação** | **13** | **0,5** |
| **Pivot** | **12** | **0,4** |
| **Cluster** | **10** | **0,4** |
| **Alertas e e-mail** | **8** | **0,3** |
| **CLI** | **7** | **0,3** |
| **Monitor de máquina** | **6** | **0,2** |
| **total** | **2712** | |

Arquivos de `src` com mais de 120 linhas e **zero** `#[test]`:

| arquivo | linhas |
|---|---:|
| `phxsql-store/src/table.rs` | 5818 |
| `phxsql-store/src/ndx.rs` | 1895 |
| `phxsql-ffi/src/lib.rs` | 1469 |
| `phxsql-server/src/main.rs` | 488 |
| `phxsql-ffi/src/valor.rs` | 290 |
| `phxsql-store/src/integridade.rs` | 278 |
| `phxsql-server/src/dblink/conexao.rs` | 272 |
| `phxsql-server/src/carga.rs` | 227 |
| `phxsql-ffi/src/punho.rs` | 188 |
| `phxsql-cmd/src/main.rs` | 186 |
| `phxsql-odbc/src/registro.rs` | 149 |
| `phxsql-odbc/src/tipos.rs` | 132 |
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
| `trava-sem-guarda-de-reentrancia` | a trava pedida duas vezes pela mesma thread pendura o servidor | 1 | ⚠️ quebrada |
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
| `pulso-do-cluster-em-claro` | o pulso da eleição saindo em claro com a cifra do cluster ligada | 1 | ✅ provada |
| `replicacao-do-cluster-em-claro` | a replicação entre os nós do cluster saindo em claro | 1 | ✅ provada |
| `alter-compacta-o-buraco` | a reescrita da coluna nova pula os slots excluídos e renumera o rowid | 1 | ✅ provada |
| `alter-sem-remapear-posicao` | a coluna nova desloca as de sistema e ninguém remapeia quem guarda posição | 2 | ✅ provada |
| `alter-espelho-para-tras` | o espelho `.bkp` fica com a largura velha depois de acrescentar coluna | 1 | ✅ provada |
| `alter-queda-no-meio` | o conjunto de volumes misturado abre e lê o volume 3 com a largura do 1 | 2 | ✅ provada |
| `ffi-panico-atravessa` | o pânico atravessa a fronteira de C em vez de virar código de erro | 2 | ✅ provada |
| `ffi-panico-nao-envenena` | o punho continua sendo usado depois de um pânico capturado | 1 | ✅ provada |
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
| `recuperar-sem-reindexar` | a recuperação não reconstrói o `.ndx` que a queda deixou para trás | — | 🟰 redundante |
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
| `disco-silencio-furado` | todo erro de E/S manda um aviso: cem mil linhas, cem mil e-mails | 2 | ✅ provada |
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
| `cluster-devolve-a-credencial-na-tela` | o resumo do cluster na op `config` leva o token entre nós e o hash do replicador | 2 | ✅ provada |
| `cifra-do-odbc-volta-a-nascer-em-claro` | a receita do driver ODBC volta a nascer em claro, e o esquecimento vira o padrao | 5 | ✅ provada |
| `replica-lista-e-pedida-nao-imposta` | replicas_autorizadas vazia libera todos -- e so isso e' pedida, nao imposta | 1 | ✅ provada |
| `posicao-nao-encolhe-em-silencio` | tabela que nao abre some da soma do diario sem marcar `incompleta` | 1 | ✅ provada |
| `eleicao-prefere-completa` | `cluster::vencedor` volta a comparar so a posicao numerica, ignorando `incompleta` | 1 | ✅ provada |
| `replica-nao-atende-escrita` | `aplicar` pela rede deixa de exigir um papel que receba replicacao | 2 | ✅ provada |
| `spare-nao-atende-ninguem` | o papel Spare deixa de recusar toda operacao que nao esta em OPS_NO_SPARE | 1 | ✅ provada |
| `read-replica-recusa-escrita` | `ReadReplica` deixa de recusar escrita e para de apontar o primario | 1 | ✅ provada |
| `pulso-fora-da-lista-e-recusado` | `op_cluster_pulso` deixa de conferir o id contra a lista viva de nos | 1 | ✅ provada |
| `cifra-do-fio-imposta` | a cifra do fio EXIGIDA por padrão, quebrando todo cliente velho | — | 🪦 aposentada (18/09/2026) |

**154 das 199 guardas do catálogo: 1 aposentada, 148 provadas, 1 quebrada, 4 redundantes** — 3631 s de mutação, medido em 2026-09-16 15:25.

> **Esta rodada NÃO julgou 46 das 199 entradas do catálogo.** Elas não estão provadas nem reprovadas — a rodada não chegou nelas, e ler a tabela acima como inventário do catálogo a lê 46 entradas curta. Para julgá-las é preciso uma corrida do `provar-guardas.py` que as alcance.

- `teto-do-fio-sem-a-constante` — o `Canal::ler` de producao troca `TETO_DO_REGISTRO` por um teto quase infinito
- `teto-do-fio-sem-a-constante-no-soquete` — a mesma troca da constante por um teto quase infinito, vista pela rede
- `perfil-decide-so-pela-lista-e-nao-pelo-reg-cifrado` — o perfil.txt decide pela lista do config e a cifra acontece pela marca de coluna
- `perfil-grava-o-erro-que-cita-o-valor` — o perfil.txt tapa o pedido e grava o erro, que cita o valor da coluna marcada
- `profiler-ligado-sem-a-raiz-dos-dados` — o Profiler liga sem a raiz de dados e volta a decidir por um campo só
- `varredura-sem-o-elo` — a varredura barata do diretorio perde a tabela alcancada por elo
- `linha-vazia-na-conferencia-de-filhas` — a linha descida para a conferencia de filhas vai vazia, e toda mae parece sem filha
- `teto-de-64-bits-satura` — número cru fora da faixa do `Int8` é GRAVADO saturado, e `1e21`, `1e30` e `1e300` viram todos o mesmo número
- `saida-do-direito-por-coluna` — a recusa do direito por coluna manda «peça as colunas por varrer» também para o `agrupar` e para o `backup`
- `check-que-se-contradiz-no-alter` — `acrescentar_coluna` aceita um `padrao` que viola o `check` declarado no MESMO comando, e todo `atualizar` da linha velha passa a recusar
- `alter-com-regra-sem-aviso` — `acrescentar_coluna` com `check` ou `calculada` numa tabela com linha é aceito SEM AVISO, e a linha velha fica fora da regra
- `upsert-parcial-vira-mescla` — o upsert sem o campo `atualizar` passa a MESCLAR, e a sincronia do DbLink perde a única forma de gravar NULO num destino
- `direcao-do-indice-sem-saida` — a recusa por direção do índice explica bem por que não dá, e não diz o que fazer
- `sha256-sem-somar-o-estado` — SHA-256 sem a realimentação do estado: a compressão vira permutação reversível
- `sha256-com-o-tamanho-em-little-endian` — SHA-256 com o tamanho da mensagem, no padding, em little-endian
- `hmac-com-a-chave-longa-truncada` — HMAC com a chave maior que o bloco TRUNCADA em vez de pré-hasheada
- `pbkdf2-com-o-contador-de-bloco-parado` — PBKDF2 com o contador de bloco parado: saída longa repete o primeiro bloco
- `pbkdf2-sem-o-xor-acumulado` — PBKDF2 sem o XOR acumulado: vira HMAC aplicado N vezes
- `juntar-sem-portao` — `juntar` sem conferência própria: a tabela negada entra como lado B
- `unir-sem-portao` — `unir` sem conferência própria: a tabela negada entra na LISTA
- `diferencas-sem-portao` — `diferencas` sem conferência própria: a tabela negada entra em `a` ou em `b`
- `derivado-sem-portao` — o portão some do irmão `executar_derivado`: o SQL inteiro vira a porta dos fundos
- `ficha-do-usuario-devolve-o-hash` — a ficha do usuário passa a devolver o `senha_hash` junto
- `senha-em-claro-no-cadastro` — a senha entra no config.json em texto puro: o `cifrar` sai do caminho de gravação
- `senha-velha-fica-no-arquivo` — trocar a senha não leva junto a que estava em texto puro no arquivo
- `cifra-reserializa-a-senha` — o `para_json` da cifra devolve a senha de verdade em vez de «(oculta)»
- `debug-da-cifra-mostra-a-senha` — o `Debug` da cifra imprime a senha: um `dbg!` apressado a joga no log
- `profiler-sem-a-senha-dentro-do-sql` — o Profiler perde a senha que está DENTRO da frase SQL, e não num campo
- `comando-invalido-vira-texto-cru` — o SQL que o léxico recusa volta inteiro para o log, com a senha dentro
- `trilha-sem-o-nome-de-segredo` — a trilha LGPD deixa de olhar o NOME da coluna e só analisa o valor
- `trilha-so-olha-o-nome-da-coluna` — a trilha LGPD deixa de ANALISAR o valor e só confia no nome da coluna
- `debug-da-ligacao-mostra-a-senha` — o `Debug` da ligação de DbLink imprime a senha e o token do outro banco
- `fio-cifrado-manda-o-claro-junto` — o fio cifrado manda a linha em claro junto do registro selado
- `diario-das-diretivas-guarda-o-segredo-anterior` — o diário das diretivas grava o valor ANTERIOR do campo sigiloso em claro
- `token-do-rest-entra-pela-tela` — o token da porta REST passa a se gravar pela tela de configuração
- `receita-odbc-devolve-a-senha` — a connection string mascarada do ODBC devolve a senha inteira
- `cifra-do-fio-reserializa-a-privada` — o `para_json` da cifra do fio devolve a chave privada em vez de «(oculta)»
- `especificacao-openapi-leva-o-token` — a especificação OpenAPI, servida sem portão, passa a carregar o token da porta
- `token-remoto-fora-da-lista-de-segredos` — o `token_remoto` sai da lista de segredos: o token do OUTRO servidor vai em claro para o `perfil.txt` e para a op `profiler`
- `job-recusa-um-nome-e-grava-os-outros` — a guarda do job volta a recusar só `token`: `senha`/`token_remoto` vão para o `jobs.json` e voltam na ficha
- `config-json-escreve-aberto-e-herda` — o `config.json` volta a nascer na permissão do `umask` e a herdar o `0644` do original
- `laco-preso-no-unico-secundario` — chave duplicada num índice único secundário prende o laço do bidirecional para sempre
- `par-parado-reapresentado-a-cada-rodada` — a tabela parada por conflito volta a ser puxada a cada rodada, e o grito se repete para sempre
- `dado-pessoal-no-grito-do-conflito` — o grito do conflito de unicidade publica a coluna marcada como dado pessoal
- `so-o-disco-vem-da-porta-e-nao-de-desligar-depois` — o empilhar volta a abrir pela porta de sempre e desligar a sobreposicao na linha seguinte
- `slot-de-outro-reg` — o sal deixa de ser por arquivo: o slot cifrado de um `.reg` abre no outro

As guardas que esta corrida ainda cita, hoje aposentadas:

- `cifra-do-fio-imposta` (18/09/2026) — o defeito que ela repunha -- `cifra_fio.exigir: true` de fabrica -- virou o PRODUTO, por ordem do dono (*a comunicacao deve obrigatoriamente ser cifrada*, pedido 370). Guarda cujo defeito deixou de existir nao tem o que repor. Ela nao foi remendada para o numero fechar: nasceu no lugar dela a `cifra-do-fio-rebaixada`, que repoe o defeito CONTRARIO (a cifra voltar a ser opcional) e cuja prova e o mesmo teste, tambem trocado de lado (`o_cliente_velho_sem_o_escape_escrito_e_recusado_com_o_motivo`). O que a petrea *guarda nova entra pedida* continua protegendo ficou com o escape escrito, e ele esta no `seguem` da nova.

As notas que a rodada deixou:

- `cadeia-sem-teto` — o binario abortou, que e como esta guarda pega
- `aad-fora-do-slot` — confirmado: tirar so o AAD nao e sentido por teste nenhum, porque o `nonce_de_pedaco` carrega o ROWID. Medido em 03/09/2026, e nao deduzido: tirando o AAD e SO o rowid do nonce -- volume e contador ficando --, o teste CAI. Volume e versao nao entram nesta conta porque o teste copia o slot INTEIRO, e os dois slots moram no mesmo volume com a mesma versao
- `nonce-sem-endereco` — confirmado: tirar so o endereco do nonce tambem passa despercebido, porque o AAD carrega o ROWID. Medido em 03/09/2026: tirando o endereco do nonce e SO o rowid do AAD -- volume e versao ficando --, o teste CAI
- `trava-sem-guarda-de-reentrancia` — a rodada estourou o prazo do executor
- `ffi-panico-atravessa` — o binario abortou, que e como esta guarda pega
- `rest-fecha-sem-escoar` — confirmado: nenhum teste de unidade sente isto, e nao poderia -- o RST e do sistema operacional, e so aparece com um soquete de verdade. Quem pega e o passo 13 de `bancada/rest/provar.py`, e esta entrada existe para dizer, com o numero da rodada, que a cobertura mora la e nao aqui
- `recuperar-sem-reindexar` — confirmado: nenhum teste de unidade pega este defeito. O indice so fica para tras quando o PROCESSO morre no meio da passada, e isso so acontece de verdade em `bancada/transacoes/provar.py` -- que e por isso que a prova por soquete existe.
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
rodada de 17/09/2026.

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
