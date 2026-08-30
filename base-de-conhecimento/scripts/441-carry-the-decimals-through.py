# Carry the decimals through
# 28/08 14:55

p='crates/phxsql-server/src/dblink/mysql.rs'
s=open(p).read()
a='''    pub tamanho: u32,
    pub nulavel: bool,
    pub primaria: bool,'''
b='''    pub tamanho: u32,
    /// Casas decimais que o servidor declara.
    ///
    /// Existe porque sem ela a tela arredonda: um `DECIMAL(12,2)` de 15000,50
    /// aparecia como 15.001, que e outro numero. Tipo de ponto flutuante vem
    /// com 31 (o "nao fixo" do protocolo) e a tela trata isso a parte.
    pub decimais: u8,
    pub nulavel: bool,
    pub primaria: bool,'''
assert a in s; s=s.replace(a,b,1)
a='''    let bandeiras = le_u16(p, i).unwrap_or(0);
    Ok(Coluna {'''
b='''    let bandeiras = le_u16(p, i).unwrap_or(0);
    i += 2;
    let decimais = *p.get(i).unwrap_or(&0);
    Ok(Coluna {
        decimais,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

p='crates/phxsql-server/src/dblink/mod.rs'
s=open(p).read()
a='''                            ("tamanho", Json::de_u64(c.tamanho as u64)),'''
b='''                            ("tamanho", Json::de_u64(c.tamanho as u64)),
                            ("decimais", Json::de_u64(c.decimais as u64)),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''function tipoDoGridExterno(c) {
  if (/DATE|TIME|YEAR/.test(c.tipo)) return "texto";   // vem como texto ISO
  return c.numerico ? "numero" : "texto";
}'''
b='''function tipoDoGridExterno(c) {
  if (/DATE|TIME|YEAR/.test(c.tipo)) return "texto";   // vem como texto ISO
  return c.numerico ? "numero" : "texto";
}

/// Quantas casas a coluna mostra.
///
/// «moeda» não serve para o dado de fora: ela prefixa R$, e um DECIMAL do
/// outro banco tanto pode ser dinheiro quanto quantidade ou taxa. O tipo é
/// «numero» com o número de casas que o próprio servidor declarou — sem isso
/// um DECIMAL(12,2) de 15000,50 aparecia como 15.001, que é outro número.
///
/// FLOAT e DOUBLE chegam com 31, o «não fixo» do protocolo: aí a casa decimal
/// vem do próprio valor, porque arredondar às cegas mentiria nos dois
/// sentidos.
function casasDoExterno(c) {
  if (!c.numerico) return undefined;
  return c.decimais >= 31 ? null : c.decimais;
}'''
assert a in s; s=s.replace(a,b,1)

a='''      campo: nomes[i],
      titulo: c.nome + (c.primaria ? " ★" : ""),
      tipo: tipoDoGridExterno(c),
      // Somar chave primária não significa nada; só medida soma.
      agregador: c.numerico && !c.primaria ? "sum" : null,
    })),'''
b='''      campo: nomes[i],
      titulo: c.nome + (c.primaria ? " ★" : ""),
      tipo: tipoDoGridExterno(c),
      decimais: casasDoExterno(c),
      // Somar chave primária não significa nada; só medida soma.
      agregador: c.numerico && !c.primaria ? "sum" : null,
    })),'''
assert a in s; s=s.replace(a,b,1)
a='''        campo: nomes[i], titulo: c.nome, tipo: tipoDoGridExterno(c),
        agregador: c.numerico ? "sum" : null,'''
b='''        campo: nomes[i], titulo: c.nome, tipo: tipoDoGridExterno(c),
        decimais: casasDoExterno(c), agregador: c.numerico ? "sum" : null,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
