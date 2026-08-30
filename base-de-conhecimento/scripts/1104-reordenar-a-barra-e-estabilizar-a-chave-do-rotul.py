# Reordenar a barra e estabilizar a chave do rotulo
# 29/08 11:04

import io,re
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

# ---------- 1. extrair as entradas da barra, com blocos multilinha ----------
ini = s.index('const FERRAMENTAS = [')
fim = s.index('];', ini) + 2
bloco = s[ini:fim]
corpo = bloco[bloco.index('[')+1:bloco.rindex(']')]

entradas = {}   # rot -> texto do bloco
atual, prof = [], 0
for linha in corpo.split('\n'):
    if not linha.strip():
        continue
    if linha.strip() == '"risco",' and prof == 0:
        continue  # separadores serao repostos na remontagem
    atual.append(linha)
    prof += linha.count('{') - linha.count('}')
    if prof == 0 and linha.rstrip().endswith(','):
        texto = '\n'.join(atual)
        m = re.search(r'rot:"([^"]+)"', texto)
        assert m, texto[:60]
        entradas[m.group(1)] = texto
        atual = []
assert not atual, atual
assert len(entradas) == 25, sorted(entradas)

# ---------- 2. a ordem nova, do mais usado para o mais raro ----------
grupos = [
    ["View DB","Tabelas","Query","Pivot","Junção","Exportar","Importar"],
    ["Bancos","Gerir Banco","Usuários","Conexões","Config","Jobs"],
    ["Backup","Lixeira","Transações","DbLink","SysTables","Diagrama ER","LGPD",
     "Replicação","Profiler","Diretivas"],
    ["Start/Stop","Repair"],
    ["Duplicar","Server Mail","Blockchain","Ajuda"],
]
usados = [r for g in grupos for r in g]
assert sorted(usados) == sorted(entradas), set(usados) ^ set(entradas)

cab = '''/* A ORDEM e a do uso, do diario para o raro -- e e julgamento declarado,
   nao medicao: o servidor ainda nao conta cliques por ferramenta. Quando a
   tela de Estatisticas de uso passar a contar, a ordem se refaz do numero.
     1. o dia a dia do dado ...... ver, consultar, cruzar, entrar e sair
     2. administracao corrente ... bancos, gente, conexoes, configuracao
     3. o ocasional .............. salvaguarda, cadastro fino, observacao
     4. raro e de risco .......... parar o servico, reparar
     5. desligadas e ajuda ....... o que ainda nao existe fica visivel no fim,
                                   como promessa, e nao no meio, como tropeço */
'''
novo_corpo = []
for gi, g in enumerate(grupos):
    if gi:
        novo_corpo.append('  "risco",')
    for r in g:
        novo_corpo.append(entradas[r])
novo_bloco = 'const FERRAMENTAS = [\n' + cab + '\n'.join(novo_corpo) + '\n];'
s = s[:ini] + novo_bloco + s[fim:]

# ---------- 3. a chave do rotulo editavel deixa de ser a POSICAO ----------
# `fer.${i}` quebraria em silencio todo nome personalizado a cada reordenacao
# -- inclusive nesta. O nome da ferramenta e estavel; a posicao nao.
s = s.replace('const rotulo = rot(`fer.${i}`, f.rot);',
              'const rotulo = rot(`fer.${f.rot}`, f.rot);')
s = s.replace('itens.push({ chave:`fer.${i}`, original:f.rot, onde:"ferramenta", grupo:"Barra de ferramentas" });',
              'itens.push({ chave:`fer.${f.rot}`, original:f.rot, onde:"ferramenta", grupo:"Barra de ferramentas" });')
io.open(p,'w',encoding='utf-8').write(s)
print('barra reordenada, chave do rotulo estavel')
