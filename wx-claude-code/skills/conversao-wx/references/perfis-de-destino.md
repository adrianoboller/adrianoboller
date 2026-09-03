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
