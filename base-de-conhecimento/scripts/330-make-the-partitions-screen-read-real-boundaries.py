# Make the partitions screen read real boundaries
# 28/08 11:42

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

v = '''function volumesDe(e) {
  const pag = e.paginacao;
  if (!pag || !pag.registros_por_arquivo) return [];
  const rpa = pag.registros_por_arquivo;
  const total = Math.max(Number(e.slots) || 0, 0);
  const quantos = Math.max(1, Math.ceil(total / rpa));
  const larg = pag.digitos || 3;
  const out = [];
  for (let v = 1; v <= quantos; v++) {
    const de = (v - 1) * rpa + 1;
    const ate = v * rpa;
    out.push({ volume:v,
      arquivo: `${e.tabela}_${String(v).padStart(larg, "0")}.reg`,
      de, ate, usados: Math.max(0, Math.min(total, ate) - de + 1) });
  }
  return out;
}'''
n = '''function volumesDe(e) {
  const pag = e.paginacao;
  if (!pag || !pag.registros_por_arquivo) return [];
  const larg = pag.digitos || 3;
  const total = Math.max(Number(e.slots) || 0, 0);
  const nome = v => `${e.tabela}_${String(v).padStart(larg, "0")}.reg`;

  // Na particao por periodo o volume NAO sai de conta: ele corta quando o
  // calendario vira, e so o servidor sabe quando isso aconteceu. As
  // fronteiras vem prontas no `esquema`; calcular aqui daria o numero errado
  // -- e dava: quatro meses apareciam como um volume so.
  if (e.volumes && e.volumes.length) {
    return e.volumes.map((v, i) => {
      const prox = e.volumes[i + 1];
      const ate = prox ? prox.primeiro_rowid - 1 : total;
      return { volume: v.volume, arquivo: nome(v.volume), periodo: v.periodo,
               de: v.primeiro_rowid, ate,
               usados: Math.max(0, ate - v.primeiro_rowid + 1) };
    });
  }

  const rpa = pag.registros_por_arquivo;
  const quantos = Math.max(1, Math.ceil(total / rpa));
  const out = [];
  for (let v = 1; v <= quantos; v++) {
    const de = (v - 1) * rpa + 1;
    const ate = v * rpa;
    out.push({ volume:v, arquivo: nome(v),
      de, ate, usados: Math.max(0, Math.min(total, ate) - de + 1) });
  }
  return out;
}'''
assert s.count(v) == 1
s = s.replace(v, n)

# a tabela da tela ganha a coluna de periodo quando ela existe
v = '''    tabela([{t:"volume",cls:"num"},{t:"arquivo"},{t:"do rowid",cls:"num"},
            {t:"até o rowid",cls:"num"},{t:"slots usados",cls:"num"}],
      vols, v => `<tr>
        <td class="num dado">${v.volume}</td>
        <td class="dado"><code>${esc(v.arquivo)}</code></td>
        <td class="num">${v.de}</td><td class="num">${v.ate}</td>
        <td class="num">${v.usados}</td></tr>`) +'''
n = '''    tabela([{t:"volume",cls:"num"},{t:"arquivo"},
            ...(porPeriodo ? [{t:"período que o abriu"}] : []),
            {t:"do rowid",cls:"num"},{t:"até o rowid",cls:"num"},{t:"slots usados",cls:"num"}],
      vols, v => `<tr>
        <td class="num dado">${v.volume}</td>
        <td class="dado"><code>${esc(v.arquivo)}</code></td>
        ${porPeriodo ? `<td class="dado">${esc(v.periodo || "—")}</td>` : ""}
        <td class="num">${v.de}</td><td class="num">${v.ate}</td>
        <td class="num">${v.usados}</td></tr>`) +'''
assert s.count(v) == 1
s = s.replace(v, n)

# o cabecalho e a nota mudam conforme a regra
v = '''  const vols = volumesDe(e);
  const gb = n => (n / (1024 * 1024 * 1024)).toFixed(2);
  folha(`Partições de ${tab}`, `${db} · ${vols.length} volume(s) do .reg`,'''
n = '''  const vols = volumesDe(e);
  const porPeriodo = pag.modo && pag.modo !== "quantidade";
  const gb = n => (n / (1024 * 1024 * 1024)).toFixed(2);
  folha(`Partições de ${tab}`,
    `${db} · ${vols.length} volume(s) do .reg · ${
      porPeriodo ? `corta ${pag.modo}, pela coluna ${pag.coluna}` : "corta por quantidade"}`,'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''       <div class="ficha"><div class="v">${esc(String(pag.registros_por_arquivo))}</div>
         <div class="r">registros por arquivo</div></div>'''
n = '''       <div class="ficha"><div class="v">${esc(porPeriodo ? pag.modo : "por faixa")}</div>
         <div class="r">regra de corte</div>
         <div class="u">${porPeriodo ? esc(`pela coluna ${pag.coluna}`) : "por quantidade"}</div></div>
       <div class="ficha"><div class="v">${esc(String(pag.registros_por_arquivo))}</div>
         <div class="r">${porPeriodo ? "teto por volume" : "registros por arquivo"}</div></div>'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    `<div class="nota">
       <p><strong>Estas faixas são conta, não busca.</strong>
       <code>volume = (rowid−1) ÷ ${esc(String(pag.registros_por_arquivo))} + 1</code>,
       e o resto da divisão é o slot dentro do volume. É por isso que paginar
       não custa nada na leitura.</p>'''
n = '''    `<div class="nota">
       ${porPeriodo ? `
       <p><strong>Aqui as faixas não são conta.</strong> O volume corta quando o
       período da coluna <code>${esc(pag.coluna)}</code> vira — ou quando enche,
       o que vier primeiro. Cada volume grava no próprio cabeçalho o rowid em
       que começou, e achar o volume de um rowid é uma busca binária nessa
       tabela pequena, em vez de uma divisão.</p>
       <p><strong>A linha atrasada não volta.</strong> Um lançamento de janeiro
       digitado em março entra no volume de março: a ordem de digitação é
       sagrada, e voltar seria escrever no meio de um arquivo já fechado. Por
       isso o período de um volume é <em>o período em que ele abriu</em>.</p>`
       : `
       <p><strong>Estas faixas são conta, não busca.</strong>
       <code>volume = (rowid−1) ÷ ${esc(String(pag.registros_por_arquivo))} + 1</code>,
       e o resto da divisão é o slot dentro do volume. É por isso que paginar
       não custa nada na leitura.</p>`}'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
