---
description: Instala o Ollama e um modelo para o juiz local do PhxJev, na nuvem do Claude Code ou no computador do usuario, depois de confirmar
argument-hint: "[modelo, padrao qwen2.5:3b]"
allowed-tools: Bash(python3:*), Bash(uname:*), Bash(printenv:*)
---

Onde estamos: !`uname -s` · nuvem do Claude Code: !`printenv CLAUDE_CODE_REMOTE || echo nao`

1. **Pergunte antes, sempre**, com a ferramenta de pergunta ao usuario, dizendo:
   - onde vai instalar (nuvem desta sessao, ou o computador dele);
   - o que baixa: o Ollama e o modelo `$ARGUMENTS` (padrao `qwen2.5:3b`,
     ~1,9 GB; `1.5b` ~1 GB; `7b` ~4,7 GB);
   - na nuvem, que **some quando a sessao acabar**, e que para ficar e preciso
     pôr `bash plugins/phxjev/bancada/montar-ollama.sh` no script de
     configuracao do ambiente (menu do ambiente, Edit, Setup script);
   - que o juiz **continua sendo o Claude** ate o modelo passar na bancada.
2. Sem um sim explicito, pare.
3. Com o sim: `bash "${CLAUDE_PLUGIN_ROOT}/scripts/instalar-local.sh" $ARGUMENTS`
4. Mostre as duas ultimas linhas da saida e rode
   `python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" juiz`.

Nao mude a chave do juiz por conta propria: `phxjev.py juiz auto` e escolha
do usuario, e a bancada ainda pode recusar.
