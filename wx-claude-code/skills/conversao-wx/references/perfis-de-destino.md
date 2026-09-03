# Perfis de destino: para qual linguagem converter, e por quê

A pergunta H do questionário não é «qual linguagem você quer». É «o que o
projeto precisa e o que a equipe consegue manter». Este documento é a
orientação que o plugin dá antes de o usuário escolher: seis perfis de
backend e cinco de frontend, cada um com o que ganha, o que custa e para que
projeto WX serve melhor. A recomendação sai da tabela de decisão no fim;
a escolha é do usuário e vira `DEC-*`.

## Backend

| Perfil | Ganha | Custa | Serve melhor para |
| --- | --- | --- | --- |
| **Rust** (Axum ou Actix + PostgreSQL) | desempenho e memória previsíveis, binário único sem runtime, erros pegos em compilação, cruzamento para Windows e Linux | curva de aprendizado alta, equipe rara, compilações lentas | serviços de alto volume, motores de cálculo, componentes que rodam onde não há runtime, quem já tem o PhxSql |
| **Python** (FastAPI ou Django + PostgreSQL) | velocidade de entrega, biblioteca para tudo (fiscal, relatórios, dados, IA), equipe fácil de achar | desempenho por processo, tipagem opcional, deploy exige runtime | sistemas de gestão com muito relatório e integração, equipes que vão evoluir o produto rápido, projetos com análise de dados |
| **C# (.NET 8) + WL_C#** | a biblioteca **WL_C#** porta mais de 480 funções do WLanguage com o mesmo nome e comportamento (strings, datas, arquivos, conversões, JSON, tabelas), o que torna a tradução das procedures quase mecânica; Visual Studio; Windows nativo | as funções de HFSQL e de tela não estão na biblioteca; .NET fora do Windows exige atenção; a biblioteca é gratuita mas de código fechado | equipes WINDEV que vão manter o código depois, sistemas desktop Windows, quem quer a menor curva de aprendizado |
| **Go** (Chi ou Echo + PostgreSQL) | simples de aprender, binário único, concorrência fácil, deploy trivial | menos biblioteca para fiscal e relatório, generics recentes, ORMs mais fracos | APIs e serviços de integração, jobs, sistemas de médio volume com equipe pequena |
| **Java** (Spring Boot + PostgreSQL) | ecossistema corporativo, ferramentas de relatório maduras, equipe abundante | verboso, memória alta, tempo de subida | ERPs grandes com integrações corporativas, empresas que já têm Java |
| **Node** (NestJS + PostgreSQL) | mesma linguagem do frontend, equipe única, rápido para APIs | tipagem só com TypeScript, CPU limitado, muitas dependências | produtos web onde o time de frontend vai fazer o backend |

## Frontend

| Perfil | Ganha | Custa | Serve melhor para |
| --- | --- | --- | --- |
| **React** (TypeScript, Vite) | maior ecossistema, grids e componentes prontos, qualquer backend | escolhas demais (estado, roteador), disciplina para não virar bagunça | WEBDEV e WINDEV que vão para a web; telas densas com tabelas |
| **Vue** | mais simples que React, boa documentação | ecossistema menor no Brasil | equipes pequenas, telas de formulário |
| **Svelte** | menos código, rápido | comunidade menor, menos componentes de grid | interfaces leves, dashboards |
| **Blazor** (C#) | um só idioma com o backend .NET, componentes C# | performance no navegador, ecossistema menor | quem escolheu C# + WL_C# e quer uma pilha só |
| **Flutter** | um código para Android, iOS e desktop | Dart, tamanho do binário, grids menos maduros | WINDEV Mobile e apps offline |
| **Tauri** (Rust + React) | desktop leve com a tela em web | duas linguagens no mesmo produto | WINDEV desktop que precisa continuar desktop |

## Tabela de decisão

Responda as quatro perguntas; a linha que mais casa é a recomendação.

| Se… | Backend | Frontend |
| --- | --- | --- |
| a equipe que vai manter é a mesma que hoje faz WINDEV e quer a menor mudança | C# + WL_C# | Blazor ou React |
| o produto é WEBDEV, ou vai para a web, e o time de front vai crescer | Python ou Node | React |
| há cálculo pesado, volume alto ou o motor de dados é o PhxSql | Rust | React (ou Tauri se for desktop) |
| é WINDEV Mobile e precisa de Android e iOS | Python ou Go (API) | Flutter |
| há muito relatório, fiscal e integração, e o prazo manda | Python | React |
| já existe Java ou .NET na empresa | Java ou C# | React ou Blazor |

Duas regras que não mudam com o perfil:

- **O banco de destino é uma decisão separada** (PostgreSQL por padrão; MySQL,
  SQL Server ou PhxSql quando houver motivo). Trocar o banco muda tipos,
  collations e transações, e isso é G3, não H.
- **Regra de negócio não muda de comportamento por causa da linguagem.** O
  golden master compara o novo com o legado, seja qual for o perfil.

## Sinais que o questionário usa

Quando o usuário não sabe, o plugin pergunta, uma por vez: quem mantém depois;
o produto é desktop, web ou mobile; volume e desempenho importam; há
linguagem já em uso na empresa; prazo manda ou qualidade manda. Com isso
aponta uma linha da tabela, diz o porquê em uma frase e registra a escolha
como `DEC-0001` na abertura do G3.

## O processo de conversão, por perfil

Escolher a linguagem sem saber **como** o legado vira código nela é escolher
às cegas. Por isso, depois de mostrar as três opções, o wizard oferece o
processo de cada uma: o que cada peça do projeto WX vira no destino e em que
gate isso acontece. O usuário pode pedir o processo de uma opção só, de todas,
ou dizer que já conhece.

### O que cada peça do WX vira

| Peça do legado | Rust | Python | C# + WL_C# | Go / Java / Node |
| --- | --- | --- | --- | --- |
| Procedures globais e locais | funções em módulos por domínio; tipos fortes desde o inventário | funções em pacotes por domínio, com type hints e pydantic para os parâmetros | métodos estáticos com **o mesmo nome** da função WLanguage quando ela existe na WL_C#; o resto vira classe de serviço | funções ou serviços por domínio |
| Classes WLanguage | `struct` + `impl`, traits para herança usada de verdade | classes Python, dataclasses para as que só carregam dados | classes C# quase um para um | classes ou structs, conforme a linguagem |
| Análise HFSQL (arquivos, chaves, ligações) | esquema PostgreSQL migrado por script; `sqlx` ou `diesel` | esquema PostgreSQL; SQLAlchemy | esquema PostgreSQL ou SQL Server; Entity Framework (**HFSQL não está na WL_C#**) | esquema PostgreSQL; ORM da pilha |
| `HReadSeekFirst`, `HAdd`, `HModify`, filtros e navegação | repositório por arquivo, com consultas SQL explícitas; cursor vira paginação | repositório por arquivo; sessão da ORM | repositório por arquivo; o padrão de leitura por chave vira consulta LINQ | repositório por arquivo |
| Queries `.WDR` | SQL revisado e parametrizado, uma função por query | SQL revisado, uma função por query | SQL revisado, uma função por query | idem |
| Janelas e páginas | API por caso de uso; a tela é do frontend | API por caso de uso | API por caso de uso, ou Blazor se a pilha for uma só | API por caso de uso |
| Eventos de controle (`Exit`, `Modification`, `Click`) | validações viram regras no backend e comportamento no frontend, com o trace_id do evento | idem | idem, e os que só formatam texto usam a WL_C# | idem |
| Relatórios `.WDE` | gerador de PDF próprio ou serviço; comparado página a página | ReportLab ou WeasyPrint | QuestPDF ou Report Viewer | biblioteca da pilha |
| Funções de string, data, arquivo, JSON | crates da `std` e do ecossistema, mapeadas no inventário | biblioteca padrão | **WL_C# com o mesmo nome** (`Left`, `DateSys`, `fFileExist`…), conferida por hash | biblioteca padrão |
| Threads, timers, sockets | `tokio` | `asyncio` ou jobs | `Task` e `HttpClient` | goroutines / executores / event loop |

### As quatro estratégias

O processo também é uma escolha, e o wizard pergunta qual, com a recomendada primeiro:

| Estratégia | Como é | Quando recomendar | Custo |
| --- | --- | --- | --- |
| **Tradução assistida** | cada procedure vira uma função no destino, na mesma ordem, com a WL_C# ou um mapa de funções; regra de negócio preservada literalmente | C# + WL_C#; equipe WINDEV mantendo; prazo curto | carrega o desenho do legado para o novo |
| **Reescrita guiada por regras** | o inventário extrai as regras de negócio (BR-*) e o código novo é escrito a partir delas, não do código velho | Rust ou Python; sistemas com muito código morto; quando o desenho vai mudar | exige regras completas antes do G3, e o golden master é a única prova de igualdade |
| **Estrangulamento por módulo** | o legado continua no ar e cada módulo migra por vez atrás de uma fachada (API ou banco compartilhado); usuários mudam de tela aos poucos | sistemas grandes em produção que não podem parar; prazo longo | duas pilhas ao mesmo tempo; sincronizar dados entre HFSQL e o banco novo |
| **Ondas com cutover único** | tudo é convertido por ondas (G5) e a virada acontece de uma vez no G7, com paralelo antes | sistemas pequenos e médios; quando o banco muda junto | período de paralelo, treinamento em bloco |

O que fica registrado: `H_backend.processo` e `I_frontend.processo` com a
estratégia, o que o usuário confirmou do mapeamento e o que quer diferente.
O `aplicar_questionario.py` escreve `.wx-migration/processo-de-conversao.md`
com a tabela do perfil escolhido, a estratégia e os gates em que cada peça é
convertida. Esse arquivo é a primeira versão do que o G3 vai detalhar.

### Como cada estratégia atravessa os gates

| Gate | Tradução assistida | Reescrita guiada | Estrangulamento | Ondas |
| --- | --- | --- | --- | --- |
| G1 inventário | procedures e funções contadas; o mapa WL → destino é medido (quantas têm equivalente direto) | regras BR-* extraídas e aprovadas | módulos e suas dependências; ordem de estrangulamento | ondas definidas por dependência |
| G2 especificação | contrato por procedure | contrato por regra | fachada e contrato de sincronização | contrato por onda |
| G3 arquitetura | `DEC-0001` linguagem, mapa de funções fechado | `DEC-0001`, desenho novo | fachada, sincronização de dados | pilha e banco |
| G4 piloto | um módulo traduzido de ponta a ponta, golden igual | um fluxo reescrito, golden igual | primeiro módulo estrangulado em produção | primeira onda em homologação |
| G5 ondas | tradução em lote, por tema WLanguage | reescrita por domínio | um módulo por vez, em produção | ondas seguintes |
| G6 endurecimento | dead code, desempenho, segurança | idem | desligar o legado módulo a módulo | paralelo |
| G7 cutover | virada | virada | último módulo; legado desligado | virada única |
