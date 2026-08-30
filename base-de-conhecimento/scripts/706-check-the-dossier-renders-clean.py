# Check the dossier renders clean
# 28/08 19:11

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()
# a tabela de arquivos por operacao ganha o .pag? nao -- mas o painel da capa
s=s.replace('<div><div class="v">{n[\'arquivos\']}</div>','<div><div class="v">{n[\'arquivos\']}</div>')
# a chamada da capa
s=s.replace('<h1>Ph<span class="x">x</span>Sql <span class="leve">— sete arquivos,<br>uma tabela</span></h1>',
            '<h1>Ph<span class="x">x</span>Sql <span class="leve">— sete arquivos,<br>uma tabela</span></h1>')
io.open(p,'w',encoding='utf-8').write(s)
