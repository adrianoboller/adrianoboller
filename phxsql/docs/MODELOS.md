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

### Rodada das sprints abertas — 4 de setembro de 2026

Quatro frentes abertas ao mesmo tempo, das dez sprints que o retrato de 03/09
listou. **Não são dez agentes**: a cláusula não pede dez, pede que nenhum papel
fique sem dono quando o trabalho toca o domínio dele — e que a dispensa seja
registrada.

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **S-A** — o `varrer` ganha `WHERE` | **projeto e risco** | é **contrato de protocolo**, e encosta no portão de permissão, que é UM só e já tem três operações que escondem tabela dele. Erro aqui não aparece no teste: aparece como porta dos fundos | **B** escreve, **C** decide o contrato, **F** prova nos dois sentidos | **D** (não mexe no ambiente), **I** (não comita nem empacota), **J** (não há receita de fora) |
| **S-B** — bancada em máquina parada | **mecânico e verificável** | **medição roteirizada**: ou o ruído do controle está abaixo do teto, ou não está. O resultado se confere sozinho | **F** dono, **G** pela catraca do ruído | **C** (não toca formato em disco), **E** (não há tela), **B** (não há motor novo) |
| **S-D** — o §33 do dossiê, linha a linha | **mecânico e verificável** | **varredura**: cada afirmação de limitação se confere contra um teste, um script ou um pedido fechado. E os números saem de gerador | **H** dona | **C**, **E**, **F**, **G** (documento não muda comportamento; o que a varredura achar de comportamento errado volta para mim, não vira código lá) |
| **S-E** — Android: medir ou nomear o bloqueio | **mecânico e verificável** | a premissa é do **ambiente** — o NDK está alcançável, sim ou não. É `rustup target list` e um `cc`, não arquitetura | **B**, **D** (zelador confere o ambiente) | **C**, **E**, **G**, **H** (se o NDK não estiver lá, a sprint não é de código e não há o que projetar) |

**Por que quatro e não sete.** Duas frentes na mesma árvore mexendo em
`crates/phxsql-server/src/` se derrubam por motivo que não é delas: uma
compila no meio da edição da outra e persegue fantasma. Só a **S-A** ficou
autorizada ali. A **S-C** (conferidor de frase vencida) entra quando a S-A
devolver a árvore; a **S-G** (trava única) espera a resposta medida da S-B,
porque *sem máquina parada não há número*; a **S-H** espera a S-G; e a **S-J**
não é de agente nenhum — é a palavra do dono sobre o pedido 175.

**O papel A, que não se delega.** A integração é minha, e não sobra de
ninguém: numa rodada de seis frentes desta casa, **três defeitos só apareceram
no encontro delas** — um teto de memória que uma frente pôs e a outra apagaria
sem conflito nenhum, uma bateria que ficou com dezoito partes quando cada
frente contou dezessete, e o dossiê que perdeu uma seção porque o merge
escolheu o lado de quem não a tinha. Por isso nenhuma frente comita
`CHANGELOG.md`, nenhuma dá `git add -A` e nenhuma dá `push`.

### Rodada do comparativo — 7 de setembro de 2026

Pedido do dono: *«Status comparativo com Hfsql, PostgreSQL, Cassandra, mysql e
SQLite de recursos que não estão ok ainda no phxsql»*.

| frente | escalão | por quê |
|---|---|---|
| o medidor comparativo e os portões dele | **projeto e risco** | é instrumento de medição, e instrumento errado publica veredito errado com a mesma confiança do certo — foi o que aconteceu cinco vezes nesta própria rodada |
| a prosa do gerador do documento | projeto e risco | a redação carrega o julgamento sobre o que é `citado` e o que é medido; delegar isso é delegar a conclusão |
| o medidor da cobertura da tela | **mecânico e verificável** | duas listas saem do código e se conferem sozinhas contra o `catalogo.rs` e o `http.rs` |
| os geradores do dossiê e das páginas | mecânico e verificável | rodar e comparar a saída |

**Nenhuma frente foi delegada a agente**, e isto é dispensa registrada, não
esquecimento: a rodada inteira coube num fio só, e o custo de integração de
duas frentes na mesma árvore (`bancada/` e `docs/`) seria maior que o trabalho.

**Os papéis, e o que cada um fez ou por que foi dispensado:**

| papel | nesta rodada |
|---|---|
| **A — orquestrador** | integrou; a §33 do dossiê e o `PENDENCIAS.md` só fecham vendo as duas frentes juntas |
| **B — engenheiro** | **dispensado**: nenhuma linha de Rust mudou. `cargo fmt --check` limpo e a suíte verde (62 binários, 1.659 testes) foram conferidos, não produzidos |
| **C — DBA** | **dispensado**: nada tocou formato em disco. Mas ele **ganhou uma pergunta** — quatro campos de esquema desconhecidos são engolidos pelo `criar_tabela`, e recusá-los é decisão dele |
| **D — zelador** | **dispensado**: a rodada não encheu disco; o medidor limpa o próprio descartável |
| **E — designer** | convocado de leve: a tabela nova da §33 usa a classe `pino` que já existe, e as crases viram `<code>` no gerador — texto de tabela não se estiliza à mão |
| **F — prova real** | **o papel central desta rodada**: os três portões do medidor foram provados nos dois sentidos, e um quarto **morreu medido** (o do `database`) |
| **G — QA** | catraca nenhuma mudou. Entrou uma camada de guarda nova (portão de medidor), e o buraco dela está declarado: não roda na bateria única |
| **H — documentação** | `COMPARATIVO.md` gerado, `HFSQL.md` corrigido no sexto veredito, sétimo gerador do dossiê |
| **I — versionador** | commit por decisão, na branch combinada |
| **J — pesquisador** | **dispensado**: não havia receita de fora para medir. O que houve foi o contrário — medir o que os outros têm e nós não |

### Rodada do batimento e dos geradores — 7 de setembro de 2026

Rodada curta e sem pedido novo do dono: manutenção que nasceu de uma
remedição. As duas frentes se encontraram por acaso e o encontro é o achado.

| frente | escalão | por quê |
|---|---|---|
| remedir a limitação do batimento de 15 min | **forte** | a decisão era «que mecanismo serve», não «rodar o de sempre» |
| o dono único do caminho do dossiê, nos nove geradores | **forte** | mexer em nove geradores de uma vez arrisca o painel inteiro |
| rodar os nove e comparar cada painel | mecânico e verificável | a saída se confere sozinha, com `md5sum` antes e depois |

**Nenhuma frente foi delegada a agente**, e é dispensa registrada: a segunda
frente edita nove arquivos que o dossiê inteiro depende, e integrar dois fios
na mesma pasta custaria mais que fazer.

**Os papéis, e o que cada um fez ou por que foi dispensado:**

| papel | nesta rodada |
|---|---|
| **A — orquestrador** | integrou, e o encontro das duas frentes é o que ensina: as duas são a mesma doença — mecanismo que se declara cumprindo sem cumprir. O `Monitor` morria anunciando que estava armado; o gerador pulava metade anunciando êxito |
| **B — engenheiro** | **dispensado**: nenhuma linha de Rust mudou. O que mudou é Python de ferramenta, e os treze arquivos da pasta compilam |
| **C — DBA** | **dispensado**: nada tocou formato em disco nem garantia de dado |
| **D — zelador** | **dispensado**: a rodada não encheu disco. As duas cópias temporárias saíram na mesma corrida que as criou, e nenhum processo foi morto |
| **E — designer** | **dispensado**: nenhum bloco novo na tela; os painéis mudaram de **número**, não de forma |
| **F — prova real** | convocado nos dois sentidos: painel forçado de volta para 198 e chamada nua — antes do conserto três linhas de êxito e o 198 fica; depois, `painel dos pedidos regravado` e o 204 volta. E os portões medidos pelo **código de saída**, não pelo texto: 1 com dois dossiês, 0 com um |
| **G — QA** | catraca nenhuma mudou, e o buraco está declarado: **não há guarda que reprove um gerador novo nascido sem padrão**. O conferidor genérico seria um casador de `sys.argv`, e casador de texto é o que esta casa já recusou com número noutra frente |
| **H — documentação** | duas cognições, `LEIA-ME.md` da pasta, `CLAUDE.md` (a quarta vez do número velho), `CHANGELOG`, pedido 204 |
| **I — versionador** | commit por decisão, na branch combinada |
| **J — pesquisador** | **dispensado**: não havia receita de fora. O que houve foi remedir a **nossa** limitação, que é o oposto — e é justamente o que a lei dele manda |

E a lição de escalão que esta rodada deixa: **remedir uma limitação é trabalho
de projeto, não de rotina.** Eu a tratei como rotina por rodadas — rearmar o
que caiu — e rotina não faz a pergunta que destrava, porque rotina repete a
pergunta que já tem resposta.

### Rodada da pergunta e do botão — 7 de setembro de 2026

Duas perguntas do dono no mesmo turno, e elas não pedem o mesmo escalão.

| frente | escalão | por quê |
|---|---|---|
| «o transaction atomic está funcionando?» | **forte** | a resposta é sobre garantia de durabilidade e recuperação; e desenhar o defeito reposto é projeto, não roteiro |
| os botões da barra ≥10% mais estreitos | **forte** | parecia mecânico e não era: qual propriedade manda muda de botão para botão, e escolher entre tracking e fonte menor é decisão de desenho |
| rodar os medidores e comparar os retratos | mecânico e verificável | a saída se confere sozinha, com dois JSON lado a lado |

**Nenhuma frente foi delegada**, e é dispensa registrada: as duas terminam na
mesma árvore, e a segunda exige olhar a captura — relatório de agente não
substitui ver a barra.

**Os papéis, e o que cada um fez ou por que foi dispensado:**

| papel | nesta rodada |
|---|---|
| **A — orquestrador** | integrou. E o que as duas frentes têm em comum é o método, não o assunto: **as duas se resolveram medindo antes de decidir**, e nas duas a primeira conclusão morreu medida |
| **B — engenheiro** | convocado de leve: nenhuma linha de Rust mudou de lógica, só o CSS embutido. Portões conferidos — `fmt` limpo, `clippy` zero avisos, **1.659 testes verdes** |
| **C — DBA** | **dispensado**: nada tocou formato em disco. Mas a frente das transações **confirma** a decisão dele — o `ROLLBACK` não queima slot, e a ordem de digitação sai intacta da queda |
| **D — zelador** | **dispensado**: a rodada não encheu disco; os medidores limpam o que sobem, e nenhum processo alheio foi tocado |
| **E — designer** | **o papel central da segunda frente**: escolher tracking em vez de fonte menor, e recusar a saída barata de encolher a caixa comendo o rótulo. A prova foi **abrindo o navegador**, como manda a lei — e a captura mostrou o ganho que o número não mostrava, a barra caindo de duas fileiras para uma |
| **F — prova real** | **o papel central da primeira**: o defeito reposto derrubou a conferência certa, e ensinou mais que a corrida limpa — 43 de 3.000 quando a marca some |
| **G — QA** | catraca nenhuma mudou, e **dois buracos ficam declarados**: um defeito reposto só nas transações (faltam marca corrompida e queda durante a recuperação), e **nenhuma catraca travando a largura dos botões** |
| **H — documentação** | duas cognições, `ACID.md` §2.3.1, pedidos 205 e 206, `CHANGELOG` |
| **I — versionador** | commit por decisão, na branch combinada |
| **J — pesquisador** | **dispensado**: não havia receita de fora |

E a lição de escalão: **«encolher um botão 10%» parecia trabalho mecânico e
não era.** O que o tornou de projeto foi a medição — descobrir que quinze
botões obedeciam a uma propriedade e oito a outra. Escalão se decide **depois**
de olhar o problema, e tarefa que parece roteiro merece uma medida antes de
receber o modelo leve.

### Rodada do quórum — 7 de setembro de 2026

Uma pergunta de arquitetura, e a resposta certa começou por **não** responder:
medir a premissa primeiro.

| frente | escalão | por quê |
|---|---|---|
| medir o custo real de esperar réplicas | **forte** | desenhar o medidor **é** o trabalho: separar sono de transporte, e impedir que a réplica compita com o cronômetro |
| ler a arquitetura para saber se quórum cabe | **forte** | a resposta depende da direção da conexão, que é decisão de projeto já paga |

**Nenhuma linha de motor foi escrita, e isso é entrega, não recuo.** O dono
perguntou se é possível e se simplifica; construir antes de responder seria
pular a pergunta.

**Os papéis, e o que cada um fez ou por que foi dispensado:**

| papel | nesta rodada |
|---|---|
| **A — orquestrador** | integrou, e a integração aqui **é** a resposta: o custo (bancada), a direção (`replica.rs`), o buraco confessado (`cluster.rs`) e a lição do Cassandra® (`CASSANDRA.md`) moram em quatro lugares, e nenhum deles sozinho responde a pergunta |
| **B — engenheiro** | **dispensado de escrever motor**, de propósito. O que escreveu foi bancada |
| **C — DBA** | convocado, e é dele a parte mais dura da resposta: **quórum custa disponibilidade**. Sem N réplicas alcançáveis o commit falha, onde hoje aceita — trocar «sempre aceita» por «às vezes recusa» é decisão de produto, e ele diz isso antes de qualquer código |
| **D — zelador** | **dispensado**: a bancada limpa o que sobe, e mata só os PIDs que ela mesma subiu |
| **E — designer** | **dispensado**: não há tela nesta frente |
| **F — prova real** | convocado nos portões do medidor: ele **para** se a réplica puxar sozinha ou se o esquema não a alcançar — e os dois dispararam de verdade na estreia, junto com um tipo de coluna inventado e um direito de permissão que não existe |
| **G — QA** | catraca nenhuma; e o buraco fica declarado — os 826 ms **continuam publicados** na bancada de replicação, agora com um campo dizendo o que somam, mas ainda sob o nome `atraso_ms` |
| **H — documentação** | `REPLICACAO.md` §19, pedido 207, cognição, `CHANGELOG`, e a bancada declarada na página de testes |
| **I — versionador** | commit por decisão, na branch combinada |
| **J — pesquisador** | **o papel decisivo**, e a lei dele foi cumprida ao pé da letra: *medir a premissa do item vem antes de implementar o item*. A premissa era «o transporte custa 826 ms», e ela **morreu medida** — custa 0,475. Sem essa medição, a resposta ao dono teria sido «inviável», com autoridade e errada |

E a lição de escalão desta rodada: **a forma que um medidor precisa ter é
informação sobre o sistema.** Este teve de puxar os eventos à mão, do Python,
e eu escrevi isso como comodidade de bancada — não era: era o *pull* dizendo
que o master não tem como fazer ninguém buscar, que é exatamente o obstáculo
ao quórum. O andaime foi o achado.

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
