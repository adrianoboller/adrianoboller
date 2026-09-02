# A escolha de modelo por frente

A cláusula pétrea dá esta decisão ao orquestrador e cobra o registro dela:

> *O orquestrador **diz qual escolheu e por quê**: modelo escolhido em silêncio
> vira custo que ninguém explica ou qualidade que ninguém entende.*

Este documento é esse registro. Ele existe porque a alternativa — o registro
morar na conversa — é justamente o que a casa recusa em toda outra frente:
*script, comando e roteiro que resolveram algo não podem morrer com a sessão.*

## Por que ESCALÃO e não o nome do modelo

Há uma restrição real, e ela precisa estar escrita para ninguém "consertar"
isto depois sem saber: **nome de modelo não entra em artefato do repositório**
— nem em commit, nem em comentário de código, nem aqui. A única exceção é o
rodapé de atribuição, que é obrigatório e vem pronto.

Registrar o **escalão** resolve os dois lados: cumpre a cláusula, sobrevive à
sessão, e não põe nome nenhum no repositório. E é o escalão que carrega a
informação útil — quem lê daqui a um ano quer saber *que tipo de trabalho
merece o modelo caro*, não qual era o nome comercial dele naquele mês.

| escalão | quando | por quê |
|---|---|---|
| **projeto e risco** | formato em disco, concorrência, criptografia, integridade referencial, protocolo, arquitetura | erro aqui não aparece no teste: aparece em produção, meses depois, como dado perdido |
| **mecânico e verificável** | tradução, documentação, varredura, medição roteirizada, rodar gerador, republicar página | o resultado se confere sozinho — ou o número bate, ou não bate |

A fronteira não é "difícil × fácil". É **"o erro se vê?"**. Traduzir 190
rótulos é trabalhoso e o erro salta na tela; desenhar o campo `verificar` da
chave estrangeira é uma linha e o erro só apareceria no dia da primeira
exclusão.

## O registro

### Rodada de 1–2 de setembro de 2026 — NÃO CUMPRIDA

Está aqui como não cumprida porque é a verdade, e porque *papel que não está
cumprindo tem de aparecer como não cumprindo*.

**27 commits, todos no escalão de projeto e risco, sem registro nenhum.**
Nenhum agente foi convocado depois da retomada da sessão.

Medido pelo que os commits tocaram:

| tocado | commits | escalão que a cláusula pedia |
|---|---|---|
| `PENDENCIAS.md` | 8 | mecânico |
| `pedidos.html` (gerado) | 7 | mecânico |
| `CHANGELOG.md` | 6 | mecânico |
| dossiê (gerado) | 5 | mecânico |
| `LEIA-ME.md` | 4 | mecânico |
| `servidor.rs` | 3 | projeto e risco |

Trinta toques em documentação e página gerada contra três no motor. O que
estava **certo** no escalão forte: a chave estrangeira conferida, o formato
PSCH v7, a regra primordial da integridade e a busca reversa — projeto e
risco, e é onde ele se paga. O resto não.

Os três agentes da rodada anterior (DbLink contra MySQL® real, toolchain e CI,
QA/PDCA) rodaram sem nenhum registro sobreviver. O que se sabe deles hoje veio
de memória de sessão — e memória de sessão é exatamente o que este documento
existe para substituir.

### Frente «toda tabela é PhxGrid» — 2 de setembro de 2026

Primeira frente com a escolha registrada na hora, e não reconstruída de memória.

| parte | escalão | por quê |
|---|---|---|
| varredura das 27 tabelas | **mecânico**, feito inline | é um `grep`; convocar agente para isso é o exagero que a cláusula avisa |
| tela de referência (`verSysTables`) | **projeto e risco** | vira o molde de 19 conversões — errar nela erra em 19 lugares |
| `verConteudoEditavel` | **projeto e risco** | grava dado; é onde mora a janela de conflito, e a gravação tinha de continuar pela ficha |
| `estruturaDbl` | **projeto e risco**, reservada | acabou de ganhar a leitura por nome; converter sem saber disso perderia o conserto |
| as 19 telas de exibição | **mecânico**, um agente | têm molde escrito, e o erro salta na tela |

**Papéis dispensados, e por quê** — dispensa registrada é decisão:

- **DBA**: nenhuma das conversões toca formato em disco, chave ou índice.
- **Pesquisador**: não há receita de fora nesta frente; o padrão saiu do
  componente que já existe aqui.
- **Designer**: convocado de fato, e não dispensado — as regras de gesto,
  cor de ação e «rótulo se traduz, dado nunca» foram escritas no pedido do
  agente porque são dele.

**Um agente e não quatro em paralelo**, e o motivo é medido: as 22 funções
moram no **mesmo arquivo**. Frentes paralelas ali produziriam exatamente o
«defeito que só aparece no encontro delas» — por construção, e não por azar.

**O escalão intermediário, e não o mais leve.** São 19 telas com preservação
sutil: o gesto de clique simples, a célula própria, a catraca. O mais leve
erraria em volume, e volume de erro numa passada única custa mais que a
diferença de preço.

**O que ainda não se sabe**: se a escolha foi certa. O relatório do agente não
é prova — o do DbLink acertou e ainda assim a prova real foi refeita aqui antes
do commit. Esta linha se fecha quando eu tiver exercitado as telas.

## Como registrar daqui em diante

Uma linha por frente, no fim da rodada, junto do resto da documentação:

```
| frente | escalão | por quê | papéis dispensados |
```

O campo dos **dispensados** não é enfeite: *dispensa registrada é decisão;
dispensa silenciosa é esquecimento*, e a cláusula cobra a diferença entre as
duas. Corrigir um typo não precisa de DBA, designer, QA e pesquisador — mas
precisa dizer que não precisou.

E vale a lição que a delegação de ontem deixou: **agente devolve relatório, não
prova.** Um afirmou que o documento de tecnologias não existia, e existia um
nível acima de onde ele procurou; o do DbLink acertou, e ainda assim a prova
real foi refeita aqui antes do commit. Isso não é argumento para não delegar —
é argumento para delegar exatamente o que se confere sozinho, que é o que o
escalão mecânico quer dizer.
