# Insert Control Center section into dossier
# 27/08 19:59

s = open('docs/dossie/dossie-phxsql.html').read()

secao = '''<!-- ============================= 11 ============================= -->
<section id="s11">
  <div class="rotulo"><span class="num">11</span><span class="traco"></span></div>
  <h2>Centro de Controle: a segunda porta</h2>

  <p>Navegador não abre soquete TCP cru. A porta 5000 fala JSON&nbsp;Lines, e o
  navegador fala HTTP — então existe uma segunda porta, que traduz. É só isso que
  ela é: uma ponte. Ela não serve arquivo do disco, não lista diretório e não
  interpreta caminho. Tem três rotas — <code>GET /</code>, <code>GET /saude</code>,
  <code>POST /api</code> — e nenhuma toca o sistema de arquivos. É a forma mais
  simples de não ter travessia de diretório: não tendo diretório.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 396" role="img" aria-label="Navegador e cliente entram por portas diferentes e guardam identidade de formas diferentes — sessão no HTTP, conexão no TCP — mas convergem no mesmo despachar, com os mesmos quatro portões; a lista de bloqueio e o log de acessos valem nas duas portas">
        <defs>
          <marker id="setaW" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
          <marker id="setaWlog" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5.5" markerHeight="5.5" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="16" font-size="10" opacity=".55" letter-spacing=".08em">PELO NAVEGADOR</text>
          <rect x="16" y="26" width="104" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="68" y="48" text-anchor="middle" font-size="11">navegador</text>
          <text x="68" y="65" text-anchor="middle" font-size="10" opacity=".6">HTTP</text>

          <path d="M120 54 L162 54" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>

          <rect x="166" y="26" width="140" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="236" y="46" text-anchor="middle" font-size="11">porta 5001</text>
          <text x="236" y="62" text-anchor="middle" font-size="10" opacity=".6">página embutida</text>
          <text x="236" y="75" text-anchor="middle" font-size="10" opacity=".6">no binário</text>

          <path d="M306 54 L342 54" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>

          <rect x="346" y="26" width="138" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.6"/>
          <text x="415" y="46" text-anchor="middle" fill="var(--acento)" font-size="11">X-Sessao</text>
          <text x="415" y="62" text-anchor="middle" font-size="10" opacity=".6">48 hex, renova</text>
          <text x="415" y="75" text-anchor="middle" font-size="10" opacity=".6">a cada clique</text>

          <text x="16" y="126" font-size="10" opacity=".55" letter-spacing=".08em">PELA PORTA DE DADOS</text>
          <rect x="16" y="136" width="104" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="68" y="158" text-anchor="middle" font-size="11">cliente</text>
          <text x="68" y="175" text-anchor="middle" font-size="10" opacity=".6">JSON Lines</text>

          <path d="M120 164 L162 164" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>

          <rect x="166" y="136" width="140" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="236" y="156" text-anchor="middle" font-size="11">porta 5000</text>
          <text x="236" y="172" text-anchor="middle" font-size="10" opacity=".6">um pedido</text>
          <text x="236" y="185" text-anchor="middle" font-size="10" opacity=".6">por linha</text>

          <path d="M306 164 L342 164" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>

          <rect x="346" y="136" width="138" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="415" y="156" text-anchor="middle" font-size="11">a conexão</text>
          <text x="415" y="172" text-anchor="middle" font-size="10" opacity=".6">a identidade dura</text>
          <text x="415" y="185" text-anchor="middle" font-size="10" opacity=".6">o que o soquete durar</text>

          <path d="M484 54 L516 92" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>
          <path d="M484 164 L516 126" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>

          <rect x="520" y="72" width="152" height="74" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.8"/>
          <text x="596" y="94" text-anchor="middle" fill="var(--acento)" font-weight="600" font-size="12">despachar()</text>
          <text x="596" y="112" text-anchor="middle" font-size="10" opacity=".7">política · token</text>
          <text x="596" y="126" text-anchor="middle" font-size="10" opacity=".7">login · permissão</text>
          <text x="596" y="140" text-anchor="middle" font-size="9.5" opacity=".55">os mesmos quatro</text>

          <path d="M672 109 L706 109" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaW)"/>

          <rect x="710" y="81" width="114" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="767" y="103" text-anchor="middle" font-size="11">motor</text>
          <text x="767" y="120" text-anchor="middle" font-size="10" opacity=".6">trava única</text>

          <path d="M236 82 L236 232" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaWlog)"/>
          <path d="M236 192 L236 232" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3"/>
          <path d="M596 146 L596 200 L470 200 L470 232" fill="none" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaWlog)"/>

          <rect x="120" y="236" width="500" height="54" rx="4" fill="none" stroke="var(--log)" stroke-width="1.7"/>
          <text x="370" y="258" text-anchor="middle" fill="var(--log)" font-weight="600" font-size="12.5">blacklist.json &#183; acessos.log</text>
          <text x="370" y="277" text-anchor="middle" font-size="10.5" opacity=".7">são do servidor, não da porta — bloqueou numa, bloqueou nas duas</text>

          <line x1="16" y1="314" x2="824" y2="314" stroke="currentColor" stroke-width="1" opacity=".3"/>
          <text x="16" y="338" font-size="11.5" opacity=".75">
            <tspan font-weight="600">A única diferença entre as duas portas é onde a identidade mora.</tspan> HTTP não tem conexão que dure, então ela mora na sessão.
          </text>
          <text x="16" y="360" font-size="11" opacity=".6">É o que faz a conta cara valer a pena uma vez só: o PBKDF2 de 210.000 iterações custa 300 ms no login e 0 ms em cada clique seguinte — medido.</text>
          <text x="16" y="380" font-size="11" opacity=".6">A porta da interface vem desligada, e ligada escuta só em 127.0.0.1. O config recusa subir com as duas portas no mesmo endereço.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 11.</b> As duas setas convergindo no
    <code>despachar()</code> são o desenho inteiro: a interface não tem um caminho
    privilegiado. Quem não pode inserir recebe a mesma recusa, tenha clicado num
    botão ou aberto um soquete.</figcaption>
  </figure>

  <h3>A senha parou de trafegar</h3>

  <p>Em <code>127.0.0.1</code> e em <code>https://</code> o navegador oferece
  <code>crypto.subtle</code>. A página então pede um desafio, deriva a prova com
  PBKDF2 <em>ali mesmo</em> e manda só a prova — é o desafio-resposta da seção 10,
  do lado do navegador. A senha não sai da máquina de quem entra, e gravar o
  diálogo para repetir depois não autentica ninguém: o desafio vale uma vez só.</p>

  <p>Em <code>http://</code> para outra máquina o navegador <strong>não</strong>
  oferece a cifra — não é escolha da página, é regra de contexto seguro. Aí ela cai
  em Base64 e <strong>diz isso na tela</strong>, com todas as letras. Base64 esconde
  a senha de quem olha por cima do ombro, não de quem captura o pacote. A porta da
  interface pertence dentro do túnel, igual à 5000.</p>

  <h3>O que a página mostra</h3>

  <div class="rolo">
    <table>
      <thead><tr><th>Onde</th><th>O que traz</th><th>De onde vem</th></tr></thead>
      <tbody>
        <tr><td>Árvore</td><td>bancos &#8594; tabelas da raiz &#8594; schemas &#8594; tabelas</td><td><code>bancos</code>, <code>tabelas</code></td></tr>
        <tr><td>Estrutura</td><td>colunas, em qual dos cinco arquivos cada uma mora, índices, chaves estrangeiras, paginação</td><td><code>esquema</code></td></tr>
        <tr><td>Conteúdo</td><td>as linhas, na ordem de digitação ou na de qualquer índice</td><td><code>varrer</code></td></tr>
        <tr><td>Índices</td><td>o que há no <code>.ndx</code>, e por que ele é o único que não pagina</td><td><code>esquema</code></td></tr>
        <tr><td>Diário</td><td>quem alterou o quê, quando, em que versão</td><td><code>diario</code></td></tr>
        <tr><td>Integridade</td><td>CRC de cada registro, página, bloco externo e evento</td><td><code>verificar</code></td></tr>
        <tr><td>Usuários</td><td>o cadastro e o poder de cada um sobre cada base</td><td><code>usuarios</code></td></tr>
        <tr><td>Acessos</td><td>IP, data, hora, operação, usuário, resultado</td><td><code>acessos</code></td></tr>
        <tr><td>Bloqueios</td><td>quem está barrado, por quê e até quando</td><td><code>bloqueios</code></td></tr>
      </tbody>
    </table>
  </div>

  <div class="nota">
    <p>Aberta sem servidor na origem, a página percebe pelo <code>GET /saude</code> e
    abre em <strong>modo demonstração</strong>, com dados embutidos e um selo visível
    no topo o tempo todo — para ser avaliada antes de instalar qualquer coisa. É um
    fragmento de propósito: o esqueleto HTML é montado pelo servidor, e o mesmo
    arquivo pode ser publicado na web sem virar uma segunda cópia que diverge.</p>
  </div>
</section>

'''

# renumera 12 -> 13 e 11 -> 12, de tras para frente
s = s.replace('<!-- ============================= 12 ============================= -->\n<section id="s12">\n  <div class="rotulo"><span class="num">12</span>',
              '<!-- ============================= 13 ============================= -->\n<section id="s13">\n  <div class="rotulo"><span class="num">13</span>')
s = s.replace('<!-- ============================= 11 ============================= -->\n<section id="s11">\n  <div class="rotulo"><span class="num">11</span>',
              '<!-- ============================= 12 ============================= -->\n<section id="s12">\n  <div class="rotulo"><span class="num">12</span>')

# insere a nova secao 11 no lugar da antiga
alvo = '<!-- ============================= 12 ============================= -->\n<section id="s12">\n  <div class="rotulo"><span class="num">12</span><span class="traco"></span></div>\n  <h2>Decisões tomadas</h2>'
assert s.count(alvo) == 1, "ancora da secao 12 nao casou"
s = s.replace(alvo, secao + alvo)

# indice
s = s.replace('''    <li><a href="#s11"><span class="n">11</span> Decisões</a></li>
    <li><a href="#s12"><span class="n">12</span> Estado e roteiro</a></li>''',
'''    <li><a href="#s11"><span class="n">11</span> Centro de Controle</a></li>
    <li><a href="#s12"><span class="n">12</span> Decisões</a></li>
    <li><a href="#s13"><span class="n">13</span> Estado e roteiro</a></li>''')

open('docs/dossie/dossie-phxsql.html','w').write(s)
print("secao 11 inserida")
