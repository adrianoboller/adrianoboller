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
`cargo test --workspace`: **1.585 testes, 0 falhas** — somado dos `test result:` de uma rodada de verdade, e não digitado: quem escreve este número é `docs/dossie/numeros-do-projeto.py`, e ele **aborta se a suíte falhar**.
<!-- testes:total:fim --> Por área,
contando `#[test]` por arquivo e agrupando:

<!-- cobertura:inicio -->
| área | testes | % |
|---|---:|---:|
| Motor de dados (arquivos, índice, diários) | 418 | 26,4 |
| Protocolo e portões (despachar) | 236 | 14,9 |
| Núcleo (JSON, tipos, UUID, zip, paralelo) | 138 | 8,7 |
| Criptografia e codificação | 122 | 7,7 |
| Configuração | 93 | 5,9 |
| DbLink | 81 | 5,1 |
| Servidor (outros) | 70 | 4,4 |
| Camada SQL (léxico, sintaxe, tradução) | 55 | 3,5 |
| Telemetria e profiler | 53 | 3,3 |
| Gatilhos e procedimentos | 42 | 2,7 |
| Mensagens (i18n do servidor) | 32 | 2,0 |
| Jobs | 31 | 2,0 |
| Interface web (servidor HTTP) | 28 | 1,8 |
| **MCP** | **19** | **1,2** |
| **Usuários e permissões** | **19** | **1,2** |
| **Console de terminal (phxsqlcmd)** | **18** | **1,1** |
| **Segurança de rede (blacklist, firewall)** | **18** | **1,1** |
| **ODBC** | **17** | **1,1** |
| **Transações** | **16** | **1,0** |
| **Exportação** | **13** | **0,8** |
| **Junções e união** | **13** | **0,8** |
| **Pivot** | **12** | **0,8** |
| **Replicação** | **11** | **0,7** |
| **Alertas e e-mail** | **8** | **0,5** |
| **CLI** | **7** | **0,4** |
| **Cluster** | **7** | **0,4** |
| **Monitor de máquina** | **6** | **0,4** |
| **total** | **1583** | |

Arquivos de `src` com mais de 120 linhas e **zero** `#[test]`:

| arquivo | linhas |
|---|---:|
| `phxsql-store/src/table.rs` | 4309 |
| `phxsql-store/src/ndx.rs` | 1580 |
| `phxsql-ffi/src/lib.rs` | 1446 |
| `phxsql-server/src/main.rs` | 488 |
| `phxsql-server/src/replica.rs` | 412 |
| `phxsql-ffi/src/valor.rs` | 290 |
| `phxsql-store/src/integridade.rs` | 278 |
| `phxsql-server/src/dblink/conexao.rs` | 275 |
| `phxsql-server/src/carga.rs` | 226 |
| `phxsql-ffi/src/punho.rs` | 188 |
| `phxsql-cmd/src/main.rs` | 171 |
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
| `catraca-dos-textos` | mais um texto de tela cravado, fora da fábrica de idiomas | 1 | ✅ provada |
| `trava-fora-do-ponto-unico` | uma tomada da trava de dados fora do `travar_dados()` | 1 | ✅ provada |
| `trava-sem-guarda-de-reentrancia` | a trava pedida duas vezes pela mesma thread pendura o servidor | 1 | ✅ provada |
| `exclusao-na-janela-por-padrao` | a exclusão entra na janela por padrão, sem ninguém pedir | 1 | ✅ provada |
| `exclusao-na-janela-sem-leitor` | `exclusao_na_janela` no config.json, no MANUAL e na tela — e ninguém o lê | 1 | ✅ provada |
| `reg-fecha-antes-do-trash` | a janela sincroniza o `.reg` antes do `.trash` | 1 | ✅ provada |
| `rodizio-do-profiler-ignora-o-zero` | `profiler.arquivo_mib: 0` deixa de querer dizer «sem rodízio» | 1 | ✅ provada |
| `cabecalho-do-profiler-forjado` | o cabeçalho do arquivo do Profiler aceita linha forjada | 1 | ✅ provada |
| `profiler-sem-descritor-calado` | sem descritor, com arquivo pedido, a linha some sem ser contada | 1 | ✅ provada |
| `trava-atras-da-rede` | o laço da réplica segura a trava de dados enquanto lê do soquete | 1 | ✅ provada |
| `ordem-pequena-aceita` | o segredo X25519 todo-zeros aceito como chave de sessão | 2 | ✅ provada |
| `contador-do-fio-parado` | o contador de registros do fio parado — nonce repetido | 3 | ✅ provada |
| `fio-cortado-vira-fim` | o fio cortado no meio devolvido como fim de conversa | 1 | ✅ provada |
| `cifra-do-fio-imposta` | a cifra do fio EXIGIDA por padrão, quebrando todo cliente velho | 1 | ✅ provada |
| `transcricao-sem-o-cifrado` | o hash da transcrição sem o texto cifrado da mensagem 2 | 2 | ✅ provada |
| `fio-sem-teto-de-registro` | a leitura do fio volta a ser ilimitada | 1 | ✅ provada |
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

**84 guardas: 80 provadas, 4 redundantes** — 884 s de mutação, medido em 2026-09-05 03:18.

As notas que a rodada deixou:

- `cadeia-sem-teto` — o binario abortou, que e como esta guarda pega
- `aad-fora-do-slot` — confirmado: tirar so o AAD nao e sentido por teste nenhum, porque o `nonce_de_pedaco` carrega o ROWID. Medido em 03/09/2026, e nao deduzido: tirando o AAD e SO o rowid do nonce -- volume e contador ficando --, o teste CAI. Volume e versao nao entram nesta conta porque o teste copia o slot INTEIRO, e os dois slots moram no mesmo volume com a mesma versao
- `nonce-sem-endereco` — confirmado: tirar so o endereco do nonce tambem passa despercebido, porque o AAD carrega o ROWID. Medido em 03/09/2026: tirando o endereco do nonce e SO o rowid do AAD -- volume e versao ficando --, o teste CAI
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
catálogo completo — agora **60 entradas**, medido com
`python3 -c "import catalogo; print(len(catalogo.GUARDAS))"` — rodou inteiro
depois das três novas: **56 provadas, 4 redundantes, 0 não pegaram, 0
estragaram, 0 quebradas** (`bancada/guardas/provar-guardas.py`, tabela acima
regravada por `tabela-no-testes.py` a partir dessa rodada). As duas últimas
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

### 13.4 O placar do dia

| | antes desta rodada | depois |
|---|---|---|
| botões da tela | 298 | 298 |
| clicados pela bateria | **28** | **85** |
| dispensados com motivo | 0 | 3 |
| **sem prova** | **268** | **211** |

`TETO_BOTAO_SEM_PROVA = 211`, em
`crates/phxsql-server/src/conferidor_botoes.rs`. **Só desce.**

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

### 13.7 O que ficou de fora, nomeado

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
`--example botoes-sem-prova` lista os **211** que faltam, por tela, do maior
lote para o menor.
