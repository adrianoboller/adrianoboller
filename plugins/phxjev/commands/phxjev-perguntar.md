---
description: Pergunta tipada livre ao juiz PhxJev (noul, choice ou score) com probabilidade e evidencia
argument-hint: "<noul|choice|score> <pergunta> [opcoes ou regua]"
allowed-tools: Bash(python3:*)
---

## O que voce precisa saber de si antes de julgar

!`python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" juiz`

!`python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" historico`

Onde o historico diz **REBAIXE**, suas probabilidades altas naquela pergunta
acertaram menos do que diziam: puxe-as para o meio. Onde diz **timido**, o
contrario. Com «anedota», julgue so pela evidencia. Se o juiz em vigor for
`local` ou `auto`, siga a secao 8 da skill em vez de responder voce mesmo.

Use a skill `phxjev` e responda como juiz tipado: $ARGUMENTS

- Sem tipo declarado: pergunta de sim/nao vira `noul`; com opcoes, `choice`;
  com regua, `score`.
- Monte o estado so com o que for lido agora; cite `arquivo:linha`.
- Saida: JSON da secao 5 da skill passado ao `phxjev.py veredito`; mostre a saida dele sem editar.

**Nao edite arquivos**: o PhxJev julga, nao conserta. Achado para documento vai na linha `motivo` e na resposta, depois da saida do script.
