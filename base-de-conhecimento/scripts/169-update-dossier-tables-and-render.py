# Update dossier tables and render
# 27/08 21:02

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
# estado e roteiro: linhas novas
s=s.replace('''<tr><td>Centro de Controle — interface web embutida no <code>phxsqld</code></td><td><span class="pino ok">pronto</span></td><td class="num">13</td></tr>''',
'''<tr><td>Centro de Controle — interface web embutida no <code>phxsqld</code></td><td><span class="pino ok">pronto</span></td><td class="num">13</td></tr>
        <tr><td>Tabela em memória e <code>SelectMemory</code> — 87× medido</td><td><span class="pino ok">pronto</span></td><td class="num">11</td></tr>
        <tr><td>Ed25519 (RFC 8032) e SHA-512, contra vetor oficial</td><td><span class="pino ok">pronto</span></td><td class="num">15</td></tr>
        <tr><td>Backup com manifesto SHA-256 e conferência</td><td><span class="pino ok">pronto</span></td><td class="num">7</td></tr>
        <tr><td>Tema claro e escuro · console para vários servidores</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>''')
# o que ainda nao faz: acrescenta o start/stop e a replicacao
s=s.replace('''    <li><strong>Sem troca de senha pelo protocolo</strong> e sem grupos: o poder é por
    usuário, e a senha muda no <code>config.json</code>.</li>''',
'''    <li><strong>Sem troca de senha pelo protocolo</strong> e sem grupos: o poder é por
    usuário, e a senha muda no <code>config.json</code>.</li>
    <li><strong>A replicação não transporta evento.</strong> A configuração entra e
    valida, o desenho está na seção 8, e o servidor avisa alto no arranque quando o
    papel não é <code>isolado</code>. Config que promete o que o código não faz é pior
    do que config que falta.</li>
    <li><strong>Sem parar e subir a porta de dados pela interface.</strong> Fazer isso
    sem derrubar o processo exige mexer no laço de aceitação, e é melhor fazer
    inteiro do que pela metade.</li>''')
# decisoes novas
s=s.replace('''        <tr>
          <td>Identidade no HTTP</td>''','''        <tr>
          <td>Cache em memória</td>
          <td>Explícito: nada entra sozinho</td>
          <td>Um cache que decide sozinho o que guardar é um cache que um dia decide errado no pior momento.</td>
        </tr>
        <tr>
          <td>Coerência da cópia em RAM</td>
          <td>A escrita atualiza as duas dentro da mesma trava</td>
          <td>Não existe janela em que disco e memória discordem. Custa uma anotação por escrita.</td>
        </tr>
        <tr>
          <td>Segundo fator</td>
          <td>Ed25519 escrito aqui, não uma crate</td>
          <td>Zero dependência continua valendo. O preço é conferir contra vetor oficial — e foi ele que achou o defeito.</td>
        </tr>
        <tr>
          <td>Identidade no HTTP</td>''')
open(p,'w').write(s)
