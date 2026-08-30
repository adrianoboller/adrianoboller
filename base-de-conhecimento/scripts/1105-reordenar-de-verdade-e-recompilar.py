# Reordenar de verdade e recompilar
# 29/08 11:04

import io,re
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
ini = s.index('const FERRAMENTAS = [')
fim = s.index('];', ini) + 2
bloco = s[ini:fim]
corpo = bloco[bloco.index('[')+1:bloco.rindex(']')]
entradas, atual, prof = {}, [], 0
for linha in corpo.split('\n'):
    if not linha.strip(): continue
    if linha.strip() == '"risco",' and prof == 0: continue
    atual.append(linha)
    prof += linha.count('{') - linha.count('}')
    if prof == 0 and linha.rstrip().endswith(','):
        texto = '\n'.join(atual)
        m = re.search(r'rot:"([^"]+)"', texto)
        entradas[m.group(1)] = texto
        atual = []
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
    if gi: novo_corpo.append('  "risco",')
    for r in g: novo_corpo.append(entradas[r])
s = s[:ini] + 'const FERRAMENTAS = [\n' + cab + '\n'.join(novo_corpo) + '\n];' + s[fim:]
s = s.replace('const rotulo = rot(`fer.${i}`, f.rot);',
              'const rotulo = rot(`fer.${f.rot}`, f.rot);')
s = s.replace('itens.push({ chave:`fer.${i}`, original:f.rot, onde:"ferramenta", grupo:"Barra de ferramentas" });',
              'itens.push({ chave:`fer.${f.rot}`, original:f.rot, onde:"ferramenta", grupo:"Barra de ferramentas" });')
io.open(p,'w',encoding='utf-8').write(s)
print('ok: 29 ferramentas em 5 grupos, chave do rotulo pelo nome')
