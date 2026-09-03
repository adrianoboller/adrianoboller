# A sonda mede o presente; o texto que ela gerou mede o passado

**03/09/2026, 09:40** — descoberto rodando as duas sondas do pedido 171 antes
de tocar em qualquer coisa.

## 1. O que aconteceu

O pedido 171 do `PENDENCIAS.md` trazia **quatro buracos medidos por sonda**, e
as sondas continuavam na árvore e rodando: `--example sonda-fk-buracos` e
`--example sonda-replica-fk`. O texto do pedido era detalhado — dizia
exatamente o que cada sonda tinha impresso, com valores.

Rodei as duas antes de mexer em nada. **Três dos quatro itens ainda batiam. O
quarto tinha mudado de forma**, e para pior:

| o que o texto dizia | o que a sonda imprimiu hoje |
|---|---|
| «a réplica diverge na ordem **entrelaçada**: filha em `Int(1)` na réplica contra `Int(2)` no source» | a ordem entrelaçada **convergia**; quem divergia eram as ordens «mãe primeiro» e «filha primeiro», e não por valor — a filha **nem existia**: `pedidos` com **0 de 2** eventos |
| «as ordens A e B fecham iguais» | as ordens A e B eram as que perdiam a linha inteira |

O texto não estava errado quando foi escrito. Entre ele e hoje, outra frente
ligou *chave declarada nasce conferida* — e o portão que ela ligou alcançou o
caminho da réplica, que ninguém tinha listado.

## 2. O que eu concluí primeiro, e estava errado

Concluí, lendo o pedido, que o trabalho era **fechar quatro buracos conhecidos**
e que a parte difícil seria a ordem entrelaçada, «a mais urgente». Ia começar
por reproduzir aquele cenário específico.

Errado em três níveis, e cada um mais caro que o anterior:

* eram **cinco** buracos, não quatro — o bidirecional não estava na lista e é o
  único cujo efeito é o **par de servidores parado**, e não uma linha errada;
* o buraco «mais urgente» tinha **mudado de causa**: o texto descrevia uma
  divergência de valor, e o defeito de hoje era recusa e perda;
* e o pior: eu teria «consertado» a ordem entrelaçada e visto o sintoma sumir,
  porque o conserto certo apaga os três de uma vez. **O errado sobrevive melhor
  quando o conserto funcionou por outro motivo** — e aqui o conserto teria
  funcionado.

## 3. O que a medição disse

Rodar as duas sondas custou **duas compilações e menos de dois minutos**. O que
elas devolveram, contra o que estava escrito:

* item 1 (filha de mãe morta): igual — `ACEITOU (rowid 1)`;
* item 2 (mãe com filha só suave): igual — recusa nos dois modos;
* item 3 (`excluir_tabela`): igual — `ACEITOU e apagou 8 arquivo(s)`;
* item 4 (réplica): **diferente**. Ordem A: `pedidos 0 eventos (source 2)`.
  Ordem B: `pedidos 0 eventos (source 2)`. Ordem C: convergiu em `Int(2)`, com
  «a cascata rodou DE NOVO na réplica e gerou 1 evento que o source não mandou».

Três de quatro sobreviveram um dia; um não. **75% de validade em 24 horas** é a
medida de quanto vale um achado transcrito.

## 4. A regra

**Rode a sonda antes de ler o que ela disse ontem.** Achado transcrito é
matéria-prima; a ferramenta que o produziu é o achado.

E o corolário, para quem escreve o item: **um pedido que carrega o comando que o
mede vale mais que um pedido que carrega o número.** O pedido 171 fez isso
certo — trazia os dois `cargo run` no fim do texto —, e foi só por isso que a
correção do próprio texto ficou barata.

## 5. Como está guardado hoje

* As duas sondas continuam versionadas em `crates/phxsql-store/examples/`, e o
  `docs/INTEGRIDADE.md` abre com o bloco `bash` que roda as quatro ferramentas
  da área — sonda, sonda, medidor e verificador.
* O `PENDENCIAS.md` do 171 passou a dizer, com todas as letras, que o item (4)
  foi **remedido e tinha três causas**, e que havia um quinto que a lista não
  tinha. A correção do texto ficou no próprio item, e não numa nota de rodapé:
  quem o ler daqui a seis meses lê a versão medida.
* **Onde o buraco ficou:** nada obriga um item do `PENDENCIAS.md` a carregar o
  comando que o mede. O 171 carregava por virtude de quem o escreveu, não por
  regra. Um conferidor que reprovasse item de estado medido sem comando de
  medição é possível e não existe.
