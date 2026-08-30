# Apply the script fixes
# 28/08 21:47

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video/roteiro.mjs")
s = p.read_text()
antigo = """await diz('06 · Cem mil linhas', 'Agora em escala. Cem mil linhas pelo protocolo, em lotes de cinco mil.', 3600);
await p.evaluate(async () => {
  const cid = [['Blumenau','SC'],['Joinville','SC'],['Itajaí','SC'],['Curitiba','PR'],['Porto Alegre','RS']];
  for (let i = 0; i < 100000; i += 5000) {
    const linhas = [];
    for (let k = i + 7; k < i + 5007 && k <= 100006; k++) {
      const c = cid[k % 5];
      linhas.push({ id: k, nome: `Cliente ${String(k).padStart(7,'0')}`,
                    cidade: c[0], uf: c[1], limite: `${(k % 9000) + 100}.00`,
                    cadastro: '2025-01-01', ficha: `ficha do cliente ${k}` });
    }
    await api('inserir_lote', { database: 'Comercial', tabela: 'cadastroClientes', linhas });
  }
});
await esperar(600);"""
novo = """await diz('06 · Vinte mil linhas', 'Agora em escala: vinte mil linhas em quatro lotes, direto pelo protocolo.', 3600);
for (let bloco = 0; bloco < 4; bloco++) {
  const r = await p.evaluate(async (b) => {
    const cid = [['Blumenau','SC'],['Joinville','SC'],['Itajaí','SC'],['Curitiba','PR'],['Porto Alegre','RS']];
    const linhas = [];
    for (let k = b * 5000 + 7; k < (b + 1) * 5000 + 7; k++) {
      const c = cid[k % 5];
      linhas.push({ id: k, nome: `Cliente ${String(k).padStart(7,'0')}`,
                    cidade: c[0], uf: c[1], limite: `${(k % 9000) + 100}.00`,
                    cadastro: '2025-01-01', ficha: `ficha do cliente ${k}` });
    }
    const x = await api('inserir_lote', { database: 'Comercial', tabela: 'cadastroClientes', linhas });
    return x && (x.gravadas ?? (x.rowids ? x.rowids.length : 0));
  }, bloco);
  console.log(`  lote ${bloco + 1}/4: ${r}`);
  await esperar(400);
}"""
assert antigo in s, "bloco 06 nao encontrado"
s = s.replace(antigo, novo)
s = s.replace("await p.locator('#pgIr').fill('400');", "await p.locator('#pgIr').fill('80');")
s = s.replace("await diz('', 'Página 400 de 500. Seis milissegundos no servidor — contra 131 andando até lá.', 4200);",
              "await diz('', 'Página 80 de 100, na hora. Numa tabela de 200 mil: 6 ms contra 131 andando até lá.', 4400);")
s = s.replace('await diz(\'\', \'Cem mil. "de 500 páginas" saiu de dois contadores do cabeçalho — sem varrer nada.\', 4200);',
              'await diz(\'\', \'Vinte mil. O "de quantas" saiu de dois contadores do cabeçalho — sem varrer nada.\', 4200);')
s = s.replace("  if (cap) capAtual = cap;", "  if (cap) { capAtual = cap; console.log('>>', cap); }")
p.write_text(s)
print("ok")
