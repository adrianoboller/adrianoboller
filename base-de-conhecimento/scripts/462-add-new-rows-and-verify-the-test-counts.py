# Add new rows and verify the test counts
# 28/08 15:16

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
a='''        <tr><td>Tema claro e escuro · console para vários servidores</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'''
b='''        <tr><td>Tema claro e escuro · console para vários servidores</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Monitores da máquina · CPU, memória, placas de rede, discos e espaço</td><td><span class="pino ok">pronto</span></td><td class="num">6</td></tr>
        <tr><td>Aviso de disco apertado por e-mail · cliente SMTP escrito aqui</td><td><span class="pino ok">pronto</span></td><td class="num">16</td></tr>
        <tr><td>DbLink para MySQL(R) · protocolo escrito aqui, grade compartilhada</td><td><span class="pino ok">pronto</span></td><td class="num">16</td></tr>
        <tr><td>SHA-1 contra o FIPS 180-4 · só para o <code>mysql_native_password</code></td><td><span class="pino ok">pronto</span></td><td class="num">3</td></tr>
        <tr><td>DbLink para PostgreSQL(R) — a definição guarda, o cliente falta</td><td><span class="pino pend">desenhada</span></td><td class="num">—</td></tr>'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
