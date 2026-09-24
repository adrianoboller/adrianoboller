---
description: Pergunta tipada livre ao juiz PhxJev (noul, choice ou score) com probabilidade e evidencia
argument-hint: "<noul|choice|score> <pergunta> [opcoes ou regua]"
---

Use a skill `phxjev` e responda como juiz tipado: $ARGUMENTS

- Sem tipo declarado: pergunta de sim/nao vira `noul`; com opcoes, `choice`;
  com regua, `score`.
- Monte o estado so com o que for lido agora; cite `arquivo:linha`.
- Saida: JSON da secao 5 da skill passado ao `phxjev.py veredito`; mostre a saida dele sem editar.
