---
description: "Questionario inicial do projeto WX: bloco 0 (empresa, prazo, orcamento, GitHub) e letras A-J. Gera .wx-migration/."
argument-hint: "[raiz-do-projeto-de-destino]"
allowed-tools: "Read, Glob, Grep, Bash, Write, AskUserQuestion"
---

# Questionário inicial do WX Claude Code

Este é o **primeiro passo** de qualquer conversão de um projeto WINDEV, WEBDEV ou WINDEV Mobile. Ele existe para que o plugin saiba **o que foi entregue** e **para onde vai o projeto** antes de qualquer análise. Nada é convertido aqui.

`$ARGUMENTS`, quando vier, é a raiz do projeto de destino. Se não vier, pergunte.

## Como perguntar

- **Uma letra por mensagem, sempre.** Pergunte a letra, encerre o turno e espere. Nunca liste duas letras na mesma mensagem, nem com `AskUserQuestion` nem em texto: quem lê dez perguntas responde três. Só a primeira mensagem tem uma frase de abertura; as seguintes começam direto pela letra.
- Use `AskUserQuestion` quando a ferramenta existir (uma pergunta por chamada); senão, faça a pergunta em texto e **encerre o turno** sem perguntar mais nada.
- A resposta de cada letra decide a próxima pergunta (tabela abaixo). Não pule letra: quem não tem o item responde «não tenho» ou «não se aplica», e isso também é resposta.
- Antes de passar à letra seguinte, confirme em uma linha o que foi registrado (`A: inputs/banco.sql, HFSQL 2025 → provided`). O usuário corrige na hora, não no fim.
- Um caminho só conta como fornecido depois de você **ler o arquivo** (`Glob` + `Read`, ou `ls -la` e `head`). Anexo que não abre é `missing`, não `provided`.
- Não prometa relatório, plano nem prazo durante o questionário. Estado desta etapa: `INTAKE_PENDING`.
- Respostas em português; o usuário pode responder em qualquer idioma.

## As perguntas

### Bloco 0 · Empresa e projeto (antes da letra A)

Quinze itens, **um por mensagem**, na ordem. É o que o PMO, o stakeholder e a entrega precisam saber antes de qualquer anexo. Quem não tem o item responde «não tenho» e segue.

| Item | Pergunta | Onde vai parar |
| --- | --- | --- |
| 0.1 | Softhouse solicitante: razão social, nome fantasia, CNPJ (opcional) e **a solicitação** em uma ou duas frases (o que converter, para que, até quando) | `empresa.md`, `pmo/projeto.json` |
| 0.2 | Quem são os diretores (nome, cargo, contato) | `empresa.md` |
| 0.3 | Endereço completo (logradouro, número, complemento, bairro, cidade, UF, CEP, país) | `empresa.md` |
| 0.4 | Logotipo da empresa: caminho do arquivo na raiz de evidências | `empresa.md` (verificado) |
| 0.5 | Logotipo do software | `empresa.md` (verificado) |
| 0.6 | Finalidade do software (uma frase) | `empresa.md` |
| 0.7 | Objetivos do projeto (lista) | `empresa.md` |
| 0.8 | Descrição do software, seus recursos e módulos | `empresa.md` |
| 0.9 | Organograma do projeto: arquivo, ou as posições (papel, nome, responde a) | `pmo/organograma.md` |
| 0.10 | Fluxograma do processo principal: arquivo, ou as etapas em ordem | `pmo/fluxograma.md` (Mermaid) |
| 0.11 | Cronograma: início, marcos com data e gate, e **o prazo final de entrega** | `pmo/cronograma.md`, `plano.json` (`previsto_para`) |
| 0.12 | Orçamento: valor, moeda, base (horas, fechado, tokens, misto) e quem aprovou | `pmo/projeto.json` |
| 0.13 | Riscos conhecidos: risco, probabilidade, impacto, resposta, dono | `pmo/riscos.md` (RSK-*) |
| 0.14 | Pessoal envolvido: nome, papel, empresa, contato | `empresa.md` |
| 0.15 | GitHub de destino: URL, branch, usuário, **onde a credencial está configurada** e o diretório de destino | `entrega.json` |

**A senha nunca é perguntada nem gravada.** No 0.15 pergunte o **nome** da variável de ambiente ou do segredo onde o token vai morar (`GITHUB_TOKEN`, `gh auth`, credential manager) e registre só isso em `credencial_ref`. Se o usuário colar a senha ou o token na conversa, **não a reproduza de nenhuma forma** (nem entre parênteses, nem mascarada, nem «a senha que termina em…»): diga só «a credencial colada não foi gravada», peça que ele a revogue e configure no ambiente; o script `aplicar_questionario.py` recusa o questionário se qualquer chave `senha`, `token`, `password` ou `secret` vier com valor. Logotipo, organograma e fluxograma em arquivo só contam como `provided` depois de abertos, como qualquer anexo.

### Letras A a J

**A) O `.SQL` do projeto.** «Informe o caminho do script SQL do seu projeto (DDL, índices, constraints, views, triggers). Qual o dialeto e a versão do banco (HFSQL Classic, HFSQL Client/Server, MySQL, PostgreSQL, SQL Server, Oracle…)? Qual o encoding e o collation?» Se não houver `.SQL`, pergunte se existe a análise (`.wda`/`.wdd`) exportada ou um dump.

**B) PDF só dos códigos.** «Informe o caminho do PDF gerado pela plataforma WX contendo **somente o código** (procedures, classes, eventos, código de projeto). É pesquisável ou é imagem?» Registre `page_count`, `searchable` e `content_scope: ["code","events"]`.

**C) PDF só das interfaces.** «Informe o caminho do PDF gerado pela plataforma WX contendo **somente as telas** (janelas, páginas, controles, relatórios).» `content_scope: ["ui","reports"]`.

**D) PDF só das Queries SQL.** «Informe o caminho do PDF gerado pela plataforma WX contendo **somente as queries** (nome, SQL, parâmetros, onde são usadas).» `content_scope: ["queries"]`.

**E) PDF completo.** «Informe o caminho do PDF **completo** gerado pela plataforma WX (documentação técnica integral do projeto).» `content_scope: ["code","events","ui","queries","business_rules","reports","integrations"]`. Se B, C ou D faltarem, este é a fonte de reserva — mas registre como `partial` o que só existe dentro dele.

**F) Qualidade gráfica e funcional das telas, com o Impeccable.** «Deseja configurar a qualidade das telas do projeto novo com o Impeccable?» Se sim, um ERP exige mais que paleta: faça as **oito subperguntas de `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/references/qualidade-erp.md`, uma por mensagem**: F1 quem opera, por quanto tempo, em que ambiente e tela; F2 atalhos de teclado do WINDEV a preservar, Enter avança campo, ordem de tabulação; F3 grids (linhas por tela, colunas fixas, filtro por coluna, edição na célula, totais, exportar, imprimir); F4 formulários (validação inline ou ao salvar, mensagens do legado, obrigatório, máscaras, autocompletar); F5 números, datas e moeda (locale, decimais, negativo, fuso); F6 relatórios e impressão (telas, papel, PDF, etiquetas); F7 estados (vazio, carregando, sem permissão, offline, erro, confirmação de destrutivo); F8 acessibilidade (WCAG, leitor de tela, daltonismo, alvo de toque); **F9 vocabulário dos botões**: imperativo (INCLUIR, ALTERAR, EXCLUIR, GRAVAR, SELECIONAR REGISTRO, VOLTAR, CANCELAR, DUPLICAR) ou substantivo (Inclusão, Alteração, Exclusão, Gravação, Selecionar, Abortar, Cancelar), maiúsculas ou capitalizado, e o texto exato das mensagens de confirmar exclusão, gravado, excluído e cancelado; **F10 posição dos botões**: acima, abaixo, à direita ou à esquerda da grade e dos campos, alinhamento, ordem, onde ficam gravar e cancelar; **F11 ícones**: usar, biblioteca, com ou sem texto, tamanho, um por ação; **F12 cores das ações**: uma por ação, contorno ou preenchido (ofereça o padrão do plugin: verde inclui e grava, amarelo altera, vermelho exclui, azul seleciona, cinza volta e cancela); **F13 fundo das telas**: cor lisa em hexadecimal ou rgb, textura ou imagem, cor do tema escuro, opacidade. Registre F9, F11 e F12 como tabela por ação, com as oito ações fixas. Só então: paleta, tema, tipografia, densidade, marca, e preservar ou redesenhar. As respostas vão para `PRODUCT.md` (F1) e para seções do `DESIGN.md` (F2–F8), cada uma ligada ao comando do Impeccable que a consome, e alimentam o `fidelity.ui` e o `/wx-claude-code:estilo-telas`.

**G) Help completo do WX em JSON.** «O plugin traz o corpus WLanguage 12k (Help do WINDEV, WEBDEV e WINDEV Mobile em JSON, estado DEGRADED/CONDITIONAL). Deseja usá-lo como fonte técnica auxiliar? Tem um Help específico da sua versão/update para usar como override?» Se sim ao corpus, rode a verificação de hash (abaixo). Lembre: o Help é fonte de **semântica técnica**, nunca de regra de negócio.

**H) Para qual linguagem converter o backend.** Esta é a pergunta que mais muda o projeto, e o plugin **orienta antes de perguntar**. Leia `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/references/perfis-de-destino.md` e siga em três passos, um por mensagem:

1. Se o usuário já sabe a linguagem, registre e pule ao passo 3. Se não sabe, faça as quatro perguntas de sinal, uma por vez: quem vai manter o código depois (a equipe WINDEV de hoje ou outra); o produto é desktop, web ou mobile; volume e desempenho importam ou o prazo manda; há linguagem já em uso na empresa.
2. Com os sinais, mostre **três opções com o porquê em uma frase cada**, sempre com estas três presentes e a recomendada primeiro:
   - **Rust** (Axum + PostgreSQL): desempenho e binário único; para volume alto, motor de cálculo ou quem já usa o PhxSql.
   - **Python** (FastAPI + PostgreSQL): entrega rápida e biblioteca para fiscal, relatório e dados; para sistemas de gestão que vão evoluir rápido.
   - **C# (.NET 8) + WL_C#**: a biblioteca WL_C# porta mais de 480 funções do WLanguage com o mesmo nome, o que torna a tradução das procedures quase mecânica; para a equipe WINDEV que vai manter o código.
   Acrescente Go, Java ou Node quando os sinais apontarem para eles. Junto com as opções, ofereça: **«Quer ver como seria o processo de conversão para alguma delas?»** Se pedir, mostre da seção «O processo de conversão, por perfil» do mesmo documento a tabela do que cada peça do WX vira naquele perfil (procedures, classes, análise HFSQL, queries, janelas, relatórios, funções de string e data), uma opção por mensagem. O usuário escolhe a linguagem; a escolha vira `DEC-0001` no G3.
3. Pergunte a **estratégia de conversão**, com a recomendada primeiro e o porquê em uma frase: tradução assistida (recomende com C# + WL_C# e equipe WINDEV mantendo), reescrita guiada por regras (Rust ou Python, muito código morto), estrangulamento por módulo (sistema grande em produção que não pode parar), ondas com cutover único (pequeno e médio, banco muda junto). Pergunte se ele confirma o mapeamento mostrado e o que quer diferente. Registre em `H_backend.processo`.
4. Feche a letra: framework, banco de destino (PostgreSQL por padrão), versões mínimas e forma de implantação.

**I) Para qual linguagem converter o frontend.** Mesma orientação, do mesmo documento. Ofereça **React** (TypeScript) como padrão para web, e mais duas conforme H e a plataforma: **Blazor** se H foi C#; **Flutter** se há Android e iOS; **Tauri** (Rust + React) se o produto continua desktop; Vue ou Svelte para equipes pequenas. Depois: plataformas (web, desktop, Android, iOS), navegadores e dispositivos mínimos. E o processo, como em H: ofereça ver como cada tela vira rota e componente, pergunte a estratégia e o ritmo (tela a tela, módulo a módulo, tudo) e o que quer diferente; registre em `I_frontend.processo`.

**J) Economia de tokens.** «Deseja ativar e configurar a economia de tokens? Isso instala no `CLAUDE.md` do projeto a instrução de estilo de resposta (direto ao ponto, frases curtas, um assunto por parágrafo, problema em uma linha, solução em passos numerados) e deixa pronto o comando `/wx-claude-code:laudo-tokens`, a auditoria em três fases que **não altera nada sem aprovação**.»

### A resposta decide a próxima

| Depois de | Se a resposta for | Então |
| --- | --- | --- |
| 0.1 | solicitação menciona a linguagem ou o prazo | anote para reaproveitar em H e em 0.11; não pergunte de novo o que já foi dito |
| 0.4, 0.5, 0.9, 0.10 | caminho informado | abra o arquivo; se não abrir, `missing` e siga (o logotipo vira `GAP-*` no G1, não trava) |
| 0.9 ou 0.10 | «não tenho arquivo» | peça as posições ou as etapas em texto, que também são resposta |
| 0.11 | prazo final sem marcos | proponha um marco por gate (G1, G4, G7) com datas e peça para confirmar |
| 0.12 | «não sei o orçamento» | registre `valor: null` e quem vai aprovar; o `pmo.py` mostra INDISPONÍVEL, nunca um número inventado |
| 0.15 | usuário cola senha ou token | não reproduza o valor em nenhuma forma, não grave; peça que revogue e configure no ambiente, e registre só o nome em `credencial_ref` |
| A | caminho informado | abra o arquivo; se não abrir, diga e pergunte de novo A antes de ir a B |
| A | «não tenho» | pergunte se existe a análise exportada ou um dump; sem nada, `missing` e siga para B |
| B, C ou D | «não tenho» | anote e siga; em **E** avise que o PDF completo vai cobrir o que faltou como `partial` |
| E | «não tenho» e B, C ou D também faltam | avise que o G0 vai dar `BLOCKED` para esses grupos e pergunte se quer seguir assim mesmo |
| F | «não» | pule paleta, tema, tipografia e densidade; vá direto a G |
| F | «sim» | faça as subperguntas de F uma por vez, na ordem F1 a F8 de `references/qualidade-erp.md` (quem opera; teclado e atalhos; grids; formulários; números, datas e moeda; impressão; estados e erros; acessibilidade), e só então preservar ou redesenhar → paleta → tema → tipografia → densidade → marca |
| F9 | usuário responde só «imperativo» ou só «substantivo» | mostre a tabela das oito ações preenchida com esse estilo e peça para confirmar ou corrigir rótulo por rótulo; texto de botão não se adivinha |
| F12 | usuário não sabe as cores | ofereça o padrão do plugin (verde, amarelo, vermelho, azul, cinza, contorno) e registre a escolha; contraste é medido na tela, não na resposta |
| F13 | fundo com imagem ou textura | pergunte a opacidade e avise que a área de dados fica lisa; registre `GAP-*` se a imagem não estiver na pasta de evidências |
| F3 | grids com milhares de linhas ou edição na célula | avise que o `grid-migration-specialist` entra no G3 e que virtualização é requisito, não opção |
| F6 | há impressão em bobina ou etiqueta | registre como `RPT-*` com papel e largura; o `reports-printing-specialist` compara página a página |
| G | «não» ao corpus | pergunte de onde virá a semântica WLanguage (Help específico ou nenhuma); sem fonte, anote o risco |
| G | «sim» | pergunte a versão e se há override; rode o `--verify` só depois de fechar o questionário |
| H | usuário não sabe a linguagem | faça as quatro perguntas de sinal, uma por vez, e só então mostre as três opções (Rust, Python, C# + WL_C#) com a recomendada primeiro |
| H | mostrou as opções | ofereça ver o processo de conversão; «todas» = uma opção por mensagem; «já conheço» = siga para a estratégia |
| H | pediu o processo de uma opção | mostre a tabela de peças daquele perfil e pergunte se confirma o mapeamento ou o que quer diferente; anote em `quer_diferente` |
| H | linguagem escolhida | pergunte a estratégia, depois framework, banco e implantação **na mesma letra**, um item por vez |
| H | estratégia estrangulamento | avise que exige fachada e sincronização de dados entre HFSQL e o banco novo, e que isso entra como `RSK-*` |
| H | escolheu C# | em I ofereça Blazor além de React; registre que o `WL.dll` será baixado da release oficial e conferido por hash |
| I | plataforma inclui mobile | pergunte versões mínimas de Android e iOS; se só web, pergunte navegadores |
| J | «sim» | confirme que o `CLAUDE.md` do projeto vai receber o bloco de estilo; se já existir, diga que não será sobrescrito |

Feche com as três perguntas de governança da skill de conversão, que o questionário não substitui: versão/update/idioma do WX; modo desejado (`inventário`, `plano`, `piloto`, `completo`); quem aprova regras, divergências e aceite.

## O que gravar

1. Grave as respostas em `<projeto>/.wx-migration/questionario.json` no formato de `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/templates/questionario.json`. Caminhos de anexo ficam **relativos à raiz de evidências** informada.
2. Aplique as respostas ao manifesto e à configuração com o script determinístico (ele não sobrescreve nada que já exista):

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/aplicar_questionario.py" \
  --questionario <projeto>/.wx-migration/questionario.json \
  --project-root <projeto> \
  --plugin-root "${CLAUDE_PLUGIN_ROOT}"
```

   Ele cria `.wx-migration/wx-inputs.manifest.json`, `.wx-migration/conversion.config.json`, o `CLAUDE.md` do projeto (com o estilo de resposta quando **J** for sim), o esboço `DESIGN.md` com a paleta quando **F** for sim, `processo-de-conversao.md` de **H** e **I**, e, do bloco 0, `empresa.md`, `entrega.json` e `pmo/{projeto.json, organograma.md, fluxograma.md, cronograma.md, riscos.md}`, que o `pmo.py iniciar` lê para preencher `previsto_para` dos gates.

3. Se **G** for sim, verifique o corpus antes de citá-lo:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" --verify
```

4. Mostre ao usuário o resumo: bloco 0 (o que ficou pendente) e, por letra, `provided | partial | missing | not_applicable`, e o que ficou em aberto.

## Depois

Diga a próxima ação, e só ela:

- anexos verificados → `/wx-claude-code:converter` (Gate G0, pré-flight);
- **F** sim → `/wx-claude-code:estilo-telas` pode rodar em paralelo ao inventário;
- **J** sim → `/wx-claude-code:laudo-tokens` quando o usuário quiser medir.

Não execute a conversão neste comando.
