# O critério de outro motor é a marca do canal de ataque DELE — copiar o critério sem o canal erra

**Estado:** PENDENTE

## O que aconteceu

Pedido 481, segunda volta, depois do BLOQUEIO da revisão SEC. A primeira volta
tinha levado à régua a pergunta «o arquivo de dono diferente se ignora?» e
contado MySQL e MariaDB a favor, porque os dois «ignoram o `my.cnf` inseguro
com aviso». A SEC apontou que o critério deles é o MODO (gravável por todos),
não o dono. Refazendo o ciclo do papel J, medi pelos binários desta máquina
(24/09/2026): o `mysqld` 8.0.46 **lê** um `my.cnf` de `nobody` com 0644 e com
0600, e um plantado de `nobody` num `!includedir` — e só ignora o 0666; o
`postgres` 16.13 **lê** um `config_file=` de `nobody` 0666 numa pasta 1777 e
sobe com o valor plantado, e só recusa o DIRETÓRIO de dados aberto ou de outro
dono. Nenhum dos dois ignora arquivo por dono.

## O que eu concluí primeiro, e estava errado

Que a régua, dando 5 × 4 para «ignorar com aviso», autorizava ignorar por
qualquer critério que parecesse mais forte — e que, se o critério deles (modo)
não pegava o arquivo plantado no nosso cenário, isso era só uma diferença de
detalhe. Não era: o critério é a metade da decisão que a régua NÃO mede. O
comportamento («ignorar com aviso») e o critério («o que conta como
inseguro») são duas perguntas, e a primeira volta respondeu a segunda com o
voto da primeira.

## O que a medição disse

O próprio fonte do MySQL diz de onde vem o critério:
`mysys/my_default.cc:1737-1749`, *«This is mainly done to protect us to not
read a file created by the mysqld server»* — e a documentação do
`SELECT … INTO OUTFILE` diz que, até a 8.0.17, o arquivo criado pelo servidor
era gravável por todos. O `0666` é a MARCA que o canal de ataque deles (um
usuário de SQL escrevendo arquivo pelo servidor) deixa. O nosso canal — um
usuário comum numa pasta com sticky bit — não deixa essa marca (ele escolhe o
modo), mas deixa outra que não forja: o dono do nome. Contas: «onde outros
gravam é inseguro» converge nos três (9); «ignorar × recusar» dá 5 × 4, com o
MariaDB (3) lido no fonte e não medido pelo binário nesta volta — sem ele,
2 × 4. Um ponto de margem virou exceção ESTREITA: sticky bit, pasta gravável
por outros, root e dono da pasta fora da conta de terceiro.

## A regra

**Ao aplicar a régua dos motores, separe o comportamento do critério: o voto
decide o comportamento; o critério se re-decide perguntando qual ameaça ele
marca no motor de origem, e qual marca o NOSSO canal de ataque deixa.**

## Como está guardado hoje

A conta e a tabela estão em `docs/SEGURANCA.md` §23; a regra em
`crates/phxsql-server/src/config_phz.rs::terceiro_no_par`, com um teste por
condição e as guardas `config-phz-par-*` em `bancada/guardas/catalogo.py`.
**Buraco que fica:** o MariaDB não foi medido pelo binário nesta volta (o
docker estava parado) — o voto dele saiu do fonte. Não há régua que obrigue a
registrar, na conta, qual peso veio do binário e qual veio do fonte.
