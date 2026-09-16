# Importar o gerador irmão para não duplicar a receita pode RODÁ-LO

Descoberto em 16/09/2026, 13:35, montando a sétima página
(`docs/status/pagina-do-status-do-projeto.py`).

## 1. O que aconteceu

A regra desta casa para um gerador novo é clara e eu a segui: *receita
duplicada é receita que diverge* — então não se escreve um segundo leitor do
`PENDENCIAS.md`, importa-se o `ler()` de quem já lê. O molde é o
`docs/planilha/planilha-das-atividades.py`, que importa três leitores e não
tem nenhum próprio.

A sétima página precisa da cobertura por área, e o leitor dela existe:
`medir()`, em `docs/dossie/cobertura-por-area.py`. Importei o módulo.

E o módulo **regravou `docs/TESTES.md` e o dossiê** na importação, sem ninguém
pedir. A última linha do arquivo era:

```python
main()
```

solta, fora de qualquer `if __name__ == "__main__":`. Quem roda o script como
script não vê diferença nenhuma. Quem o **importa** dispara o `main()` inteiro,
que varre os crates, monta o bloco e escreve em dois arquivos versionados.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o problema era o **tempo**: o import demorou, e a minha primeira
explicação foi «o `medir()` varre todos os crates, é caro, vou cachear». Isso
é verdade e é irrelevante — a varredura é o trabalho que eu queria. O que
estava acontecendo por baixo é que o import fazia **muito mais** que a
varredura: fazia a escrita.

O erro só apareceu porque conferi `git status` logo depois e vi dois arquivos
modificados que eu não tinha tocado. Se a árvore já estivesse suja — e estava,
o `CAPABILITIES.json` diz `"sujo": true` —, a modificação teria passado
despercebida, e o portão dos geradores teria ficado verde por acidente: os
arquivos estariam atualizados, só que por um efeito colateral que ninguém
declarou.

## 3. O que a medição disse

- **1 gerador dos 18 do portão** tinha `main()` solto: `cobertura-por-area.py`.
  Conferido nos outros: `pagina-dos-testes.py`, `graficos-dos-testes.py`,
  `tecnologias/extrair.py`, `fluxo-do-motor.py`, `numeros-do-projeto.py`,
  `pagina-dos-pedidos.py`, `rollup.py` e os dois painéis — todos com guarda.
- **2 arquivos versionados** eram reescritos por importação: `docs/TESTES.md`
  e o dossiê (bloco `cobertura:inicio`/`fim`).
- O conserto é **uma linha** e não muda nada para quem o roda como script:
  o comportamento de `python3 docs/dossie/cobertura-por-area.py` é idêntico
  antes e depois, e o portão dos geradores continua verde nesse gerador.

## 4. A regra

**Antes de importar um gerador para reusar a receita dele, confira que o
`main()` está sob guarda — e ponha a guarda quando não estiver.** Módulo que
escreve na importação transforma «reusar a receita» em «rodar o outro gerador
por engano», e o estrago é invisível numa árvore suja.

## 5. Como está guardado hoje

- O `main()` de `docs/dossie/cobertura-por-area.py` está sob
  `if __name__ == "__main__":`, com o comentário dizendo **por quê** (o
  comentário explica o motivo, não o que a linha faz).
- **Onde o buraco ficou:** não há conferidor que reprove um `main()` solto num
  gerador do `PLANO` do portão. Achei este olhando; o próximo aparece do mesmo
  jeito. Um crivo seria barato — importar cada gerador do `PLANO` num processo
  filho, com a árvore limpa, e reprovar quem sujar —, mas ele **não existe** e
  esta linha diz isso em vez de fingir cobertura.
