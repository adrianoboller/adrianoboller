# Add decisions rows and stage dossier
# 27/08 20:03

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
velho='''        <tr>
          <td>Concorrência</td>
          <td>Trava única por enquanto</td>
          <td>Lento sob carga e correto, em vez de rápido e corrompido.</td>
        </tr>
      </tbody>'''
novo='''        <tr>
          <td>Concorrência</td>
          <td>Trava única por enquanto</td>
          <td>Lento sob carga e correto, em vez de rápido e corrompido.</td>
        </tr>
        <tr>
          <td>Interface web</td>
          <td>Porta própria, desligada por padrão, presa a <code>127.0.0.1</code></td>
          <td>Quem fala HTTP não é quem fala JSON&nbsp;Lines. Abrir a porta é decisão de quem administra, não padrão herdado.</td>
        </tr>
        <tr>
          <td>A página</td>
          <td>Um arquivo, embutido no binário</td>
          <td>Sem servidor web para instalar e sem diretório para servir — logo, sem travessia de diretório.</td>
        </tr>
        <tr>
          <td>Identidade no HTTP</td>
          <td>Sessão com prazo, renovada a cada uso</td>
          <td>O PBKDF2 de 210.000 iterações roda uma vez por login, não a cada clique.</td>
        </tr>
      </tbody>'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
print("decisoes ok")
