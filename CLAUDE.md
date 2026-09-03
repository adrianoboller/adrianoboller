# PhxSql — convenções do projeto

Motor de dados em Rust no modelo de arquivos separados do HFSQL. O código vive
em `phxsql/`. Especificação do formato em `phxsql/docs/FORMATO.md`, roteiro em
`phxsql/docs/PLANO.md`.

## Ao terminar cada rodada de trabalho: atualize o dossiê

O dossiê é a página que o Adriano usa para enxergar o projeto inteiro:

- **URL:** https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033
- **Fonte:** `phxsql/docs/dossie/dossie-phxsql-0.18.html` (versionado, para que
  qualquer sessão consiga atualizá-lo)

Publique sempre **passando essa URL**, para cair na mesma página em vez de
criar outra. Instruções e as armadilhas de estilo em
`phxsql/docs/dossie/LEIA-ME.md`.

O nome muda a cada refação — era `dossie-phxsql.html`, virou `-0.15` e agora é
`-0.18` — e **só existe um por vez**: o anterior sai do repositório no mesmo
commit, para que ninguém atualize o errado. Todos os scripts aceitam o caminho
do HTML como argumento, então trocar o nome de novo não exige editá-los.

Os números do painel são **medidos, nunca estimados** — já saíram errados três
vezes: arredondamento para cima, depois 276 testes quando eram 280, depois um
rodapé inteiro parado numa versão anterior. **Nenhum número visível se digita
mais**: são **cinco** geradores, listados no `LEIA-ME.md` da pasta, e eles
escrevem o título, o selo, o painel da capa, o rodapé, os idiomas, a bancada, o
painel da replicação, os pedidos, a cobertura por área e as capturas.

E o corolário que a revisão da 0.18 pagou: **a receita de um número também
envelhece.** A do KiB de interface era uma lista de três arquivos copiada no
script; o `http.rs` passou a embutir nove, e o rodapé publicou **780 KiB**
quando a interface tinha **1.032**. Hoje a lista sai do próprio `http.rs`.
Quando um gerador depende de uma lista, a lista tem de sair do código.

O que falta no projeto está em `phxsql/docs/PENDENCIAS.md` — atualize junto com
o dossiê.

Dessa lista sai uma **segunda página**, a relação dos pedidos com o estado de
cada um:

- **URL:** https://claude.ai/code/artifact/d6c8f13c-e4a2-444e-9f19-0e047e230352
- **Fonte:** `phxsql/docs/dossie/pedidos.html`, que **não se edita** —
  `python3 phxsql/docs/dossie/pagina-dos-pedidos.py` a gera do `PENDENCIAS.md`
  e conta os três estados sozinho.

## Todo aprendizado novo vira um arquivo de cognição

Ordem do dono, 02/09/2026: *«Todo aprendizado novo seu deve virar um
`cognicao_assunto_data_hora.md`.»*

Eles moram em `phxsql/docs/cognicao/`, um por aprendizado, com o formato e as
cinco seções em `phxsql/docs/cognicao/LEIA-ME.md`. A terceira seção é a que
impede o documento de virar anedota: **o que eu concluí primeiro, e estava
errado**. Sem ela o arquivo ensina só a resposta, e o erro volta pelo mesmo
caminho.

A hora do nome é a da **descoberta**, não a do commit — é ela que diz o que já
se sabia quando outra frente errou o mesmo na mesma tarde. E numa rodada com
frentes paralelas isso deixa de ser detalhe: nesta base, um número digitado
envelheceu em **noventa minutos** porque duas frentes mexeram na mesma catraca
sem se verem.

**O que não vira cognição nova:** reafirmação de pétrea que já existe. Quando
uma pétrea quebra de novo, o aprendizado é o **alcance** dela — «a guarda
existe, mas só cobre `ui/`» —, e é isso que o arquivo registra. Terceira cópia
da mesma lei não acrescenta lei: acrescenta lugar onde a lei pode divergir de
si mesma.

E a divisão com este arquivo: o `CLAUDE.md` é a **lei**, curta e sem discussão;
a cognição é o **processo** — como se descobriu, o que se errou antes, e o
número. Lei sem processo vira dogma que ninguém sabe defender; processo sem lei
vira história que ninguém aplica.

## A marca é oficial

Os arquivos estão em `phxsql/marca/`, com a especificação em
`phxsql/marca/LEIA-ME.md`. Tipografia **Exo 2**, fundo `#010418`, assinatura
*Built to store. Engineered to scale.*

A marca **manda** sobre qualquer paleta inventada. Duas adaptações já
decididas e documentadas: o corpo de texto longo não usa Exo 2, e o vermelhão
escurece para `#C63C0A` no tema claro, por contraste.

Atenção: a folha de marca afirma *ACID compliant* e *built-in replication*.
O segundo **virou verdade** — a replicação funciona, está medida com quatro
servidores, e o cluster faz eleição e promoção automática. O primeiro
**continua falso**, e continuará enquanto não houver transação: sem ela não há
o A nem o I do ACID. Não repita *ACID compliant* em documento técnico.

## Regras que não se quebram

**Zero dependências externas.** Só a `std`. Foi o que fez a compilação cruzada
para Windows funcionar de primeira e o que permite `cargo build --offline`.
JSON, CRC-32, SHA-256, HMAC e PBKDF2 são escritos aqui. Se algo parecer exigir
uma crate, primeiro pergunte — não acrescente.

**Bancada compara trabalho igual, não só pergunta igual.** Os dois erros já
cometidos aqui saíram do mesmo lugar e apontaram para lados opostos: primeiro
um `WHERE id IN (…)` contra vinte mil buscas separadas (41× a favor do outro
motor), depois um `COUNT(*)+SUM` sobre 1.250.000 linhas contra a leitura de
20.000 (5× a favor do nosso). Nenhum dos dois aparecia no número. As quatro
regras estão em `phxsql/bancada/LEIA-ME.md`.

**Criptografia se confere contra vetor oficial.** Nada de "parece certo": os
testes trazem FIPS 180-4, RFC 4231 e os vetores de PBKDF2.

**Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do
protocolo. Há teste que falha se a ficha de usuário vazar o hash.

E o corolário, que o Profiler obrigou a escrever: **funcionalidade que mostra
texto cru redige ANALISANDO, nunca recortando.** Recortar depende de o pedido
estar escrito de um jeito; analisar e reserializar não. O que não se analisa
não vira texto — vira o tamanho em bytes. Se a estrutura não se lê, não há como
tapar o campo dentro dela.

**Regra primordial da integridade: nunca se mata o pai que tem filhos.**
Palavra do dono: *«1 para muitos. Cascade/Restrict sempre. Nunca pode matar o
registro pai se tem filhos em outra tabela(s). A opção Cascade/Cascade não
existe em PhxSql.»*

Em código: `ao_excluir` aceita **só** `restringir`, e `ao_alterar` nasce
`cascata`. Cascata, anular e nada **não existem** no lado do excluir — e o par
Cascata/Cascata some por **consequência**, não por uma segunda regra: sem
cascata no excluir, não há par com cascata dos dois lados.

A recusa acontece na **declaração**, não na gravação, e isso é decisão: uma
tabela nasce uma vez e grava um milhão de vezes. Recusar cedo custa um erro
lido enquanto se cria a tabela; recusar tarde custa um banco inteiro modelado
errado, descoberto no dia do primeiro `excluir`. Está em
`phxsql/crates/phxsql-server/src/valores.rs`, com o par de testes que trava os
dois sentidos — `ao_excluir_so_aceita_restringir` e o irmão que impede um
portão que recusaria tudo.

**E ela é imposta na gravação**, não só na declaração: `excluir` — de vez e
suave — recusa a linha que tem filha. O suave também, porque pai logicamente
morto deixa filha apontando para linha que a tela não mostra mais, e órfã que
ninguém vê é pior que órfã que dá erro.

A busca reversa custa o que custa por escolha: a chave é declarada na **filha**,
então a mãe pergunta às irmãs — uma varredura dos esquemas do diretório, por
exclusão. **Excluir é raro, inserir é o laço quente**, e pagar ali mantém o
`inserir` sem custo nenhum. Um catálogo reverso guardado faria o inverso:
barateia a exclusão e cobra manutenção de toda criação e alteração de tabela,
inclusive das que não têm chave nenhuma.

E uma consequência que vale saber antes de modelar: **a chave conferida precisa
de índice dos dois lados** — na mãe para responder «existe este pai?» ao gravar
a filha, e na filha para responder «alguém aponta para esta linha?» ao apagar a
mãe. Sem um deles o motor **recusa** dizendo qual falta, em vez de esconder uma
varredura dentro de um `excluir` que parece barato.

**Chave declarada NASCE conferida** — decisão do dono, tomada quando esta casa
percebeu que as duas pétreas se contradiziam. A regra primordial diz «nunca»,
sem condição, e uma chave que precisa ser *lembrada* de conferir não honra um
«nunca»: o esquecimento vira o padrão.

O interruptor `verificar` continua existindo, e agora só para o lado contrário
— quem **quer** declarar sem conferir manda `"verificar": false`, e aí é
escolha escrita em vez de omissão. O par de testes trava os dois sentidos:
`a_chave_declarada_nasce_conferida` e `quem_pede_para_nao_conferir_continua_podendo`.

E isto **não** quebra banco que já existe, por causa do formato: o `PSCH` v7
grava o byte por chave, então o esquema em disco volta com o que foi gravado
nele. Chave declarada antes desta decisão continua sem conferir até alguém
ligar. Muda o que nasce daqui em diante — que é exatamente o alcance que
«guarda nova entra pedida, não imposta» protege: ela protege o **dado que já
está lá**, não o esquecimento de quem modela amanhã.

Consequência medida ao ligar: declarar chave para uma tabela que ainda não
existe é ordem legítima de modelagem, e a gravação passa a recusar. A recusa
**diz isso** — nomeia a tabela que falta — em vez de vazar o erro cru «nenhum
volume de clientes.reg em /tmp/…», que mandava procurar arquivo em vez de
criar tabela.

**A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot excluído.
Qualquer proposta que quebre isso precisa ser discutida antes.

**Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar o
formato é barato; depois vira migração. Foi assim com o volume no ponteiro.

**Receita de fora se mede contra o nosso gargalo antes de virar plano.** Chegou
uma arquitetura completa para acelerar escrita — WAL sequencial, group commit,
MemTable, LSM. É uma boa receita **para o gargalo que ela descreve**, que é o
`fsync` do InnoDB. Medi o nosso antes de aceitar: **83,5% do tempo de uma
inserção está no `.ndx`**, e o arquivo de dados já é *append-only* e custa
16,5%. Das dez propostas, cinco já existiam aqui, duas miravam um problema que
não temos, uma quebraria a ordem de digitação, e duas eram reais. Está em
`phxsql/docs/DESEMPENHO.md`, com o medidor (`--example onde-doi`).

**Interface só se prova exercitando.** Gravar um vídeo de demonstração achou
**três defeitos em cinco minutos** que ler o código não acharia — e o pior deles
quebrava *todo salvar e todo incluir* pela tela desde que o `rownum` entrou. O
padrão dos três é o mesmo: **coluna de sistema nova quebra quem filtra pela
primeira**. Quando entrar uma peça no fim de uma lista, procure quem usa
`find(...)` onde devia usar `filter(...)`.

**Guarda nova entra pedida, não imposta.** A janela de conflito de escrita
podia recusar toda gravação sem versão — e aí todo cliente escrito antes dela
pararia de gravar de um dia para o outro, recebendo um erro que não sabe tratar.
Quem manda `"versao"` ganha a garantia; quem não manda continua como antes; a
interface web manda sempre, porque é onde existe gente e existe a janela de
minutos entre abrir a ficha e clicar em salvar. **Proteção que quebra todo
cliente antigo não é proteção, é estrago** — e o teste que trava isso é o do
comportamento *velho*, não o do novo.

**Merge de conflito marca quem MEXEU, não quem perguntou por último.** Deixar
«o meu» marcado em todas as colunas desfaria em silêncio o trabalho do outro
nas colunas que eu nem toquei — o mesmo estrago de antes, com mais cliques. O
padrão certo é por coluna: a que eu digitei fica comigo, a que só o outro mudou
fica com ele. Dois que editaram campos diferentes saem com os dois trabalhos e
sem escolher nada.

**O CSS global morde todo componente novo da tela.** `input{width:100%}` e
`label{text-transform:uppercase}` são certos para um formulário e errados
dentro de uma tabela: o rádio virou uma bolinha do tamanho da célula, e
«Blumenau» apareceu como «BLUMENAU» — que é uma **mentira sobre o dado**, porque
quem olha não sabe se está gravado assim. Nenhum dos dois aparece lendo o
código. Componente novo se abre no navegador e se olha, e é a mesma lição do
vídeo por outro caminho.

**A lista do que falta também é palpite até alguém medir.** O pedido 113 dizia
«ordene as chaves do lote antes do `.ndx`» e vinha com o alvo certo — os 83,5%
estavam mesmo lá. Só que o custo não era de **localidade**: era de reler do
arquivo e recalcular o CRC-32 da **mesma página** a cada descida da árvore. A
desordem custava 1,06×; ordenar teria comprado quase nada, e teria custado uma
garantia. Um cache de páginas de leitura comprou **2,40×**. *Medir a premissa do
item vem antes de implementar o item* — inclusive quando o item é nosso.

E o corolário: o mesmo medidor dizia «~20 toques de página por linha», citando
um `strace` de outro dia. Eram 10,86, e é por isso que a conta do CRC nunca
fechava naquele documento. **Número citado é número que não se mede** — hoje o
medidor conta os toques por dentro.

**Conserto entra no caminho que o motivou, e o caminho IRMÃO fica.** Pago
**três vezes em 03/09/2026**, e as três com prova real: `conferir_a_arvore`
entrou no `atualizar` e não no `recascatear` (pedido 173, meia cascata na
recuperação); o `reindexar` do arranque alcança as tabelas **nomeadas na
marca** e não a filha da cascata, que nunca vira `Escrita` (pedido 172); e o
recado que não deve mandar reparar índice são entrou no `conferir_fks` e não no
`planejar_ao_alterar` (pedido 176, os dois lados da mesma chave).

Irmão é **quem chama as mesmas funções na mesma ordem**, não quem tem nome
parecido. E o agravante das três: em duas delas o **comentário acima da linha
já dizia que o erro cru era ruim**, com o `({e})` logo abaixo mandando o texto
cru junto. **Envolver não é substituir**, e comentário que se declara resolvido
é o motivo de ninguém olhar de novo.

E o conferidor genérico para isso está **recusado com número**: são **8**
interpolações de erro cru em mensagem no repositório inteiro, e só **2** eram
defeito — as duas consertadas. Um casador reprovaria as outras seis, que são
legítimas porque o erro interno ali **informa** em vez de dar uma ordem. O que
distingue o defeito é o erro interno carregar um **imperativo** que a
explicação de fora desmente, e isso não se acha por padrão de texto: acha-se
procurando o irmão.

**Portão de permissão é UM só — e o campo que ele lê é o furo.** O direito por
tabela entrou no despachar, que confere o campo `"tabela"` do pedido. **Três**
operações escondem tabela dele — e a terceira só apareceu em 03/09/2026, na
varredura das 116 operações uma a uma: `juntar` guarda as tabelas em `a.tabela`
e `b.tabela`; `unir` guarda numa **lista**; e `pivotar` põe a tabela de FATOS no
campo que o portão lê, mas as de **consulta** dentro de um item de `juntar`
aninhado. Sem conferência própria, bastaria pedir a tabela negada como o lado B
de uma junção.

**As três pagam conferência própria, e é por isso que a falta nunca apareceu
como defeito: a lei estava incompleta e o código estava certo.** Lei que lista
menos casos do que existem não protege menos hoje — protege menos no dia em que
alguém usar a lista como inventário. São **7 das 116** operações nomeando tabela
onde o portão não olha.

E um risco que só existe daqui em diante, nomeado pela frente que mapeou o
arquivo: numa divisão do `servidor.rs`, essa conferência própria **parece
duplicação** do portão geral. Limpá-la reabre a porta dos fundos, e **nenhum
teste do portão acusa** — os três que acusam viajam com as três operações. **Quando o portão passar a
olhar um campo novo, procure quem não tem esse campo** — e não espalhe o portão
por quarenta operações, porque a que alguém esquecer vira a porta dos fundos e
ninguém acha por leitura.

E o teste que mais importa numa regra de permissão nova é o do comportamento
**velho**: `sem_regra_de_tabela_nada_muda`. Regra que muda o significado da
configuração que já existe tira o direito de alguém sem ninguém ter pedido.

**Instrumentação desligada tem de custar zero — e o portão que decide isso vem
ANTES do trabalho.** O Profiler desligado cobrava 7% da carga pela rede: o ponto
de captura fazia dois `Json::analisar` do corpo inteiro, três `String` e um
mutex, e só então perguntava se estava ligado. Num lote de cinco mil linhas era
analisar meio megabyte de JSON duas vezes para jogar fora. Quando entrar um
observador novo, procure o que ele faz antes de olhar o próprio interruptor.

E o corolário sobre a **explicação** disso, que eu errei primeiro: escrevi que
«o mutex era o pior pedaço, porque serializa». Medido, o `lock` sem disputa
custa **13,2 ns** e o parse do lote custa **3.456 µs** — 262.000× mais. O mutex
nunca foi o gargalo, e neste servidor nem poderia ser: a trava global de dados
já serializa tudo, e é tomada depois e segurada por mais tempo. **Diagnóstico
plausível não é diagnóstico medido** — e o errado sobrevive melhor quando o
conserto funcionou por outro motivo.

**Teste unitário não prova queda de conexão — soquete prova.** Os dez testes do
`BULKINSERT` passavam, e a prova pelo soquete mostrou que a queda da conexão
**não soltava a reserva**. A causa não estava no servidor: era o teste, porque
`socket.makefile()` do Python segura o descritor e fechar só o soquete deixa o
fd aberto — o servidor nunca via o fim da conexão. Duas lições numa: o que
depende do sistema operacional se prova contra o sistema operacional, e um teste
que passa por engano é pior que um teste que falta.

**Configuração que não é lida mente.** `recursos.cache_paginas` estava no
`config.json`, no MANUAL e na tela desde a 0.13.0, e **nenhuma linha de código
o lia** — o campo dizia "4096 páginas do `.ndx` em memória" quando não havia
cache nenhum. Campo de configuração sem leitor é pior que campo ausente: o
ausente ninguém ajusta esperando efeito.

**Medidor com binário velho mede o passado.** `cargo build --release` não
recompila os *examples*, e a bancada chama `target/release/examples/carga`
direto: uma rodada inteira de ganhos (16,4 → 7,5 µs) ficou invisível na bancada
porque o binário dela era de antes — e a conclusão «o esquema custa 2,2×»
nasceu, com tabela e tudo, dessa diferença. Antes de medir:
`cargo build --release --examples -p phxsql-store`.

**Número digitado à mão envelhece calado.** O selo da capa do dossiê passou
**quatro lançamentos** dizendo 0.11.0 — e o script que existe justamente para
impedir isso não cobria aquele pedaço. Todo número visível ou sai de um gerador,
ou está errado e ninguém percebeu ainda.

**Texto de tela entra pela fábrica de idiomas — isso é pétreo.** Palavra do
dono: *«o agente multi linguagem deve fazer uma revisão constante para manter a
possibilidade de mudar entre português, inglês… pelo login e pela tela de
configuração. A cada nova implementação esse agente tradutor deve atualizar
strings fixas por variáveis de multi linguagem.»* A máquina existe desde a
0.17.0 — `phxsys.mensagens`, a `FABRICA_TELA` do `idiomas.rs`, as bandeiras do
login. O que faltava era o laço que **conta**: medido antes desta rodada,
11.987 linhas de interface e **16** `data-txt`. Máquina que funciona e que
ninguém usa é promessa, não garantia.

O conferidor é `crates/phxsql-server/src/conferidor.rs`, e ele roda junto dos
testes: `cargo run --example textos-fora-da-fabrica -p phxsql-server` lista
arquivo e linha de cada texto que ainda está cravado, e a **catraca** (`TETO`)
reprova quem acrescentar mais um. O número só desce — traduziu um punhado,
baixe a catraca no mesmo commit, porque catraca frouxa não segura nada. O
procedimento de acrescentar um texto está em `docs/MENSAGENS.md`.

Três armadilhas que esta frente já pagou:

- **Rótulo se traduz; dado, nunca.** É a mesma lição do «Blumenau» virando
  «BLUMENAU». No conferidor ela virou crivo: tudo o que a página **interpola**
  (`${…}`) some antes da varredura, e só sobra o que alguém digitou no fonte.
- **Texto se resolve por CHAVE, nunca por comparação da frase.** No dia em que
  alguém melhorar a redação, quem compara frase para decidir a tradução quebra
  calado — e quebra em silêncio, mostrando português.
- **Chave morta é pior que chave faltando.** O tradutor a vê na tabela, traduz
  nos seis idiomas, e nada muda na tela. Há teste para os dois lados do laço:
  chave que a tela pede e não existe, e texto que existe e ninguém pede.

E as **três mensagens que não se traduzem de propósito** continuam assim, com o
motivo escrito no `mensagens.rs`: `erro.redireciona` (o cliente recorta o
prefixo — é protocolo vestido de texto), `erro.sinal` (a `MESSAGE_TEXT` é do
dono do banco) e `erro.cancelado` (o texto já vem montado). Achou uma quarta do
mesmo naipe? Documente a decisão em vez de traduzir.

**Toda bateria de testes tem prova real e aprendizado documentado — frutífero
ou infrutífero.** Prova real é nos dois sentidos: o teste novo tem de **falhar
com o defeito reposto** e passar com o conserto (já houve teste que passava por
engano, e ele é pior que teste que falta). O aprendizado vai para o documento
da área (`DESEMPENHO.md`, `SEGURANCA.md`…), não só para a conversa — inclusive
quando a hipótese **morre**: a recusa com o número é resultado tão válido
quanto o ganho, e é o que impede a mesma ideia de voltar sem medição. E
hipótese infrutífera não encerra a bateria: **gera a próxima hipótese**, como
na caça aos 2,3× do insert, em que cinco suspeitos caíram medidos antes de o
binário velho aparecer.

## Cláusula pétrea: os dez papéis, e o modelo de cada um

Vale para **todo projeto**, e a versão curta está no `~/.claude/CLAUDE.md`.

**A obrigação não é abrir dez agentes por tarefa.** Corrigir um typo não precisa
de DBA, designer, QA e pesquisador — e regra que ninguém consegue cumprir é
regra que todo mundo ignora, que é como se perde a força de uma cláusula.

A obrigação é outra, e mais dura de burlar: **nenhum papel fica sem dono quando
o trabalho toca o domínio dele, e o orquestrador registra quais papéis
convocou e quais dispensou.** Dispensa registrada é decisão; dispensa
silenciosa é esquecimento — e a diferença entre as duas é a única coisa que
esta cláusula realmente cobra.

### A — Orquestrador / Supervisor

Divide o trabalho, **escolhe o modelo de IA de cada agente e subagente**, e
integra o que volta.

A escolha do modelo é dele porque o custo e a qualidade não são iguais em toda
tarefa: desenhar um gestor de transações e varrer 190 rótulos para seis idiomas
não pedem a mesma coisa. Trabalho de **projeto e risco** — arquitetura,
criptografia, formato em disco, concorrência — vai no modelo mais forte
disponível. Trabalho **mecânico e verificável** — tradução, documentação,
varredura, medição roteirizada — vai no mais leve que ainda faça direito. O
orquestrador **diz qual escolheu e por quê**; modelo escolhido em silêncio vira
custo que ninguém explica ou qualidade que ninguém entende.

**No repositório vai o NÍVEL e o motivo, nunca o nome** — decisão do dono, e o
motivo é um limite de plataforma e não uma preferência: esta sessão é proibida
de pôr identificador de modelo em artefato versionado (commit, comentário,
documento). Então o `docs/MODELOS.md` guarda «modelo forte porque é formato em
disco» e «modelo leve porque é varredura roteirizada», e o nome fica só na
conversa. O que a cláusula cobra — explicar custo e qualidade — sobrevive
inteiro; o que se perde é a etiqueta, que era a parte que menos ensina.

E a integração é papel dele por um motivo medido: numa rodada de seis frentes,
**três defeitos só apareceram no encontro delas** — um teto de memória que uma
frente pôs e a outra apagaria sem conflito nenhum aparecer, uma bateria que
ficou com dezoito partes quando cada frente contou dezessete, e o dossiê que
perdeu uma seção inteira porque o merge escolheu o lado de quem não a tinha.
**Nenhuma frente sozinha podia ver nada disso.**

### B — Engenheiro de desenvolvimento

Escreve o código, e responde pelos portões: `fmt`, `clippy` com zero avisos,
suíte inteira verde. Não entrega meia funcionalidade se a metade for pior que
nada — a frente das transações devolveu o terreno pronto e **recusou** entregar
meia transação, e foi a decisão certa.

### C — DBA sênior

Manda no formato em disco e nas garantias de dado. É dele a palavra sobre
**ordem de digitação, chave, índice, integridade referencial e migração** — e é
ele quem diz não quando uma proposta boa quebra uma garantia. Aqui o `.reg`
nunca reaproveita slot excluído, e foi esse papel que recusou o «slot que
nasceu e morreu» com quatro motivos, sendo o decisivo o da replicação.

**Mudança de formato entra cedo**: enquanto não há dado em produção é barata;
depois vira migração.

### D — Zelador do ambiente

Mantém espaço de trabalho. Existe como `phxsql/zelador.sh` — e **não roda de
hora em hora**, ao contrário do que esta linha dizia: não há `cron` neste
contêiner, e cada corrida foi alguém chamando o script. Papel que não está
cumprindo aparece como não cumprindo.

A regra que decide se ele ajuda ou destrói: **nada é apagado sem antes se
provar que nenhum processo vivo está usando aquilo**. Zelador que apaga o
`target` de quem está compilando não economiza espaço, perde uma rodada de
trabalho. E ele **não mata processo** — matar o servidor de um agente vizinho
já derrubou a própria sessão aqui.

A primeira corrida achou **80.088 diretórios de teste soltos, 6,4 GB**. O
ambiente tinha chegado a 560 MB livres, e ninguém sabia por quê.

### E — Designer gráfico

Responde pela tela: paleta, tipografia, contraste, responsividade, e a marca
mandando sobre qualquer paleta inventada.

**Interface só se prova exercitando.** O CSS global morde todo componente novo:
`input{width:100%}` virou uma bolinha do tamanho da célula, e
`label{text-transform:uppercase}` fez «Blumenau» aparecer como «BLUMENAU» — que
é uma **mentira sobre o dado**, porque quem olha não sabe se está gravado
assim. Nenhum dos dois aparece lendo o código.

### F — Usuários de teste e revisor de prova real

O papel mais fácil de fingir que se cumpriu. **Prova real é nos dois sentidos:
o teste tem de FALHAR com o defeito reposto e passar com o conserto.**

Teste que passa por engano é pior que teste que falta, e isso não é teoria: uma
prova escrita aqui passou com o defeito reposto porque conferia o veredito, e a
conferência acontecia **depois** do dano — o número certo saiu quando ela passou
a medir *quanto* foi lido, não *se* recusou.

**O que depende do sistema operacional se prova contra o sistema operacional**,
não por teste unitário.

### G — QA

Dona das catracas e do catálogo de guardas: cada guarda com o defeito que a
motivou, provada periodicamente contra ele. Catraca **só desce**; quem traduz e
esquece de baixar deixa a catraca frouxa, e catraca frouxa não segura nada.

**E ela NUNCA sobe — nem quando a régua muda.** A regra nasceu de um caso real:
o conferidor de grades contava só `<table>` cru, aprendeu a ver também a
chamada ao ajudante `tabela(`, o número pulou de 24 para 43, e eu **subi** o
teto com o motivo escrito ao lado. Decisão do dono: isso não se faz. Régua que
passa a medir mais **aposenta** a catraca antiga e faz nascer uma nova, no
número medido do dia, dizendo no próprio nome e no comentário que substitui a
outra — é o que `TETO_TABELA_NA_MAO` registra.

A série com o passado se perde de propósito, e o preço é o certo: perder a
comparação é mais barato que deixar «mudei a régua» virar a porta pela qual se
afrouxa uma catraca. Quem sobe um teto está pedindo confiança no motivo; quem
aposenta e recomeça está pedindo confiança em nada.

### H — Documentação

**Todo número visível sai de um gerador, ou está errado e ninguém percebeu
ainda.** O selo da capa passou quatro lançamentos dizendo uma versão que não
era.

E o corolário que custou caro: **a receita de um número também envelhece.**
Quando um gerador depende de uma lista, a lista tem de sair do código — uma
lista digitada fez o rodapé publicar 780 KiB quando eram 1.032, e fez o
conferidor de idiomas medir cinco sextos da tela dizendo o número inteiro.

### I — Versionador e backup

Commit que conta a decisão e o motivo, não a lista de arquivos. Branch
combinada. Pacote de fontes e binários gerado por script, **nunca montado à
mão** — pacote feito à mão é pacote que ninguém consegue refazer igual.

**Este papel esteve degradado, e deixou de estar em 03/09/2026**: o `push`
recusava com 403 e o backup saía por pacote git entregue à mão; hoje o `push`
funciona — 391 commits no `origin`, conferidos por `git ls-remote` e não pela
palavra do `git`. O pacote continua, como segunda via.

E a lição que a correção deixou vale mais que ela: **limitação registrada também
envelhece.** O 403 estava medido, com cabeçalho e request-id ao lado, e foi
justamente a prova que impediu qualquer um de tentar de novo por três rodadas.
**Limitação que bloqueia um papel se remede a cada rodada** — a receita está no
`docs/BACKUP.md`.

### J — Pesquisador

Traz o que os outros fazem, e o traz **medido contra o nosso gargalo antes de
virar plano**. Chegou uma arquitetura completa de escrita — WAL, group commit,
MemTable, LSM: das dez propostas, cinco já existiam aqui, duas miravam um
problema que não temos, uma quebraria a ordem de digitação, e **duas eram
reais**.

**Medir a premissa do item vem antes de implementar o item** — inclusive quando
o item é nosso: o pedido que mandava ordenar as chaves do lote tinha o alvo
certo e a causa errada, e o conserto certo comprou 2,40× onde o proposto teria
comprado quase nada.


### A base de conhecimento é entregável, não sobra

**Todo projeto mantém um documento de tecnologias, e ele é obrigatório.** Não é
o `README` (que diz como usar) nem o manual (que diz o que faz): é o inventário
do que se usou **para fazer o produto e para fazer o trabalho** — as duas
metades, porque a segunda é a que se reaproveita e é a que ninguém escreve.

O que ele carrega:

- **Linguagens e volume, contados** — não «usamos Rust», mas quantas linhas e onde.
- **Dependências, e o que a escolha comprou ou custou.** Se há uma decisão de
  arquitetura por trás (aqui, zero dependências externas), o documento diz o
  que ela pagou em números medidos.
- **O que foi escrito à mão, e as normas conferidas** — com RFC e vetor.
- **As ferramentas do trabalho**: como se orquestrou, como se mediu, como se
  provou, como se compilou para outra arquitetura.
- **O que foi avaliado e RECUSADO, com o número.** Esta seção é a que mais
  poupa tempo depois: recusa medida impede a mesma proposta de voltar.

E o corolário que vale como regra: **script, comando e roteiro que resolveram
algo não podem morrer com a sessão.** Um transcrito de 99 MB não é base de
conhecimento — é matéria-prima. A base sai dele por **extrator**, para que se
refaça na sessão seguinte em vez de envelhecer: base montada à mão é base que
ninguém consegue atualizar.

**Quando escrever:** ao fim de cada rodada de trabalho, junto do restante da
documentação. Documento de tecnologia adiado é documento que se escreve de
memória — e memória é exatamente o que ele existe para substituir.


---

## Antes de commitar

```bash
cargo fmt --all
cargo clippy --workspace --all-targets     # tem de dar zero avisos
cargo test --workspace
```

Mexeu no formato em disco? Atualize `docs/FORMATO.md` no mesmo commit.

## Cores da ação, na interface

Convenção decidida e aplicada: **verde inclui, amarelo altera, rosa marca (o
excluir que volta), vermelho exclui de vez, azul consulta.**

Sempre **contorno, nunca fundo cheio** — a lição já estava num comentário do
CSS antes de virar regra: fundo laranja com texto escuro em cima ficava
ilegível, e foi assim que o botão de excluir apareceu. O preenchimento só
acontece no `hover`, quando há intenção. No tema claro as cinco escurecem, pelo
mesmo motivo do vermelhão da marca: verde e rosa claros não passam de 4,5:1
sobre papel.

## Estilo

- Código, comentários, documentação e mensagens de commit em **português**.
- Identificadores e comentários **sem acento** (o texto de interface pode ter).
- Comentário explica **por que**, não o que — o código já diz o que.
- Mensagem de commit conta a decisão e o motivo, não a lista de arquivos.

## Branch

Trabalhe em `claude/capacidades-disponiveis-y6auxh`, em
`adrianoboller/adrianoboller`. Não abra PR sem pedido explícito.
