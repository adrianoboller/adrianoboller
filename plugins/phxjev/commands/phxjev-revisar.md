---
description: Tria achados de revisao com probabilidade por pergunta e limiar fixo (manter ☐, ⏸ depois da versao, descartar)
argument-hint: "[arquivo de achados, PR ou diff]"
---

Use a skill `phxjev` e aplique o preset **revisar** em: $ARGUMENTS

Se nao vier alvo, os achados sao os da ultima revisao nesta conversa.

Para cada achado:

1. Leia o codigo que ele cita e ponha os trechos no estado (`arquivo:linha`).
2. Responda `real`, `alcancavel`, `ja_tratado`, `defeito_ativo` (noul) e
   `severidade` (score 0 cosmetico · 1 menor · 2 dado errado ou garantia que
   nao vale · 3 seguranca, perda de dado ou travamento).
3. `defeito_ativo` segue a decisao do dono de 24/09/2026: dado errado,
   seguranca, travamento, garantia que nao vale e teste que floca sao ativos.
4. Aplique os limiares da secao 4 da skill, na ordem da tabela; o primeiro
   `descartar` encerra o achado.

Saida: JSON da secao 5 da skill passado ao `phxjev.py veredito`; mostre a saida dele sem editar.

**Nao edite arquivos**: o PhxJev julga, nao conserta. Achado para documento vai na linha `motivo` e na resposta, depois da saida do script.
