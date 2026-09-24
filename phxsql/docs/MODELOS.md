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

### Rodada das dezoito do comparativo — 8 de setembro de 2026

Ordem do dono: *«Falta esses itens»*, sobre a tabela «E o comparativo, medido
contra quem tem». Os contratos estão em `docs/propostas/comparativo-19.md`, e
a divisão saiu **depois** de medir o que já existia por baixo — o avaliador
dos gatilhos, o agregador do `pivotar`, o upsert do DbLink, o diário com
carimbo e imagem.

| frente | escalão | por quê | papéis dispensados, e por quê |
|---|---|---|---|
| **F-NÚCLEO** — `phxsql_core::expressao`, PSCH v9, DEFAULT/CHECK/calculada, índice parcial e por expressão | **projeto e risco**, feita pelo orquestrador | é **formato em disco** e **caminho de escrita**: a regra tem de morar onde a linha se grava, senão FFI, réplica e example gravam por fora dela | **E** (não há tela), **J** (a receita é o SQL dos outros motores, já medida no comparativo), **D** (o zelador rodou de hora em hora, sem decisão desta frente) |
| **F-SQL** — GROUP BY, expressão, WITH, IN (SELECT), OVER, VIEW, upsert, `?` | **mecânico e verificável** | tradução para contratos já escritos, e o JSON produzido se confere campo a campo em teste de unidade | **C** (não toca disco), **E**, **D**; **F** fica com a frente, que prova por sabotagem |
| **F-CONSULTA** — `agrupar`, `consultar`, visões, `diferencas`, `varrer.expressao`, upsert, parâmetros | **projeto e risco** | toda sub-consulta passa pelo **portão de permissão, que é um só** — e a porta dos fundos não aparece em teste de tradução | **E**, **J**; **C** consultado só pelo contrato (nada de formato) |
| **F-DIREITO** — direito por coluna | **projeto e risco** | segurança: a op que alguém esquecer vira a porta dos fundos, e o `UPDATE` de quem não lê a coluna zeraria a coluna do outro calado | **E**, **J**, **D** |
| **F-PITR** — restaurar a um instante | **projeto e risco** | durabilidade e **ordem** de reaplicação; o backup pode mudar de formato | **E**, **J** |
| **F-BANCADA** — sondas vivas, remedição, documentação | **mecânico e verificável** | roteirizado: a célula vira TEM pelo medidor ou não vira | **C**, **E** |

Dois itens ficaram **fora de frente, por decisão do dono**: o nível de
isolamento (é a Sombra, parada em 05/09) e TLS (a pétrea das zero
dependências). Os dois têm o custo escrito no documento dos contratos.

**O que a integração vai ter de olhar**, porque nenhuma frente sozinha vê:
`servidor.rs` é tocado por quatro delas (portão, `op_sql`, ops novas, backup);
a F-SQL entrega `planejar_sobre` e `analisar_comando_com` que a F-CONSULTA
consome; e a coluna negada (F-DIREITO) atravessa o `consultar` (F-CONSULTA)
por `executar_derivado` — é o encontro em que o direito por coluna ou vale
ou vaza.

#### O que cada frente entregou de fato

A F-BANCADA integrou as seis a partir do HEAD já mesclado e reconferiu com o
próprio `cargo test`/`cargo run` desta sessão — marcado **medido aqui** — onde
foi o caso; o resto vem do relatório de cada frente, marcado **relatório da
frente**, porque remedir a suíte inteira de cada uma sairia do escopo desta
rodada.

- **F-NÚCLEO** — `phxsql_core::expressao`, o PSCH v9 com `padrao`/`check`/
  `calculada`/índice por `onde`/índice por expressão, e o caminho de escrita
  da `Table` lendo os quatro. Contagem de testes da própria crate: **relatório
  da frente**.
- **F-SQL** — `GROUP BY`, expressão no `WHERE`, `WITH`, `IN (SELECT …)`,
  `ROW_NUMBER() OVER`, `CREATE`/`DROP VIEW`, `INSERT … ON CONFLICT`/
  `ON DUPLICATE KEY`, `?`. **127 → 229 testes: relatório da frente.**
- **F-DIREITO** — direito por coluna, com o teste do comportamento velho
  (`sem_colunas_no_cadastro_nada_muda`). **919 → 937 testes: relatório da
  frente.** Confirmado nesta sessão por uma sonda VIVA própria
  (`bancada/comparativo/medir.py::sonda_direito_coluna`): servidor com
  cadastro de coluna, `ana` (salário negado) volta sem a coluna, `bea`
  (controle) volta com ela — TEM, medido pelo soquete.
- **F-PITR** — `restaurar_backup` com `ate`/`ate_ms`. **Lib 846 → 858 testes,
  mais `bancada/pitr/provar.py` com 22 conferências: relatório da frente**
  (não re-executado nesta sessão). Confirmado nesta sessão por uma sonda VIVA
  própria e mais enxuta (`sonda_pitr`, um veredito com o controle na mesma
  corrida, não as 22 do `provar.py`): TEM, medido pelo soquete.
- **F-ODBC** — `SQLBindParameter` liga o `?` do lado do driver.
  **24 → 42 testes**: a chegada em **42** é **medida aqui** —
  `cargo test -p phxsql-odbc` desta sessão fechou em **42 passed, 0 failed, 1
  ignored**; a saída (24) vem do **relatório da frente**. A prova de ABI
  (`bancada/odbc/provar.py`, que inclui `prova-abi.py`) também rodou **aqui**,
  código de saída 0, com **89 linhas de `ok`** contadas nesta corrida — número
  medido por esta frente, e não o mesmo contador interno do **73 → 86** que o
  relatório da F-ODBC cita, então os dois não se somam nem se substituem. O
  passo 7c (parâmetros) saiu do desvio "não medida" e mediu o EFEITO: a linha
  do id ligado voltou certa. E o teste `#[ignore]`
  `ponta_a_ponta_where_id_igual_pergunta` passou **aqui**, contra um `phxsqld`
  próprio em `PHXSQL_ODBC_PROVA` — 1 passed.
- **F-CONSULTA** — `agrupar`, `consultar` (com `juntar`/`escalar`/`janela`),
  visões, `diferencas`, `inserir.se_existir`, `sql.parametros`, e o acréscimo
  de junções/subconsulta escalar do dono (08/09 16:50). **925 → 1.042 testes:
  relatório da frente.** Confirmado nesta sessão pelas sondas vivas de
  `parametro_no_prepared`, `diff_de_dados` e o efeito de `criar_visao`/
  `SELECT * FROM v_c`, todas TEM, dentro de `bancada/comparativo/medir.py`.
- **F-BANCADA** (esta frente) — as quatro sondas vivas do comparativo; a
  remedição (18 de 19 faltando/pela metade → **3 de 19**, todas as três por
  decisão do dono ou limite documentado); o `#[ignore]` do ODBC fechado; os
  pedidos 233-239; o `CHANGELOG.md`; a corrente dos catorze geradores do
  dossiê. Tudo **medido aqui**.
- **Workspace** — `cargo test --workspace --offline`: **2.119 testes, 0
  falhas** — **medido aqui**, pelo `numeros-do-projeto.py` desta sessão
  (o mesmo valor que `CAPABILITIES.json` grava, com o commit conferido contra
  o `HEAD` desta árvore).

### Rodada dos limites nomeados — 9 de setembro de 2026

Ordem do dono: *«Next»*, depois de a rodada das dezoito fechar 17 de 19. A
rodada anterior deixou seis pedidos nomeados (234 a 239); o 239 é decisão do
dono e fica de fora; os outros cinco são esta rodada. Os contratos estão na
seção «Os limites nomeados» de `docs/propostas/comparativo-19.md`, escritos
**depois** de medir: os dois defeitos do 237 têm uma raiz só (o `consultar`
compõe linhas JSON sem tipo), e foi essa medição que decidiu a divisão —
`RIGHT`/`FULL`/`CROSS` entram no motor e não por troca de lados no tradutor.

| frente | escalão | por quê | papéis dispensados, e por quê |
|---|---|---|---|
| **C20-CONSULTA** — modelo tipado de cada lado do `consultar`, `direito`/`completo`/`cruzado`, `existe`, `COUNT(coluna)` no motor | **projeto e risco** | **forma da linha** (o defeito que já pagou três vezes: coluna que some quebra quem lê por posição), **protocolo** (`colunas` na resposta) e **portão de permissão** (`existe[].de` tem de ser visto pelo direito por coluna, ou vira a porta dos fundos) | **C** (nada de formato em disco: o modelo tipado vive na resposta, não no `.reg`), **E** (não há tela), **J** (semijunção por espalhamento é técnica de todo motor; a divergência nossa — só igualdade, porque cada sub-pedido passa inteiro pelo portão — está escrita no contrato) |
| **C20-SQL** — `ORDER BY` qualificado, `COUNT(coluna)`, `RIGHT`/`FULL`/`CROSS JOIN`, `[NOT] EXISTS` no tradutor | **mecânico e verificável** | tradução para contrato já escrito; o JSON produzido se confere campo a campo em teste de unidade, e o encontro com o motor é da integração | **C**, **E**, **D**, **J** |
| **C20-DIREITO** — regra de coluna que cita coluna inexistente recusa na carga | **projeto e risco** | segurança: **configuração que não é lida mente** — quem escreveu `salrio` acha que restringiu `salario`; e a decisão «tabela que ainda não existe aceita com aviso» é a mesma da chave estrangeira, e tem de ser tomada com a lei na mão | **E**, **J**, **D**; **C** só pela decisão da tabela-que-ainda-não-existe, herdada da FK |
| **C20-QA-ODBC** — o teste 234 fora do estado global; `SQL_C_WCHAR` nos dois sentidos | **mecânico e verificável** | o erro se vê: a suíte verde N vezes, e UTF-16 conferido contra um buffer montado à mão com par substituto | **C**, **E**, **J** |
| **C20-INTEGRAÇÃO** — merge, portões, PENDENCIAS/CHANGELOG/MODELOS, geradores, cinco páginas, push, backup | **projeto e risco**, feita pelo orquestrador | é onde aparece o defeito que nenhuma frente vê: `docs/SQL.md` e `bancada/guardas/catalogo.py` são tocados por mais de uma; o `existe` da C20-CONSULTA tem de ser o que a C20-SQL gera; e o direito por coluna (C20-DIREITO) atravessa o `existe` (C20-CONSULTA) | **E**; **D** rodou de hora em hora sem decisão desta rodada |

**Papéis convocados fora de frente:** **D** (zelador) — não por decisão desta
rodada: as **dezesseis** worktrees das rodadas anteriores, todas já
integradas (`git merge-base --is-ancestor`), saíram na abertura desta, com a
prova de que nenhum processo tinha `cwd` nelas nem havia `cargo`/`rustc`
vivo; e o cache incremental do `target` principal (2,8 GB) saiu pelo mesmo
motivo da cognição de 07/09 — o disco foi de 4,5 para 8,5 GB livres antes de
abrir as quatro. **F** (prova real) fica com cada frente: teste que falha com
o defeito reposto, saída guardada, e só depois o conserto. **G** (QA) é a
C20-QA-ODBC mais a entrada de cada guarda nova no catálogo. **H** (documentação)
e **I** (versionador) são a integração.

**Uma decisão de modelo que merece o motivo escrito:** as duas frentes de
projeto e risco foram no escalão **mais forte disponível**, e não no forte
comum, porque as duas tocam o portão de permissão — a C20-CONSULTA pelo
`existe`, a C20-DIREITO pela carga do cadastro — e o erro ali não aparece em
teste de tradução nem em tela: aparece como dado de outro que alguém leu.

### Rodada dos limites nomeados e dos gaps — 9 de setembro de 2026 (continuação)

Ordem do dono, depois de a rodada Next fechar: *«Fazer a revisão e fazer os
gaps»*. A revisão foi feita por três revisores só de leitura e prova viva; os
gaps de código, por frentes que reaproveitaram as worktrees das frentes Next à
medida que elas devolviam — o `target` já compilado de cada uma, e o disco em
5–8 GB, não comportava abrir quatro worktrees novas do zero.

| frente | escalão | por quê | papéis dispensados, e por quê |
|---|---|---|---|
| **Revisor do motor** | **projeto e risco** | procura vazamento e quebra de integridade nas ops novas, provando por soquete — o erro aqui não aparece em teste unitário | (não conserta; só mede) |
| **Revisor da documentação** | **mecânico e verificável** | número visível casado com o código; o erro salta na comparação | **C**, **E**; **F** é a própria medição |
| **Revisor da tela** | **mecânico e verificável**, mas exercitando | interface só se prova exercitando; o CSS global morde o componente novo | — |
| **G3-CIFRA** (gap 210) | **projeto e risco** | formato em disco e cifra; a guarda vermelha esperava desde 05/09, e o irmão só apareceu na reabertura | **E**, **J**; **C** é a própria frente (byte de material, `FORMATO.md`) |
| **G3-REPLICA** (gap 203) | **projeto e risco** | segurança: mexer em bloqueio sem medir o alcance abre a porta que ele fecha | **E**; **J** consultado (o `QUORUM` do Cassandra já medido) |
| **G3-CLAUDE** (gap 231) | **mecânico e verificável** | roteiro de navegador contra servidor falso; o número ou bate ou não | **C** (não toca motor), **E** (não edita tela) |
| **G4-MOTOR** (conserta os achados do revisor do motor) | **projeto e risco** | segurança e integridade: coluna negada que vazava, virava oráculo ou era zerada, e a junção que materializava antes do teto — o erro aparece como dado vazado ou perdido, não em teste feliz | **E** (a metade de tela foi para a G5-TELA), **J** |
| **G5-TELA** (front-end do direito por coluna e da coluna calculada) | **mecânico e verificável, exercitando** | interface só se prova exercitando; a prova é a bateria 51/51 nos dois temas e as capturas antes/depois. A decisão de risco de dentro — mandar a linha como objeto por nome só é seguro porque a coluna negada é de sistema ou calculada, e o servidor recusa o resto — está escrita na cognição | **C** (não toca formato em disco), **J** |
| **Integração** (orquestrador) | **projeto e risco** | o defeito do encontro: `catalogo.py` e `SQL.md` tocados por mais de uma frente, e o `existe` da C20-CONSULTA tinha de ser o que a C20-SQL gera; e o encontro da G5-TELA com o `adae44a`, medido antes de confiar (o doc do dono não toca tela nem motor) | **E** |

**O defeito do encontro apareceu, de novo, e foi visto só na integração:** as
frentes C20-CONSULTA e C20-SQL escreveram entradas no fim do mesmo
`bancada/guardas/catalogo.py` e no mesmo `docs/SQL.md`; o merge nu conflitou
nos dois. Resolvido mantendo as duas listas no catálogo (as guardas de cada
frente são defeitos distintos) e combinando o `SQL.md` por seção — o que o
motor faz e o que o tradutor faz são a mesma capacidade contada de dois lados,
e a prosa tinha de dizer isso sem repetir nem se contradizer.

**Uma decisão de modelo com o motivo escrito:** o revisor do motor e as duas
frentes de gap que tocam segurança (G3-CIFRA, G3-REPLICA) foram no escalão de
projeto e risco; o revisor de documentação, o de tela e a G3-CLAUDE, no
mecânico — porque o erro deles se vê na comparação ou na captura, e o erro dos
primeiros aparece como dado vazado ou tabela que não reabre, meses depois. As
duas frentes que consertaram o que os revisores acharam seguiram cada uma o seu
revisor: a **G4-MOTOR** no projeto e risco (conserta dado que vaza ou some), a
**G5-TELA** no mecânico-exercitando (a prova é a bateria e a captura) — com a
ressalva de que a única decisão de risco da G5-TELA, mandar a linha como objeto
por nome, foi decidida contra a restrição de que só a coluna de sistema ou
calculada pode sair assim, e escrita na cognição.

**O limite de uso da plataforma entrou na conta desta rodada.** Às ~03:50 a
sessão bateu no teto (HTTP 429, reset 05:20) e os sete agentes e a corrente do
batimento morreram juntos. Não é escolha de modelo, é teto de plataforma — e a
lição é a mesma da limitação do 403: **limitação que bloqueia um papel se
remede a cada rodada, e some quando o teto passa**. A retomada foi por
`SendMessage` a cada agente com o contexto preservado, e a worktree limpa de
cada um provou que nenhum tinha escrito antes de morrer.

## Fechar a rodada e o portão que amarra os geradores (11/09/2026)

Ordem do dono: *«pode seguir com o trabalho, ative o time»*. A revisão de gaps
tinha achado a rodada anterior fechada nas **fontes** (PENDENCIAS, STATUS, as
seis decisões de 10/09) mas parada nos **derivados**: os painéis apontavam para
`ccb45b1`, antes do 211. Fechar a rodada foi re-rodar os geradores e, o mais
importante, **amarrá-los a um portão** para não envelhecerem calados de novo.

| frente | escalão | por quê | papéis dispensados |
|---|---|---|---|
| **H — fechar a rodada** (14 geradores) | **mecânico e verificável** | rodar geradores roteirizados; o diff prova, é o «o que se confere sozinho» que a delegação mecânica pede | C, D, E, J — «fechar derivados» não toca formato, ambiente, paleta, nem pede receita de fora |
| **Portão — G+B+F** | **projeto e risco** | portão de QA cujo falso-negativo reabre a porta que a rodada fechou; os três modos (`exato`/`sem-carimbo`/`nota-cargo`) e a prova real RED→GREEN são desenho | (F é a própria prova; C/E não tocam) |
| **G-audit** (catálogo de guardas) | **mecânico** | varredura código↔catálogo, só leitura; o erro se vê na comparação | escreve nada |
| **Comparativo** (papel J, a pedido do dono) | **mecânico-síntese** | reunir número medido com procedência «medido/citado»; o erro se vê no doc | só leitura |
| **Integração** (orquestrador) | **projeto e risco** | o defeito do encontro apareceu: a H achou o `TECNOLOGIAS` **fora dos 14** (imprime-e-cola, envelhece), e o portão do agente, rodado contra o `6858fa4`, foi VERMELHO nomeando a própria falha que a rodada fechou. A prova real do portão foi **refeita aqui** — agente devolve relatório, não prova | — |

**Dois motivos de escalão medidos.** O escalão forte só no portão, porque é o
único com risco de projeto — um portão em que ninguém acredita não segura nada.
O resto no mecânico, porque o erro deles se vê no diff ou na comparação.

**O paralelismo segurado, com o número.** Só **uma** worktree isolada (a do
portão): o disco a 3,2 GB não comporta dois `target/` sem furar o piso de 2 GB
do zelador, e as frentes serializariam no mesmo `flock` do cargo de qualquer
jeito. Segurar frente por disco é decisão do dono — a regra do zelador.

**A lição de plataforma voltou por outro lado.** A corrente do batimento fino
«arrebentou» num falso positivo — um elo disparou **atrasado** (06:02 soltou às
06:20), e eu, de cabeça na rodada, quase forjei um segundo elo. Reconciliado
sem quebrar «nunca dois elos»: o elo atrasado só refrescou o pulso, o sucessor
já pendente seguiu sozinho.

## Rodada dos três tipos de database, da estrutura do hive e do charset do dossiê — 11/09/2026

Ordem do dono, três pedidos numa mensagem: criar a **infraestrutura dos três
tipos de database** (padrão/hive/vetorial); explicar **por que a página de
status apareceu com acentuação errada e formato diferente** das outras; e
desenhar **a estrutura do banco hive**. E, por cima: *«acione o time»*.

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Infra dos 3 tipos** (enum no núcleo + marcador `_database.json` + portão do motor) | **projeto e risco** | é formato em disco e invariante de concorrência de catálogo; a decisão «onde mora o portão do motor — no store, não no despachar» é arquitetura, não digitação | A, C, B, F, H, I | D (nada a limpar na hora), E (sem tela — o tipo ainda não tem UI), G (sem catraca nova — invariante provada por teste, não por `TETO`), J (os motores são frente futura, que começa por medir a premissa) |
| **Estrutura do hive** (proposta de formato) | **projeto e risco** | desenho de formato em disco inspirado no REGF, decidido contra as pétreas (append-only, cache no lugar de `mmap`, `.tx`, UTF-8) — não delegado, o núcleo arquitetural fica no forte | A+C+J (o orquestrador) | B/D/E/G — proposta, não código: nada a compilar, limpar, pintar ou travar; I só entra no commit |
| **STATUS — charset do dossiê** | **mecânico e verificável** | emitir `<meta charset="utf-8">` nos geradores, regenerar, conferir os bytes; o defeito se **reproduz** (latin-1 sobre UTF-8 dá o mojibake exato) e se vê no diff | E+H+F | C/D — não toca formato de dado nem ambiente; I é o integrador |
| **Zelador** | **mecânico** | inventário roteirizado + limpeza pelo `zelador.sh` sancionado, que erra para o lado seguro (prova por `cwd` real, nunca por data/nome) | D | — |
| **Integração** (orquestrador) | **projeto e risco** | o defeito do encontro apareceu de novo: o portão dos geradores voltou **VERMELHO** no `cobertura-por-area.py` — drift da contagem de testes que a **própria infra dos 3 tipos** criou (adicionei testes e não regenerei a cobertura no mesmo commit). Nenhuma frente sozinha via: o agente do charset não sabia que a contagem mudara, e a infra não sabia que a cobertura era derivada. Regenerado aqui (sem cargo — contagem estática), portão VERDE | A | — |

**O escalão pela natureza, não pelo tamanho.** Duas frentes de charset e
zelador são mecânicas — o erro se vê no diff ou na comparação de bytes, e é o
que o escalão leve/médio quer dizer. As duas de database são projeto: uma põe um
invariante de concorrência, a outra desenha formato em disco — e formato errado
depois vira migração, não `git revert`.

**O encontro cobrou de novo, e o culpado fui eu.** A regra «há defeito que só
aparece no encontro das frentes» bateu no `cobertura-por-area.py`: o meu commit
da infra dos 3 tipos (`61cf097`) somou testes e **não** rodou o gerador de
cobertura — o número ficou velho, calado, até a frente do charset rodar o portão
e ele acusar. A lição não é nova (é a pétrea «todo número sai de gerador»); o
**alcance** é: *quem soma teste fecha a cobertura no mesmo fôlego*, senão o
portão da próxima frente herda o vermelho.

## ACID-C — a cascata entra no conjunto de escrita — 15/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **ACID-C** (cascata no conjunto de escrita, marca v3) | **projeto e risco** | é **formato em disco** (marca `.tx` v3), **concorrência** (ordem de travas entre transações, plano→trava→replaneja) e **integridade referencial** (a cascata da FK) ao mesmo tempo — as três coisas que a cláusula manda no modelo mais forte. Foi feito pelo integrador, sem fan-out: uma mudança coerente que se dividida esconderia o defeito no encontro | A, C, B, F | D (nada a limpar), E (sem tela — transação não tem UI), G (a catraca aqui é a marca v3, provada por teste RED→GREEN, não um `TETO` novo), J (a receita de fora já viera medida na rodada anterior — o super-journal do SQLite e o handle único do InnoDB, `dba-bases-2026-09.md` §1.1) |

**Por que o forte, e não o leve.** A tentação de escalão leve seria «só empilhar
mais escritas na lista». Mas o que decide certo aqui é o que NÃO se vê no diff:
que manter a mãe cascateando no commit e sombrear as filhas dá **duas verdades
que divergem**; que travar as filhas com a trava de dados na mão **congela o
servidor** na espera; que reaplicar sem o byte da marca **grava a filha duas
vezes**. Cada uma é decisão de projeto, e errar qualquer uma só aparece rodando
— foi o escalão forte que as pesou antes do código. A prova real nos dois
sentidos (reposto o defeito, os testes de comportamento falham) é o que fecha.

## Ledger — travar `ALTER` na tabela-cadeia — 15/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Ledger `ALTER`** (guarda de `acrescentar_coluna` em modo ledger) | **projeto e risco** | é **integridade de dado** (o hash da cadeia é a garantia que a mudança quebraria) e uma **decisão de desenho não-óbvia**: se a régua relaxada do SQL Server mapeia para o nosso hash — e não mapeia, porque o conteúdo canônico pula coluna por NOME, não por posição. Errar isso deixaria a guarda passar coluna nula que ainda quebra a cadeia. Feito pelo integrador, sem fan-out: uma guarda de uma linha e três provas | A, C, B, F | D (nada a limpar), E (sem tela — `ALTER` de ledger não tem UI própria), G (a guarda aqui é teste RED→GREEN, não um `TETO` novo — catraca nova só quando há um número que só sobe), J (a receita de fora já veio medida: o §2.1 da `dba-bases-2026-09.md` trouxe as *ledger tables* do SQL Server, e a medição contra o nosso hash é justamente o que recusou a régua relaxada) |

**Por que o forte, e não o leve.** «Recusar um `ALTER`» soa mecânico. O que
pede o escalão forte é a pergunta que decide o ESCOPO da recusa: adotar a régua
do SQL Server (nula, no fim, fora do hash) ou proibir inteiro? A resposta certa
— proibir inteiro — só sai de medir o nosso `conteudo_canonico`, que exclui
coluna por nome e não por posição, então coluna nula no fim **entra** no hash. A
régua alheia teria compilado e deixado a cadeia quebrável, com teste verde. Foi
o escalão forte que fez a pergunta antes de copiar a resposta.

## Os agentes viraram arquivos — o tier de cada um — 16/09/2026

Os dez papéis pétreos deixaram de ser só governança e viraram agentes
invocáveis em `.claude/agents/` (mais o SEC e dois subagentes de pesquisa, do
comparativo com o Phoenix Cast). **Nenhum arquivo traz `model:`** — a pétrea
proíbe identificador de modelo em artefato versionado —, então o tier mora aqui
e o orquestrador o aplica na convocação.

| agente | papel | escalão | por quê |
|---|---|---|---|
| `engenheiro` | B | **forte** quando toca motor/formato/concorrência; **leve** no mecânico | o portão é o mesmo; a decisão de projeto não |
| `dba` | C | **forte** | formato em disco e garantias de dado são projeto e risco por definição |
| `designer` | E | **meio** — **forte** na marca/acessibilidade, **leve** na varredura de textos | a tela se prova exercitando; o CSS global é a armadilha que pede olho |
| `prova-real` | F | **forte** | a prova é o que mais engana; medir o VERMELHO é decisão, não roteiro |
| `qa` | G | **leve** para rodar catraca; **forte** para desenhar guarda nova | rodar é roteirizado; decidir o que a guarda cobre não |
| `documentacao` | H | **leve** | rodar gerador e medir número é mecânico e verificável |
| `pesquisador` | J | **forte** | pesar a receita de fora contra o nosso gargalo é projeto |
| `pesquisa-motor` | J-sub | **forte** | cripto, formato e norma — a fronteira do zero-deps |
| `pesquisa-bancada` | J-sub | **meio** | a medição é roteirizada; a interpretação do número é projeto |
| `pesquisa-rede` | J-sub | **forte** | transporte P2P, gossip e anti-entropia — o pilar do e-mail P2P é domínio novo, e desenho de rede é projeto e risco |
| `seguranca` | SEC | **forte** (sempre o mais forte) | adversário; para a segurança, a saída mais conservadora |

**Papéis que NÃO viraram agente, e por quê:** A (orquestrador) é esta sessão —
não se delega a si mesmo; D (zelador) e I (versionador/backup) rodam por
**script** (`zelador.sh`, `backup.sh`), não por subagente, e o integrador é
quem comita. **Dispensa registrada é decisão; dispensa silenciosa é
esquecimento** — e a camada de domínio SaaS do Phoenix Cast (conectores, redes,
anúncios, marketplaces, React) foi **dispensada por escopo**: serve a um produto
que o PhxSql-motor ainda não é.

**Por que arquivo, e por que agora.** Convocar um papel à mão a cada tarefa
funciona, mas deixa o modelo de cada um sem contrato escrito e o fan-out de
pesquisa sem forma. O arquivo dá o contrato (o que o agente faz, e o que ele
NÃO faz — revisor não escreve, ninguém comita sozinho) e o tier, um lugar só.
Não é abrir dez por tarefa: é ter o dono pronto quando o trabalho tocar o
domínio dele.

## O escalão por atividade virou pétrea, e os três pilares — 16/09/2026

Ordem do dono, no mesmo dia: *«Use o modelo 5.1 para atividades difíceis.
Redistribua as atividades do backlog e gaps do projeto com o modelo de iA
adequado para não gastar tokens de forma desnecessária. Isso é regra pétria.»*

O que **muda** e o que **não** muda:

- **Não muda o mecanismo** — casar o escalão à atividade já era a decisão do
  orquestrador, registrada aqui rodada a rodada. A ordem a torna **pétrea**: não
  é mais boa prática, é lei, e a economia de token é o motivo dito com todas as
  letras. Abrir o escalão forte para uma varredura de `grep` é o desperdício que
  a cláusula sempre avisou; agora desperdiçar é **quebrar pétrea**.
- **Fixa o escalão forte** como o modelo que o dono chama de «5.1». O **nome
  continua fora do repositório** — é a mesma regra do topo deste arquivo, e vale
  para «5.1» como valia para qualquer outro: o versionado guarda «escalão forte»
  e o porquê; o nome mora na conversa. Quem lê o repositório seis meses depois vê
  *que atividade mereceu o modelo caro*, não a etiqueta comercial dele.
- **A redistribuição do backlog aberto por escalão** é um entregável, não uma
  frase: vive em `docs/pmo/BACKLOG.md`, com cada item aberto marcando pilar,
  papel-dono, escalão e o teste que decide o escalão — *«o erro se vê?»*. É a
  planilha de controle que o dono pediu no molde do Phoenix Cast, adaptada à
  nossa casa: **ela não se digita à mão, deriva de `PENDENCIAS.md` e do
  `STATUS.md`** pela mesma razão de todo número visível — lista digitada
  envelhece calada.

E o **escopo** que a ordem fixa, porque muda o que é «projeto e risco» daqui em
diante: o produto deixou de ser só a base de dados. São **três pilares** sobre o
mesmo motor de zero dependências — a **base de dados** (PhxSql, o que já existe),
um **sistema de e-mails P2P** e uma **estrutura blockchain genérica para
gerenciar mini-contratos sigilosos**. O registro está em `docs/VISAO.md`, e a
pesquisa de blockchain que o fundamenta em
`docs/propostas/phxblockchain-melhorias-2026-09.md` (com a URL de cada fonte
primária, 16/09/2026). Os dois pilares novos abrem domínio novo — transporte
P2P e sigilo de contrato — e é por isso que nasce o subagente `pesquisa-rede` e
que o `seguranca`/`pesquisa-motor` ganham o contrato do sigilo: **nenhum papel
sem dono quando o trabalho toca o domínio dele.**

## Leitura repetível pela trava — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Fatias 1–4 e o tradutor SQL** (trava compartilhada, portão `dentro_da_transacao`/`travar_leitura_repetivel`, `BEGIN ISOLATION LEVEL REPEATABLE READ`) | **forte** | é **concorrência** (trava nova entre transações, ordem de espera contra intenção/exclusiva/linha alheia) e **protocolo** (novo campo `"leitura_repetivel"`, novo texto de `transaction_isolation`) ao mesmo tempo — as duas coisas que a cláusula manda no modelo mais forte. Errar a ordem de travas aqui trava o servidor inteiro; errar o portão único deixa um caminho de leitura sem a garantia que o nome promete | A, C, B, F | D (nada a limpar), E (a tela só ganhou duas chaves de texto, `tela.tx_isolamento_a/b`, pela fábrica — sem desenho novo), G (a guarda aqui é o par de testes RED→GREEN de `testes_leitura_repetivel`, não um `TETO` novo), J (a via já estava nomeada em `docs/SOMBRA.md` §5b desde a pesquisa de MVCC; o dono só precisava reabrir e escolher) |
| **Esta varredura de documentação** (ACID.md, TRANSACOES.md, SQL.md, CONCORRENCIA.md, CONTRATO-1.0.md, HFSQL.md, PDCA-GAPS.md, PENDENCIAS.md, CHANGELOG.md, BACKLOG.md, STATUS.md) | **leve** | é propagar um fato já decidido e já provado pela frente forte, verificável por `grep` linha a linha — não há decisão de projeto para tomar, só o texto para deixar de contradizer o código | H | A (orquestra, não se convoca), B/C (o código e o formato já estavam prontos e revisados), D (nada a limpar), E (nenhuma tela nesta varredura), F (nenhuma prova nova — as provas já existiam e só foram citadas), G (nenhuma catraca nova), I (não comita), J (nenhuma pesquisa nova) |

**Por que o forte na trava, e o leve na varredura.** A trava compartilhada
decide se um leitor pode travar um escritor e por quanto tempo — errar isso é
o mesmo risco de qualquer trava nova nesta casa (§ acima, ACID-C). A
varredura, em contraste, não decide nada: ela lê `docs/SOMBRA.md` §5b, os
testes de `servidor.rs` e o texto do `CLAUDE.md` já escrito pelo dono, e
propaga a mesma frase para onze documentos. O teste que decide — **«o erro se
vê?»** — dá sim para a trava (esconderia um servidor congelado) e não para a
prosa (um `grep` reprova o documento na hora).

**Custo medido do escalão leve nesta rodada** (somado do transcrito do agente,
não estimado): 246.557 tokens, 175 chamadas de ferramenta, 19 min 24 s de
parede, correndo em paralelo com fmt, clippy e a suíte inteira (cerca de 6
min), que rodaram no processo principal. Treze arquivos `.md` tocados, 368
inserções e 91 remoções. Dois achados que o escalão leve fez e valeram o
custo: a pendência nova era a #246 e não a #245 (o número no `CLAUDE.md`
estava errado), e o `COMPARATIVO.md` é gerado por sonda contra um servidor
vivo, então não se edita — se remede.

## Rollup do board e o agente tradutor (GOV-3, GOV-1) — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **GOV-3** — `docs/pmo/rollup.py`, o gerador que conta aberto/entregue/parado por pilar e os «forte» abertos do `BACKLOG.md`, e a entrada dele no portão dos geradores | **leve** | é varredura roteirizada de uma tabela Markdown com regra explícita (a primeira palavra da célula decide o estado; palavra fora do léxico PARA nomeando a linha) — o resultado se confere sozinho por hash: duas corridas sem mexer na fonte deram o mesmo arquivo | A, H | B/C (nenhum código do motor, nenhum formato), E (não há tela), F (a prova é a idempotência por SHA-256, feita pelo próprio H), G (o portão dos geradores já é a catraca; a entrada nova entrou nele), J (nada de fora a medir), D/I (nada a limpar; o integrador comita) |
| **GOV-1** — `.claude/agents/tradutor.md`, o agente multilíngua pétreo (pedido #110), e as duas linhas no `README.md` dos agentes | **leve** | é escrever uma definição de papel a partir de leis que já existem (`docs/MENSAGENS.md`, as três armadilhas, a catraca `TETO_ROTULOS_E_CRASE`) — copiar o molde dos agentes irmãos e citar as fontes certas, verificável por leitura | A, H | os mesmos de cima; e o próprio tradutor **não rodou** nesta rodada (a árvore estava ocupada pela frente T compilando), então a primeira corrida dele fica registrada como pendente no board, não como feita |

**Custo medido do escalão leve nas duas** (do transcrito do agente, não
estimado): 151.598 tokens, 35 chamadas de ferramenta, 9 min 47 s de parede,
em paralelo com a frente T (forte) e a pesquisa P2-DESIGN. Um achado
colateral que valeu o custo: o `PLANO` do portão já tinha **15** entradas
antes desta rodada — o `docs/tecnologias/extrair.py` está no portão e fora da
conta «catorze» do `LEIA-ME.md`, que por definição só conta os scripts de
`docs/dossie/`. Com o rollup são 16, e a conta «catorze» continua certa pela
definição dela — mas a frase do `CLAUDE.md` que diz «listados no LEIA-ME da
pasta» passa a valer só para o dossiê, e o portão é a lista completa.

## Pesquisa do transporte P2P (P2-DESIGN) — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **P2-DESIGN, a pesquisa** — `docs/propostas/p2p-transporte-2026-09.md` (descoberta, NAT, gossip/anti-entropia, identidade sem domínio, transporte sem servidor central) | **meio** | o board marca o item como **forte** porque o *desenho* é arquitetura de rede e formato — mas esta rodada não desenhou: **mediu a premissa antes do item**, que é leitura de fonte primária (RFC, paper, fonte de projeto) confrontada com números que a casa já tinha (`bancada/replicacao`, `bancada/quorum`, contagem de `UdpSocket`/`Condvar` no fonte). O erro aqui se vê — fonte sem URL, número sem «quem mediu e quando» — e o resultado é um documento que o integrador confere linha a linha, não um formato que grava dado. O escalão forte fica reservado para o desenho que vier depois, quando o dono decidir o AAD (pendência #251) | A, J (`pesquisa-rede`) | B (nenhuma linha de Rust, por contrato), C e SEC (**convocados por nome no documento** para a rodada seguinte: quatro itens são formato em disco e um muda o modelo de ameaça — não cabiam nesta pesquisa), F (só quando houver código: P4 e P5 pedem prova real nos dois sentidos), D/E/G/H/I (nada a limpar, nenhuma tela, nenhuma catraca, nenhum commit) |

**Custo medido do escalão meio** (do transcrito do agente, não estimado):
224.989 tokens, 75 chamadas de ferramenta, 18 min 8 s de parede, em paralelo
com a frente T (forte) e a governança (leve). O que o escalão meio comprou e o
leve não compraria: achou que a recusa de DHT do `P2P-DISTRIBUIDO.md` §5.2
**herdava uma premissa que caducou** (IP fixo e registro `A` no Cloudflare) e a
remediu em vez de copiá-la — e achou o choque de frentes do AAD do selo, que é
exatamente o defeito que só aparece no encontro das frentes.

## Painel PMO — a sexta página, gerada — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Painel PMO** — `docs/pmo/pagina-do-status-do-projeto.py` → `status-do-projeto.html` (painel, fluxo, equipe), no molde dos três slides do Phoenix Cast que o dono mandou | **meio** | os números são mecânicos — os leitores já existiam (`pagina-dos-pedidos.py`, `rollup.py`, `CAPABILITIES.json`) e o portão confere a página com o defeito reposto —, mas a **tela** não é: marca, contraste nos dois temas, forma além da cor, 400 px, e a lei «interface só se prova exercitando». O erro aqui se vê **só na captura**, não no código: quatro defeitos saíram das capturas (legenda atravessando a caixa do fluxograma, «sim» encostando no PARADO, a caixa do J esticando a grade, a branch partindo no meio da palavra). É o escalão do designer para tela nova, e não pediu o forte porque nada aqui grava formato ou toca o motor | A, E (com os leitores do H reaproveitados) | B/C (nenhum código do motor, nenhum formato), F (a prova real é o portão dos geradores reprovando com um número trocado à mão — código 1 —, feita pelo próprio E), G (o portão já é a catraca; a entrada nova entrou nele), J (nada de fora a medir), D/I (nada a limpar; o integrador comita e publica) |

**Custo medido do escalão meio** (do transcrito do agente, não estimado):
228.499 tokens, 75 chamadas de ferramenta, 19 min 32 s de parede, em paralelo
com a frente T (forte). O que o meio comprou e o leve não compraria: as
quatro correções de captura acima, e a decisão de mostrar o escalão pelo nível
com «não registrado por papel» para o tradutor em vez de inventar um.

## Semáforo e teto das threads (frente T, pedido 248) — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Frente T** — `Semaforo` do core, permissão RAII na porta de dados, teto e 503 na web, `FichaViva` na telemetria, monitor em runtime, mapa das threads com catraca, enxurrada de 500 conexões | **forte** | é **concorrência** pura: um semáforo escrito sobre `Mutex`+`Condvar` (a `std` não tem), com `Drop` que roda no desenrolar de um pânico e mutex envenenado no caminho — errar aqui é o pior tipo de defeito, o servidor de pé recusando todo mundo. E a lei da casa: a instrumentação desligada tem de custar zero e o portão vem antes do trabalho (a vaga se pede ANTES de subir a thread). O erro **não se vê** no teste comum; vê-se em produção depois de N pânicos | A, B, F (as quatro guardas vermelhas com o defeito reposto), G (mapa com catraca, item 0c), E (a régua no gestor, exercitada nos dois temas), H (CONCORRENCIA §17, TELEMETRIA, MENSAGENS, MANUAL) — todos no mesmo agente, por contrato | C (nenhum formato em disco), D (nada a limpar; as corridas ficam versionadas), J (a convergência do trio em `max_connections` já estava medida no quadro), SEC (revisão adiada para a frente D, que toca alerta e e-mail — o 503 não expõe nada além do `Retry-After`) |

**Custo medido do escalão forte** (do transcrito do agente, não estimado):
412.136 tokens, 157 chamadas de ferramenta, 49 min 43 s de parede — a frente
mais cara do dia, e a única que compilou nesta árvore enquanto as outras
corriam em paralelo. O que o forte comprou: achou que o fecho **já tinha
teto** (o quadro do orquestrador estava errado, e o agente mediu em vez de
obedecer), desenhou a «fila declarada cheia» que o contrato não previa (sem
ela, uma saturação longa entregaria um 503 a cada 2 s), e trouxe o monitor em
runtime pedido no meio da tarefa sem largar a prova real do que já estava
feito.

## Saúde do disco (frente D, pedido 249) — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Frente D** — sonda canário, classificação de erro por `ErrorKind` e errno, gancho no sumidouro de erros com aviso imediato fora da trava, e-mail e SMS por gateway da operadora, painel, cartão, guardas, revisão SEC | **forte** | é caminho de **alerta de falha de disco**: errar para um lado é silêncio no dia em que o disco morre, errar para o outro é 100 mil e-mails numa carga com erro por linha — e o gancho mora no caminho de gravação, sob a trava global, onde um envio de rede congela o servidor inteiro (foi exatamente o que a primeira versão fez, e a catraca pegou). O que depende do sistema operacional se prova contra ele, e aqui é errno a errno | A, B, F (quatro guardas vermelhas, prova pelo soquete e contra o SO), E (cartão exercitado nos dois temas), SEC (revisão escrita em `SEGURANCA.md` §3), H (`SAUDE-DO-DISCO.md`, MANUAL, exemplo de config), G (entrada no catálogo do mapa das threads) — todos no mesmo agente, por contrato | C (nenhum formato em disco: o canário é arquivo próprio, fora do `.reg`), D (nada a limpar), J (nenhuma receita de fora a medir), I (o integrador comita) |

**Custo medido do escalão forte**: 415.895 tokens no transcrito inteiro; a
contagem de chamadas e a duração de parede **se perderam no reinício do
contêiner** às 07:30 (a notificação final só cobre o trecho retomado: 9
chamadas, 6 min 38 s) — número que não se mediu não se publica. O que o
forte comprou: a classificação por errno que a `std` não dá (`EIO` é
`Uncategorized`), a prova real contra o SO feita como root sem fingir que
`chmod` segura, e o desenho da fila para o aviso imediato depois de a catraca
`rede-ou-espera` reprovar a primeira versão.

## Chutar a tomada (bancada, pedido do dono) — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Bancada «chutar a tomada»** — SIGKILL na transação aberta, no BULKINSERT, no `inserir_lote`, no `reindexar` e na transação dentro da reserva; 408 quedas; quatro guardas | **forte** | é prova de **consistência sob queda**, o lugar onde um teste que passa por engano é pior que teste que falta: cada ponto exige ler o código antes de afirmar o que ele promete, e o veredito depende de um byte no disco lido antes de reabrir, não da palavra do servidor. O escalão se pagou duas vezes na própria corrida: o instrumento cegava a si mesmo recortando o erro antes de casar o texto, e a terceira guarda «não pegou» na primeira troca porque o estado chegava ao disco por caminho irmão — os dois viraram cognição | A, F (prova real nos dois sentidos), G (quatro guardas no catálogo), J (a hipótese sobre o `criar` do índice, morta medida) | B (achados #254/#255 vão para ele, não foram consertados aqui por contrato), C (nenhum formato tocado), E (sem tela), H (a entrada na página de testes é do integrador), I (o integrador comita) |

**Custo medido do escalão forte**: 391.025 tokens no transcrito inteiro; a
contagem de chamadas e a duração de parede **atravessaram o reinício do
contêiner** (a notificação final cobre só o trecho retomado: 19 chamadas,
31 min 39 s; a corrida publicada durou 672,6 s de bancada mais 694 s
esperando o portão). O que o forte comprou: três achados reais (#254, #255,
#256), uma hipótese morta com número (111/111) e a leitura de que
«BULKINSERT em transação» é recusa do motor, não ponto de queda.

## Colmeia × SQLite × padrão, as quatro operações (bancada) — 16/09/2026

| frente | escalão | por quê | papéis convocados | dispensados, e por quê |
|---|---|---|---|---|
| **Bancada CRUD** — modo `crud` do exemplo, `medir-crud.py`, dois regimes, três lados, syscalls por operação, §1.1 do `colmeia.md`, linha C do `STATUS-TIPOS.md` | **forte** | bancada de três motores com trabalho igual é onde esta casa já errou **duas vezes** (o `WHERE id IN` contra vinte mil buscas; o `COUNT` sobre 1.250.000 contra a leitura de 20.000), e as duas apontaram para lados opostos sem aparecer no número. Aqui o append-only da colmeia, o casamento de durabilidade e o custo por chamada do Python são três lugares para errar calado; o resultado que importa — a colmeia **perdendo** a 100.000 sem fsync — só aparece para quem mede o fanout em vez de declarar vencedor | A, B (o exemplo e o maestro), J (a leitura contra o gargalo), F (read-back e contagem nos três lados, vencedor só fora do ruído) | C (o formato PSHV continua proposta; a premissa nova do fanout vai para a decisão dele), E (sem tela), G (sem catraca nova — o achado do portão virou pendência #260), H (a entrada na página de testes é do integrador), I (o integrador comita) |

**Custo medido do escalão forte**: 337.597 tokens no transcrito inteiro; a
contagem de chamadas e a duração atravessaram o reinício do contêiner às
07:30 (a notificação final cobre o trecho retomado: 31 chamadas, 41 min 48 s
— o trecho anterior não se mediu e não se publica); corrida de bancada 1.154,6 s com 181 s de espera
pelo `cargo` da frente vizinha. O que o forte comprou: a derrota da colmeia a
100.000 sem fsync dita como resultado e não escondida, a contagem de fsync
por lado que explica o número inteiro (2 / 8–9 / 4), e dois achados novos
sobre o padrão (#258, #259).

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

## Rodada de 16/09/2026 — os consertos que nao dependiam do dono

Sete frentes. O criterio foi o de sempre: projeto e risco no escalao forte,
mecanico e verificavel no leve. A conta que o dono cobra e esta.

| frente | escalao | por que | papeis dispensados |
|---|---|---|---|
| B1 — pedido 254, a marca `.tx` que sobrevive ao `bulkinsert(false)` | forte | transacao e durabilidade: quem erra aqui perde commit confirmado | E (nenhuma tela muda), J (o achado e nosso e ja medido, nao ha receita de fora) |
| B2 — pedidos 258 e 259, os `fsync` por operacao e o custo do excluir | forte | caminho quente e garantia de dado, com mudanca no que vai ao prato | E, J |
| C — parecer: pode pular descritor limpo? | forte | e o papel que diz NAO quando uma proposta boa quebra uma garantia; so leitura | — |
| G1 — pedidos 256 e 260, o `pkill` sem PID e o portao que ve a casca | medio | ferramental de bancada, e a regra de QA foi decidida pelo orquestrador antes, entao o agente implementa em vez de desenhar | C (nao toca formato nem chave), E, J |
| F247 — a falha nao reproduzida do gerador de identificador | forte | garantia de identidade do dado, e a causa podia estar no relogio, no teste ou na concorrencia | E, J |
| U — pedido 245, o gatilho do upsert | forte | semantica de gatilho e correcao de dado; e a lei dos tres motores decide | C, E |
| integracao | — | commit por caminho explicito, portoes, geradores, paginas, backup | — |

**O que a escolha do escalao comprou, medido nesta rodada:** as quatro frentes
fortes derrubaram a premissa escrita do proprio pedido em **tres** dos quatro
casos — o 254 («chame a mesma drenagem» nao consertaria nada), o 245 («o BEFORE
ve a linha errada», e ele nao rodava) e o 259 (os `openat` eram do
`/dev/urandom`, e o instrumento mudava o ritmo que mudava a contagem). A frente
media entregou os dois consertos dela sem derrubar premissa nenhuma, que e
exatamente o perfil de trabalho que ela devia receber.

**E o custo de nao ter dispensado ninguem em silencio:** o papel C foi
convocado por uma pergunta so, e a resposta dele mudou o desenho do 258 antes
de haver codigo — o sinal em RAM e cego ao processo morto, e sem o batismo a
mudanca perderia commit confirmado sem bilhete.

## Rodada dos parciais e planejados — 264, 265, 266, 267 e 245 — 16/09/2026

Quatro frentes. Escritas **fora de hora** — o registro delas nao entrou no
mesmo dia, e a falta apareceu na rodada seguinte ao conferir este arquivo.
Papel A que nao registra a escolha dele cobra dos outros o que nao faz: a
falha fica dita aqui em vez de ser corrigida em silencio.

| frente | escalao | por que | papeis dispensados |
|---|---|---|---|
| 264 — o gerador de riscos: `docs/RISCOS.md` e `docs/status/riscos.py` | forte | a secao nao nascia por falta de FONTE, nao de gerador; escrever a fonte e decidir o vocabulario fechado de probabilidade e impacto e projeto, nao transcricao | C (nao toca formato), E, J |
| 265+266 — bancada de telemetria e a serie historica | forte | medir o custo de um observador ligado contra desligado numa maquina em carga e estatistica aplicada, e a primeira corrida entregou a telemetria «acelerando» o servidor em 37% | C, E |
| 267 — os tres panicos que sob carga viram dois | forte | concorrencia: o alvo era o motor e o defeito estava no TESTE, e distinguir os dois exige medir 1.000 corridas em vez de ler | E, J |
| 245 (O2-O6) — teto de 64 bits, saida do direito por coluna, contratos | forte | semantica de tipo e de permissao, e o O2 acabou escalado ao dono em vez de implementado | E, J |

**O que a escolha comprou, medido:** tres das quatro frentes derrubaram a
premissa do proprio pedido. O 267 nao tinha defeito no motor — medido em 1.000
corridas sob carga 13-15, a terceira conexao SEMPRE entrava, e o que faltava
era uma vaga. O 265 trocou a media pelo **piso (p10)** depois que a mediana do
`ping` andou de 59 para 161 us entre duas corridas de dois minutos. E o 266
teve a prova real acontecendo **em campo**: a segunda linha da serie chegou
sozinha, gravada por uma frente vizinha.

**Uma regua trocada, e ela e do dono decidir:** a frente 265 substituiu a faixa
min-max do pedido 155 por um teste de sinal a 3 sigma, **so na bancada de
telemetria**, com o motivo escrito — com efeito de ~1% e ruido de 3x, a faixa
crua diria «nao sei» ate se a telemetria dobrasse o custo. A troca esta
registrada, nao aplicada as outras bancadas, e espera a palavra do dono.

## Rodada das duas petreas sem guarda, e dos ponteiros errados — 16/09/2026

Duas frentes, e a segunda nasceu de uma varredura em vez de um pedido.

| frente | escalao | por que | papeis dispensados |
|---|---|---|---|
| G-CRIPTO — as petreas «criptografia se confere contra vetor oficial» (0 entradas) e «portao de permissao e UM so» (2 de 13 catalogadas) | forte | escolher QUAL defeito repor e o trabalho; defeito que derruba tudo nao ensina nada, e a tabela «quem NAO o pega» so sai de quem entende a norma | C, E, J |
| PONTEIROS — dois ponteiros errados no fonte, e o pedido 268 que um deles devia citar | medio | varredura verificavel: o alvo e conferir se o que o comentario aponta ainda existe, e isso se confere sozinho | C, E, F, J |

**O que a escolha comprou:** a frente forte **corrigiu um numero meu** — o
briefing dizia «11 testes de vetor» e sao **11 vetores** em 28 funcoes de teste
contra vetor publicado, em 9 normas. Briefing de orquestrador tambem e numero
citado, e numero citado e numero que nao se mede.

E ela achou, por consequencia, o pedido **269**: fechar os dois buracos levou o
catalogo de 151 para 160 entradas, e a tabela publicada continuou dizendo 143 —
17 ids que a corrida publicada nunca julgou. Achado que so aparece porque
alguem mexeu no numero ao lado.

## Rodada da quinta regua, da senha e do parecer da divida — 16/09/2026

Tres frentes, todas nascidas do `docs/CATRACAS.md` §15.5, que nomeia **medido**
o que ficou descoberto. Escolher os alvos de uma secao que se escreveu a si
mesma e o oposto de escolher de memoria.

| frente | escalao | por que | papeis dispensados |
|---|---|---|---|
| F-269 — a quinta regua e o gerador que confessa | forte | desenho de catraca: a recusa de encolher nao pode transformar aposentadoria de guarda em parada permanente, e errar a direcao cria uma regua que ESCONDE o encolhimento | C (nao toca formato nem chave), E, J |
| F-SENHA — a petrea «senha nunca em texto puro», 0 entradas | forte | mesmo motivo da G-CRIPTO: escolher o defeito plausivel e dizer quem NAO o pega e o trabalho todo | C, E, J |
| F-DIVIDA — parecer: a divida marcada merece catraca? | medio | leitura e medicao com uma pergunta ja delimitada pelo orquestrador; o agente mede e argumenta, nao desenha | B, C, E, F |

**A pergunta que a terceira existe para responder**, e por isso ela e parecer e
nao conserto: catraca sobre marca de divida pune quem marca honestamente, e o
jeito mais barato de ficar verde vira **apagar a marca sem pagar a divida**.
Recomendacao «nenhuma catraca, com o numero» e resultado valido — hipotese que
morre medida impede a mesma ideia de voltar sem medicao.

## O time inteiro — ordem do dono, 17/09/2026 00:24

«Continue. Ative o time inteiro.» Nove frentes de uma vez, mais A e I (o
integrador). A regra que decidiu cada frente: **domínio real, tirado do estado
medido da noite** — nenhuma tarefa de figuração para dar papel a quem nao
tinha. Papel sem frente real nesta rodada e dispensa registrada, nao papel
convocado por cortesia.

| papel | frente | escalao | por que | dispensa |
|---|---|---|---|---|
| B+G | o conferidor de `derive(Debug)` com segredo — a regua que trava a LEI, nao a struct | forte | desenho de catraca; o crivo tem uma parte que so se decide lendo, e a lista de falsos positivos e onde a regua mente se ficar escondida | — |
| C | parecer do 268: a migracao da cifra e o cabecalho do `.reg` | forte | formato em disco e ordem de digitacao; e o papel que diz NAO quando uma proposta boa quebra garantia; so leitura | — |
| D | o zelador apaga 1,4 GB por hora que o provador refaz — medir o custo antes de continuar | medio | a regra do zelador diz quando NAO apagar; nao diz quando apagar vale a pena — e um numero, com as maos de engenheiro para mudar o script se o numero mandar | — |
| E | as quatro paginas que ganharam conteudo esta noite, exercitadas no navegador nos dois temas e duas larguras | medio | interface so se prova exercitando; o conteudo e novo (lista de 27 ids num blockquote, R19 reescrito, rosca com 269) e ninguem abriu | — |
| F | as 26 provas da senha ainda sem guarda — uma por SAIDA, com o raio medido por sonda | forte | escolher o defeito plausivel e dizer quem NAO o pega e o trabalho todo; e a armadilha do trecho ambiguo foi paga ha uma hora | — |
| H | a documentacao da rodada: 269 fechado, os dois pedidos novos, 268 anotado, CHANGELOG, STATUS, e os inventarios que envelheceram esta noite | medio | todo numero sai do gerador ou da mensagem de commit; e trabalho de varredura verificavel | — |
| J | newtype `Segredo` vs nove `impl` a mao vs conferidor — a premissa que ninguem mediu antes de construir | forte | e a pergunta que muda o que a frente B+G entrega; le o fonte do `secrecy` e do `zeroize` como leitura, nunca como `Cargo.toml` | — |
| SEC | revisao adversaria: o `Debug` fechou, e `Display`, erro, log, `para_json`, panico, rede? | forte | so leitura, mas cada achado exige prova de leitura e severidade contra as tres saidas da petrea | — |
| tradutor | baixar a catraca dos 1.049 textos cravados — um arquivo zerado, o TETO descendo no mesmo commit | leve | mecanico e verificavel: o conferidor conta, a catraca so desce, e a prova e repor um texto e ver a catraca reprovar | — |
| B-SEC | os achados A1, A2 e A4 da revisao SEC: `token_remoto` fora da lista `SEGREDOS` do profiler, o job que guarda o pedido inteiro, e os dois JSON gravados em 0644 — aberta as 00:52, DEPOIS de conferir o A1 no fonte | forte | conserto de seguranca cuja FORMA e a decisao: a lista por nome e o defeito, e o conserto certo e a regua que a impede de envelhecer, nao mais um nome | C (nenhum formato muda), E, J |
| A | orquestrar, integrar, registrar — este arquivo | — | — | — |
| I | o fecho: portoes, commit por caminho explicito, push conferido, os 20 geradores, `./backup.sh` provado restaurando | — | — | — |

**Onde a integracao vai doer, dito antes de doer:** tres frentes tocam o
`cargo` (B+G, F, tradutor) e serializam pelo `flock`; F e B+G tocam a mesma
familia de arquivos (`bancada/guardas/`) e a fronteira e por ARQUIVO — F e dona
do catalogo e do `CATRACAS.md`, B+G entrega o texto dela em relatorio; E
regenera paginas que H tambem regenera por outro gerador, e as duas convergem
no fecho. Tres frentes escrevem `docs/propostas/` (C, J, SEC), cada uma num
arquivo so, e nenhuma escreve no `SEGURANCA.md` — o achado de SEC entra na
integracao, nao por ela.

**O que o time inteiro custa, para o dono saber:** nove agentes em paralelo
sobre uma arvore com ~4 GiB livres e um compilador serializado. O zelador
mediu o custo de apagar o cache do provador exatamente porque tres frentes vao
pagar compilacao fria ao mesmo tempo.

**O que a escolha do escalao comprou, medido no fecho (17/09, 01:15–01:50):**
as nove frentes voltaram e **nove vezes o briefing do orquestrador estava
errado, e foi a frente que corrigiu** -- «quatro crates» eram tres, o campo
era `token` e nao `token_remoto`, «0 entradas/11 testes» eram 2/40, «nove
nomes» na lista de segredos eram onze, o CHANGELOG mora na raiz e nao em
`docs/`, o `STATUS.md` nao tem linha de seguranca nem de QA, o `TESTES.md`
nao e o que a `pagina-dos-testes.py` desenha, e os riscos sao a §09 e nao a
§18. Todas as nove vieram de frentes **fortes e medias**: quem le o fonte
antes de obedecer ao briefing e o que o escalao paga, e nesta rodada pagou
nove vezes. A frente **leve** (tradutor) entregou o que prometeu -- a catraca
de 1.049 para **950**, 25 testes -- e custou uma coisa que nao estava no
contrato: um `git stash` na arvore compartilhada as 00:48 escondeu o
`zelador.sh` do D do commit `86e0b8c`, e ele so entrou em `7219699`. Nao e
defeito de escalao, e de regra: **nenhuma frente mexe no indice do git**, e a
regra passou a estar escrita no briefing. O encontro das frentes deu **B-SEC**
-- aberta as 00:52 depois de A conferir o achado A1 de SEC no fonte, porque
frente aberta por relatorio de outra frente e frente aberta por ouvir dizer.
O portao dos geradores saiu **VERDE na primeira** com os vinte, e o
`CAPABILITIES.json` andou 2.394 → **2.411** testes, 66% → **69%** da tela na
fabrica. E a regua do pedido 273 entrou no fecho **pelo proprio A, com o
chapeu de G, sem agente**: cabia em trinta linhas, e a decisao de forma (so
`--lib`) saiu da medicao antes do codigo -- a receita do pedido nasceria em
**164**, nao em 0. Dispensa registrada, com o numero.

## Rodada da replicação — bateria, revisão e conclusão — 17/09/2026 02:27 UTC

Ordem do dono: *«Dossiê atualizado · Status · Replicação bateria de testes,
revisão e conclusão»*. Board: `docs/pmo/RODADA-2026-09-17-replicacao.md`.
Onda 1, em paralelo — cada frente **só leitura** ou bancada isolada, para não
disputar `flock`/soquetes com a bateria de tempo:

| papel | frente | escalao | por que | dispensa |
|---|---|---|---|---|
| F | a bateria inteira: build, `montar`+`medir`, `modos`, `trava`, `credencial-recusada`, `cluster/provar`+`fresta`+`escalonar`, `quorum/medir`+`canal`, docker se o daemon subir | forte | prova real e o papel mais facil de fingir; cada ERRO precisa de diagnostico medido separando motor/bancada/encontro de frentes | — |
| SEC | revisao adversaria de replica/cluster/quorum/portoes 2a-2b-bis, so leitura | forte | sempre o mais forte; cada achado exige prova de leitura e severidade | — |
| C | parecer das garantias de dado: o que a replica garante, o commit com cascata, a posicao, o bidirecional, PITR, formato pendente | forte | formato em disco e garantias; e quem diz NAO | — |
| G | inventario guarda x petrea da replicacao, ESTATICO, e a lista dos `--so` para depois da bateria | medio | inventario e mecanico mas «petrea sem guarda» exige leitura; nao compila para nao disputar o flock com F | — |
| J | frente paralela: material do Query Designer do Phoenix — verificar se ha algo a aproveitar | forte | recusar ou aproveitar codigo de outro projeto exige ler o fonte inteiro e medir contra o nosso gargalo (injecao, portao unico, zero-deps), nao so ler o mockup | — |
| E (designer) | nenhuma tela nova; as paginas se regeneram do mesmo molde exercitado esta noite | — | — | dispensa registrada |
| D (zelador) | rodou as 02:15 (33 MiB; 2,3 GiB livres); o gatilho de hora em hora continua; F apaga o que a bancada cria, por caminho | — | — | dispensa registrada |
| tradutor | nenhum texto de tela nesta rodada | — | — | dispensa registrada |

**Onda 2** — depois da onda 1: G roda `provar-guardas.py --so` da familia da
replicacao (compila, so depois de F soltar as portas e o flock); H (medio)
escreve `REPLICACAO.md` §21, `STATUS.md` linha B, `PENDENCIAS.md` e
`CHANGELOG.md` — todo numero sai de gerador ou dos relatorios das frentes, e e
varredura verificavel; A/I integram, rodam os portoes e publicam as sete
paginas.

**B foi convocado nesta onda, largada 02:59 UTC** — SEC/C acharam defeito com
conserto delimitado (o contrato de A5/A2/A1-parcial/A3/continuidade, escrito
as 02:58 UTC), e a condicao do briefing («so entra se houver defeito
delimitado») se cumpriu cinco vezes de uma vez. Escalao **forte**, pelo mesmo
motivo que separa B dos papeis mecanicos desta rodada: os cinco itens tocam
motor e concorrencia — o portao de permissao (2b-bis), o gate das replicas
autorizadas, o teto de leitura do diario sob trava global, e a conferencia de
continuidade entre dois diarios — e cada um exige prova real nos dois
sentidos (o teste FALHA com o defeito reposto) contra servidor de pe, nao
so leitura. Integrado em `49a3af7` (03:53 UTC, 49 min 12 s de frente, 124
ferramentas), com os cinco itens inteiros e nenhum entregue pela metade —
"meia funcionalidade que for pior que nada volta como parecer, nao como
codigo" era a propria regra do contrato.

**Por que forte em quatro papeis de uma vez**: e projeto e risco em quatro
eixos diferentes — SEC audita seguranca de um protocolo de rede com
credencial compartilhada, C decide sobre formato em disco e garantia de dado,
F prova concorrencia e queda de processo pelo soquete, e J precisa ler
898 linhas de Rust alheio e decidir se algo entra na base zero-dependencias
desta casa. Nenhum dos quatro e varredura roteirizada. G e o unico medio
desta onda porque o trabalho e mecanico (grep no catalogo, leitura de
`ultima-corrida.json`) apesar de exigir leitura para nomear a petrea — e H e
medio pelo mesmo motivo: documentacao desta rodada e transcrever numero
medido por quem mediu, com a fonte e a data ao lado, nao decidir arquitetura.

## Onda 3 — «Continue fazendo os gaps» (ordem do dono, 17/09/2026 03:58 UTC)

| papel | frente | escalao | por que | largada |
|---|---|---|---|---|
| B2 | nove gaps com conserto delimitado e sem formato, em ordem de valor: rownum no lote (291), A1 pleno pelo tunel (278), A8 trilha LGPD do `replicar` (285), A4 `propagar:false` (281), A6 `cluster_estado` partido (283), A5 resto (282), A9 (286), A10 (287), A11 (288) | forte | seguranca, concorrencia e integridade — cada item mede a premissa antes e volta como parecer se ela cair, no molde do contrato da onda 2 | 04:01 UTC |
| G2 | estatico: reancorar a guarda `trava-atras-da-rede`; o provador passa a copiar o que os testes leem fora de `crates/`, com conferidor derivado do proprio codigo; `--so --json` mescla por id em vez de sobrescrever; sete petreas + `cluster.rs` entram no catalogo (pedidos 301/302) | medio | catalogo e ferramenta em Python, sem `cargo` — o provador so copia a arvore de trabalho, e B2 estava mutando `crates/` ao mesmo tempo | 04:02 UTC |

**B2 fechou oito dos nove itens inteiros, e o nono voltou como parecer** —
integrado em `eeb9925` (04:46 UTC, 41 min 43 s de frente, 116 ferramentas),
+1.385/−52 em nove arquivos, 20 testes novos, RED medido em cada um dos oito.
Escalao **forte** pelo mesmo motivo da onda 2: os itens tocam o motor
(`numerar_linha`/`consumir_rownum` no *store*), o portao do cluster
(`cluster_no_remover`/`cluster_no_acrescentar`, `cluster_estado`), a trilha de
LGPD e a classificacao de erro de rede — nenhum e varredura roteirizada, e
cada um exige prova real nos dois sentidos contra servidor de pe. O item que
voltou como parecer (A1 pleno, identidade do no pelo tunel) nao e falha de
escalao: a premissa **morreu na leitura** do `fio.rs` (Noise NX so autentica o
respondedor) antes de qualquer linha de conserto ser escrita — exatamente o
que «medir a premissa antes de implementar» pede, mesmo quando o item e
seguranca e o escalao e o mais forte da casa.

**G2 fechou tudo o que o contrato pedia**, integrado em `6470943` (04:31 UTC,
28 min 10 s de frente, 128 ferramentas, sem `cargo` — o proprio motivo do
escalao medio e da largada em paralelo com B2). `trava-atras-da-rede`
reancorada no laco do `puxar`; o `COPIAR` do provador ganhou os dois arquivos
que testes leem por `CARGO_MANIFEST_DIR` (um deles, `mapa-das-threads.py`, era
buraco latente que ninguem tinha achado); `--so --json` passou a mesclar por
id; sete entradas novas no catalogo e `cluster.rs` saiu do zero,
`PISO_DAS_ENTRADAS` 180→187. Medio porque o trabalho e sobre **ferramenta e
catalogo em Python** — nenhuma linha de `crates/` mudou —, mas a decisao de
quais sete petreas entram e qual teste cada uma reprova exige leitura, e por
isso nao e leve.

## Onda 4/5 — os dois pedidos do dono, 164 e 190 (17/09/2026, madrugada)

| frente | escalao | por que | largada |
|---|---|---|---|
| 164-B (concorrencia e o gatilho `BEFORE`) | forte | e o mesmo eixo da onda 2/3: medir a repartição do tempo **dentro** da secao critica antes de encurtar, e decidir se ha o que tirar da trava sem mudar o que o gatilho enxerga — projeto e risco (concorrencia, semantica de transacao), nao varredura | 04:58 UTC |
| 190-E (exercitar os botoes que faltavam: os dois assistentes, DbLink, pivo, idiomas e backup) | forte | tela so se prova exercitando, e exercitar aqui bateu em defeito de **servidor** (excecao sem dono, campo de resposta trocado) alem do de tela — nao e so gravar clique, e diagnosticar por que a folha trava e o que a resposta de verdade carrega | — |

**164-B teve duas partidas, e a primeira nao produziu linha nenhuma**: o
primeiro lancamento no escalao forte ficou **sem credito** no modelo
escolhido antes de qualquer medicao rodar, e foi relancado no **mesmo
escalao** (forte), no primeiro modelo dessa faixa com credito disponivel —
sem baixar de nivel, porque o trabalho continuava sendo concorrencia e
formato de secao critica, nao varredura. A frente relancada e a que produziu
o medidor `reparticao-do-gatilho.rs`, a recusa medida de encurtar o gatilho e
o conserto do `empilhar` (commit `20d2c59`). **Registrar a queda de credito
aqui, e nao so na conversa, e a mesma lei do `docs/BACKUP.md` sobre o 403 do
GitHub: limitacao que bloqueia um papel se remede e se registra, para
ninguem gastar uma rodada inteira redescobrindo o mesmo bloqueio.**

**190-E** fechou em commit `6319396`: 194 botoes sem prova caem para 119,
com os quatro defeitos de tela do §13.9 do `docs/TESTES.md` achados
clicando, nenhum deles visivel so lendo o codigo. Forte pelo mesmo motivo do
190 original (pedido 190, ondas anteriores): a bateria aqui nao só grava
clique, ela precisou **diagnosticar** por que `backupAgora` travava («rodando…»
para sempre) e por que o CSV do pivo saia mudo — os dois exigiram ler a
funcao inteira, nao so o seletor do botao.

### Rodada da revisão SEC e dos gaps de 434/435 — 23/09/2026 (noite)

Nove frentes convocadas na mesma rodada: seis de código em `crates/`, uma de
pesquisa de semântica de motor (262), e duas que não compilam — a varredura do
QA, com julgamento de pergunta, e a redação do H.

| frente | escalão | por quê |
|---|---|---|
| 434 — teto antes da identidade | forte | protocolo de rede e memória pré-credencial |
| 435 — oráculo do erro do pulso | forte | protocolo de cluster e criptografia |
| 262 — gatilho AFTER no COMMIT (pesquisa) | forte | decide semântica de transação, não varre texto |
| 419 — corte calado da composição | forte | o diagnóstico do pedido estava errado e a premissa tinha de ser medida antes |
| 372 — senha do DbLink, a camada sem dono | forte | pétrea da senha |
| 436 M1–M3 — guardas do pulso | forte | segurança de cluster |
| 426 + 262 etapa 1 — o caminho do COMMIT | forte | concorrência e atomicidade da transação, com a trava global na mão |
| 446 + 447 — hexadecimal do fio e trava envenenada | forte | pânico alcançável pela rede e estado do cluster na eleição |
| 372 — cifra do dblink.json com chave externa | forte | formato em disco e criptografia |
| 450 etapa 1 — PhxZip, o 7-Zip em Rust | forte | formato de arquivo, AES e descompressor escritos aqui, para nove alvos |
| 382/383/384/423 — inventários de QA | médio | documentação que exige medir e um crivo em Python; não compila |
| C — parecer 451 e 448 | forte | garantias de dado sob pânico e atomicidade do COMMIT |
| J — parecer 444 | médio | pesquisa de padrões de fábrica com fonte primária, verificável |
| 439/442/453 — tetos e ecos | forte | memória pré-credencial e segurança |
| Interface do PhxZip | forte | produto novo, marca, idiomas e segurança de arquivo no navegador |
| SEC — revisão de 434 e 435 | forte | leitura adversária de criptografia |
| QA — inventário do mesmo motor | médio | varredura com julgamento de pergunta, verificável |
| H — cognições e MODELOS | médio | redação a partir de fatos dados; o leve arriscaria a nuance da terceira seção |
| SEC e C — revisões da 372 antes do commit | forte | criptografia e formato em disco; os dois acharam o mesmo apagamento calado por caminhos diferentes |
| 456 + 457 — o pânico que grava o `.ndx` rasgado como limpo | forte | durabilidade e garantia de índice sob pânico, com prova pela ABI do FFI |
| SEC — revisão da crate PhxZip | forte | leitura adversária de parser de arquivo hostil e de derivação de chave |
| 471 — PhxZip abrindo arquivo hostil | forte | negação de serviço por CPU e memória em entrada não confiável |
| H — página dos testes com as provas | leve | gerador conferível contra os arquivos; nada decide, só mostra |
| G — 477, a régua do `Debug` enxerga o `Segredo` | leve | crivo em Python que se prova rodando; o integrador repôs os dois defeitos e conferiu a régua velha em 0 |
| B — 476, um comando para todas as catracas | leve | script em Python que se prova rodando; o integrador achou na integração o chamador que lia a prosa em vez do código de saída |
| 450 etapa 2 — o `.phz` ligado ao config | forte | seguranca e migracao de arquivo em disco; duas voltas, porque DBA e SEC bloquearam a primeira |
| C — parecer da etapa 2 do 450 | forte | formato em disco e migracao; achou a dica que truncaria o config extraido |
| SEC — revisao da etapa 2 do 450 | forte | leitura adversaria de arquivo com segredo; provou pelo binario a copia em claro 644 e a pasta esvaziada |
| 478 — a instalacao nova termina em `.phz` | leve | roteiro, documentacao e uma prova em Python sobre um comando que ja existia; o integrador achou o empacotador pendurado com binario velho |
| 483 — o `phxsqld` recusa flag desconhecida | leve | parser pequeno com inventario medido dos chamadores e prova pelo binario |
| 484 — o quarto estado `⏸` nos geradores | medio | mexe no leitor unico que tres paginas e o documento de tecnologias usam, e a garantia principal e «sem `⏸` nada muda», provada byte a byte; o integrador achou a prova presa ao `HEAD` e a porcentagem calculada e nao mostrada |
| 421 — o portao de commit num codigo so | forte (o integrador) | costura de portoes e prova com defeito reposto; achou a evidencia por commit que reprovava na arvore exata |
| 482 — o log dos jobs pelo nome inteiro | medio | conserto delimitado com varredura roteirizada dos `with_extension(`; o motor do `temporario_de` generalizado em vez de duplicado |
| 485 — a espera do teste da replica que caia na primeira resposta | forte (o integrador) | teste que floca e defeito ativo: medido no HEAD antes de culpar o commit, causa lida no teste, 20 de 20 com o conserto |
| 481 — primeira volta | medio | frente de codigo delimitada; a SEC bloqueou (root tratado como terceiro) |
| 481 — correcao do bloqueio da SEC | forte | seguranca de arranque: regua refeita, prova pelo SO com o servico como usuario comum |
| SEC — revisao do 481, duas voltas | forte | leitura adversaria de arranque e permissao; provou o ALTO pelo binario e confirmou o conserto por mutacao |
| 438 — o `truncado` na tela e no ODBC | medio | tela + driver com contrato pronto; o integrador devolveu uma vez (servidor falso duplicado subia o `ISENTOS`, aviso enterrado na terceira linha) |
| C — segunda revisao do 368 | forte | formato em disco e queda no meio: mediu a colisao `x`/`x_001` e o ativo de 0 byte que tranca a tabela |
| SEC — modelo de ameaca do 495 | forte | leitura adversaria das portas de injecao; achou o erro cru no `acessos.log` (497) |
| J — hipoteses do 495, e C — catalogo de catastrofes do 496 | forte | desenho de seguranca e previsao de falha de dado: arquitetura e risco |
| 497 — erro cru no `acessos.log` | forte | redacao de dado pessoal: petrea do texto cru e busca dos irmaos |
| 501 — o `;` do comando empilhado | medio | conserto delimitado numa funcao pura; o integrador devolveu uma vez (a primeira versao abria evasao) e validou o vermelho dos dois lados antes de promover a cognicao |
| 368 — expurgo da trilha, tres voltas | forte | formato em disco e queda no meio; o DBA bloqueou duas vezes (durabilidade e formato B, depois nome `_NNN` e ativo curto) |
| C — segunda e terceira revisoes do 368, segunda do 451, segunda do 448 | forte | formato em disco, durabilidade e concorrencia: sao os pareceres que decidem se entra |
| C — catalogo de catastrofes do 496 | forte | prova contra o SO em montagem privada; achou cinco defeitos ativos |
| SEC — conferencia do 497 | forte | leitura adversaria da redacao de erro |
| 451 — o panico com a trava na mao, tres voltas | forte | concorrencia e durabilidade da marca do COMMIT; o integrador devolveu uma vez (quinto cliente de teste subia o `ISENTOS`) e o M4 do parecer foi trocado por um conserto que a frente provou necessario |
| 448 — FK e unicidade conferidas antes da marca, duas voltas | forte | integridade referencial e durabilidade do COMMIT; o DBA bloqueou a primeira (lista meio gravada, COMMIT quadratico, regressao do DEFAULT) |
| 509+512 — fsync falho e `.ndx` sujo | forte | durabilidade provada contra o SO |
| 506+507 — nome com letra e com ponto | medio | recusa na declaracao por uma funcao que ja existe |
| SEC 497, terceira volta — portao da senha pelo lexico | forte | seguranca adversaria: evasao do portao e irmaos que gravam texto cru |
| 514 — FK conferida na linha FINAL (depois do DEFAULT e da calculada) | forte | regra primordial da integridade, e o irmao em cada caminho que confere FK |
| DBA 509+512 e DBA 514 — revisoes | forte | durabilidade contra o SO e integridade referencial: e o papel que diz nao |
| 520+521 — relogio do login e PBKDF2 de senha longa | forte | criptografia conferida contra vetor, e oraculo de tempo |
| lote 502+452+466+504+510 — o servidor fica de pe | forte | panico sob trava, threads de fundo e arranque: concorrencia e disponibilidade |
| 522 — o `fechar` baixa o byte 52 sem fsync | forte | formato em disco e durabilidade contra o SO |
| juiz PhxJev — 30 pedidos julgados (preset revisar) | forte | o veredito sai do limiar do script, mas a probabilidade por pergunta e juizo sobre defeito ativo |
| lote 491+492+515+516+490 — integridade na transacao | forte | regra primordial da integridade e concorrencia da transacao |
| 249 — a sonda do disco a cada 5 minutos (ordem do dono) | leve | troca de um padrao e dos seus irmaos em texto, mecanica e verificavel por teste |
| faceis A — 276, 369, 443, 462, 463, 529 | medio | itens locais de um arquivo, com teto ou texto; ordem do dono: os faceis primeiro |
| faceis B — 345, 473, 524, 518 | medio | permissao de arquivo, mensagem, fsync do backup e NULL no diff: locais e provaveis por teste |
| SEC — revisao do lote faceis A (369, 443, 529) | forte | seguranca: redacao por analise, parser de fio em claro e oraculo de tempo do login |
| J — auto-laco medido nos quatro motores | medio | medicao roteirizada de um comportamento, com fonte primaria; o empate subiu ao dono |
| DBA — re-checagem das condicoes do lote de integridade | forte | concorrencia (desempate do ciclo de COMMIT barrado) e garantia de dado |
| faceis C — 464, 365, 458, 499 | forte | redacao de dado pessoal por analise, trava envenenada e portao de escrita da replica |
| faceis D — 523, 544, 530, 463 (resto) | forte | durabilidade por caminho canonico, parser do fio, threads sem teto e prazo da conversa |
| DBA — revisao do lote faceis B | forte | durabilidade do backup e disponibilidade: o fsync no destino que derrubava o servidor |
| integridade 2 — 539, 538, 537, 540 | forte | concorrencia (trava no empilhar sem reabrir ciclo), gatilho na transacao e marca da cascata solta |
| H — documentacao da rodada (CHANGELOG, tecnologias, cognicao) | medio | leitura de commit e extrator; verificavel, sem projeto |
| J — 533, permissao dos arquivos e normalizacao do 365 | forte | formato em disco e ordem de escrita contra queda: projeto e risco |
| tradutor — revisao multilingue da rodada | medio | chave e traducao nos seis idiomas: mecanico, mas sem compilar tem de acertar a sintaxe a mao |
| QA — catracas e guardas da rodada | medio | leitura de diff e rodar as reguas em Python |
| F — auditoria estatica das guardas novas | medio | ler troca e teste por guarda; o provador roda no fecho |
| SEC — revisao da rodada (ABI nova, recusas novas) | forte | seguranca: ponteiro na ABI, oraculo em mensagem de recusa |
| 533 + 542 — subida do byte 52 duravel e arquivos 0600 | forte | durabilidade contra queda (formato em disco) e permissao do dado em repouso |
| juiz PhxJev — propostas do SEC e do DBA sobre faceis C | forte | o veredito sai do limiar do script, mas a probabilidade por pergunta e juizo sobre defeito ativo |

**Convocacao do dono em 24/09/2026, 18:40** («toda a equipe em alerta e revisao
nessa rodada»). Convocados: A (integrador), B (quatro frentes), C (revisoes em
curso), F, G, H, J, SEC e o tradutor. **D, o zelador**, rodou e se ADIOU pela
propria regra: quatro frentes compilando, e ele nao apaga o que processo vivo
usa; o vigia segue a cada 30 min. **E, o designer**, fica convocado para o FECHO:
a tela mudou hoje (438, 481, 372 e o PhxZip), e interface so se prova
exercitando — o que pede um binario novo, que hoje competiria por disco com as
quatro frentes. **I** e o integrador, com o pacote provado das 18:16.

**A integração achou o que nenhuma frente via, de novo por território de
arquivo não bastar em worktree compartilhado**: `cargo fmt --all` de uma
frente reformatou o arquivo de outra; a suíte da frente 435 fechou 2.820
verdes e 0 vermelhos mas com `rc=1` porque outra frente trocou o `.rlib` do
servidor no mesmo `target` no meio da corrida (registrado no próprio commit
`a272d8f`, não arredondado para verde); e a árvore inteira parou de compilar
quando a frente 372 mudou `cfg.senha()` para devolver `Result` e o chamador
em `email.rs` ainda não tinha acompanhado — as três, cognição
`cognicao_territorio-de-arquivo-nao-isola-frente_20260923_2349.md`.

O achado central de 435 — o conserto do relógio (pino cego) abrindo um
caminho onde a prova forjada fecha para qualquer par do cluster — só apareceu
lendo a derivação da chave, não o relatório da frente; cognição
`cognicao_conserto-que-abre-porta_20260923_2338.md`. E o pedido 437 nasceu de
uma leitura cedo demais de um arquivo de saída (0 em 15 publicado, 0 em 45
medido); cognição `cognicao_arquivo-de-saida-lido-cedo_20260923_2310.md`.

**Papéis dispensados nesta rodada, com o motivo de quem decidiu** — e a primeira
versão deste parágrafo, escrita por uma frente de redação, errou três dos quatro,
e errou no sentido perigoso: dispensa registrada com o motivo errado ensina que um
papel foi pensado quando não foi.

- **C-DBA** — sem pergunta nova de formato ou garantia: 255 e 372 já têm parecer
  (`docs/propostas/parecer-dba-372-e-255.md`) e o 426 tem a pesquisa dos quatro
  motores; nenhuma frente mudou formato em disco.
- **D-zelador** — **não** porque o disco esteve folgado: ele chegou a **2 GiB**. O
  zelador rodou às 23:11 e liberou 1.172 MiB, e não toca o `target` de 11.955 MiB
  porque há processo vivo com `cwd` ali. Rodá-lo de novo não liberaria nada.
- **J-pesquisador** — convocado para o 262 e dispensado depois: as medições
  pendentes (custo do 255, a mensagem do pânico do 437) pedem máquina parada, e com
  três frentes compilando qualquer número seria ruído.
- **Tradutor** — colide com a frente 372 na fábrica de idiomas e na catraca de
  textos. É o caso dos «noventa minutos»: duas frentes na mesma catraca sem se verem.

**Não dispensados**, ao contrário do que a primeira versão dizia: o **E-designer**
está **dentro** da frente 372, que acrescenta `token_remoto_env` à tela do DbLink e
tem de exercitá-la no navegador; e o **I-versionador** é o integrador — *só o
integrador comita*, e foi ele quem comitou o 262 e o 435 no instante em que
devolveram.
