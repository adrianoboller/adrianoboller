# Wire the markers and run the measurement script
# 28/08 18:12

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()

velho='''  <div class="painel">
    <div><div class="v">34.156</div><div class="r">linhas de Rust</div></div>
    <div><div class="v">453</div><div class="r">testes</div></div>
    <div><div class="v">0</div><div class="r">dependências</div></div>
    <div><div class="v">4</div><div class="r">crates</div></div>
    <div><div class="v">5</div><div class="r">arquivos/tabela</div></div>
    <div><div class="v">5.619</div><div class="r">linhas de doc</div></div>
  </div>'''
novo='''  <div class="painel">
  <!-- projeto:inicio (gerado por docs/dossie/numeros-do-projeto.py) --><!-- projeto:fim -->
  </div>'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''<footer>
  <p>PhxSql 0.11.0 · 34.156 linhas de Rust em 4 crates, mais 422 KiB de interface ·
  453 testes · nenhuma dependência externa. Especificação byte a byte em <code>docs/FORMATO.md</code>, cadastro e
  permissões em <code>docs/USUARIOS.md</code>, desenho da replicação em
  <code>docs/REPLICACAO.md</code>, roteiro em <code>docs/PLANO.md</code>,
  o DbLink em <code>docs/DBLINK.md</code>, as junções em <code>docs/JUNCOES.md</code>,
  a revisão contra os motores maduros em <code>docs/COMPARACAO.md</code>,
  e o que ainda falta em <code>docs/PENDENCIAS.md</code>.</p>
</footer>'''
novo2='''<footer>
  <!-- rodape:inicio (gerado por docs/dossie/numeros-do-projeto.py) --><!-- rodape:fim -->
</footer>'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

s=s.replace('<h1>Ph<span class="x">x</span>Sql <span class="leve">— cinco arquivos,<br>uma tabela</span></h1>',
            '<h1>Ph<span class="x">x</span>Sql <span class="leve">— sete arquivos,<br>uma tabela</span></h1>',1)
io.open(p,'w',encoding='utf-8').write(s)
print('marcas postas')
