# Update CHANGELOG and TELEMETRIA.md
# 29/08 19:10

import io
p="CHANGELOG.md"
s=io.open(p,encoding="utf-8").read()
velho = """  bytes do disco são preservados; o teste fabrica um arquivo antigo de verdade.

### Adicionado"""
novo = """  bytes do disco são preservados; o teste fabrica um arquivo antigo de verdade.
- **A tela de telemetria existia e ninguém a achava.** O pedido chegou como
  «falta o botão do SQL Check» com o botão no ar havia semanas — no terceiro
  grupo da barra, entre coisas que se fazem uma vez por mês, e sem aparecer em
  menu nenhum, embora o menu *Ferramentas* se anuncie como «a mesma lista pelo
  teclado». Telemetria e Profiler subiram para junto de *Conexões* (as três
  respondem à mesma pergunta: o que está acontecendo agora), entraram no menu
  *Ferramentas*, e a referência que o Adriano usa para nomear a tela passou a
  aparecer nela: o balão do botão e o subtítulo dizem **«no molde do SQL Check
  da Idera(R)»**. O nome de fábrica continua *Telemetria* — a marca é da Idera,
  e a casa cita marca de terceiro, não a adota; quem quiser outro rótulo troca
  no *Editor de menu*. Lugar errado na barra é o mesmo que não existir.

### Adicionado"""
assert s.count(velho)==1
io.open(p,"w",encoding="utf-8").write(s.replace(velho,novo))

p2="docs/TELEMETRIA.md"
t=io.open(p2,encoding="utf-8").read()
v2 = """estado, e clicar abre o descritivo inteiro. A tela é `Telemetria`, no menu
lateral, ao lado do Profiler."""
n2 = """estado, e clicar abre o descritivo inteiro.

**Onde ela fica:** o botão `Telemetria` na barra de ferramentas, logo depois de
*Conexões* — e também em **Ferramentas → Telemetria ao vivo…**, pelo teclado. O
Profiler é o vizinho, nos dois caminhos: as três respondem à mesma pergunta —
o que está acontecendo agora."""
assert t.count(v2)==1, t.count(v2)
io.open(p2,"w",encoding="utf-8").write(t.replace(v2,n2))
print("ok")
