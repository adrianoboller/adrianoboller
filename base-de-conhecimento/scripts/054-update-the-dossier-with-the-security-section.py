# Update the dossier with the security section
# 27/08 19:12

p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/artefato/dossie-phxsql.html"
s=open(p).read()

s=s.replace('<div><div class="v">10.241</div><div class="r">linhas de Rust</div></div>',
            '<div><div class="v">12.400</div><div class="r">linhas de Rust</div></div>')
s=s.replace('<div><div class="v">137</div><div class="r">testes</div></div>',
            '<div><div class="v">166</div><div class="r">testes</div></div>')
s=s.replace('<div><div class="v">1.624</div><div class="r">linhas de doc</div></div>',
            '<div><div class="v">2.000</div><div class="r">linhas de doc</div></div>')

# indice: seccao nova
s=s.replace('''    <li><a href="#s9"><span class="n">09</span> Decisões</a></li>''',
            '''    <li><a href="#s9"><span class="n">09</span> Quem pode o quê</a></li>
    <li><a href="#s10"><span class="n">10</span> Decisões</a></li>''')
s=s.replace('''    <li><a href="#s10"><span class="n">10</span> Estado e roteiro</a></li>''',
            '''    <li><a href="#s11"><span class="n">11</span> Estado e roteiro</a></li>''')

# renumera as duas ultimas seccoes
s=s.replace('<section id="s9">\n  <div class="rotulo"><span class="num">09</span>',
            '<section id="s10">\n  <div class="rotulo"><span class="num">10</span>')
s=s.replace('<section id="s10">\n  <div class="rotulo"><span class="num">10</span><span class="traco"></span></div>\n  <h2>Estado e roteiro</h2>',
            '<section id="s11">\n  <div class="rotulo"><span class="num">11</span><span class="traco"></span></div>\n  <h2>Estado e roteiro</h2>')

nova = '''<!-- ============================= 09 ============================= -->
<section id="s9">
  <div class="rotulo"><span class="num">09</span><span class="traco"></span></div>
  <h2>Quem pode o quê</h2>

  <p>O cadastro de usuários mora no <code>config.json</code> — nome completo, login,
  e-mail, telefone, supervisor e o poder sobre cada base. A senha, não: o que fica
  gravado é o <strong>hash</strong>.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 300" role="img" aria-label="Um pedido passa por três portões em ordem: o token verifica a rede, o login verifica a identidade e a permissão verifica o poder sobre aquela base">
        <defs>
          <marker id="setaP" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
          <marker id="setaN" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">
          <rect x="16" y="44" width="96" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="64" y="70" text-anchor="middle" font-size="11">pedido</text>

          <path d="M112 66 L152 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaP)"/>

          <rect x="156" y="36" width="150" height="60" rx="4" fill="none" stroke="currentColor" stroke-width="1.5"/>
          <text x="231" y="56" text-anchor="middle" font-size="11" font-weight="600">1 · TOKEN</text>
          <text x="231" y="73" text-anchor="middle" font-size="10.5" opacity=".7">a chave da porta</text>
          <text x="231" y="87" text-anchor="middle" font-size="10.5" opacity=".7">da rede</text>

          <path d="M306 66 L346 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaP)"/>

          <rect x="350" y="36" width="150" height="60" rx="4" fill="none" stroke="currentColor" stroke-width="1.5"/>
          <text x="425" y="56" text-anchor="middle" font-size="11" font-weight="600">2 · LOGIN</text>
          <text x="425" y="73" text-anchor="middle" font-size="10.5" opacity=".7">a identidade</text>
          <text x="425" y="87" text-anchor="middle" font-size="10.5" opacity=".7">uma vez por conexão</text>

          <path d="M500 66 L540 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaP)"/>

          <rect x="544" y="36" width="150" height="60" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="619" y="56" text-anchor="middle" fill="var(--acento)" font-size="11" font-weight="600">3 · PERMISSÃO</text>
          <text x="619" y="73" text-anchor="middle" font-size="10.5" opacity=".7">o poder nesta</text>
          <text x="619" y="87" text-anchor="middle" font-size="10.5" opacity=".7">base</text>

          <path d="M694 66 L734 66" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaP)"/>
          <rect x="738" y="44" width="86" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="781" y="70" text-anchor="middle" font-size="11">executa</text>

          <path d="M231 96 L231 132" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaN)"/>
          <path d="M425 96 L425 132" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaN)"/>
          <path d="M619 96 L619 132" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaN)"/>

          <rect x="150" y="136" width="546" height="34" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5"/>
          <text x="423" y="157" text-anchor="middle" fill="var(--log)" font-size="11.5">a negativa de qualquer portão entra no acessos.log, com IP, hora e login</text>

          <line x1="16" y1="198" x2="824" y2="198" stroke="currentColor" stroke-width="1" opacity=".3"/>
          <text x="16" y="222" font-size="11.5" opacity=".78" font-weight="600">Sem cadastro nenhum, o token dá poder total — o comportamento anterior.</text>
          <text x="16" y="244" font-size="11.5" opacity=".72">Cadastrar usuários faz o portão 2 passar a valer. Ou seja: cadastrar <tspan font-weight="600">só aperta</tspan>, nunca afrouxa.</text>
          <text x="16" y="274" font-size="11" opacity=".6">A senha é conferida com PBKDF2 de 210.000 iterações, que custa ~100 ms de propósito: irrelevante uma vez por conexão,</text>
          <text x="16" y="290" font-size="11" opacity=".6">inviável a cada pedido. É por isso que a identidade fica presa à conexão, e não ao pedido.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 8.</b> Os três portões são independentes e cumulativos. O
    token diz que você chegou ao servidor certo; o login diz quem você é; a permissão diz
    o que você pode naquela base específica.</figcaption>
  </figure>

  <h3>A senha nunca fica em texto puro</h3>
  <p>Um <code>config.json</code> vai para backup, para o Git, para o anexo de um chamado
  de suporte. Um hash nesses lugares é um aborrecimento; uma senha é um incidente.</p>

<pre>echo -n 'a senha de verdade' | phxsqld --senha
<b>"senha_hash"</b>: "pbkdf2-sha256$210000$7570c880e8815d94...$becbc17c5d0efce2..."
                    ^         ^        ^                    ^
                    algoritmo iterações sal de 16 bytes      derivado da senha</pre>

  <p>SHA-256, HMAC e PBKDF2 foram escritos neste projeto para não quebrar a regra de zero
  dependências, e são conferidos contra os vetores oficiais — FIPS 180-4, RFC 4231 e os
  vetores usuais de PBKDF2. O número de iterações viaja dentro da própria linha, então
  aumentar o custo amanhã não invalida as senhas de hoje.</p>

  <h3>Dez atividades, base por base</h3>
  <div class="rolo">
    <table>
      <thead><tr><th>Atividade</th><th>Cobre</th></tr></thead>
      <tbody>
        <tr><td class="dado">ler</td><td>bancos, tabelas, esquema, ler, varrer, buscar</td></tr>
        <tr><td class="dado">inserir</td><td>inserir</td></tr>
        <tr><td class="dado">alterar</td><td>atualizar</td></tr>
        <tr><td class="dado">excluir</td><td>excluir</td></tr>
        <tr><td class="dado">criar</td><td>criar_database, criar_schema</td></tr>
        <tr><td class="dado">reindexar</td><td>reindexar</td></tr>
        <tr><td class="dado">diario</td><td>diario</td></tr>
        <tr><td class="dado">verificar</td><td>verificar</td></tr>
        <tr><td class="dado">administrar</td><td>acessos, ips, config, usuarios</td></tr>
        <tr><td class="dado">replicar</td><td>posicao, replicar</td></tr>
      </tbody>
    </table>
  </div>

  <div class="nota">
    <span class="t">Nega por omissão, em toda parte</span>
    <p>Atividade que não aparece vale <code>false</code>. A base listada manda — o
    curinga <code>"*"</code> não completa o que faltou nela. Base ausente e sem curinga
    nega tudo. E <strong>operação desconhecida exige <code>administrar</code></strong>:
    uma op nova que alguém esqueça de mapear é negada, não liberada.</p>
  </div>

  <p>O rastro fecha nos dois registros: o <code>acessos.log</code> guarda o login de toda
  tentativa, inclusive as negadas, e o <code>.log</code> da tabela guarda o <em>id</em>
  numérico de quem alterou o dado — o campo <code>usuario</code> existia no formato desde
  o primeiro dia e só agora carrega sentido.</p>
</section>

'''
s=s.replace('<!-- ============================= 09 ============================= -->\n<section id="s10">', nova + '<!-- ============================= 10 ============================= -->\n<section id="s10">')

# linhas novas na tabela de estado
s=s.replace('''        <tr><td>Replicação — <code>.log</code> v2 com imagem da linha</td>''',
'''        <tr><td>Cadastro de usuários, senha em hash, permissão por base</td><td><span class="pino ok">pronto</span></td><td class="num">29</td></tr>
        <tr><td>Replicação — <code>.log</code> v2 com imagem da linha</td>''')
s=s.replace('''<tr><td>Servidor TCP na porta 5000 · <code>config.json</code></td><td><span class="pino ok">pronto</span></td><td class="num">23</td></tr>''',
            '''<tr><td>Servidor TCP na porta 5000 · <code>config.json</code></td><td><span class="pino ok">pronto</span></td><td class="num">23</td></tr>''')
s=s.replace('''    <li><strong>Sem TLS.</strong> O protocolo trafega em claro e depende do túnel.</li>''',
'''    <li><strong>Sem TLS.</strong> O protocolo trafega em claro — inclusive a senha no
    <code>login</code> — e depende do túnel.</li>
    <li><strong>Sem troca de senha pelo protocolo</strong>, sem bloqueio por tentativas
    e sem grupos: o poder é por usuário, e a senha muda no <code>config.json</code>.</li>''')

s=s.replace('''PhxSql 0.1.0 · 10.241 linhas de Rust em 4 crates · 137 testes · nenhuma dependência
  externa. Especificação byte a byte em <code>docs/FORMATO.md</code>, desenho da
  replicação em <code>docs/REPLICACAO.md</code>, roteiro em <code>docs/PLANO.md</code>.''',
'''PhxSql 0.2.0 · 12.400 linhas de Rust em 4 crates · 166 testes · nenhuma dependência
  externa. Especificação byte a byte em <code>docs/FORMATO.md</code>, cadastro e
  permissões em <code>docs/USUARIOS.md</code>, desenho da replicação em
  <code>docs/REPLICACAO.md</code>, roteiro em <code>docs/PLANO.md</code>.''')
s=s.replace('<div class="selo">Dossiê técnico · versão 0.1.0</div>',
            '<div class="selo">Dossiê técnico · versão 0.2.0</div>')
open(p,'w').write(s)
print("dossie atualizado")
