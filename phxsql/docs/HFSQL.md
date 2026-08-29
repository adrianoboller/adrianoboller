# O que o HFSQL(R) tem, e o que o PhxSql tem

Leitura da documentação técnico-comercial do HFSQL(R) (PC SOFT, versão
2013-10), item por item, contra o código deste projeto. O HFSQL(R) é o modelo
que o PhxSql copiou de propósito — **arquivos separados com papéis distintos** —,
então a comparação é legítima; ela também mostra 25 anos de distância.

Os números do PhxSql são medidos; o do HFSQL(R) é o que a folha declara.

---

## 1. O que já está aqui

| HFSQL(R) | PhxSql | |
|---|---|---|
| Arquivos separados por papel | `.reg` `.ndx` `.bin` `.memo` `.log` `.trash` `.reason` + `.bkp` `.pag` | **7 contra 4** — os deles são `.fic`/`.ndx`/`.mmo`/`.ftx` |
| Índice simples e **composto** | `IndexDef` com lista de colunas, livre ou única | igual |
| Restrição de unicidade | conferida **antes de gravar** | igual, com uma razão a mais: o `.reg` não reaproveita slot |
| Identificador automático | `Sequence`, `Uuid` v4/v7, `Uuid256` | o v7 deles é recente; aqui já é RFC 9562 |
| Chave primária e estrangeira | declaradas no esquema | FK **não é aplicada** na gravação — ver §3 |
| Tipos: texto, numérico, decimal, data, hora, booleano, blob/memo | todos | o decimal deles vai a 38 dígitos; aqui, 38 também (`i128`) |
| Autenticação com direitos granulares por servidor/banco/tabela | por usuário, por base **e por tabela**, 10 atividades | empatado desde a 0.17.0; o que falta dos dois lados é a **coluna** |
| Restringir acesso por IP | `ips_permitidos` + blacklist com bloqueio automático | aqui é mais rígido: violação grave bloqueia na primeira |
| Log de acessos, estatísticas de uso | `acessos.log`, percentis, histograma, uso por tabela | igual |
| Backup a quente, agendado | com manifesto SHA-256 e ZIP escrito aqui | o manifesto é uma coisa a mais |
| Replicação servidor→servidor | `.log` v2 com a imagem da linha, 4 servidores medidos | ver §4 |
| Cluster | **não** — ver `docs/CLUSTER.md` | |
| Importação de CSV/XML com separador configurável | JSON, CSV, TXT, XML **e HTML**, adivinhando o formato | um formato a mais, e a conferência antes de gravar |
| Exportação para vários formatos | XLSX, JSON, XML, HTML, CSV, DOCX, TXT | igual |
| Tabela dinâmica (ROLAP) | Pivot com *hash join*, teto de 500.000 | igual |
| Ferramenta de administração gráfica | Centro de Controle na web, sem instalar nada | a deles é executável Windows |
| Monitor de máquina (CPU, memória, rede, disco) | painel com sete gráficos numa chamada | igual |
| Aviso por e-mail quando detecta incidente | alerta de disco por SMTP escrito aqui | o deles cobre mais casos |
| Unicode | UTF-8 em todo campo de texto | o deles ordena por idioma; aqui, não — ver §3 |

---

## 2. O que o PhxSql tem e o HFSQL(R) não

Curto, e vale registrar:

- **Ordem de digitação garantida por formato.** O `.reg` nunca reaproveita
  slot. Nenhum motor da lista promete isso.
- **Endereço por conta, não por busca.** `offset = data_offset + (rowid−1) ×
  slot_size`. É o que faz o salto para a página 500 custar 6 ms.
- **Zero dependência externa.** A imagem Docker é `scratch`, 4,7 MB.
- **A exclusão que deixa rastro.** `.trash` com o conteúdo dos anexos e
  `.reason` com o porquê, quem e quando — sobrevivendo à linha.
- **Profiler que mostra o texto do pedido antes de ele virar dado**, com a
  senha redigida.

---

## 3. O que falta, e que a leitura do PDF trouxe à tona

Em ordem de valor, na minha leitura:

### 3.1 Direitos no nível da TABELA

O HFSQL(R) afina direito por servidor, por banco **e por tabela**, e a lista
dele é granular a ponto de separar «direito de ler as linhas» de «direito de
iniciar uma reindexação».

**Feito na 0.17.0.** Dentro do objeto da base, `"tabelas"` escreve a regra de
cada tabela, e ela **substitui** a da base ali — a mesma coisa que a base já
fazia com o `"*"`. É o que permite as duas coisas que a prática pede: tirar
`folha` de quem lê o banco inteiro, e dar `clientes` a quem não lê o banco
nenhum. Uma regra de *interseção* resolveria só a primeira.

O portão continua sendo **um só** — espalhado por quarenta operações, a que
alguém esquecesse de conferir viraria a porta dos fundos. Duas operações
precisaram de conferência própria porque não têm o campo `"tabela"` que o
portão lê: `juntar`, cujas tabelas estão em `a.tabela` e `b.tabela`, e `unir`,
cujas tabelas estão numa lista. Sem isso, bastaria pedir a tabela negada como o
lado B de uma junção.

A árvore e o catálogo passaram a listar só o que dá para abrir: o nome de uma
tabela já conta parte da história.

Detalhes em `docs/USUARIOS.md`. O que ainda não desce é o direito por
**coluna** — esconder o salário dentro de uma tabela que a pessoa pode ler.

### 3.2 Índice de texto completo (*full text*)

Eles acham uma palavra em um milhão de linhas em menos de 2 ms. Aqui, procurar
uma palavra dentro de um `.memo` é varredura. É um índice invertido por termo —
um arquivo novo, `.fts`, com o mesmo desenho de página do `.ndx`.

### 3.3 Índice parcial e coluna calculada

Índice só sobre as linhas que atendem a uma condição (o `WHERE` do índice) e
coluna cujo valor sai de uma expressão. Os dois são baratos no formato atual e
mudam muito o custo de consultas comuns.

### 3.4 Ordenação linguística

Eles ordenam índice pela ordem alfabética do idioma — russo, chinês de Taiwan,
etc. Aqui há `NOCASE`, e só. Para português a diferença aparece no acento: hoje
«Álvaro» não cai junto de «Alvaro» no índice. A partição alfanumérica já tem uma
tabela de dobra de acento escrita à mão; ela é o começo desse caminho.

### 3.5 A janela de conflito de escrita

A tela mais interessante do PDF inteiro: dois usuários alteram a mesma linha e
o segundo recebe uma janela com **três colunas** — «valor anterior», «o outro
escreveu», «você escreve» — e escolhe. É a resolução de conflito no lugar certo:
na frente de quem sabe o que fazer, e não num log que ninguém lê.

**Feito na 0.17.0**, e sem mudar formato: o `.reg` já guardava uma **versão por
registro** desde a v1, que sobe a cada regravação. A ficha lê a linha e guarda a
versão; o «Salvar» manda a versão de volta; o servidor recusa com o erro 3004
(`CONFLITO`) quando ela não é mais a atual. Conferir custa 24 bytes de leitura.

A janela mostra as três colunas do PDF e vai um passo além dele: **já vem
marcado quem mexeu em cada coluna**. A que você digitou fica com o seu valor, a
que só o outro mudou fica com o dele — duas pessoas que editaram campos
diferentes da mesma linha saem daí com os dois trabalhos preservados, sem ter de
escolher nada. Marcar tudo como «o meu» por omissão desfaria em silêncio o
trabalho do outro nas colunas que eu nem toquei, que é o mesmo estrago de antes
com mais cliques.

Três decisões que valem registro:

- **Não é trava.** Travar a linha na leitura resolveria o mesmo problema e
  criaria dois piores: a linha fica presa quando alguém fecha o navegador com a
  ficha aberta, e duas sessões que travam em ordem trocada se abraçam. O
  contador não prende nada — só recusa a segunda gravação.
- **A conferência é pedida, não imposta.** Quem manda `"versao"` ganha a
  garantia; quem não manda continua com a última gravação vencendo. Imposta,
  todo cliente escrito antes da 0.17.0 pararia de gravar de um dia para o
  outro — e o que ele receberia não seria proteção, seria um erro que ele não
  sabe tratar. A interface web manda sempre, porque é ali que existe gente e a
  janela de minutos entre abrir a ficha e clicar em salvar.
- **Excluída de vez também é conflito**, e não «não encontrado»: quem leu a
  linha há um minuto precisa saber que ela foi apagada, e não que o rowid nunca
  existiu.

### 3.6 Bloqueio de linha e de tabela

Eles travam por linha, automaticamente. Aqui há uma trava global única — todo
acesso a dado se serializa. É correto e é lento sob carga; está no roteiro como
«trava por tabela», e a de linha viria depois.

### 3.7 Reconexão automática

O cliente deles reconecta sozinho quando a conexão cai. Aqui a réplica reconecta
(é o laço dela), mas o cliente comum não — quem escreve a aplicação trata.

### 3.8 Comparar duas tabelas (o WDHFDiff deles)

Comparar **estrutura** e **dados** de duas tabelas. A estrutura é fácil: o bloco
de esquema já se serializa, então é comparar dois blocos. Os dados também têm
meio caminho: já existe uma soma de verificação de tabela, que diz *se* diferem
sem transportar nada. Falta dizer **onde**.

### 3.9 GDPR / LGPD: marcar a coluna como dado pessoal

Uma marca por coluna dizendo «isto é dado pessoal», e uma tela que audita onde
eles estão. O cadastro de campos já tem `caption`, `descricao` e `mascara` — é
mais um campo, e uma consulta ao dicionário. **Barato, e num assunto que hoje é
obrigação legal, não enfeite.**

### 3.10 O que eles têm e que aqui é projeto grande

Transações ACID com quatro níveis de isolamento; gatilhos e procedimentos
guardados; ODBC e OLE DB; camada SQL. Todos já estão em `docs/PENDENCIAS.md`, e
nenhum é «faltou notar» — são decisões de sequência.

---

## 4. Replicação: o desenho deles e o nosso

Eles têm quatro tipos: entre servidores HFSQL(R), com bancos heterogêneos
(Oracle, por exemplo), com dispositivos móveis, e **offline** (sem link
permanente). Aqui há o primeiro, com um assistente a menos e um número a mais:
4.273 eventos/s por réplica, quatro servidores, retrato SHA-256 idêntico.

A replicação **offline** é a mais interessante das que faltam, e o formato quase
a permite de graça: a posição é o ordinal do evento no `.log`, então um arquivo
com os eventos de um intervalo é um pacote de sincronização. Falta o empacotador
e a conferência de conflito.

---

## 5. Onde a comparação é desconfortável

Duas coisas que a folha deles diz e que aqui são falsas, e vale escrever:

- **«ACID»**. Não há transação aqui. A inserção desfaz o que gravou se um índice
  falhar, e é só isso.
- **Volume.** Eles falam em «mais de 1 TB» e «300 milhões de linhas em cartão de
  memória» em depoimentos de clientes. Aqui o maior teste medido é de 10 milhões
  de linhas, numa máquina só.

E uma na direção contrária: a folha deles não publica um único número de
desempenho reproduzível. Todos os desta comparação são refazíveis com os
comandos de `bancada/`.
