# Test the delete success path
# 28/08 10:42

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/gestao.mjs')
s = p.read_text()
v = "p.on('dialog', d => { perguntas.push(d.message().split('\\n')[0]); d.accept(d.defaultValue()); });"
n = """// A exclusão pede o nome digitado; quem responde aqui é este roteiro.
let respostaDoPrompt = null;
p.on('dialog', d => { perguntas.push(d.message().split('\\n')[0]);
  d.accept(respostaDoPrompt !== null ? respostaDoPrompt : d.defaultValue()); });"""
assert s.count(v) == 1
s = s.replace(v, n)

v = '''await p.locator('.op:has-text("Excluir tabela")').click(); await p.waitForTimeout(1600);
console.log(`10. excluir -> "${await p.locator('#aviso').textContent()}" · ${await p.locator('.linha-tab').count()} tabela(s)`);'''
n = '''await p.locator('.op:has-text("Excluir tabela")').click(); await p.waitForTimeout(1600);
console.log(`10a. nome errado -> "${await p.locator('#aviso').textContent()}" (a tabela sobrevive)`);
await p.locator('.ferramentas .fer:has-text("Tabelas")').click(); await p.waitForTimeout(900);
console.log(`     ainda na grade: ${await p.locator('.linha-tab').count()} tabela(s)`);
respostaDoPrompt = 'pedidos_copia';
await p.locator('.linha-tab:has-text("pedidos_copia")').click(); await p.waitForTimeout(700);
await p.locator('.op:has-text("Excluir tabela")').click(); await p.waitForTimeout(1800);
respostaDoPrompt = null;
console.log(`10b. nome certo -> "${await p.locator('#aviso').textContent()}" · ${await p.locator('.linha-tab').count()} tabela(s)`);'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
