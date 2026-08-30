# Restore the guard with the honest note and run gates
# 30/08 04:26

import io
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
velho="  const aindaEMinha = () => true; // DEFEITO REPOSTO"
assert s.count(velho)==1
s=s.replace(velho,"  const aindaEMinha = () => minhaVez === admGeracao;",1)
# a honestidade entra no proprio comentario
s=s.replace(""" * O contador e a resposta mais simples que funciona: cada chamada pega um
 * numero, e so escreve se ainda for a ultima. Comparar o titulo, ou o `qual`,
 * nao serve -- duas chamadas seguidas do MESMO `qual` tambem se atropelam. */""",
""" * O contador e a resposta mais simples que funciona: cada chamada pega um
 * numero, e so escreve se ainda for a ultima. Comparar o titulo, ou o `qual`,
 * nao serve -- duas chamadas seguidas do MESMO `qual` tambem se atropelam.
 *
 * ATENCAO, e isto e desconforto honesto: esta guarda NAO tem prova real. A
 * sonda que escrevi (`prova-atropelo.mjs`, no rascunho) passa com a guarda E
 * passa com o defeito reposto -- ou seja, ela nao reproduz a corrida, e teste
 * que passa por engano e pior que teste que falta. A suspeita, nao
 * confirmada, e que o modo multitela ja tenha fechado esse caminho: cada tela
 * virou um elemento proprio e so a que tem foco carrega o `id="painel"`,
 * entao o `p` capturado na entrada pode nem ser mais o elemento visivel. A
 * guarda fica porque e barata e correta; o pedido continua ABERTO no
 * PENDENCIAS ate alguem escrever a sonda que de fato reproduz. */""")
io.open(p,"w",encoding="utf-8").write(s)
print("guarda restaurada, com o desconforto escrito")
