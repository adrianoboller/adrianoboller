# O gerador estava certo, e o número mesmo assim envelheceu

**07/09/2026, 08:46 UTC.**

## 1. O que aconteceu

Três painéis do dossiê estavam parados, e **nenhum deles tinha sido digitado
por ninguém** — todos os três saem de gerador, como a lei desta casa manda:

| painel | dizia | era | atraso |
|---|---:|---:|---|
| pedidos, ao todo | 198 | **203** | cinco pedidos |
| testes na maior área | 428 | **451** | uma rodada de testes |
| replicação, linhas/s no master | 26.762 | **37.810** | um retrato inteiro |

A causa é uma só, e é de **chamada**, não de conta. O
`pagina-dos-pedidos.py` aceita dois alvos — a página e o dossiê — e o do
dossiê **só existia se viesse por argumento**:

```python
dossies = [a for a in argumentos if "dossie" in pathlib.Path(a).name]
saidas  = [a for a in argumentos if a not in dossies]
saida   = pathlib.Path(saidas[0]).resolve() if saidas else PADRAO   # <- tem padrao
for d in dossies:                                                   # <- nao tem
    gravar_no_dossie(d, itens)
```

Chamado nu, ele gravava a página, gravava a contagem de volta no
`PENDENCIAS.md`, imprimia **três linhas de sucesso** e pulava o dossiê. Nada na
saída dizia que faltou metade. E eu o chamei assim, porque a saída não me deu
motivo para desconfiar.

O irmão era o `cobertura-por-area.py`: mesmo laço sobre `sys.argv`, mesma falta
de padrão. Outros quatro tinham padrão, mas era o **nome digitado**
(`dossie-phxsql-0.18.html`), em sete lugares — padrão que morre na próxima
refação, quando o dossiê virar `-0.19`. E o `tetos-da-trava.py` era o único que
**exigia** argumento: fazer diferente dos oito irmãos é a armadilha, porque
quem repete a receita nua deixa aquele bloco para trás.

## 2. O que eu concluí primeiro, e estava errado

Que a lei «todo número visível sai de um gerador» já cobria isto, e que um
painel atrasado só poderia vir de número digitado. Fui procurar o dígito na
mão. **Não havia nenhum.**

O erro é sobre o **alcance** da lei, e é o que este arquivo existe para
registrar: a lei prova a *origem* do número e não prova a *chegada* dele. Um
gerador certo, com a conta certa, lendo a fonte certa, entrega número velho
quando é chamado pela metade — e entrega **anunciando sucesso**, que é o que
faz ninguém olhar de novo. É o mesmo padrão do «comentário que se declara
resolvido»: a saída dizia `pagina gravada`, e gravada ela estava.

## 3. O que a medição disse

- **7** padrões com o nome do dossiê digitado, em 7 arquivos; mais **5** em
  linhas de uso. Zero depois do conserto.
- **2** geradores sem padrão nenhum para o dossiê (`pagina-dos-pedidos.py`,
  `cobertura-por-area.py`) — os dois que produziram os painéis parados.
- **1** gerador que exigia argumento (`tetos-da-trava.py`), diferente dos
  outros oito.
- Prova real nos dois sentidos, com o defeito reposto: com o painel forçado
  para `198` e a chamada **nua**, antes do conserto o script imprimia 3 linhas
  e o painel ficava em 198; depois do conserto imprime
  `painel dos pedidos regravado` e o painel volta a **204**.
- Portão medido pelo código de saída, e não pelo texto: dois dossiês na pasta
  → **saída 1**; um dossiê → **saída 0**. (A primeira medição disse `0` para os
  dois casos, porque eu li o `$?` **depois de um cano** — o status era o do
  `tail`. *Cano mente sobre código de saída.*)

## 4. A regra

**Gerador que faz menos do que o nome dele promete tem de dizer que fez menos
— e o padrão dele nunca é um nome digitado.**

Ou, do lado de quem lê: número que sai de gerador ainda envelhece se o gerador
for chamado pela metade. A lei cobre a origem; a chegada é outra prova.

## 5. Como está guardado hoje

- `docs/dossie/dossie_da_pasta.py` — **um dono só** para achar o dossiê, por
  varredura de `dossie-phxsql-*.html`. A pétrea «só existe um por vez» virou o
  portão: zero é parada com o motivo, dois é parada com os dois nomes, nunca um
  palpite sobre qual atualizar.
- Os **nove** geradores importam esse dono. Chamada nua alcança o dossiê em
  todos, e a receita do `LEIA-ME.md` perdeu o nome do arquivo — trocar o nome
  na próxima refação deixou de exigir editar script nenhum, que era o que o
  `CLAUDE.md` já prometia e não era inteiramente verdade.
- **Onde o buraco fica:** não há guarda que reprove um gerador novo nascido sem
  padrão. O conferidor genérico seria um casador de `sys.argv`, e casador de
  texto é justamente o que esta casa já recusou com número noutra frente. Por
  ora o que protege é a receita nua no `LEIA-ME.md`: quem a repete exercita os
  nove sem argumento, e um gerador que não alcance o dossiê aparece como painel
  que não mexeu.
