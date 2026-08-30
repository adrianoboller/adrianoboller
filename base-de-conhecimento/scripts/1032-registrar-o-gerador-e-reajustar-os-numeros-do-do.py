# Registrar o gerador e reajustar os numeros do dossie
# 29/08 03:20

import io
p='docs/dossie/LEIA-ME.md'
s=io.open(p,encoding='utf-8').read()
anc="Duas armadilhas de estilo da página:"
novo = """## A outra página: os 129 pedidos

`pedidos.html` é a relação de tudo que o Adriano pediu, com o estado de cada
item. Ela **não se edita** — sai de

```bash
python3 docs/dossie/pagina-dos-pedidos.py [saida.html]
```

que lê `docs/PENDENCIAS.md` e conta os três estados sozinho. A fonte da verdade
é o `.md`; mexeu lá, rode isto. Uma lista de 129 linhas com três contadores
mantida à mão estaria errada no dia seguinte — é a mesma razão do selo.

Duas coisas que só apareceram abrindo no navegador, e ficam registradas para
quem mexer:

- **`thead` grudento dentro de `overflow-x:auto` cai por cima da primeira
  linha.** O `.rolo` vira contexto de rolagem próprio, e o `position:sticky`
  passa a se medir por ele. Quem gruda é a barra de filtro.
- **Busca em português tem de achatar acento.** Sem `normalize('NFD')`, quem
  digita «indice» não acha «índice» — e a busca falha calada.

Duas armadilhas de estilo da página:"""
assert s.count(anc)==1
io.open(p,'w',encoding='utf-8').write(s.replace(anc,novo))
print('ok')
