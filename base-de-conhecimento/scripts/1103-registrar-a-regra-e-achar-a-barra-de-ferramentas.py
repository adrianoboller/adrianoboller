# Registrar a regra e achar a barra de ferramentas
# 29/08 11:03

import io
p='CLAUDE.md'
s=io.open(p,encoding='utf-8').read()
anc='''## Antes de commitar'''
novo='''**Toda bateria de testes tem prova real e aprendizado documentado — frutífero
ou infrutífero.** Prova real é nos dois sentidos: o teste novo tem de **falhar
com o defeito reposto** e passar com o conserto (já houve teste que passava por
engano, e ele é pior que teste que falta). O aprendizado vai para o documento
da área (`DESEMPENHO.md`, `SEGURANCA.md`…), não só para a conversa — inclusive
quando a hipótese **morre**: a recusa com o número é resultado tão válido
quanto o ganho, e é o que impede a mesma ideia de voltar sem medição. E
hipótese infrutífera não encerra a bateria: **gera a próxima hipótese**, como
na caça aos 2,3× do insert, em que cinco suspeitos caíram medidos antes de o
binário velho aparecer.

## Antes de commitar'''
assert s.count(anc)==1
io.open(p,'w',encoding='utf-8').write(s.replace(anc,novo))
print('regra registrada')
