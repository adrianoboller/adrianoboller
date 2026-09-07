# O medidor que inventa nome de operação publica ausência que não existe

**07/09/2026, 05:10** — descoberto ao medir as atividades de gestão do banco
para responder «liste os recursos com ok, parcial e planejado».

## 1. O que aconteceu

A primeira corrida da `bancada/gestao/` publicou esta tabela:

```
INSERT em lote        planejado   lote recusado ou contagem errada: None
SELECT varrer/buscar  planejado   varrer={'registros': 1, ...}
DELETE suave          planejado   rowid 2 fora da faixa
DELETE de vez         planejado   operacao desconhecida
Backup                planejado   informe "destino"
```

**Cinco «planejado», e os cinco eram defeito meu.** O motor faz as cinco
coisas. O que estava errado era o medidor:

| eu chamei | o que existe |
|---|---|
| `contar` | não existe — a contagem vem do `registros` do `varrer` |
| `excluir_de_vez` | não existe — é `excluir` com `fisico: true` |
| `bulkinsert` | existe, mas é o **modo de carga**; inserir muitas é `inserir_lote` |
| `inserir` com `linha` | o contrato pede `valores` |
| `backup` sem `destino` | `destino` é obrigatório |

## 2. O que eu concluí primeiro, e estava errado

**Primeiro:** li a tabela e comecei a escrever o texto do documento em cima
dela — «a camada de lote está planejada», «o excluir de vez não existe». Era
tudo falso, e nada na saída dizia isso: um `planejado` com uma mensagem de
erro do lado tem exatamente a cara de uma lacuna do produto.

**Segundo, e mais grave:** eu já tinha escrito, dois documentos atrás, que
*recusa não lida vira ausência publicada*. Escrevi a lei e repeti o erro na
bancada seguinte. A lei não estava errada — estava **sem portão**, e lei sem
portão é conselho.

**Terceiro:** ao corrigir, afirmei que o `excluir` suave manda a linha para a
lixeira. É o contrário, e o código diz por quê — `excluir_suave` só marca e
registra o motivo; a linha continua inteira no `.reg`, então não há o que
preservar. **A lixeira é a rede do excluir que DESTRÓI**, e ele copia até o
`.bin` e o `.memo` antes, porque a própria exclusão vai liberar esses blocos.
Minha asserção estava invertida e teria publicado a garantia ao contrário.

## 3. O que a medição disse

- **5** vereditos falsos na primeira corrida; **0** na corrida com o portão.
- **15** operações agora conferidas contra o catálogo antes de qualquer medida.
- O portão reprovou **3** declarações minhas na estreia, e uma delas era real:
  a operação `sql` documenta o parâmetro **`texto`**, e `sql` é só um apelido.
- **12 ok · 0 parcial · 3 planejado** é o número verdadeiro. Os três são
  `INSERT`, `UPDATE` e `DELETE` pela camada SQL — e esses o motor recusa com
  uma frase que diz exatamente isso.

## 4. A regra

**Pergunte o contrato antes de medir, e pare quando não bater.** O servidor
publica o próprio catálogo: nome, parâmetros e quais são obrigatórios. Um
medidor que chama uma operação sem conferir isso não está medindo o motor —
está medindo a minha lembrança do motor, e publicando a diferença como se
fosse ausência do produto.

E o corolário sobre a forma da parada: o portão **não** devolve «planejado»
quando o contrato não bate. Ele **para**, com o nome do que está errado. Um
medidor que degrada para «não tem» é o mesmo defeito com outra roupa.

## 5. Como está guardado hoje

- `bancada/gestao/medir.py`, `CONTRATO` e `conferir_contrato()`: a lista do que
  eu chamo, conferida contra `{"op":"catalogo","operacao":…}` do servidor. Duas
  reprovações param a corrida — operação que o catálogo não conhece, e
  obrigatório que eu não mando.
- O controle do leitor (`grava 42, lê 42`) continua antes de tudo, como na
  `bancada/comparativo/`.
- `docs/GESTAO.md` §1 conta a história para quem for mexer.

**O buraco que fica:** o portão confere **nome e obrigatórios**, não o
**significado**. Ele teria deixado passar o `bulkinsert` se eu tivesse mandado
os parâmetros certos dele — porque a operação existe, só não faz o que eu
achava. Contra isso só há o que salvou aqui: a sonda medir o **efeito**, e o
efeito não bater.
