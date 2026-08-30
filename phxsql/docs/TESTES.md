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

`cargo test --workspace`: **1.229 testes, 0 falhas** (1.228 `#[test]` mais um
doc-test) — somado dos `test result:` de uma rodada, e não digitado. Por área,
contando `#[test]` por arquivo e agrupando:

<!-- cobertura:inicio -->
| área | testes | % |
|---|---:|---:|
| Motor de dados (arquivos, índice, diários) | 323 | 26,0 |
| Protocolo e portões (despachar) | 180 | 14,5 |
| Núcleo (JSON, tipos, UUID, zip, paralelo) | 133 | 10,7 |
| Configuração | 82 | 6,6 |
| Criptografia e codificação | 80 | 6,5 |
| DbLink | 65 | 5,2 |
| Telemetria e profiler | 45 | 3,6 |
| Camada SQL (léxico, sintaxe, tradução) | 44 | 3,5 |
| Gatilhos e procedimentos | 38 | 3,1 |
| Jobs | 31 | 2,5 |
| Interface web (servidor HTTP) | 28 | 2,3 |
| MCP | 19 | 1,5 |
| Usuários e permissões | 19 | 1,5 |
| **Console de terminal (phxsqlcmd)** | **18** | **1,5** |
| **Segurança de rede (blacklist, firewall)** | **18** | **1,5** |
| **ODBC** | **17** | **1,4** |
| **Mensagens (i18n do servidor)** | **14** | **1,1** |
| **Exportação** | **13** | **1,0** |
| **Junções e união** | **13** | **1,0** |
| **Pivot** | **12** | **1,0** |
| **Replicação** | **11** | **0,9** |
| **Servidor (outros)** | **9** | **0,7** |
| **Alertas e e-mail** | **8** | **0,6** |
| **CLI** | **7** | **0,6** |
| **Cluster** | **7** | **0,6** |
| **Monitor de máquina** | **6** | **0,5** |
| **total** | **1240** | |

Arquivos de `src` com mais de 120 linhas e **zero** `#[test]`:

| arquivo | linhas |
|---|---:|
| `phxsql-store/src/table.rs` | 2458 |
| `phxsql-store/src/ndx.rs` | 1580 |
| `phxsql-server/src/main.rs` | 452 |
| `phxsql-server/src/replica.rs` | 394 |
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

Como rodar, os treze casos e o que ela deliberadamente não faz:
`testes-web/LEIA-ME.md`.

O resumo do desenho:

- **Sobe o próprio servidor**, nas portas 6200/6201, num diretório temporário,
  e o derruba **pelo PID**. A senha não fica em claro em lugar nenhum: o hash
  sai do próprio `phxsqld --senha`, como no `bancada/replicacao/montar.py`.
- **Entra pela tela de login**, com o desafio-resposta de verdade. Se a página
  cair em modo demonstração, o caso falha — sem essa guarda a bateria inteira
  passaria sem tocar no motor.
- **Percorre 120 telas** clicando cada item dos nove menus e cada botão da
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

### 5.6 O `abrirAdmin` escreve na tela depois do `await`, sem perguntar se ainda é a dele

Achado por esta rodada e **medido**: `ui/index.html`, `abrirAdmin`, faz
`p.innerHTML = await vPainel()`. Entre o `await` e a escrita cabe qualquer
navegação, e quem clica em Configurações antes de o Painel terminar de carregar
fica com o **título de Configurações e o corpo do Painel**. A reprodução, a
sonda e o rastro estão em §9.8.

**Por que não consertei:** `ui/index.html` é a tela, e há frentes mexendo nela
nesta mesma rodada; o conserto é uma linha, mas a decisão é de quem manda no
Centro de Controle. Enquanto isso a parte `telemetria-cores` da bateria única
fica **vermelha**, que é o comportamento certo.

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

### As dezessete partes

| parte | o que prova | portas |
|---|---|---|
| `motor` | o motor, o protocolo e os portões — `cargo test --workspace` | — |
| `guardas` | que cada teste ainda **pega** o defeito que o motivou (§8) | — |
| `tela` | a interface contra o servidor de verdade: 120 telas, CSS global, contraste, primeira pintura | 6950/6951 |
| `idiomas` | o caminho do idioma de ponta a ponta, e o comportamento velho | 6952/6953 |
| `ponta-a-ponta` | os seis itens do dono pelo soquete, mais a passada pela tela | 6300/6301 |
| `rotinas` | gatilhos e procedimentos pelo soquete, com SIGNAL, lote e reinício | 5301/5701 |
| `profiler` | a redação do Profiler por soquete: vinte pedidos torcidos, sentinela no anel e no `.txt` | 6251 |
| `telemetria-desenho` | o painel de bolhas por medida: rótulo na esfera, alvo de clique, contraste | — |
| `telemetria-interacao` | clicar na bolha menor com o painel em movimento, descer de nível, voltar | — |
| `telemetria-cores` | as cores configuráveis, exercitando: escolher, salvar, conferir no painel | 6600/6601 |
| `cluster` | eleição e promoção automática com três servidores e um SMTP falso | 5310-5312, 5316 |
| `replicacao` | os quatro modos por soquete, com o comportamento velho no fim | 5330-5339 |
| `trava` | a trava de dados contra a leitura de rede: corte silencioso, alcance, queda de conexão e o abraço do bidirecional | 7050-7055 |
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
| `catraca-dos-textos` | mais um texto de tela cravado, fora da fábrica de idiomas | 1 | ✅ provada |
| `trava-atras-da-rede` | o laço da réplica segura a trava de dados enquanto lê do soquete | 1 | ✅ provada |

**19 guardas: 17 provadas, 2 redundantes** — 182 s de mutação, medido em 2026-08-30 06:10.

As notas que a rodada deixou:

- `cadeia-sem-teto` — o binario abortou, que e como esta guarda pega
- `aad-fora-do-slot` — confirmado: tirar so o AAD nao e sentido por teste nenhum, porque o `nonce_de_pedaco` carrega (rowid, volume, versao)
- `nonce-sem-endereco` — confirmado: tirar so o endereco do nonce tambem passa despercebido
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
