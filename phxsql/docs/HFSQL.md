# O que o HFSQL(R) tem, e o que o PhxSql tem

Leitura da documentação técnico-comercial do HFSQL(R) (PC SOFT, versão
2013-10), item por item, contra o código deste projeto. O HFSQL(R) é o modelo
que o PhxSql copiou de propósito — **arquivos separados com papéis distintos** —,
então a comparação é legítima.

**Duas honestidades antes da primeira linha, e as duas mudam como se lê o
resto:**

1. **O lado deles é de 2013 e não se remede aqui.** A folha não está nesta
   máquina; o que este documento sabe do HFSQL(R) é o que a leitura de então
   registrou. Onde o produto deles andou desde 2013 — e andou —, esta
   comparação não enxerga. **Vantagem nossa medida contra folha de treze anos
   atrás não é vantagem provada contra o produto de hoje.**
2. **O lado daqui se remede, e foi remedido em 06/09/2026** contra a 0.18.0.
   Quatro verdictos deste documento tinham vencido; estão corrigidos, e a §6
   diz quais eram — porque veredito velho é pior que veredito ausente: o
   ausente ninguém cita.

Os números do PhxSql são medidos e refazíveis pelos comandos de `bancada/`; o
do HFSQL(R) é o que a folha declara.

---

## 1. O que já está aqui

| HFSQL(R) | PhxSql | |
|---|---|---|
| Arquivos separados por papel | `.reg` `.ndx` `.bin` `.memo` `.log` `.trash` `.reason` + `.bkp` `.pag` `.lgpd` `.tx` | os deles são `.fic`/`.ndx`/`.mmo`/`.ftx` |
| Índice simples e **composto** | `IndexDef` com lista de colunas, livre ou única | igual |
| Restrição de unicidade | conferida **antes de gravar** | igual, com uma razão a mais: o `.reg` não reaproveita slot |
| Identificador automático | `Sequence`, `Uuid` v4/v7, `Uuid256` | o v7 deles é recente; aqui já é RFC 9562 |
| Chave primária e estrangeira | declaradas no esquema, e a declarada **nasce conferida** | a diferença que sobra é o `SET NULL`, que aqui não existe por pétrea (`INTEGRIDADE.md` §1) |
| Tipos: texto, numérico, decimal, data, hora, booleano, blob/memo | todos | o decimal deles vai a 38 dígitos; aqui, 38 também (`i128`) |
| Direitos granulares por servidor/banco/tabela | por usuário, por base **e por tabela**, 10 atividades | empatado desde a 0.17.0; o que falta **dos dois lados** é a coluna |
| Restringir acesso por IP | `ips_permitidos` + blacklist com bloqueio automático | aqui é mais rígido: violação grave bloqueia na primeira |
| Log de acessos, estatísticas de uso | `acessos.log`, percentis, histograma, uso por tabela | igual |
| Backup a quente, agendado | com manifesto SHA-256 e ZIP escrito aqui | o manifesto é uma coisa a mais |
| Replicação servidor→servidor | `.log` v2 com a imagem da linha, 4 servidores medidos | ver §4 |
| Cluster | **sim**, com eleição e promoção automática, medido em `bancada/cluster/` | ver §6, verdicto 5 |
| Transação | `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`, com escopo, prazos e travas | ver §3.1: o que falta não é a transação, é o **nível de isolamento** |
| Gatilhos e procedimentos guardados | sintaxe similar à do MySQL(R)/MariaDB(R), um interpretador só | entram pela op `sql`, não por operação própria (`TRIGGERS.md`) |
| ODBC | driver ODBC 3.x de verdade, `cdylib` de ABI C, 73 conferências | OLE DB pela ponte oficial `MSDASQL` — recusa fundamentada, não pendência (`ODBC.md` §6–7) |
| Marcar coluna como dado pessoal (GDPR) | `dado_pessoal` no esquema, ops `marcar_lgpd`, `dados_pessoais` e `trilha`, arquivo `.lgpd` | ver §6, verdicto 3 |
| Importação de CSV/XML com separador configurável | JSON, CSV, TXT, XML **e HTML**, adivinhando o formato | um formato a mais, e a conferência antes de gravar |
| Exportação para vários formatos | XLSX, JSON, XML, HTML, CSV, DOCX, TXT | igual |
| Tabela dinâmica (ROLAP) | Pivot com *hash join*, teto de 500.000 | igual |
| Ferramenta de administração gráfica | Centro de Controle na web, sem instalar nada; **122 operações** no protocolo | a deles é executável Windows |
| Monitor de máquina (CPU, memória, rede, disco) | painel com sete gráficos numa chamada | igual |
| Aviso por e-mail quando detecta incidente | alerta de disco por SMTP escrito aqui | o deles cobre mais casos |
| Unicode | UTF-8 em todo campo de texto | o deles ordena por idioma; aqui, não — §3.4 |

As **122** operações são contadas do próprio `catalogo.rs` (entradas do array
`OPERACOES`), não lembradas. O `PENDENCIAS.md` #30 ainda publica 108, que era o
número de uma rodada anterior.

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
- **A janela de conflito marca quem MEXEU, não quem perguntou por último** — a
  tela deles escolhe uma coluna inteira; a daqui resolve por coluna, e dois que
  editaram campos diferentes saem com os dois trabalhos.

---

## 3. O que falta — e é isto que responde «o que falta para ser melhor»

Em ordem de valor, e cada um medido em 06/09/2026 contra a 0.18.0.

### 3.1 Leitura repetível — o buraco mais caro, porque parece fechado

A transação existe. O **isolamento** não está inteiro, e o `docs/ACID.md` o diz
com a prova: leitura suja não acontece e a transação enxerga a própria escrita,
mas *«leitura repetível não existe: entre duas instruções tudo pode mudar»* —
fantasma, leitura não repetível e **skew de escrita** acontecem, e estão
medidos. Nenhum ajuste de configuração compra leitura repetível hoje.

Eles anunciam quatro níveis de isolamento. **Este é o item em que ter o verbo
sem o nível é mais perigoso que não ter o verbo**, porque quem lê `BEGIN`
supõe o resto.

### 3.2 Índice de texto completo (*full text*)

Eles acham uma palavra em um milhão de linhas em menos de 2 ms. Aqui, procurar
uma palavra dentro de um `.memo` é **varredura** — medido: nenhum módulo do
motor implementa índice invertido (a única ocorrência de «fts» no código é a
fábrica de idiomas, que é outra coisa). É um arquivo novo, `.fts`, com o mesmo
desenho de página do `.ndx`.

### 3.3 Trava por linha

Eles travam por linha, automaticamente. Aqui há trava global, e desde 05/09 com
**duas fichas**: a leitura de grade (`varrer`) toma a compartilhada e deixa de
esperar outra leitura; toda escrita continua na exclusiva. É correto e continua
lento sob carga de escrita. A trava **por tabela** está respondida e **recusada
com número** — ler em tabelas separadas não é mais rápido que ler na mesma
(≈1,00× em quatro medições): não é a tabela que serializa. A de linha viria
depois. `docs/CONCORRENCIA.md` §11.1 e §16.

### 3.4 Ordenação linguística

Eles ordenam índice pela ordem alfabética do idioma. Aqui há `NOCASE`, e só —
medido: **zero** ocorrências de colação no motor. Para português a diferença
aparece no acento: hoje «Álvaro» não cai junto de «Alvaro» no índice. A
partição alfanumérica já tem uma tabela de dobra de acento escrita à mão; ela é
o começo desse caminho.

### 3.5 Índice parcial e coluna calculada

Índice só sobre as linhas que atendem a uma condição, e coluna cujo valor sai
de uma expressão. Medido: **zero** ocorrências de índice parcial. Os dois são
baratos no formato atual e mudam muito o custo de consultas comuns.

### 3.6 Direito por COLUNA

O direito desce até a tabela e para aí — `docs/USUARIOS.md` o escreve sem
enfeite. Esconder o salário dentro de uma tabela que a pessoa pode ler não se
faz. **Falta dos dois lados**, pela leitura de 2013; se o produto deles ganhou
isso desde então, esta linha está errada e a §0 já avisa por quê.

### 3.7 O que a camada SQL não tem embaixo

Quatro peças, e nenhuma é «faltou notar» — são decisões de sequência
(`docs/SQL.md` §3):

- **Expressão.** `WHERE preco * 1.1 > 100` não tem quem avalie.
- **Planejador.** Escolher qual índice usar quando há dois candidatos; hoje
  quem chama escolhe pelo nome.
- **`GROUP BY` geral.** O `pivotar` faz a tabulação cruzada, que é um caso.
- **Subconsulta e CTE.** Não há.

O que **não** falta mais nessa lista é a transação, e a §3 do `SQL.md` ainda a
lista — é a mesma vencida da §6 aparecendo num segundo lugar. Corrigir ali é
trabalho de outra rodada, e fica registrado aqui para não se perder.

### 3.8 Comparar duas tabelas (o WDHFDiff deles)

Comparar **estrutura** e **dados**. A estrutura é fácil: o bloco de esquema já
se serializa. Os dados têm meio caminho: já existe soma de verificação de
tabela, que diz *se* diferem sem transportar nada. **Falta dizer onde** — e não
há operação de comparação no catálogo.

### 3.9 Reconexão automática do cliente

O cliente deles reconecta sozinho. Aqui a réplica reconecta (é o laço dela), e o
cliente comum não — quem escreve a aplicação trata.

### 3.10 Volume provado

Eles citam depoimentos de mais de 1 TB e 300 milhões de linhas. Aqui o maior
teste medido é **10 milhões de linhas, numa máquina só**. Não é afirmação de que
não escala: é a afirmação de que **não foi medido acima disso**, que é coisa
diferente e a única que se pode fazer.

---

## 4. Replicação: o desenho deles e o nosso

Eles têm quatro tipos: entre servidores HFSQL(R), com bancos heterogêneos, com
dispositivos móveis, e **offline** (sem link permanente). Aqui há o primeiro,
com quatro modos (A source→réplica, B multi-master, C spare, D read replica),
agendamento por origem, cascata medida, cifra do fio, e 17.450 eventos/s por
réplica com retrato SHA-256 idêntico (`docs/DESEMPENHO.md` §4.5).

A replicação **offline** é a mais interessante das que faltam, e o formato quase
a permite de graça: a posição é o ordinal do evento no `.log`, então um arquivo
com os eventos de um intervalo é um pacote de sincronização. Falta o empacotador
e a conferência de conflito.

E o `docs/REPLICACAO.md` §13 lista sete itens ainda abertos do que já existe —
entre eles **buscar o lote fora da trava de dados** (medido: `varrer` esperou
30,7 s numa réplica cortada em silêncio), que é o de maior consequência.

---

## 5. Onde a comparação é desconfortável

- **«ACID».** Já não é falso, e ainda não é verdadeiro: as quatro letras estão
  medidas uma a uma em `docs/ACID.md`, e a resposta não é sim nem não para
  nenhuma. O que impede a frase é a §3.1 — sem leitura repetível, o **I** não
  fecha. Continua valendo **não escrever *ACID compliant* em documento
  técnico** enquanto a decisão da §7 daquele documento não for tomada.
- **Volume.** §3.10.
- **A folha deles não publica um único número de desempenho reproduzível.**
  Todos os desta comparação são refazíveis com os comandos de `bancada/`. Isto
  continua sendo a vantagem estrutural do lado daqui, e é a única que não
  envelhece com o produto deles.

---

## 6. Os cinco verdictos que este documento publicou errados

Escrito porque a lição é maior que a correção: **um documento de comparação
envelhece pelo lado que anda**, e o lado que anda é o nosso. Cada linha abaixo
esteve neste arquivo dizendo que faltava algo que já existia.

| o que dizia | o que é, medido em 06/09/2026 |
|---|---|
| 1. «Não há transação aqui» (§5) | Há: `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`, e desde o pedido 162 a transação vê a própria escrita. O que falta é o **nível**, não o verbo (§3.1) |
| 2. «Gatilhos e procedimentos guardados… projeto grande» (§3.10) | Existem, com um interpretador só para os dois, entrando pela op `sql` (`TRIGGERS.md`) |
| 3. «GDPR: marcar a coluna como dado pessoal… barato, e falta» (§3.9) | Existe: `dado_pessoal` no esquema, ops `marcar_lgpd`/`dados_pessoais`/`trilha`, arquivo `.lgpd` |
| 4. «ODBC e OLE DB… projeto grande» (§3.10) | O ODBC existe e está provado com 73 conferências pela ABI literal; o OLE DB é recusa fundamentada com caminho (`MSDASQL`) |
| 5. «Cluster — **não**» (§1) | Há cluster com eleição e promoção automática desde o pedido 126, medido em `bancada/cluster/` |

**O padrão dos cinco é um só, e é o que interessa guardar:** todos eram
verdicto de *ausência*. Um número errado alguém reconfere quando bate o olho;
uma ausência declarada ninguém reconfere, porque não há o que olhar. **Ausência
escrita se remede por data, não por suspeita** — e é por isso que este documento
passa a trazer a data da remedição no alto, como as páginas de teste já trazem.
