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

**F) Estilo das telas com o Impeccable.** «Deseja configurar o estilo das telas do projeto novo com o Impeccable? Se sim: qual a paleta (cores principal, secundária, fundo, texto e ação), o tema (claro, escuro, os dois), a tipografia preferida, a densidade (compacta ou espaçada) e há uma marca a respeitar (logo, manual de marca)? Prefere preservar o visual do WX ou redesenhar?» A resposta alimenta o `fidelity.ui` (`pixel`, `behavioral` ou `redesign`) e o comando `/wx-claude-code:estilo-telas`.

**G) Help completo do WX em JSON.** «O plugin traz o corpus WLanguage 12k (Help do WINDEV, WEBDEV e WINDEV Mobile em JSON, estado DEGRADED/CONDITIONAL). Deseja usá-lo como fonte técnica auxiliar? Tem um Help específico da sua versão/update para usar como override?» Se sim ao corpus, rode a verificação de hash (abaixo). Lembre: o Help é fonte de **semântica técnica**, nunca de regra de negócio.

**H) Linguagens para o Backend.** «Qual linguagem e framework para o backend? (exemplos: Rust + Axum, Go + Chi, C# + ASP.NET, Java + Spring, Python + FastAPI, Node + NestJS, PHP + Laravel, Phoenix). Qual banco de destino? Versões mínimas? Forma de implantação?»

**I) Linguagens para o Frontend.** «Qual linguagem e framework para o frontend? (exemplos: React, Vue, Svelte, Angular, Flutter, Kotlin/Swift nativo, Tauri, Blazor, HTML+HTMX). Quais plataformas: web, desktop, Android, iOS? Navegadores e dispositivos mínimos?»

**J) Economia de tokens.** «Deseja ativar e configurar a economia de tokens? Isso instala no `CLAUDE.md` do projeto a instrução de estilo de resposta (direto ao ponto, frases curtas, um assunto por parágrafo, problema em uma linha, solução em passos numerados) e deixa pronto o comando `/wx-claude-code:laudo-tokens`, a auditoria em três fases que **não altera nada sem aprovação**.»

### A resposta decide a próxima

| Depois de | Se a resposta for | Então |
| --- | --- | --- |
| A | caminho informado | abra o arquivo; se não abrir, diga e pergunte de novo A antes de ir a B |
| A | «não tenho» | pergunte se existe a análise exportada ou um dump; sem nada, `missing` e siga para B |
| B, C ou D | «não tenho» | anote e siga; em **E** avise que o PDF completo vai cobrir o que faltou como `partial` |
| E | «não tenho» e B, C ou D também faltam | avise que o G0 vai dar `BLOCKED` para esses grupos e pergunte se quer seguir assim mesmo |
| F | «não» | pule paleta, tema, tipografia e densidade; vá direto a G |
| F | «sim» | faça as subperguntas de F uma por vez: preservar ou redesenhar → paleta → tema → tipografia → densidade → marca |
| G | «não» ao corpus | pergunte de onde virá a semântica WLanguage (Help específico ou nenhuma); sem fonte, anote o risco |
| G | «sim» | pergunte a versão e se há override; rode o `--verify` só depois de fechar o questionário |
| H | linguagem informada | pergunte framework, banco e implantação **na mesma letra**, um item por vez |
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
