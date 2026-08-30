# Document the script and stage everything
# 28/08 18:17

import io
p='docs/dossie/LEIA-ME.md'
s=io.open(p,encoding='utf-8').read()
velho='''## O que conferir antes de publicar

Os números do painel são medidos, nunca estimados:

```bash'''
novo='''## O que conferir antes de publicar

**Os números do painel e do rodapé não se digitam mais.** Saem de

```bash
python3 docs/dossie/numeros-do-projeto.py
```

que mede tudo e reescreve os dois blocos entre as marcas `<!-- projeto:… -->`
e `<!-- rodape:… -->`. Ele segue **a receita abaixo, na letra** — mexeu numa,
mexa na outra, senão volta a existir número de vitrine que ninguém reproduz.
`--so-medir` mostra sem gravar; `--sem-testes` pula o `cargo test`, que demora.

A receita, para conferir à mão:

```bash'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
