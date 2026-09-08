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

### Rodada das 26 perguntas — 7 de setembro de 2026

Três perguntas abertas, vinte e seis itens de A a Z, um PDF e a atualização
do dossiê. O trabalho se dividiu em **oito frentes paralelas**, cada uma com
faixa de portas própria e o mesmo contrato de resposta (`docs/pdf/LEIA-ME.md`),
mais o lote do orquestrador. A regra de escalão foi a de sempre — **«o erro se
vê?»** — e é ela que separa as duas colunas:

| frente | itens | escalão | por quê |
|---|---|---|---|
| F1 — SQL e exemplos | A E F G H | **mecânico** | cada comando vai ao motor e volta `ok`/erro; o resultado se confere sozinho — e foi ela que achou três divergências doc×motor, porque o erro salta |
| F2 — gaps dos outros motores | B C D O P Q | **forte** | «essencial» é julgamento com critério escrito, e cada gap listado tem de vir com a recusa colada; listar demais ou de menos não aparece em teste nenhum |
| F3 — replicação, cluster, quórum | I J T | **mecânico** | as bancadas existiam; o trabalho era rodá-las e colar — e ainda assim ela achou que escalonar a quente não funciona, porque exercitou em vez de ler |
| F4 — conexões e DBLINK | K L | **mecânico** | nativo, ODBC, REST, FFI e dblink têm bancada; o erro aparece como falha de conferência (73/0, 40/0, 44/0, 10/10) |
| F5 — diretivas e comandos proibidos | M X | **forte** | permissão é portão, e portão errado não falha em teste: falha no dia em que alguém entra por onde não devia; e o e-mail da violação podia exigir código no servidor |
| F6 — segurança da porta e injeção | W Y | **forte** | a bateria tem de provar o que o motor recusa E o que engole, e o segundo é o que ninguém vê; proposta de bloqueio por injeção é decisão de política |
| F7 — partições, transação travada, backups | R S U V | **mecânico** | bancadas e documentação existiam; um milhão de linhas em dez volumes é roteiro, e os prazos da transação têm código de erro documentado para conferir |
| F8 — telas de configuração | N | **mecânico** | captura e exercício de salvar/recarregar; o erro (segredo em claro, campo esticado) salta na tela |
| orquestrador | 0.1 0.2 0.3 Z, gerador do PDF, Figuras 1 e 8, integração | **forte** | a integração é onde o defeito do encontro aparece — e apareceu: o `.fts` faltava em três inventários, e eu consertei o errado primeiro |

**Os papéis, e o que cada um fez ou por que foi dispensado:**

| papel | nesta rodada |
|---|---|
| **A — orquestrador** | dividiu em oito frentes com faixa de portas e contrato comum; integrou, e a integração achou o que nenhuma frente via: o `.fts` faltava em **três** inventários (Figura 1, Figura 8, tabela do `FORMATO.md`) e eu consertei a figura errada primeiro; e o encontro F5×F6 foi provado rodando as duas baterias de F6 contra o binário com o código de F5 — 13/0/3 e 6/0/2, idênticas |
| **B — engenheiro** | convocado em uma frente só, e ali entregou inteiro: o e-mail da violação grave **não existia** — F5 mediu antes (0 e-mails com o SMTP falso provadamente funcionando), implementou dentro do único portão (`violacao_grave`), opt-in, envio em thread própria, silêncio por IP, corpo sem o pedido; `fmt`, `clippy` zero, 1.674 testes |
| **C — DBA** | convocado no `.fts`: por dentro é um `.ndx` (mesma assinatura, CRC), derivado, **fora do desfazer** — e é ele quem diz que o `.tx` fica fora da Figura 1 de propósito, por ser do database e não da tabela; e na sequência: contador único do `.reg`, o índice único recusa a repetição, não o contador |
| **D — zelador** | **dispensado de rodar**: cada frente derrubou por PID guardado e apagou o `/tmp/phx-f*` dela — conferido no fim, zero sobrou; e a suíte inteira deixou **0** lixo em `/tmp` |
| **E — designer** | convocado no PDF (tipografia da marca, tema de impressão, uma olhada na capa e numa resposta — que achou o negrito com código dentro saindo cru), nas Figuras 1 e 8 (provadas no navegador) e na seção 36 (provada aberta e fechada) |
| **F — prova real** | em toda frente: F1 exercitou os 56 comandos; F4 as 73/40/44/10 conferências; F5 comentou a chamada do e-mail e viu o teste **falhar**; F6 provou o que o motor engole com controle positivo; F7 mediu os prazos com o código de erro colado; F8 salvou, recarregou e conferiu o arquivo em disco |
| **G — QA** | catraca nova nenhuma, e é decisão: os achados viraram **pedidos** (213–222), não tetos — teto sem medidor não segura, e o medidor de cada um ainda não existe |
| **H — documentação** | 29 respostas no contrato, `FORMATO.md` (linha e §17), `SEGURANCA.md` §3 (F5), `TESTES.md` §17, `LEIA-ME` do dossiê e do PDF, `CLAUDE.md` (onze geradores), duas cognições — e o gerador da seção 36, que lê **as mesmas** respostas do PDF |
| **I — versionador** | commit por decisão, backup provado, quatro páginas republicadas, o PDF entregue |
| **J — pesquisador** | F2: os gaps dos cinco motores **conferidos contra o motor de hoje**, com a recusa colada em cada um — sprint antigo que fechou aparece fechado com a prova, e não some da lista |


### Rodada das diretivas HFSQL e do fluxo do auto number — 7 de setembro de 2026

Ordem do dono: *«Abra diversos agentes especializados para atender cada uma
das demandas acima. Ative o time.»* — as demandas eram os pedidos 213–225 que
a rodada anterior abriu, mais o 139, o 164, o 207 e o 208, mais dois pedidos
novos: o estudo das diretivas do HFSQL (`ALTER SERVER SET …`, `SHOW … SETTINGS`)
e o fluxo do auto number e do sequence «como está e como seria o ideal».

**O arranjo, e o número que o decidiu.** Seis frentes de Rust em **worktrees
próprias** (`/home/user/frentes/f1…f6`, cada uma com o seu `target`) e duas na
árvore principal (documentação e tela, que não brigam). O primeiro plano era
todas na mesma árvore, porque medi 7,6 GB livres contra 9,8 GB de `target` —
e estava medindo o acumulado de dias, não o custo de uma worktree (30 MB de
fonte; 4,2 GB eram só cache incremental). Está na cognição
`cognicao_medi-o-target-acumulado-como-custo-por-worktree_20260907_1710.md`.
O `cargo` das frentes é o `cargo-da-frente.sh`: **duas vagas** de compilação
para 4 núcleos, `-j2` cada, `CARGO_INCREMENTAL=0`.

A regra de escalão foi a de sempre — **«o erro se vê?»**:

| frente | itens | escalão | por quê |
|---|---|---|---|
| F1 — segurança | 214 216 215 | **forte** | portão errado não falha em teste: falha no dia em que alguém entra; e o 214 exige distinguir «réplica que aplica» de «servidor em somente leitura», que é leitura de desenho |
| F2 — cluster | 218 217 208 207 | **forte** | concorrência e eleição; e o 207 tem a decisão de não entregar meia transação, que precisa de julgamento com número |
| F3 — diretivas | HFSQL → `ALTER … SET`, `SHOW … SETTINGS`, diário, 220 | **forte** | gramática nova + o mesmo portão de permissão + formato em disco do diário e da diretiva por banco — três domínios de risco numa frente |
| F4 — SQL e catálogo | 223 219 224 222 | **mecânico** | cada pedido traz o comando que reproduz; o resultado se confere sozinho (`ok`/erro, a bancada dos 56 comandos) |
| F5 — usuários | 221 | **forte** | credencial: a senha tem de sair tapada em três lugares (arquivo, log, Profiler), e o esquecimento não aparece em teste que não se escreveu |
| F6 — guardas e caminhos | 213 225 | **mecânico** | conferidor no molde de dois que já existem; resolução de caminho se prova levantando o processo de outro diretório |
| F8 — fluxo do auto number | documento, figuras, resposta 0.5 | **forte** | o «ideal» é decisão de formato em disco e de cluster — papel do DBA, com a pergunta «onde diverge, e qual restrição nossa causou» |
| F9 — tela | 139 (regiões com aba), 164 (conferir) | **mecânico** | a prova é a captura nas quatro larguras; o erro salta na tela |
| orquestrador | worktrees, ajudante, ordem do PDF, integração, pacote | **forte** | a integração é onde o defeito do encontro aparece |


**Os papéis, e o que a integração achou (07/09/2026).** Nenhum ficou sem dono:

| papel | nesta rodada |
|---|---|
| **A — orquestrador** | dividiu em oito frentes; integrou, e a integração achou o que nenhuma frente via — **três frentes (F2, F5, F3) refizeram o mesmo escritor do `config.json`** com nomes e assinaturas diferentes, reconciliado para um só; e a união da seção de usuários com a de diretivas **comeu o `}` do `op_usuario`**, pego por compilar ENTRE os merges. Cognição `tres-frentes-refizeram-o-mesmo-escritor…`. |
| **B — engenheiro** | as seis frentes de código; e o orquestrador **assumiu a worktree da F4** quando o agente dela travou às 17:52 sem commitar — leu o diff, rodou os portões, commitou 223/219/224 (`9447ba2`) e deixou o 222 aberto, honesto. |
| **C — DBA** | o escritor único do `config.json` é decisão de formato/gravação atômica; e o `.reg`/`Sequence` do auto number (F8). |
| **E — designer** | as telas de cluster (F2), abas (F9) e as figuras do auto number (F8), provadas no navegador; a faixa do 214 (F1) fica com a **ressalva** de não ter sido exercitada. |
| **F — prova real** | cada frente com defeito reposto nos dois sentidos; a suíte inteira do workspace **exit 0** na integração. |
| **G — QA** | catraca dos textos reconciliada — **1050 medido = 1050 teto**, as telas novas entraram todas pela fábrica; nasce `TETO_INVENTARIO_DESCASADO` (F6). |
| **H — documentação** | onze geradores re-rodados (ops 123→**130**, figuras renumeradas, o portão `2b-bis` do 214c ganhou rótulo no desenho do fluxo); `PENDENCIAS` (230 pedidos: 212/7/11), `TESTES`, `MODELOS`; a página de revisão da versão completa entregue ao dono. |
| **I — versionador** | os merges e os commits por decisão; push e backup provados no fim. |
| **J — pesquisador** | o mapa das diretivas do HFSQL medido contra o motor (F3), o fluxo do auto number contra PostgreSQL/MariaDB/HFSQL/Cassandra (F8). |
| **multilíngua (tradutor pétreo)** | **convocado**, por ordem do dono: a varredura única confirmou que os `data-txt` das três telas novas entraram pela `FABRICA_TELA` e a catraca não subiu (1050=1050). |
| **D — zelador** | rodou no pico de compilações — o disco chegou a 1,6 GiB com seis `target`, liberado para 16 GiB por prova de processo, sem matar ninguém. |


### Rodada de 7 de setembro de 2026 (noite) — channel binding do login

Uma frente só, e o dono pediu «fazer os gaps» da cifra do fio. O orquestrador
escolheu **fechar um** — a amarração da credencial ao canal — e deixar os
outros três (ODBC, `Remoto`, pulso do cluster) como pedido, porque cifrar
metade do cluster parece protegido e não está: *meia funcionalidade pior que
nada* é decisão, não preguiça.

| frente | escalão | por quê | papéis dispensados |
|---|---|---|---|
| channel binding do `login` | **projeto e risco** | é criptografia e protocolo: o erro não aparece no teste feliz, aparece no dia do homem-no-meio. Foi por isso que a prova real precisou de dois defeitos, e o fácil escondia que o difícil não fora medido | **C-DBA** (não muda formato em disco — a transcrição é de sessão, em memória; o `PSCH` não se toca), **E-designer** (o login web é HTTP, fora do túnel; nenhuma tela muda) |

Papéis cumpridos: **F** (prova real nos dois sentidos — `desafio` e servidor, e
a guarda `amarra-ao-canal-ignorada` **PROVADA**, 1/1 caíram); **G** (a guarda
nova no catálogo, catraca dos textos intacta — a mensagem `erro.amarra_sem_tunel`
entrou pela `FABRICA_TELA` nos seis idiomas); **H** (`CIFRA-DO-FIO.md` §1/§10/§11,
`SEGURANCA.md` §7, `PENDENCIAS.md` item 8, esta linha, e a cognição do dia); **J**
dispensado com registro — o desenho já estava escrito no §10 do documento, medido
contra o rebaixamento do `exigir`; não havia receita de fora a trazer.

### Rodada de 8 de setembro de 2026 — onda dos gaps (4 frentes paralelas)

O dono mandou «ativar agentes para acelerar a resolução dos gaps». O
orquestrador abriu **quatro frentes em worktrees isoladas**, escolhendo o
escalão de cada uma, e **segurou** cluster/`Remoto`/ODBC/226 para uma 2ª onda,
com o motivo escrito (cifrar metade do cluster é pior que nada; `Remoto` é
mudança de formato que colidiria com o `config.rs` da G1; ODBC é driver).

| frente | escalão | por quê | papéis dispensados |
|---|---|---|---|
| G1 — servidor exige a amarração ao canal | **projeto e risco** | cripto/protocolo; o erro só aparece no dia do homem-no-meio | C-DBA (config runtime, sem formato), E-designer (login web é HTTP) |
| G2 — os três defeitos do auto number 229 | **projeto e risco** | formato/concorrência/precisão; o teto 2⁵³ é o `f64` do `Json`, medido no analisador | E-designer, D-zelador (sem UI nem limpeza) |
| G3 — `esquema.volumes` na partição por quantidade (222) | **mecânico e verificável** | preencher a lista dos volumes em disco, sem tocar a aritmética — o erro salta no número | C-DBA (não muda formato), E, D |
| G4 — `declarar_fk` imposta (227) + rodízio dos logs (228) | **mecânico e verificável** | dois consertos locais com prova nos dois sentidos | C, E, D |

E a integração foi papel do orquestrador **porque o defeito do encontro não
aparece para nenhuma frente sozinha** — e apareceu: G2 e G3 refizeram vizinho
no `table.rs` (G3 mudou a assinatura de `fronteiras()` para `Vec`, G2 pôs
`reconciliar_sequencia` ao lado), e o merge conflitou. Resolvido **por função**
(as duas ficam, `fronteiras()` com a assinatura da G3, que é a que o `reg.rs`
mesclado exige), compilando **entre** os merges — é a lição da cognição do
encontro das seis frentes, paga de novo. As outras três frentes uniram sem
conflito, por tocarem funções diferentes do `servidor.rs`. Portões inteiros do
workspace verdes no fim (fmt, clippy zero, suíte inteira), 824 no server lib.

### Onda 2 da rodada dos gaps — 8 de setembro de 2026 (as 4 frentes cifradas)

A 2ª onda pegou os quatro que a 1ª segurou de propósito, cada um pelo motivo que
o segurou. As quatro tocam a **cifra do fio**, então o escalão foi **projeto e
risco** nas quatro: é criptografia e protocolo, e o erro mora no dia do
homem-no-meio, não no teste feliz.

| frente | escalão | por quê | papéis dispensados |
|---|---|---|---|
| H1 — cluster INTEIRO cifrado (pulso **e** replicação, um só interruptor, pino por nó) | **projeto e risco** | cripto/protocolo/concorrência do cluster; medido antes, o trabalho era **fiação, não formato**, e por isso entrou inteiro em vez de meia cifra — meia cifra parece protegida e não está (§12) | C-DBA (config em runtime, sem `PSCH`), D-zelador; **E-designer NÃO dispensado, fica PENDENTE**: o `/saude` já diz `cifra`/`tem_pino`, mas tela só se prova exercitando |
| H2 — o `Remoto` da interface liga o túnel | **projeto e risco** | cripto **mais** a mudança de formato do `config.json` (`web.servidores`: texto **ou** objeto), retrocompatível — texto solto continua valendo | **C-DBA convocado** para a mudança de formato, e aprovou por ser aditiva e retrocompatível; E-designer PENDENTE (o aviso do login se prova na tela) |
| H3 — o driver ODBC fala o aperto de mão | **projeto e risco** | cripto/protocolo dentro do driver: reusa o `fio::Canal` do cliente da réplica, com `CIFRA`/`CHAVE_DO_FIO` na string de conexão | C-DBA, E-designer, D-zelador |
| H4 — compressão no fio, pedida por PEDIDO (226) | **projeto e risco** | o DEFLATE já existia; o **risco** é a decisão de segurança — comprimir-e-cifrar vaza o tamanho (CRIME/BREACH), então a compressão **recusa** dentro do túnel, medido nos dois sentidos | C, E, D |

Papéis cumpridos na onda: **F** — prova real por soquete em cada frente, com o
defeito reposto (`compressao-do-fio.rs`, `o_remoto_liga_o_tunel_e_carrega_um_
pedido_real`, `o_pino_certo_entra_o_errado_derruba`, as bancadas do cluster);
**G** — as guardas `remoto-em-claro-para-quem-exige`, `pulso-do-cluster-em-claro`,
`cluster-cifrado` e `replicacao-do-cluster-em-claro` no catálogo, catracas
intactas; **H** — `CIFRA-DO-FIO.md` §8/§10/§12, `PENDENCIAS.md` (226 fechado, com
os dois números medidos, 9,69× do `zlib` e 5,66× do DEFLATE da casa), e a
cognição do dia; **J** dispensado com registro — o desenho já estava escrito no
§10, e o único conhecimento de fora (CRIME/BREACH) é princípio conhecido, citado,
não receita a medir.

E a integração foi de novo papel do orquestrador, e de novo o defeito **só
apareceu no encontro**: as quatro frentes ramificaram **antes** de as irmãs
mesclarem, então cada uma escrevia no §10 do `CIFRA-DO-FIO.md` o próprio «feito»
e via as outras três como «não feito». O merge nu teria publicado três bullets
desmentindo o que já estava pronto. Resolvido **por seção**, combinando o bullet
FEITO de cada lado (ODBC da H3, `Remoto` da H2, cluster INTEIRO da H1), com
`cargo check` limpo **entre** os merges. É a cicatriz da onda 1 e das seis
frentes, paga mais uma vez: *ramo que ramificou cedo mostra a irmã como
não-feita, e o merge acredita*. Portões verdes no fim — `fmt`, `clippy` zero, e a
suíte inteira sem falha (271 no core, 837 no server, 9 no store, 1 no odbc).

### Rodada do acelerador de memoria (o `.tbm`) — 8 de setembro de 2026

O dono trouxe de fora um "Memory Table Accelerator" (arquivo `.tbm` colunar, LSN,
checkpoint, MVCC) e mandou **ativar o time com 3 DBAs senior**. A obrigacao da
clausula nao e abrir agentes — e nenhum papel ficar sem dono onde o trabalho toca
o dominio dele. Aqui o dominio e formato em disco, concorrencia e medicao, entao
foram tres DBAs e o pesquisador, todos no escalao **forte** (projeto/risco/medicao
de arquitetura), e a documentacao/integracao com o orquestrador.

| frente | escalao | por que | papeis dispensados |
|---|---|---|---|
| DBA-1 — `.tbm` e cache reconstruivel ou toca o formato? | **projeto e risco** | garantia de dado e familia de arquivos (`.reg`/`.ndx`/`.tx`), decisao que «entra cedo» | E, D, I |
| DBA-2 — MVCC ou invalidar-na-escrita? | **projeto e risco** | concorrencia; mapear a proposta sobre a Sombra e o `RwLock` **ja medidos** | E, D, I |
| DBA-3 — o NUMERO: onde vai o tempo da consulta analitica | **projeto e risco** | medicao na maquina parada, pela porta `esta-medindo.sh`; escreveu o medidor `onde-doi-na-agregacao.rs` | E, D |
| J — como DuckDB/SQL Server/MySQL/Cassandra aceleram | **projeto e risco** | receita de fora medida contra o crivo, inspiracao e nao copia | B, E, G, H, I |

**Dispensado com registro em todas: B-dev** — nenhuma linha do acelerador foi
escrita, porque a medicao mandou **nao construir**; as duas melhorias baratas que
sobraram (o double-read do `op_pivotar`, o `BufReader` no `varrer`) sao follow-up
de B, com prova real, e nao entraram nesta rodada.

A integracao foi papel do orquestrador, e aqui o «encontro» **nao** teve defeito
de merge — as quatro frentes eram relatorios de leitura mais um medidor, sem
ramo a mesclar. O que o orquestrador procurou foi **contradicao entre os
pareceres**, e nao houve: os quatro convergiram no mesmo veredito, cada um por um
caminho — o acelerador ja existe (`TabelaMemoria`, **87x medido**), o `.tbm` novo
evita um custo que ja nao e disco (0,0 MiB lidos) e que a `TabelaMemoria` ja
cobre, e a unica alavanca nova (cache colunar leve dentro do `SelectMemory`)
espera **uma** premissa medida. `docs/MEMORIA.md` carrega a decisao inteira.

### Rodada da corrida de I/O — 8 de setembro de 2026

O dono pediu: «o PhxSql supere a velocidade de I/O do MySQL e do MariaDB». O
numero do MySQL ja existia (superado nas quatro), mas era de binario velho e de
maquina com carga, e o MariaDB **nunca fora medido** — receita de fora se mede,
nao se supoe. O orquestrador montou o ambiente (o Docker de pe, o MariaDB 11 num
container por TCP:3307, o MySQL 8 local por soquete, os dois vivos ao mesmo tempo
sem conflito), recompilou o `carga`, e mediu na maquina parada pela porta
`esta-medindo.sh`.

| papel | o que fez |
|---|---|
| **A — orquestrador** | montou o ambiente, sequenciou a medicao DEPOIS do `.tbm` (duas bancadas juntas medem a carga), integrou o 4o motor nos tres desenhos |
| **B — dev** | o parametro `cli` no `medir.py`, reusando a MESMA logica de fase para o MariaDB entrar por TCP sem duplicar codigo |
| **F — prova real** | o `confere_trio` provou trabalho igual dos quatro (mesmas somas em cada fase); o smoke test dos quatro antes da corrida de 1M |
| **E — designer** | a cor teal do 4o motor nos dois temas, tirada do token `--bin` da marca (ja validado), e o `viewBox` crescendo com o numero de motores |
| **H — documentacao** | o dossie, os graficos e o SVG regenerados com os quatro motores, numeros medidos |

**Dispensados, com registro:** **C-DBA** — o `medir.py` e ferramenta de medicao,
nao formato em disco, e o MariaDB e motor externo; **G-QA** — nenhuma catraca
nova (a corrida e medicao, nao guarda); **D**, **I** (rotina), e **J** — o MariaDB
e motor conhecido, sem receita de fora a trazer alem do numero.

O veredito medido: **MySQL superado nas quatro** (inserir 1,35x, buscar 9,5x,
alterar 8,5x, excluir 1,8x); **MariaDB superado em tres** (buscar 16x, alterar
11x, excluir 2x), com o **inserto em massa do MariaDB 1,18x a frente** (6,69 vs
7,87s) — e isso e o gargalo do `.ndx` (63,6% da insercao), ja medido, unico lugar
onde sobra I/O de escrita a ganhar. As duas honestidades que a bancada gravou: o
MariaDB rodou `sync_binlog=0` (uma sincronizacao por fase de diferenca, nao
explica 1,18s no milhao), e o piso do TCP (1,75s) e maior que o do soquete
(0,92s), medido a parte.

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
