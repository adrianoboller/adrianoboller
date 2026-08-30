# Apply the whole guard properly this time
# 30/08 04:22

import io
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()

# desfaz o pedaco solto que sobrou da edicao abortada
solto = """    // Recado de erro tambem nao pinta por cima da tela seguinte.
    if (!aindaEMinha()) return;
"""
assert s.count(solto)==1
s=s.replace(solto,"")

velho = """async function abrirAdmin(qual) {
  est.atual = null;"""
novo = """/* Quem chamou por ultimo manda no painel.
 *
 * Todo ramo daqui faz `p.innerHTML = await ...`, e o `await` e onde a pessoa
 * clica noutra coisa. Sem esta guarda, clicar em Configuracoes antes de o
 * Painel terminar de carregar deixava **titulo de uma tela e corpo da outra**
 * -- a tela mentindo sobre si mesma. A bateria de guardas mediu: o `#painel`
 * caia de 31.092 para 13.818 caracteres 2,5 s depois de a tela ja ter trocado.
 *
 * O contador e a resposta mais simples que funciona: cada chamada pega um
 * numero, e so escreve se ainda for a ultima. Comparar o titulo, ou o `qual`,
 * nao serve -- duas chamadas seguidas do MESMO `qual` tambem se atropelam. */
let admGeracao = 0;

async function abrirAdmin(qual) {
  const minhaVez = ++admGeracao;
  const aindaEMinha = () => minhaVez === admGeracao;
  est.atual = null;"""
assert s.count(velho)==1
s=s.replace(velho,novo)

trocas = [
 ("      p.innerHTML = await vPainel();\n      ligarMonitor();",
  "      const htmlPainel = await vPainel();\n      if (!aindaEMinha()) return;\n      p.innerHTML = htmlPainel;\n      ligarMonitor();"),
 ("      const us = await api(\"usuarios\");\n      p.innerHTML = tabela(",
  "      const us = await api(\"usuarios\");\n      if (!aindaEMinha()) return;\n      p.innerHTML = tabela("),
 ("      const a = await api(\"acessos\", { max:200 });\n      p.innerHTML = ",
  "      const a = await api(\"acessos\", { max:200 });\n      if (!aindaEMinha()) return;\n      p.innerHTML = "),
 ("      p.innerHTML = await vIdiomas();\n      ligarIdiomasAdmin();",
  "      const htmlIdiomas = await vIdiomas();\n      if (!aindaEMinha()) return;\n      p.innerHTML = htmlIdiomas;\n      ligarIdiomasAdmin();"),
 ("      const b = await api(\"bloqueios\");",
  "      const b = await api(\"bloqueios\");\n      if (!aindaEMinha()) return;"),
]
for a,b in trocas:
    assert s.count(a)==1, a[:70]
    s=s.replace(a,b)

io.open(p,"w",encoding="utf-8").write(s)
print("guardas aplicadas:", s.count("aindaEMinha()"))
