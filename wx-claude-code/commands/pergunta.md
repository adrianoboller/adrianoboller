---
description: "Faz UMA pergunta do questionario pelo id (0.16, F9, K7, L6, M, H...) e grava so ela no questionario.json, sem repetir o resto."
argument-hint: "<id> [raiz-do-projeto]"
allowed-tools: "Read, Glob, Grep, Bash, Write, AskUserQuestion"
---

# Uma pergunta do questionário, pelo id

`$1` é o id da pergunta; `$2` é a raiz do projeto (padrão: o diretório atual).

Serve para **voltar a um item** sem refazer o questionário inteiro: o cliente mudou o prazo, chegou o logotipo, o aprovador trocou, a linguagem de destino mudou de ideia.

## O que fazer

1. **Ache a pergunta.** Se `$1` estiver vazio, liste tudo e pare:

   ```bash
   python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/listar_perguntas.py"
   ```

   Com id, confirme que existe e pegue o caminho no JSON:

   ```bash
   python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/listar_perguntas.py" --id "$1" --json
   ```

   Id que não existe: diga isso, mostre os parecidos, e **pare** — não adivinhe qual o usuário quis.

2. **Mostre o que já está gravado** naquele caminho de `<raiz>/.wx-migration/questionario.json`, se houver. Resposta antiga que ninguém lembra é a causa de perguntar duas vezes a mesma coisa.

3. **Pergunte**, seguindo as regras da letra em `/wx-claude-code:questionario`: um item por vez, a resposta decide a próxima, sugestão adequada antes da pergunta aberta. As regras de segurança valem inteiras — **senha, token ou certificado nunca são gravados nem repetidos**, nem parcialmente, nem mascarados: grava-se o **nome da variável**.

4. **Grave só aquele caminho.** Leia o JSON, altere o ramo, escreva de volta com `indent=2` e `ensure_ascii=False`. Não toque em nada fora do caminho: o resto do questionário é de outra conversa.

5. **Reaplique**, para que os arquivos gerados reflitam a resposta nova:

   ```bash
   python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/aplicar_questionario.py" \
     --questionario <raiz>/.wx-migration/questionario.json --project-root <raiz> \
     --plugin-root "${CLAUDE_PLUGIN_ROOT}"
   ```

   O que já existe não é sobrescrito; `INDEX_FILES.md` e `respostas_questionario.md` são regravados, porque são renderização do JSON.

6. **Diga o que mudou**: o caminho, o valor anterior e o novo, e quais arquivos gerados mudaram. Se a mudança invalida decisão já tomada (trocar a linguagem de destino depois do G3, por exemplo), diga isso e proponha uma `DEC-*`.

Artefatos (bloco M) não se gravam por aqui: use `/wx-claude-code:artefato`, que confere segredo e calcula o hash.
