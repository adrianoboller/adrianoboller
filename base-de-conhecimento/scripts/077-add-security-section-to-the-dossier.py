# Add security section to the dossier
# 27/08 19:33

p="docs/dossie/dossie-phxsql.html"
s=open(p).read()

s=s.replace('<div class="v">11.775</div>','<div class="v">13.334</div>')
s=s.replace('<div class="v">166</div>','<div class="v">197</div>')
s=s.replace('<div class="v">1.918</div>','<div class="v">2.292</div>')
s=s.replace('PhxSql 0.2.0 · 11.775 linhas de Rust','PhxSql 0.2.0 · 13.334 linhas de Rust')

# indice
s=s.replace('''    <li><a href="#s10"><span class="n">10</span> Decisões</a></li>
    <li><a href="#s11"><span class="n">11</span> Estado e roteiro</a></li>''',
'''    <li><a href="#s10"><span class="n">10</span> Quem não entra</a></li>
    <li><a href="#s11"><span class="n">11</span> Decisões</a></li>
    <li><a href="#s12"><span class="n">12</span> Estado e roteiro</a></li>''')

# renumera as duas ultimas
s=s.replace('<section id="s11">\n  <div class="rotulo"><span class="num">11</span>',
            '<section id="s12">\n  <div class="rotulo"><span class="num">12</span>')
s=s.replace('<section id="s10">\n  <div class="rotulo"><span class="num">10</span>',
            '<section id="s11">\n  <div class="rotulo"><span class="num">11</span>')

nova = '''<!-- ============================= 10 ============================= -->
<section id="s10">
  <div class="rotulo"><span class="num">10</span><span class="traco"></span></div>
  <h2>Quem não entra</h2>

  <p>Antes dos três portões da seção anterior existe um portão zero — a
  <strong>política</strong> — e antes dele a <strong>lista de bloqueio</strong>.
  A política vale para todo mundo, root incluso: não é permissão de usuário, é o
  que este servidor não faz por esta porta.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 330" role="img" aria-label="Uma conexão passa primeiro pela lista de bloqueio e depois pela política de comandos proibidos; violação grave bloqueia o IP na hora e violação leve conta tentativas até o limite">
        <defs>
          <marker id="setaB1" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
          <marker id="setaB2" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">
          <rect x="16" y="40" width="96" height="42" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="64" y="66" text-anchor="middle" font-size="11">conexão</text>

          <path d="M112 61 L150 61" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaB1)"/>

          <rect x="154" y="32" width="150" height="58" rx="4" fill="none" stroke="var(--log)" stroke-width="1.6"/>
          <text x="229" y="52" text-anchor="middle" fill="var(--log)" font-size="11" font-weight="600">blacklist.json</text>
          <text x="229" y="69" text-anchor="middle" font-size="10.5" opacity=".7">está na lista?</text>
          <text x="229" y="83" text-anchor="middle" font-size="10" opacity=".55">antes de tudo</text>

          <path d="M304 61 L342 61" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaB1)"/>

          <rect x="346" y="32" width="160" height="58" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="426" y="52" text-anchor="middle" fill="var(--acento)" font-size="11" font-weight="600">0 · POLÍTICA</text>
          <text x="426" y="69" text-anchor="middle" font-size="10.5" opacity=".7">comando proibido?</text>
          <text x="426" y="83" text-anchor="middle" font-size="10" opacity=".55">vale até para o root</text>

          <path d="M506 61 L544 61" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaB1)"/>

          <rect x="548" y="40" width="270" height="42" rx="4" fill="none" stroke="currentColor" stroke-width="1.3" opacity=".65"/>
          <text x="683" y="66" text-anchor="middle" font-size="11" opacity=".75">token → login → permissão</text>

          <path d="M229 90 L229 140" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="4 3" marker-end="url(#setaB2)"/>
          <text x="243" y="118" font-size="10.5" fill="var(--log)">recusa</text>

          <path d="M426 90 L426 140" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaB2)"/>
          <text x="440" y="112" font-size="10.5" fill="var(--log)">GRAVE:</text>
          <text x="440" y="126" font-size="10.5" fill="var(--log)">bloqueia na hora</text>

          <path d="M683 82 L683 120 L560 120 L560 140" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="4 3" marker-end="url(#setaB2)"/>
          <text x="600" y="112" font-size="10.5" fill="var(--log)">LEVE: conta</text>

          <rect x="154" y="144" width="500" height="46" rx="4" fill="none" stroke="var(--log)" stroke-width="1.6"/>
          <text x="404" y="164" text-anchor="middle" fill="var(--log)" font-size="11.5" font-weight="600">blacklist.json — ip · data e hora · motivo · comando · até quando</text>
          <text x="404" y="181" text-anchor="middle" font-size="10.5" opacity=".7">e opcionalmente uma regra de firewall, sem shell, com o IP validado</text>

          <line x1="16" y1="216" x2="824" y2="216" stroke="currentColor" stroke-width="1" opacity=".3"/>
          <text x="16" y="240" font-size="11.5" opacity=".78" font-weight="600">Duas gravidades, porque errar a senha uma vez é humano e pedir o proibido não é.</text>
          <circle cx="26" cy="264" r="3" fill="var(--acento)"/>
          <text x="40" y="268" font-size="11.5"><tspan font-weight="600">Grave</tspan> — comando ou base proibida: bloqueia na hora, sem dar cinco chances.</text>
          <circle cx="26" cy="290" r="3" fill="var(--acento)"/>
          <text x="40" y="294" font-size="11.5"><tspan font-weight="600">Leve</tspan> — token, senha ou IP de fora: conta na janela e bloqueia ao passar do limite.</text>
          <text x="16" y="320" font-size="11" opacity=".6">O bloqueio nunca depende do firewall: um IP na lista é recusado dentro do servidor, sem root e sem poder falhar.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 9.</b> A lista de bloqueio vem antes do token — quem
    está de fora não gasta nem um PBKDF2 do servidor. A regra de firewall é o
    extra à direita, não o mecanismo.</figcaption>
  </figure>

  <h3>Base64 não é criptografia</h3>

  <p>Vale dizer antes de tudo, porque a confusão é comum e cara:</p>

<pre>$ echo 'YWRyaWFubzpzZW5oYTEyMw==' | base64 -d
<b>adriano:senha123</b></pre>

  <p>O <code>login</code> aceita <code>senha_b64</code>, e isso resolve coisas
  reais — a senha some do <code>grep</code> casual e do olho de quem passa atrás
  da cadeira, e senha com aspas atravessa o JSON sem escape. Mas <strong>não
  protege a senha na rede</strong>, e achar que protege é o que leva alguém a
  expor a porta 5000 fora do túnel.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 260" role="img" aria-label="Comparação do que trafega na rede em cada forma de login: em texto puro vai a senha legível, em Base64 vai a senha codificada e decodificável, e no desafio-resposta vai apenas uma prova que não revela a senha">
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">
          <text x="16" y="24" font-size="11" opacity=".6">O QUE UM SNIFFER VÊ, EM CADA FORMA</text>

          <rect x="16" y="38" width="808" height="58" rx="4" fill="none" stroke="currentColor" stroke-width="1.2" opacity=".5"/>
          <text x="32" y="58" font-size="11" opacity=".7">texto puro</text>
          <text x="150" y="58" font-size="11.5">"senha":"Senha Do Adriano"</text>
          <text x="150" y="76" font-size="10.5" fill="var(--log)">a senha, legível</text>
          <text x="150" y="90" font-size="10" opacity=".5">quem capturou, tem</text>

          <rect x="16" y="102" width="808" height="58" rx="4" fill="none" stroke="currentColor" stroke-width="1.2" opacity=".5"/>
          <text x="32" y="122" font-size="11" opacity=".7">Base64</text>
          <text x="150" y="122" font-size="11.5">"senha_b64":"U2VuaGEgRG8gQWRyaWFubw=="</text>
          <text x="150" y="140" font-size="10.5" fill="var(--log)">a senha, codificada</text>
          <text x="150" y="154" font-size="10" opacity=".5">quem capturou, tem — um comando resolve</text>

          <rect x="16" y="166" width="808" height="62" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="32" y="186" font-size="11" fill="var(--acento)" font-weight="600">desafio</text>
          <text x="150" y="186" font-size="11.5">"prova":"fdf0feed2316334ac59a0d79..."</text>
          <text x="150" y="204" font-size="10.5" fill="var(--acento)">uma assinatura, não a senha</text>
          <text x="150" y="219" font-size="10" opacity=".55">só serve para aquele nonce, que vale uma vez</text>

          <text x="16" y="250" font-size="11" opacity=".6">Nas três, o RESTO do tráfego continua em claro. Isso é problema do túnel, não do login.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 10.</b> A diferença que importa está na terceira
    linha: a prova não revela a senha e não serve de novo, porque o nonce do
    servidor é sorteado a cada desafio e vale uma vez.</figcaption>
  </figure>

  <div class="nota">
    <span class="t">Sem nada de proprietário</span>
    <p>O desafio-resposta é PBKDF2 mais HMAC-SHA256, e um cliente Python com
    <code>hashlib</code> e <code>hmac</code> da biblioteca padrão autentica —
    foi assim que o teste ao vivo confirmou. Quem quiser escrever um cliente em
    qualquer linguagem não precisa de biblioteca nenhuma deste projeto.</p>
  </div>

  <h3>A regra de firewall é um extra, não o mecanismo</h3>
  <p>Um IP na lista é recusado <em>dentro do servidor</em> — sem firewall, sem
  root, sem poder falhar. A regra de <code>iptables</code> vem desligada, roda
  <strong>sem shell</strong> com o comando vindo inteiro do
  <code>config.json</code> como lista de argumentos, e o IP é validado como
  endereço antes de entrar no lugar do <code>{ip}</code>. Há teste recusando
  <code>"; rm -rf /"</code> e <code>"$(whoami)"</code>. Se o comando falhar, o
  bloqueio continua valendo e a falha vira aviso — firewall quebrado não vira
  porta aberta.</p>
</section>

'''
s=s.replace('<!-- ============================= 10 ============================= -->\n<section id="s11">', nova + '<!-- ============================= 11 ============================= -->\n<section id="s11">')

s=s.replace('''        <tr><td>Cadastro de usuários, senha em hash, permissão por base</td><td><span class="pino ok">pronto</span></td><td class="num">29</td></tr>''',
'''        <tr><td>Cadastro de usuários, senha em hash, permissão por base</td><td><span class="pino ok">pronto</span></td><td class="num">29</td></tr>
        <tr><td>Login por desafio-resposta e por Base64</td><td><span class="pino ok">pronto</span></td><td class="num">15</td></tr>
        <tr><td>Política, blacklist e gancho de firewall</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>''')
s=s.replace('''    <li><strong>Sem troca de senha pelo protocolo</strong>, sem bloqueio por tentativas
    e sem grupos: o poder é por usuário, e a senha muda no <code>config.json</code>.</li>''',
'''    <li><strong>Sem troca de senha pelo protocolo</strong> e sem grupos: o poder é por
    usuário, e a senha muda no <code>config.json</code>.</li>''')
s=s.replace('''    <li><strong>Sem TLS.</strong> O protocolo trafega em claro — inclusive a senha no
    <code>login</code> — e depende do túnel.</li>''',
'''    <li><strong>Sem TLS.</strong> O tráfego vai em claro e depende do túnel. A
    credencial já não vai, quando se usa o desafio-resposta; os dados, sim.</li>''')
s=s.replace('''      cadastro e
  permissões em <code>docs/USUARIOS.md</code>''','''      cadastro e
  permissões em <code>docs/USUARIOS.md</code>, política e bloqueio em
  <code>docs/SEGURANCA.md</code>''')
open(p,'w').write(s)
print("dossie: secao 10 acrescentada, numeros conferidos")
