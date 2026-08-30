# Update dossier with zip, scheduling and levels
# 27/08 21:24

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
s=s.replace('<div><div class="v">17.783</div><div class="r">linhas de Rust</div></div>',
            '<div><div class="v">19.283</div><div class="r">linhas de Rust</div></div>')
s=s.replace('<div><div class="v">254</div><div class="r">testes</div></div>',
            '<div><div class="v">276</div><div class="r">testes</div></div>')
s=s.replace('<div class="selo">Dossiê técnico · versão 0.2.0</div>',
            '<div class="selo">Dossiê técnico · versão 0.3.0</div>')
s=s.replace('''<p>PhxSql 0.2.0 · 17.783 linhas de Rust em 4 crates, mais 69 KB de interface ·
  254 testes · nenhuma dependência externa.''',
'''<p>PhxSql 0.3.0 · 19.283 linhas de Rust em 4 crates, mais 69 KB de interface ·
  276 testes · nenhuma dependência externa.''')

# secao 14 (backup) ganha o zip e o agendamento
s=s.replace('''  <p>O <code>conferir-backup</code> sai com código de erro quando não bate, para caber
  numa linha de <em>cron</em>: <code>phxsql conferir-backup /backup/phxsql ||
  mandar-email</code>. Conferido virando um bit num <code>.reg</code> e num
  <code>.ndx</code> copiados — os dois foram apontados pelo nome.</p>''',
'''  <p>O <code>conferir-backup</code> sai com código de erro quando não bate, para caber
  numa linha de <em>cron</em>: <code>phxsql conferir-backup /backup/phxsql ||
  mandar-email</code>. Conferido virando um bit num <code>.reg</code> e num
  <code>.ndx</code> copiados — os dois foram apontados pelo nome.</p>

  <h3>Um arquivo só, que o mundo abre</h3>

  <p>Com <code>--zip</code>, a cópia vira um arquivo chamado
  <code>Comercial_adriano_2026-08-27_2114.zip</code> — banco, quem fez, data e
  hora. É assim que se acha o arquivo certo numa pasta com trezentos backups
  <em>sem abrir nenhum</em>. O manifesto vai dentro, então a cópia carrega a
  própria conferência.</p>

  <p>ZIP de verdade comprime, e comprimir exige DEFLATE. Trazer uma
  <em>crate</em> quebraria a regra de zero dependência — então o DEFLATE está
  escrito aqui: Huffman fixo mais casamento LZ77 por tabela de dispersão. O
  <code>.reg</code> é <em>slot</em> de tamanho fixo e o <code>.ndx</code> é
  página com enchimento; é exatamente disso que a compressão vive.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Prova</th><th>Resultado</th></tr></thead>
      <tbody>
        <tr><td><code>unzip -t</code> do sistema</td><td>todos os CRC OK, nenhum erro</td></tr>
        <tr><td><code>zipfile</code> do Python, extraindo</td><td>cada arquivo idêntico ao original, byte a byte</td></tr>
        <tr><td>Tamanho, no cadastro de exemplo</td><td class="num">18.311 → 2.406 bytes (87% menor)</td></tr>
      </tbody>
    </table>
  </div>

  <p>Comprimir é fácil; comprimir de um jeito que os outros leiam é o ponto. Por
  isso a prova não é o teste de ida e volta com o próprio código — é o mundo
  abrir.</p>

  <h3>Agendado</h3>

  <p>A seção <code>backup</code> do <code>config.json</code> liga o relógio:
  <code>hora</code> para uma vez por dia, ou <code>cada_horas</code> para
  intervalo, com <code>manter</code> para a retenção. <strong>Vem
  desligada</strong> — backup que roda sozinho num destino que ninguém conferiu é
  backup que enche o disco e para.</p>

  <p>O relógio confere de minuto em minuto em vez de dormir até a hora marcada:
  dormir horas seguidas é frágil — a máquina suspende, o relógio anda, e o backup
  não acontece sem ninguém notar.</p>

  <div class="nota">
    <p>A faxina do <code>manter</code> só apaga arquivo com a cara dos nossos:
    <code>.zip</code> no formato <code>Banco_Admin_Data_HoraMin</code>. Alguém pode
    ter guardado outra coisa na pasta, e há teste com um
    <code>relatorio-do-contador.zip</code> no meio para garantir que ele fica.</p>
    <p>E todo backup agendado entra no <code>acessos.log</code>. Sem isso, a única
    prova de que ele rodou seria o arquivo existir.</p>
  </div>''')

# secao 9 (quem pode o que) ganha o nivel
s=s.replace('''  <h3>Dez atividades, base por base</h3>''','''  <h3>Nível: uma palavra no lugar de dez booleanos</h3>

  <p>Escrever dez permissões por base, para cada usuário, é onde alguém erra —
  esquece uma, deixa <code>administrar</code> ligado sem querer, copia a linha
  errada. O nível resolve o caso comum com uma palavra.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Nível</th><th>O que acrescenta ao anterior</th></tr></thead>
      <tbody>
        <tr><td><code>nenhum</code></td><td>nada — e é o <strong>padrão</strong>, porque a regra é negar por omissão</td></tr>
        <tr><td><code>leitor</code></td><td>ler, diário, verificar</td></tr>
        <tr><td><code>operador</code></td><td>inserir, alterar, excluir</td></tr>
        <tr><td><code>dono</code></td><td>criar, reindexar, replicar</td></tr>
        <tr><td><code>admin</code></td><td>administrar: acessos, bloqueios, usuários, backup</td></tr>
      </tbody>
    </table>
  </div>

  <p>A regra de uma base específica <strong>ganha</strong> do nível, inclusive
  para <em>tirar</em> poder. É o que permite dar <code>admin</code> a alguém e
  ainda assim fechar uma base para ele.</p>

  <div class="nota">
    <p><strong>O padrão <code>nenhum</code> não é detalhe.</strong> A primeira
    versão deste campo tinha <code>leitor</code> como padrão, e isso mudava o
    comportamento de todo <code>config.json</code> já existente: base sem regra
    explícita passava de <em>nega tudo</em> para <em>lê tudo</em>. Um teste antigo
    quebrou e apontou o problema. Campo novo com padrão errado afrouxa a
    segurança de quem não mudou nada — e é o tipo de defeito que ninguém
    percebe.</p>
  </div>

  <h3>Dez atividades, base por base</h3>''')

# estado
s=s.replace('''<tr><td>Backup com manifesto SHA-256 e conferência</td><td><span class="pino ok">pronto</span></td><td class="num">7</td></tr>''',
'''<tr><td>Backup com manifesto SHA-256 e conferência</td><td><span class="pino ok">pronto</span></td><td class="num">11</td></tr>
        <tr><td>ZIP com DEFLATE escrito aqui · backup agendado</td><td><span class="pino ok">pronto</span></td><td class="num">10</td></tr>
        <tr><td>Nível de usuário: nenhum, leitor, operador, dono, admin</td><td><span class="pino ok">pronto</span></td><td class="num">6</td></tr>''')
open(p,'w').write(s)
print('dossie ok')
