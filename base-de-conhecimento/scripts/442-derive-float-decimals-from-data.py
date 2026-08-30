# Derive float decimals from data
# 28/08 14:55

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''/// FLOAT e DOUBLE chegam com 31, o «não fixo» do protocolo: aí a casa decimal
/// vem do próprio valor, porque arredondar às cegas mentiria nos dois
/// sentidos.
function casasDoExterno(c) {
  if (!c.numerico) return undefined;
  return c.decimais >= 31 ? null : c.decimais;
}'''
b='''/// FLOAT e DOUBLE chegam com 31, o «não fixo» do protocolo — não há casa
/// declarada. Aí a casa sai do próprio dado que veio: a maior que apareceu na
/// coluna, até dez. Fixar zero arredondaria 0,5 para 1, e fixar um número
/// alto encheria a tela de zeros que ninguém escreveu.
function casasDoExterno(c, linhas, i) {
  if (!c.numerico) return undefined;
  if (c.decimais < 31) return c.decimais;
  let maior = 0;
  for (const l of linhas || []) {
    const v = l[i];
    if (typeof v !== "string") continue;
    const ponto = v.indexOf(".");
    if (ponto >= 0) maior = Math.max(maior, Math.min(10, v.length - ponto - 1));
  }
  return maior;
}'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('      decimais: casasDoExterno(c),','      decimais: casasDoExterno(c, r.linhas, i),',1)
s=s.replace('        decimais: casasDoExterno(c), agregador: c.numerico ? "sum" : null,',
            '        decimais: casasDoExterno(c, r.linhas, i), agregador: c.numerico ? "sum" : null,',1)
open(p,'w').write(s)
print('ok')
