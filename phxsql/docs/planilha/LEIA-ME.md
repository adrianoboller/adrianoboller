# A planilha das atividades

    python3 docs/planilha/planilha-das-atividades.py [saida.xlsx]

Sai em `docs/planilha/atividades-phxsql.xlsx`, com cinco abas.

## Ela nao tem leitor proprio de nada, e isso e' a regra

O gerador **importa** os leitores que ja existem em vez de escrever os seus:

| aba | de onde vem | pelo leitor de |
|---|---|---|
| Resumo | tudo abaixo + `CAPABILITIES.json` | — |
| Pedidos | `docs/PENDENCIAS.md` | `docs/dossie/pagina-dos-pedidos.py` |
| Board PMO | `docs/pmo/BACKLOG.md` | `docs/pmo/rollup.py` |
| Decisoes do dono | `PENDENCIAS.md` x lexico | `docs/pmo/pagina-do-status-do-projeto.py` |
| Rodada `<data>` | `git log` do dia | — |

*Receita duplicada e receita que diverge.* Uma planilha que contasse os pedidos
por conta propria divergiria da pagina dos pedidos no primeiro pedido com
estado esquisito — foi exatamente assim que o pedido 150 passou meses invisivel
na pagina, com um simbolo que o leitor nao conhecia. Hoje o leitor **para** com
o numero da linha em vez de seguir calado, e a planilha herda essa parada.

## O `openpyxl` nao fere a petrea de zero dependencias

A petrea diz «so a `std`», e ela e' do **motor em Rust**: o que se entrega ao
dono nao carrega biblioteca de ninguem. O `openpyxl` e' ferramenta de
**trabalho**, da mesma classe do Playwright que roda a bateria de tela e do
`strace` que conta os `fsync` — nenhum dos tres entra no produto.

Se um dia ele sumir da maquina, o conserto e' escrever o `.xlsx` a mao com
`zipfile` + XML da biblioteca padrao: o formato e' um zip de XML, e esta casa
ja escreveu SHA-256, HMAC, PBKDF2, CRC-32 e JSON do zero pelo mesmo motivo.

## O que a planilha NAO mede

A coluna «Trava com o dono» sai do **lexico** do gerador do painel PMO — ela
diz que o texto do pedido casa uma expressao como «decisao do dono», e nao que
alguem auditou o pedido. A frase que casou vai na aba `Decisoes do dono`, ao
lado do trecho: **quem le julga o casamento em vez de acreditar nele.**
