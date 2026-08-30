# Update the disk-impact table with the new files
# 28/08 18:14

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()

s=s.replace('''  <p><strong>53 das 56 operações do protocolo têm tela.</strong> Fora:
  <code>buscar</code>, <code>desbloquear</code> e <code>criar_schema</code> — este
  último acontece sozinho quando a tela cria uma tabela dentro de um schema que
  ainda não existe.</p>''',
'''  <p><strong>57 das 60 operações do protocolo têm tela.</strong> Fora:
  <code>buscar</code>, <code>desbloquear</code> e <code>criar_schema</code> — este
  último acontece sozinho quando a tela cria uma tabela dentro de um schema que
  ainda não existe.</p>''',1)

# a tabela ganha as duas colunas novas e as linhas da exclusao
velho='''      <thead><tr><th>Operação</th><th class="num">.reg</th><th class="num">.ndx</th>
        <th class="num">.bin</th><th class="num">.memo</th><th class="num">.log</th>
        <th class="num">.bkp</th><th>Desfaz?</th></tr></thead>'''
novo='''      <thead><tr><th>Operação</th><th class="num">.reg</th><th class="num">.ndx</th>
        <th class="num">.bin</th><th class="num">.memo</th><th class="num">.log</th>
        <th class="num">.trash</th><th class="num">.reason</th>
        <th class="num">.bkp</th><th>Desfaz?</th></tr></thead>'''
assert velho in s
s=s.replace(velho,novo,1)

# cada linha ganha duas celulas antes da do .bkp
linhas = [
 ('Estrutura','—','—'), ('Partições','—','—'), ('Configurações e diretivas','—','—'),
 ('Editar conteúdo','grava','grava'), ('Nova tabela','cria','cria'),
 ('Duplicar','copia','copia'), ('Copiar / colar','copia','copia'),
 ('Reparar tabela','—','—'), ('Reparar índice','—','—'),
]
for nome, trash, reason in linhas:
    alvo = f'<tr><td class="dado">{nome}</td>'
    i = s.index(alvo)
    fim = s.index('</tr>', i)
    linha = s[i:fim]
    # insere as duas celulas antes da penultima <td> (a do .bkp)
    corte = linha.rindex('<td class="num">')
    nova = linha[:corte] + f'<td class="num">{trash}</td><td class="num">{reason}</td>' + linha[corte:]
    s = s[:i] + nova + s[fim:]

# a linha do Excluir tabela, reescrita
velho2='''        <tr><td class="dado">Excluir</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td><strong>não</strong></td></tr>'''
novo2='''        <tr><td class="dado">Excluir linha (suave)</td><td class="num">marca</td><td class="num">grava</td><td class="num">—</td><td class="num">—</td><td class="num">grava</td><td class="num">—</td><td class="num">grava</td><td class="num">se ligado</td><td><code>restaurar</code></td></tr>
        <tr><td class="dado">Excluir linha (física)</td><td class="num">libera</td><td class="num">tira</td><td class="num">libera</td><td class="num">libera</td><td class="num">grava</td><td class="num"><strong>grava</strong></td><td class="num">grava</td><td class="num">se ligado</td><td>reinserindo, com outro rowid</td></tr>
        <tr><td class="dado">Lixeira e motivos</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">lê</td><td class="num">lê</td><td class="num">—</td><td>não altera · <strong>só admin</strong></td></tr>
        <tr><td class="dado">Esvaziar a lixeira</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num"><strong>apaga</strong></td><td class="num">grava</td><td class="num">—</td><td><strong>não</strong></td></tr>
        <tr><td class="dado">Excluir tabela</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td><strong>não</strong></td></tr>'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
