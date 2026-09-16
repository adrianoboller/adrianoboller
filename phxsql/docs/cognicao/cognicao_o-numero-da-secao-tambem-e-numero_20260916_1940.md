# O número da seção também é número visível — e o meu envelheceu em minutos

- **Assunto:** as duas seções do pedido 264 entraram **no meio** da sétima
  página, e a numeração dela era digitada à mão em 24 lugares.
- **Descoberto em:** 16/09/2026, 19:40 (papel H, frente 264).
- **Onde:** `docs/status/pagina-do-status-do-projeto.py`,
  `docs/status/riscos.py`.

## 1. O que aconteceu

A sétima página tinha 21 seções, e cada função de seção trazia o **próprio
número escrito** no cabeçalho:

```python
h = ['<h2 id="s11"><span class="n">11 ·</span>As catracas</h2>', …]
```

Mais duas referências em prosa: `§01` no texto dos crates e `§21` no rodapé e
na saída do comando. Somando os cabeçalhos repetidos nos ramos de saída
antecipada, **24 lugares**.

O pedido 264 mandou nascer *Riscos* e *Dívida técnica*. O lugar editorial
delas é depois de «O que está travado com você» (§08) — isto é, **no meio**.
Pô-las ali renumera treze seções e as duas referências em prosa.

## 2. O que eu concluí primeiro, e estava errado

Concluí que isto era só uma inconveniência de edição: *«são treze cabeçalhos,
troco os treze e pronto»*. E, em cima disso, escrevi na seção nova uma frase
com o número do vizinho digitado dentro:

```python
'uma marca — ao contrário das catracas do §11, que só descem. '
```

O raciocínio parecia seguro porque eu **acabara de ler** que as catracas eram a
§11. Estava errado no mesmo minuto em que escrevi: as minhas duas seções
entravam **antes** delas, e as catracas passaram a ser a §13. A lei da casa
diz «todo número visível sai de um gerador» — e eu tinha lido «número» como
«medida», nunca como «o número da seção ao lado».

## 3. O que a medição disse

- **24** lugares com número de seção digitado no gerador da página, antes desta
  frente — contados pela expressão `id="s\d+"` no fonte, não de memória.
- **13 seções** teriam envelhecido de uma vez com a inserção no meio — **14
  cabeçalhos**, porque a Replicação escreve o dela em dois ramos (com e sem
  resultado de bancada), e mais as duas referências em prosa.
- O meu `§11` envelheceu em **minutos**, dentro da mesma rodada e do mesmo
  arquivo em que foi escrito: não precisou de um lançamento, nem de uma
  semana. Foi o caso mais curto de envelhecimento medido nesta casa — o selo da
  capa levou quatro lançamentos.
- Depois do conserto: **zero** números de seção digitados. O `ORDEM` (uma lista
  de chaves) é a única coisa que numera, e a página passou a ter **23** seções
  sem ninguém recontar nada.

## 4. A regra

**Número de seção é número visível: tire-o de uma lista de chaves, nunca do
cabeçalho.** Quem escreve a seção usa `H2["chave"]`; quem cita a seção usa
`ref("chave")`; e o gerador **para** se a quantidade de seções montadas não
bater com a de chaves — seção sem chave, ou chave sem seção, desloca todas as
outras em silêncio.

## 5. Como está guardado hoje

- `ORDEM` + `H2` + `ref()` em `docs/status/pagina-do-status-do-projeto.py`, com
  o motivo escrito ao lado; a conferência `len(secoes) != len(ORDEM)` mata a
  geração em vez de publicar deslocado.
- `docs/status/riscos.py` recebe o módulo da página e chama `P.h2(...)` e
  `P.ref(...)`: uma seção montada **fora** do arquivo da página também não sabe
  em que posição caiu — e não precisa saber.
- O índice do `docs/status/LEIA-ME.md` continua sendo **tabela escrita à mão**,
  e é aí que o buraco ficou: se alguém mover uma chave na `ORDEM`, a página
  renumera sozinha e aquela tabela fica velha. Ela não publica número medido
  (é um índice de leitura), mas envelhece do mesmo jeito. Dito aqui em vez de
  escondido — o gerador que a escreveria não existe.
- A mesma lição já tinha sido paga pelo dossiê, com a numeração das figuras: no
  dia em que duas figuras entraram no meio do documento, dezesseis legendas
  envelheceram juntas. **O alcance que faltava era «a numeração de SEÇÃO é da
  mesma família que a de FIGURA»** — e é só isso que este arquivo acrescenta à
  lei que já existia.
