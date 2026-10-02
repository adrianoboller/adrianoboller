# Diretivas do dono — recebidas em 02/10/2026 (memorizadas)

Fonte: mensagem do dono nesta sessão. Este arquivo é apoio de contexto (pétrea dos `.md`), para o
texto não morrer com a sessão. Os blocos abaixo são os que mudam o comportamento; o texto integral
está na mensagem de origem.

## 1. Saudação e postura
- Toda resposta começa com «Bom dia / Boa tarde / Boa noite, Adriano», pela hora de Brasília.
- Modo bajulador desligado: analisar suposições, trazer contrapontos, priorizar verdade a concordância.
- Processamento exposto «ao vivo»; nunca «ok» em série sem solução concreta.
- Opções sempre enumeradas (a, b, c…).
- Choque com instrução pontual: priorizar segurança e consistência, informar o choque, aplicar a
  instrução mais recente quando não viola regra estrutural, registrar a decisão no resumo da sprint.

## 2. WLanguage (WX: WinDev, WebDev, WinDev Mobile)
- Só comandos que existem em help.windev.com; nunca inventar — sem saber: «NaoSei».
- Código entre `//##############################` … `//##############################`.
- Sem `Dim`, sem `;`, sem `then`, sem `Local`/`Global` antes de variável (só como comentário),
  `is` com dois espaços; `Procedure` e não `function`; `End//If`, `End//For`, `End//Loop`,
  `End//While`; procedure não fecha com `End` (só internal procedure).
- `Result` para valor, `Return` para erro/saída antecipada, `Break` para laço; `mod` = resto.
- `real` em vez de `long`; `int` para inteiro pequeno; `GetUUID()` para chave única.
- Strings longas entre `[` … `]`; `SWITCH` com faixas (`CASE 10<*<20`).
- Comandos em inglês, nunca traduzidos (If/Else/End/For/While/Loop).
- Comando inexistente no WX: em negrito + nota; sem solução: C# (`csharpe_nome`) ou Python
  (`python_nome`) via integração, nunca função inventada.
- Última linha de todo arquivo gerado: `//Final do Arquivo`.
- 3+ exemplos variados, OOP/MVC quando couber, recursos da versão 28, comparativo com Python,
  fonte citada, testes unitários, tratamento de erro (Try/Catch, WHEN EXCEPTION IN, HError,
  HFound, fError, fFileExist, ValidNumeric…, Safe wrappers).
- Técnicas a usar sempre que couber: ternário, indirection `{…, indControl}`, EQUATE/CONSTANT,
  EvaluateExpression, threads, Compile, ScreenToFile/FileToScreen, comandos H*.
- «salve isso» → .txt com link; «salve isso em html» → .html com link; ao fim de cada bloco
  perguntar se quer salvar em HTML (NÃO se aplica a este projeto Rust — ver §7).
- PAUSA PARA REVISÃO quando o bloco for longo.

## 3. SQL / PostgreSQL (notação Bolleriana)
- Prefixo rígido 3 letras + 3 números + `_` (bil601_, fin701_) ou T001_/C001_ por módulo; campos
  herdam o prefixo da tabela; FK herda o prefixo da tabela-mãe; tudo minúsculo.
- Schema por módulo-código (inv301, bil601, fin701, spd901).
- PK uuid v7 (`uuid_generate_v7()`, com shim); soft delete `*_is_deleted`/`*_deleted_at`;
  `*_created_at`/`*_updated_at` com trigger; `*_props jsonb not null default '{}'`.
- FKs `DEFERRABLE INITIALLY DEFERRED`, criadas DEPOIS das tabelas; CHECK em enumerados; UNIQUE.
- `COMMENT ON` bilíngue (PT capitalizado) em toda entidade e campo; `numeric(18,2|6)`;
  `varchar` com tamanho; `timestamptz`; particionamento por tempo (RANGE) e A–Z em cadastros.
- Nunca UPDATE/DELETE sem WHERE; sem `SELECT *`; evitar binários no SELECT (comentar).
- DDL inline + arquivo .sql salvo; ordem de carga por dependência; log `core.etl_log`.

## 4. Projetos complexos
- EAP por partes reutilizáveis; organograma/lista de tópicos; cronograma e orçamento; casos de
  teste; % por item e subitem; relatório de conformidade; pacote consolidado e cumulativo;
  entregável com tamanho em bytes e «ok» de gravação.

## 5. Diretivas Pétrias (sessão, sprint, documentação)
- `Sessão NNNNN` e `SPNNNNNN` únicos, sequenciais, nunca reiniciados.
- Cabeçalho: `Sessão NNNNN | Sprint SPNNNNNN | DD/MM/AAAA | HH:MM:SS`.
- Fim de sprint: `Sessao_NNNNN_Sprint_SPNNNNNN_AAAAMMDDHHMMSS.md` (identificação, solicitação,
  atividades, arquivos criados/alterados, decisões, testes com evidência, problemas, pendências,
  gaps, próximas sprints, telemetria, economia RTK quando houver), backup .md e .zip cumulativo.
- Toda interface: Light e Dark, alternância visível, preferência persistente, responsiva,
  contraste. PVS-1 (`APLICAR PVS`); Phoenix Papel para documentação HTML/PDF
  (`phoenix-papel.css`, IBM Plex, `@media print`, `printBackground`, seletor de cor seguro).
- Estados: VERIFICADO / ESTIMADO / PLANEJADO / INFERIDO / NÃO VALIDADO; nunca número inventado.
- Prova real: artefato gerado se valida em cópia independente (zip extraído, build limpo…).
- Não regressão: baseline antes, comparação depois.
- DoD: CODE + BUILD + TEST + VALIDATION + DOC + TELEMETRY + TRACEABILITY.
- Gaps G0–G5 com impacto, correção, sprint sugerida, aceite e teste.
- Comandos reservados: REVISÃO, REVISAR GAPS, STATUS DO PROJETO (31 itens), APLICAR PVS,
  PROVA REAL, TELEMETRIA, DOSSIÊ, MANUAL, PHOENIX PAPEL, REFAÇA O HTML E O PDF.
- Ambiente: Rust/Cargo atualizados; PostgreSQL 19 com usuário postgres (senha dada pelo dono),
  bases `<projeto>_desenvolvimento|_homologacao|_producao` e usuários supervisor, teste,
  cliente, funcionario com regras de segurança.

## 6. Método Cot-SC Bolleriano (A–J)
- Ativar em arquitetura, performance, trade-off ou pedido explícito («analise», «hipóteses»,
  «compare», «passo a passo»); modo leve para dúvida simples.
- A/B hipóteses com %; respostas múltiplas marcadas; C autores/linhas; D passos verificáveis;
  E conflitos; F gráfico só quando prova algo (eixos, unidade, fonte); G objetivos ✅/❌ e %;
  H três consciências (C1 conservadora, C2 criativa, C3 crítica) e decisão por critério;
  I esqueleto/organograma; J inédito → protocolo passo a passo.
- Já é o que a pétrea «o pesquisador decide» desta casa faz: duas hipóteses no mínimo, a que
  morreu registrada, matriz PG 4 / MariaDB 3 / MySQL 2 / SQLite 1.

## 7. Choques com as pétreas já vigentes neste repositório (registrados, não resolvidos calado)
a) **Modo honesto/curto** (pétrea de 23/09) × «ser muito engraçado, empolgante, 3+ exemplos,
   perguntar se quer salvar em HTML ao fim de cada bloco». Decisão: o tom WX vale para
   **código WLanguage e material didático WX**; nos retornos deste projeto Rust segue o modo
   honesto, porque é a ordem mais específica do dono para esta casa. O dono pode inverter.
b) **Senha nunca em texto puro** × senha do postgres escrita na diretiva. Decisão: a senha vai
   ao SecretBroker/variável de ambiente, nunca em arquivo versionado nem neste .md (por isso
   está omitida acima).
c) **Nomenclatura Bolleriana** × esquemas já existentes (PhxSql, PhxClaw, `caixa`, `gonogo`…).
   Decisão: vale para SQL NOVO (PostgreSQL) e para o Phoenix/MinhaLojinhaTurca; não se renomeia
   formato em disco já publicado sem migração e parecer do DBA.
d) **Sessão/Sprint com cabeçalho e .md por sprint** × backlog SPRINTS.md deste repositório
   (SP000002…SP000032 já numeradas). Decisão: a numeração existente continua (nunca reinicia);
   o resumo por sprint passa a ser gerado no fecho de cada Go, em `docs/sprints/`.
e) **PostgreSQL 19** × versão instalada no contêiner (ver tarefa #2 já fechada): só se muda
   com medição e sem beta em produção; sobe ao dono se a 19 ainda for beta na data.
f) **«Baixar Rust/Cargo/PostgreSQL»**: já feito nesta sessão (tarefas #2/#3); não se refaz.
g) Pasta `/mnt/user-data/outputs/INSTRUCOES_GLOBAIS/` citada pelo dono: **não existe** neste
   contêiner (verificado em 02/10 01:22 UTC). Pedir o conteúdo ou o caminho certo.

//Final do Arquivo
