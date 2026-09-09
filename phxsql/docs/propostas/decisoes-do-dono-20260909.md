# As seis decisões que esperam o dono — 09/09/2026

Reunidas depois da rodada «Next» e da revisão. Cada uma tem o número já medido
e as saídas; a recomendação é do orquestrador e vale o que vale — quem decide é
o dono. Ordem do dono (tarefa 106): perguntar UMA A UMA. Este documento é o que
sobrevive à sessão; a pergunta viva vem quando o dono voltar.

# Pareceres para o dono — as decisões que só ele toma (09/09/2026)

Uma a uma, cada uma com o número já medido e as saídas; a recomendação é do
orquestrador e vale o que vale: quem decide é o dono.

1. **179 — construir a Sombra (leitura repetível) ou não.** Sete decisões suas
   já estão de pé (SOMBRA.md §0: versão velha em RAM, teto de 5 min, .ndx sem
   marca, promete snapshot isolation). Não compra desempenho (1,00×–1,21× em
   por_lote, medido 04/09) e não muda formato em disco. Custo em código (§3.4):
   portão por tabela, mapa por tabela, registro de visões abertas (a peça com
   trava dentro de trava, NÃO medida — o mapa da trava tem de rodar sobre o
   desenho antes do código) e a consulta em 28+10 seções de leitura. Fecha 2
   dos 4 fenômenos do ACID.md §4.1 (leitura não repetível e fantasma); não
   fecha perda de atualização nem skew. Saídas: (a) fazer agora, com o mapa da
   trava primeiro; (b) não agora, e a linha do comparativo continua NÃO com o
   motivo escrito. Recomendação: (b) até haver quem precise de leitura
   repetível de fato; é o único item da lista cujo custo não está inteiramente
   medido.
2. **194 — senha própria por tabela na cifra em repouso.** Medido: o cofre é
   um static do processo e a sessão é por conexão — nenhum dos dois guarda
   segredo; a alternativa (chave como parâmetro) toca 35 chamadas em 7
   arquivos e 3 crates. Recusado com número: cifrar o .ndx (0,23 µs/linha e
   não esconde a chave) e «senha por tabela resolve a replicação» (3 de 3
   casos falham). Saídas: (a) não fazer, e a proteção por tabela fica no
   direito por coluna que já existe; (b) fazer, pagando os 35 sítios.
   Recomendação: (a).
3. **197 — senha do banco vinda do login.** Sua palavra de 05/09: senha do
   SGBD e opcional por banco, e a senha do banco não pode ser a de
   administração (§13.12, argumento técnico que fecha). Medido: 17 sítios
   abrem tabela com sessão, 15 sem (6 de sistema, 8 de tabela de usuário, 1 de
   teste); a tranca do cache de derivadas já está provada 1/1; a senha do banco
   precisa viajar em claro (dentro do túnel) para derivar a chave. As três
   saídas da §13.11 são defensáveis; a diferença está nos 8 sítios de tabela
   de usuário que abrem sem sessão (jobs, réplica, gatilhos). Recomendação:
   a saída em que esses 8 recebem a senha do banco pela configuração do
   serviço (não pela sessão), porque é a única que não deixa job nem réplica
   sem acesso quando ninguém está logado.
4. **207 — quórum de escrita.** O canal existe (0,089 ms vazio, 0,466 ms com
   evento) e o campo cluster.quorum_minimo entrou com quorum_imposto=false.
   Falta a decisão 1 de 5: o que o «ok» da réplica significa — recebeu,
   aplicou (page cache, o padrão por_lote), ou aplicou e sincronizou. Sem ela
   o número publicado é o do Cassandra: soa como disco e é memória.
   Recomendação: «aplicou e sincronizou» (fsync na réplica antes do ok),
   porque é a única garantia que um cliente consegue nomear sem ler o manual;
   custa o fsync da réplica dentro do commit do master, e a bancada do caso
   ruim (réplica morta no meio) vem junto.
5. **211 — posição do diário com tabela que não abre.** Hoje a tabela ilegível
   é descartada e a posição publicada é menor que a real; a eleição compara
   esse número. Saídas: (a) recusar (a réplica não publica posição, sai da
   eleição até consertar); (b) publicar como incompleta, com a flag, e a
   eleição prefere posição completa. Recomendação: (b), porque (a) tira do
   ar uma réplica que ainda serve leitura, e (b) mantém a verdade visível
   no pulso e na tela.
6. **239 — isolamento acima de READ COMMITTED e TLS.** Isolamento é o item 1.
   TLS: a pétrea das zero dependências; TLS 1.3 em casa exige X.509/ASN.1,
   ECDSA P-256 e gestão de certificado; o Noise já protege a porta de dados e
   o WINDEV tem TLS e não tem Noise (o gargalo é o cliente). Saídas: (a) não
   fazer, e a linha do comparativo fica NÃO com o motivo; (b) exceção só na
   camada de rede: um proxy que termina TLS na frente do phxsqld (documentado,
   sem crate); (c) escrever TLS 1.3 em casa, provado contra vetores.
   Recomendação: (b) agora, (c) só se um cliente exigir TLS nativo.
