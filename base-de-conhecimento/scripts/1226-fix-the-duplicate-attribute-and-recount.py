# Fix the duplicate attribute and recount
# 29/08 23:43

import io
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
velho = '''    <button class="tema" id="btTema" data-txt-al="tela.alternar_tema" title="Alternar tema"
            aria-label="Alternar tema claro e escuro" data-txt-al="tela.tema_dica">🌓</button>'''
# o `aria-label` ja e traduzido por `tela.tema_dica`; o `title` repetia a mesma
# coisa em portugues cravado -- e atributo duplicado nao existe em HTML.
novo = '''    <button class="tema" id="btTema"
            aria-label="Alternar tema claro e escuro" data-txt-al="tela.tema_dica">🌓</button>'''
assert s.count(velho)==1
io.open(p,"w",encoding="utf-8").write(s.replace(velho,novo))
