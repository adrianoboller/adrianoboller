# A guarda de «tela corrente» existia desde o pedido 170 e só protegia quem a chamava

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 636. O «Criar» do cartão de nova tabela (`index.html`) fazia
`await api("criar_tabela"); await montarArvore(false); telaDiagramaER(db)`, e o
diagrama pintava por cima da tela que a pessoa abrira no meio-tempo. A bateria
web reprovou duas vezes por isso; o caso 34 passou a esperar `ER.esquemas` ter a
tabela, o que escondia o defeito. Reproduzido segurando a resposta de
`criar_tabela` no fio e abrindo a Telemetria: título «Diagrama ER», corpo do
diagrama, Telemetria coberta.

## 2. O que eu concluí primeiro, e estava errado

Que o defeito era do «Criar» e que um `if` no repintar dele bastava. Não bastava:
`telaDiagramaER` anuncia «lendo o esquema», espera o laço de `esquema` e pinta o
corpo final por `folha()`, que **sempre toma a posse** — então o «Redesenhar» e
qualquer chamada a `telaDiagramaER` tinham o mesmo furo. E o furo não era do
diagrama: a guarda do 170 (`tomarPainel`/`aindaNoPainel`) só era conferida por
`abrirAdmin` e `desenharAba`. Varredura do fonte: **83 chamadas a `folha()`
depois de um `await`, em 59 funções**, quase todas sem conferência. A frase do
próprio comentário da guarda — «espalhar a conferência por cinquenta funções
deixaria a esquecida virar a porta dos fundos» — descrevia o *toma*, não o
*confere*: o motor não pode conferir por quem, porque a `folha()` do fim é
indistinguível de uma tela nova pedida agora.

Segundo erro, do conserto: fiz `folha()` devolver a posse e a bateria cheia
reprovou `botoes-do-pivot` (2 de 73, os dois temas). O `pv_json` era
`folha(...) || ($("#pv_volta3").onclick = ...)` — contava com o `undefined`
que `folha` sempre devolveu, e com um número verdadeiro o `||` curto-circuitou:
o «← Resultado» nascia sem `onclick`. **Mudar o que uma função devolve muda os
consumidores dela**, e o caso isolado da frente (38) e o 34 não passam por ali;
só a bateria inteira achou. Varredura de uso do retorno (parênteses casados
sobre os 116 usos): 1 consumidor real.

## 3. O que a medição disse

- Caso `38-pintura-tardia.mjs` sobre o fonte sem conserto: FALHOU nos dois
  temas, «esperava "Telemetria", achei "Diagrama ER"».
- Com o conserto: 2 de 2; caso 34 sem a espera, 5 corridas, 10 de 10.
- `prova-real-botoes.mjs --so pintura`: controle verde, **8 de 8** defeitos
  pegos. O primeiro patch de `telaDbLink` **passou**: mirava o ramo «com
  ligações» e a base do caso não tem ligação — a prova real da prova achou a
  prova errada.

## 4. A regra

Guarda de «ainda estou na vez» que mora no motor só vale para quem a **toma**;
a **conferência** tem de estar em quem pinta depois do `await` — e, ao
consertar uma, varrer **todas** as que pintam depois de `await`, não só a do
sintoma (conserto entra no caminho que o motivou, e o irmão também).

## 5. Como está guardado hoje

Caso web `38-pintura-tardia` + 8 defeitos na `prova-real-botoes.mjs`. **Buraco:**
a convenção é por tela — nenhuma catraca estática acusa a tela nova que
esqueça de conferir (o catálogo de guardas prova testes Rust, não navegador).
A varredura de texto que achou os 83 virou `testes-web/varrer-pintura-tardia.py` (0 achados hoje), mas não está ligada às catracas — decisão do papel G.
