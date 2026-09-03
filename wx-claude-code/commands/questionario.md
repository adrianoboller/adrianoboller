---
description: "Questionario inicial A-J do projeto WX: anexos, Impeccable, Help, linguagens de destino e economia de tokens. Gera .wx-migration/."
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

**A) O `.SQL` do projeto.** «Informe o caminho do script SQL do seu projeto (DDL, índices, constraints, views, triggers). Qual o dialeto e a versão do banco (HFSQL Classic, HFSQL Client/Server, MySQL, PostgreSQL, SQL Server, Oracle…)? Qual o encoding e o collation?» Se não houver `.SQL`, pergunte se existe a análise (`.wda`/`.wdd`) exportada ou um dump.

**B) PDF só dos códigos.** «Informe o caminho do PDF gerado pela plataforma WX contendo **somente o código** (procedures, classes, eventos, código de projeto). É pesquisável ou é imagem?» Registre `page_count`, `searchable` e `content_scope: ["code","events"]`.

**C) PDF só das interfaces.** «Informe o caminho do PDF gerado pela plataforma WX contendo **somente as telas** (janelas, páginas, controles, relatórios).» `content_scope: ["ui","reports"]`.

**D) PDF só das Queries SQL.** «Informe o caminho do PDF gerado pela plataforma WX contendo **somente as queries** (nome, SQL, parâmetros, onde são usadas).» `content_scope: ["queries"]`.

**E) PDF completo.** «Informe o caminho do PDF **completo** gerado pela plataforma WX (documentação técnica integral do projeto).» `content_scope: ["code","events","ui","queries","business_rules","reports","integrations"]`. Se B, C ou D faltarem, este é a fonte de reserva — mas registre como `partial` o que só existe dentro dele.

**F) Qualidade gráfica e funcional das telas, com o Impeccable.** «Deseja configurar a qualidade das telas do projeto novo com o Impeccable?» Se sim, um ERP exige mais que paleta: faça as **oito subperguntas de `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/references/qualidade-erp.md`, uma por mensagem**: F1 quem opera, por quanto tempo, em que ambiente e tela; F2 atalhos de teclado do WINDEV a preservar, Enter avança campo, ordem de tabulação; F3 grids (linhas por tela, colunas fixas, filtro por coluna, edição na célula, totais, exportar, imprimir); F4 formulários (validação inline ou ao salvar, mensagens do legado, obrigatório, máscaras, autocompletar); F5 números, datas e moeda (locale, decimais, negativo, fuso); F6 relatórios e impressão (telas, papel, PDF, etiquetas); F7 estados (vazio, carregando, sem permissão, offline, erro, confirmação de destrutivo); F8 acessibilidade (WCAG, leitor de tela, daltonismo, alvo de toque). Só então: paleta, tema, tipografia, densidade, marca, e preservar ou redesenhar. As respostas vão para `PRODUCT.md` (F1) e para seções do `DESIGN.md` (F2–F8), cada uma ligada ao comando do Impeccable que a consome, e alimentam o `fidelity.ui` e o `/wx-claude-code:estilo-telas`.

**G) Help completo do WX em JSON.** «O plugin traz o corpus WLanguage 12k (Help do WINDEV, WEBDEV e WINDEV Mobile em JSON, estado DEGRADED/CONDITIONAL). Deseja usá-lo como fonte técnica auxiliar? Tem um Help específico da sua versão/update para usar como override?» Se sim ao corpus, rode a verificação de hash (abaixo). Lembre: o Help é fonte de **semântica técnica**, nunca de regra de negócio.

**H) Para qual linguagem converter o backend.** Esta é a pergunta que mais muda o projeto, e o plugin **orienta antes de perguntar**. Leia `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/references/perfis-de-destino.md` e siga em três passos, um por mensagem:

1. Se o usuário já sabe a linguagem, registre e pule ao passo 3. Se não sabe, faça as quatro perguntas de sinal, uma por vez: quem vai manter o código depois (a equipe WINDEV de hoje ou outra); o produto é desktop, web ou mobile; volume e desempenho importam ou o prazo manda; há linguagem já em uso na empresa.
2. Com os sinais, mostre **três opções com o porquê em uma frase cada**, sempre com estas três presentes e a recomendada primeiro:
   - **Rust** (Axum + PostgreSQL): desempenho e binário único; para volume alto, motor de cálculo ou quem já usa o PhxSql.
   - **Python** (FastAPI + PostgreSQL): entrega rápida e biblioteca para fiscal, relatório e dados; para sistemas de gestão que vão evoluir rápido.
   - **C# (.NET 8) + WL_C#**: a biblioteca WL_C# porta mais de 480 funções do WLanguage com o mesmo nome, o que torna a tradução das procedures quase mecânica; para a equipe WINDEV que vai manter o código.
   Acrescente Go, Java ou Node quando os sinais apontarem para eles. O usuário escolhe; a escolha vira `DEC-0001` no G3.
3. Feche a letra: framework, banco de destino (PostgreSQL por padrão), versões mínimas e forma de implantação.

**I) Para qual linguagem converter o frontend.** Mesma orientação, do mesmo documento. Ofereça **React** (TypeScript) como padrão para web, e mais duas conforme H e a plataforma: **Blazor** se H foi C#; **Flutter** se há Android e iOS; **Tauri** (Rust + React) se o produto continua desktop; Vue ou Svelte para equipes pequenas. Depois: plataformas (web, desktop, Android, iOS), navegadores e dispositivos mínimos.

**J) Economia de tokens.** «Deseja ativar e configurar a economia de tokens? Isso instala no `CLAUDE.md` do projeto a instrução de estilo de resposta (direto ao ponto, frases curtas, um assunto por parágrafo, problema em uma linha, solução em passos numerados) e deixa pronto o comando `/wx-claude-code:laudo-tokens`, a auditoria em três fases que **não altera nada sem aprovação**.»

### A resposta decide a próxima

| Depois de | Se a resposta for | Então |
| --- | --- | --- |
| A | caminho informado | abra o arquivo; se não abrir, diga e pergunte de novo A antes de ir a B |
| A | «não tenho» | pergunte se existe a análise exportada ou um dump; sem nada, `missing` e siga para B |
| B, C ou D | «não tenho» | anote e siga; em **E** avise que o PDF completo vai cobrir o que faltou como `partial` |
| E | «não tenho» e B, C ou D também faltam | avise que o G0 vai dar `BLOCKED` para esses grupos e pergunte se quer seguir assim mesmo |
| F | «não» | pule paleta, tema, tipografia e densidade; vá direto a G |
| F | «sim» | faça as subperguntas de F uma por vez, na ordem F1 a F8 de `references/qualidade-erp.md` (quem opera; teclado e atalhos; grids; formulários; números, datas e moeda; impressão; estados e erros; acessibilidade), e só então preservar ou redesenhar → paleta → tema → tipografia → densidade → marca |
| F3 | grids com milhares de linhas ou edição na célula | avise que o `grid-migration-specialist` entra no G3 e que virtualização é requisito, não opção |
| F6 | há impressão em bobina ou etiqueta | registre como `RPT-*` com papel e largura; o `reports-printing-specialist` compara página a página |
| G | «não» ao corpus | pergunte de onde virá a semântica WLanguage (Help específico ou nenhuma); sem fonte, anote o risco |
| G | «sim» | pergunte a versão e se há override; rode o `--verify` só depois de fechar o questionário |
| H | usuário não sabe a linguagem | faça as quatro perguntas de sinal, uma por vez, e só então mostre as três opções (Rust, Python, C# + WL_C#) com a recomendada primeiro |
| H | linguagem escolhida | pergunte framework, banco e implantação **na mesma letra**, um item por vez |
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

   Ele cria `.wx-migration/wx-inputs.manifest.json`, `.wx-migration/conversion.config.json`, o `CLAUDE.md` do projeto (com o estilo de resposta quando **J** for sim) e o esboço `DESIGN.md` com a paleta quando **F** for sim.

3. Se **G** for sim, verifique o corpus antes de citá-lo:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" --verify
```

4. Mostre ao usuário o resumo: por letra, `provided | partial | missing | not_applicable`, e o que ficou em aberto.

## Depois

Diga a próxima ação, e só ela:

- anexos verificados → `/wx-claude-code:converter` (Gate G0, pré-flight);
- **F** sim → `/wx-claude-code:estilo-telas` pode rodar em paralelo ao inventário;
- **J** sim → `/wx-claude-code:laudo-tokens` quando o usuário quiser medir.

Não execute a conversão neste comando.
