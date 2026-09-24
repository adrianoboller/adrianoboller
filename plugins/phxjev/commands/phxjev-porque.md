---
description: Julga hipoteses de causa de um defeito com probabilidade e aponta o teste que mais separa as duas primeiras
argument-hint: "<sintoma> [hipotese A | hipotese B | ...]"
allowed-tools: Bash(python3:*)
---

## O que voce precisa saber de si antes de julgar

!`python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" juiz`

!`python3 "${CLAUDE_PLUGIN_ROOT}/scripts/phxjev.py" historico`

Onde o historico diz **REBAIXE**, suas probabilidades altas naquela pergunta
acertaram menos do que diziam: puxe-as para o meio. Onde diz **timido**, o
contrario. Com «anedota», julgue so pela evidencia. Se o juiz em vigor for
`local` ou `auto`, siga a secao 8 da skill em vez de responder voce mesmo.

Use a skill `phxjev` e aplique o preset **porque** em: $ARGUMENTS

1. **Antes de abrir arquivo**, escreva as hipoteses (no minimo duas; as do
   usuario, se vieram, e `outra`).
2. Leia o que cada hipotese preve e ponha no estado.
3. `causa` (choice): `p` por hipotese, cada uma com a evidencia que a moveu.
4. `proximo_teste` (choice): entre 2 e 4 comandos ou testes, qual **mais
   separa** as duas hipoteses do topo — o que da resultado diferente conforme
   uma ou outra seja verdade.
5. Empate pela secao 4 → rode o `proximo_teste`, acrescente ao estado e julgue
   de novo. Maximo de tres voltas; depois, sobe como empate real.

Hipotese que morreu fica no bloco com a `p` dela — e resultado, nao sobra.

Saida: JSON da secao 5 da skill passado ao `phxjev.py veredito`; mostre a saida dele sem editar.

**Nao edite arquivos**: o PhxJev julga, nao conserta. Achado para documento vai na linha `motivo` e na resposta, depois da saida do script.
