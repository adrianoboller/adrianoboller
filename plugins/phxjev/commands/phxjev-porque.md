---
description: Julga hipoteses de causa de um defeito com probabilidade e aponta o teste que mais separa as duas primeiras
argument-hint: "<sintoma> [hipotese A | hipotese B | ...]"
---

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
