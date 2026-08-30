# Tighten the machine card layout
# 28/08 14:35

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

a='''  const medidores = `<div class="medidores">
    <div>${medidor(primeira ? 0 : cpu.uso_percentual, "cpu",
      `${cpu.nucleos || "?"} núcleos${carga ? " · carga " + carga : ""}`)}</div>
    <div>${medidor(memPerc, "memória",
      `${fmtBytes((mem.usada_kb || 0) * 1024)} de ${fmtBytes((mem.total_kb || 0) * 1024)}`)}</div>
  </div>`;'''
b='''  // Medidores à esquerda, discos à direita: os dois arcos ocupam largura
  // fixa, e o resto da carta larga fica todo para as barras de disco — que é
  // o que cresce com o número de caminhos vigiados.
  const medidores = `<div class="medidores">
    ${medidor(primeira ? 0 : cpu.uso_percentual, "cpu",
      `${cpu.nucleos || "?"} núcleos${carga ? " · carga " + carga : ""}`)}
    ${medidor(memPerc, "memória",
      `${fmtBytes((mem.usada_kb || 0) * 1024)} de ${fmtBytes((mem.total_kb || 0) * 1024)}`)}
  </div>`;'''
assert a in s; s=s.replace(a,b,1)

a='''      medidores + (m.discos && m.discos.length ? discosHtml(m) : ""), true) +'''
b='''      `<div class="maquina">${medidores}
         <div>${m.discos && m.discos.length ? discosHtml(m) : ""}</div>
       </div>`, true) +'''
assert a in s; s=s.replace(a,b,1)

a='''function discosHtml(m) {
  const apertados = new Set(m.apertados || []);
  return barrasCheias((m.discos || []).map(d => ({'''
b='''function discosHtml(m) {
  const apertados = new Set(m.apertados || []);
  // 800 é a largura real da coluna da direita nesta carta; o viewBox tem de
  // nascer perto dela, senão o texto sai esticado ou espremido.
  return barrasCheias((m.discos || []).map(d => ({'''
assert a in s; s=s.replace(a,b,1)
a='''    texto: `${fmtBytes(d.livre_kb * 1024)} livres de ${fmtBytes((d.utilizavel_kb ?? d.total_kb) * 1024)}`
      + (d.reservado_kb ? `  ·  ${fmtBytes(d.reservado_kb * 1024)} reservados` : ""),
  })));
}'''
b='''    texto: `${fmtBytes(d.livre_kb * 1024)} livres de ${fmtBytes((d.utilizavel_kb ?? d.total_kb) * 1024)}`
      + (d.reservado_kb ? `  ·  ${fmtBytes(d.reservado_kb * 1024)} reservados` : ""),
  })), 800);
}'''
assert a in s; s=s.replace(a,b,1)

a='''.medidores{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));
           gap:10px;margin-bottom:16px}
.medidores svg{width:100%;max-width:172px;margin:0 auto;height:auto;display:block}'''
b='''.maquina{display:grid;grid-template-columns:auto 1fr;gap:8px 26px;align-items:center}
.medidores{display:flex;gap:14px}
.medidores svg{width:168px;height:auto;display:block}
/* Numa janela estreita os arcos passam para cima das barras: lado a lado eles
   espremeriam o disco ate o texto do caminho nao caber. */
@media (max-width:960px){.maquina{grid-template-columns:1fr}
                         .medidores{justify-content:center}}'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
