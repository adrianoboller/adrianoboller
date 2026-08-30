# Repoint and remove the superseded screen
# 28/08 16:40

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''    { g:"operacao", ico:"⇋", rot:"Conexões",
      diz:"Quem está falando com este servidor agora.",
      faz:verConexoes },'''
b='''    { g:"operacao", ico:"⇋", rot:"Sessões e conexões",
      diz:"Quem está falando agora, o que está executando, e como derrubar.",
      faz:verSessoes },
    { g:"operacao", ico:"◷", rot:"Estatísticas de uso",
      diz:"A cauda da latência, o que custa caro e por qual tabela.",
      faz:() => verEstatisticas() },'''
assert a in s; s=s.replace(a,b,1)

# a tela antiga sai: o que ela mostrava esta no painel e na tela nova, e
# funcao morta em arquivo grande e o que confunde quem procura de onde vem a tela.
import re
i = s.index('async function verConexoes() {')
j = s.index('\nasync function verReplicacao()', i)
s = s[:i] + s[j+1:]
open(p,'w').write(s)
print('ok')
