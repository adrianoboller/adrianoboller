# Escrever as secoes novas do dossie
# 29/08 03:11

import io
p='docs/dossie/dossie-phxsql-0.15.html'
s=io.open(p,encoding='utf-8').read()

# --- 1. capa: o resumo estava parado na 18,5 us e em tres itens ---
velho = """  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.17.0</strong>,
  refeita contra o código. Ela fecha três itens da lista do que faltava, e nenhum
  deles é recurso inventado aqui: a <a href="#s7">janela de conflito de escrita</a>,
  o <a href="#s10">direito no nível da tabela</a> — os dois apontados pela leitura
  do HFSQL(R) — e o ataque ao custo da inserção, que a medição
  <a href="#s21">redirecionou no meio do caminho</a>: o item pedia «ordene as
  chaves do lote», a desordem custava 1,06&#215;, e o que estava caro era o
  <strong>CRC-32 de página inteira</strong> relido a cada descida da árvore. Um
  cache de páginas no <code>.ndx</code> levou a inserção de 44,4 para
  18,5&#8201;µs por linha, e a bancada de dez milhões de 884 para
  <strong>303 segundos</strong>.</p>"""
novo = """  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.17.0</strong>,
  refeita contra o código. Ela fecha quatro itens da lista do que faltava, e nenhum
  deles é recurso inventado aqui: a <a href="#s7">janela de conflito de escrita</a>,
  o <a href="#s10">direito no nível da tabela</a> — os dois apontados pela leitura
  do HFSQL(R) —, o <a href="#s21">BULKINSERT</a> que reserva a tabela para quem
  carrega, e o ataque ao custo da inserção, que a medição
  <a href="#s21">redirecionou no meio do caminho</a>: o item pedia «ordene as
  chaves do lote», a desordem custava 1,06&#215;, e o que estava caro era o
  <strong>CRC-32 de página inteira</strong> relido a cada descida da árvore. Três
  cortes depois a inserção saiu de 44,4 para <strong>15,9&#8201;µs por linha
  (2,79&#215;)</strong>, e a bancada de dez milhões de 884 para
  <strong>303 segundos</strong>.</p>"""
assert s.count(velho)==1, 'capa'
s=s.replace(velho,novo)

# --- 2. as tres secoes novas do capitulo 21 ---
ancora = """  <h3>A inserção ainda é onde o MySQL(R) ganha — e 2,9&#215; já voltaram</h3>"""
assert s.count(ancora)==1, 'ancora'

novas = """  <h3>E o <code>.log</code>, que gravava duas vezes por evento</h3>

  <p>O diário fazia <strong>duas escritas por evento</strong>: os 44 bytes do
  evento em si, e os 64 bytes do cabeçalho com <code>fim</code> e
  <code>qtd_eventos</code>. O evento tem de ir na hora — ele é a história, e é a
  posição de que a replicação depende. O cabeçalho é um <em>contador</em>, e a
  leitura sabe recalculá-lo varrendo os próprios eventos.</p>

  <p>Ele passou a ir no <code>sincronizar</code>: <strong>1,22 → 0,67&#8201;µs por
  evento (1,82&#215;)</strong>, e a inserção completa com dois índices de
  <strong>17,0 para 15,9&#8201;µs</strong>.</p>

  <div class="nota">
    <span class="t">Adiar o contador pediu um caminho de reparo</span>
    <p>Uma queda antes do <code>sincronizar</code> deixaria o cabeçalho atrasado, e
    a próxima gravação escreveria <strong>por cima</strong> dos eventos já
    gravados — evento destruído, e não apenas invisível. Então <code>abrir</code>
    varre para a frente a partir do <code>fim</code> gravado, validando cada evento
    pelo CRC-32 que ele já carrega, e para no primeiro que não confere.</p>
    <p><strong>Segurar os eventos em RAM continua fora</strong>, e a razão não é de
    tamanho — mediu-se 4,2% — e sim de natureza: índice perdido se reconstrói do
    <code>.reg</code>; evento perdido não se reconstrói.</p>
  </div>

  <h3>E o Profiler desligado, que custava 7% de uma carga</h3>

  <p>O ponto de captura fazia o trabalho <strong>antes</strong> de conferir se
  havia o que capturar: dois <code>Json::analisar</code> do corpo inteiro, três
  <code>String</code> e um mutex, para no fim descobrir que o Profiler estava
  desligado e devolver nada. Num <code>inserir_lote</code> de 5.000 linhas isso é
  analisar meio megabyte de JSON <em>duas vezes</em>, para nada. O portão virou um
  <code>AtomicBool</code> lido antes de qualquer trabalho: <strong>40.600 →
  43.450 linhas/s</strong> na carga pela rede.</p>

  <div class="nota">
    <span class="t">Diagnóstico plausível não é diagnóstico medido</span>
    <p>Estava escrito em três lugares deste projeto que «o mutex era o pior pedaço,
    porque ele serializa». Medido, em <code>--example quem-custava</code>: um
    <code>lock</code>/<code>unlock</code> sem disputa custa
    <strong>13,2&#8201;ns</strong>; analisar o corpo de um lote custa
    <strong>3.456&#8201;µs</strong>. São <strong>262.000&#215;</strong> — o mutex
    nunca foi o problema. O errado sobreviveu justamente porque o conserto
    <em>funcionou</em>, por outro motivo.</p>
  </div>

  <h3><code>BULKINSERT</code>: a tabela reservada, e a janela que não fecha</h3>

  <p>Uma carga longa quer duas coisas que o servidor não dava: ninguém mais
  mexendo naquela tabela enquanto ela entra, e <strong>uma sincronização só</strong>,
  no fim. As duas saem da mesma reserva:</p>

  <div class="rolo">
    <table>
      <tbody>
        <tr><td><code>{"op":"bulkinsert",…,"ligado":true}</code></td><td>a tabela passa a ser sua</td></tr>
        <tr><td>… as inserções …</td><td>a janela de durabilidade não fecha</td></tr>
        <tr><td><code>{"op":"bulkinsert",…,"ligado":false}</code></td><td>um <code>fsync</code>, e a tabela volta</td></tr>
      </tbody>
    </table>
  </div>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 250" role="img" aria-label="Sem reserva a janela de durabilidade fecha várias vezes durante a carga, e cada fechamento é um fsync; com a reserva ela não fecha, e a carga inteira termina num fsync só, o que mede 1,53 vezes mais rápido">
        <defs>
          <marker id="setaCg" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="20" font-size="10" opacity=".55" letter-spacing=".08em">SEM RESERVA — A JANELA FECHA SOZINHA</text>
          <line x1="16" y1="60" x2="700" y2="60" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaCg)"/>
          <text x="16" y="46" font-size="10" opacity=".6">as 5.000 linhas do lote</text>
          <g stroke="currentColor" stroke-width="1.6">
            <line x1="120" y1="50" x2="120" y2="70"/>
            <line x1="236" y1="50" x2="236" y2="70"/>
            <line x1="352" y1="50" x2="352" y2="70"/>
            <line x1="468" y1="50" x2="468" y2="70"/>
            <line x1="584" y1="50" x2="584" y2="70"/>
            <line x1="700" y1="50" x2="700" y2="70"/>
          </g>
          <text x="120" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="236" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="352" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="468" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="584" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="700" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="726" y="64" font-size="12" font-weight="600">43.500/s</text>

          <line x1="16" y1="118" x2="824" y2="118" stroke="currentColor" stroke-width="1" opacity=".25"/>

          <text x="16" y="146" font-size="10" fill="var(--acento)" letter-spacing=".08em">COM A RESERVA — A JANELA NÃO FECHA</text>
          <line x1="16" y1="186" x2="700" y2="186" stroke="var(--acento)" stroke-width="1.6" marker-end="url(#setaCg)"/>
          <text x="16" y="172" font-size="10" opacity=".6">as mesmas 5.000 linhas</text>
          <line x1="700" y1="176" x2="700" y2="196" stroke="var(--acento)" stroke-width="1.8"/>
          <text x="700" y="212" text-anchor="middle" font-size="10" fill="var(--acento)">um fsync</text>
          <text x="726" y="190" font-size="12" font-weight="600" fill="var(--acento)">66.500/s</text>

          <text x="16" y="238" font-size="10" opacity=".6">Os outros que pedirem esta tabela recebem 4002 EM_CARGA na hora, dizendo quem reservou e desde quando.</text>
        </g>
      </svg>
    </div>
    <figcaption>O ganho não vem de escrever menos: vem de <strong>confirmar
    menos vezes</strong>. <strong>1,53&#215; medido</strong> — 43.044 e 44.026
    linhas/s sem reserva contra 65.737 e 67.339 com ela, dois pares de
    corridas.</figcaption>
  </figure>

  <p>Os outros recebem <strong>erro na hora</strong>, e não espera: o novo
  <strong>4002 <code>EM_CARGA</code></strong>, dizendo <strong>quem</strong>
  reservou e <strong>desde quando</strong> — sem isso, «tabela em carga» manda a
  pessoa procurar sozinha quem está segurando. Ele vem com <code>repetir: true</code>,
  e é o <strong>segundo</strong> erro do protocolo que pede nova tentativa (o outro
  é o de E/S): é o que separa «espere um pouco» de «você não pode».</p>

  <div class="nota">
    <span class="t">Contra reserva órfã há duas redes, e não uma</span>
    <p>A <strong>queda da conexão</strong> solta na hora, por qualquer caminho de
    saída. Mas o soquete que fica pendurado <em>vivo</em>, com o cliente morto do
    outro lado, é exatamente o caso em que ela não pega — e aí vale o
    <strong>prazo</strong>, <code>recursos.carga_prazo_min</code>, padrão 30
    minutos, ajustável no <code>config.json</code> e visível na tela de
    configuração, que também lista as cargas em andamento.</p>
    <p>Foi a prova pelo soquete, em <code>bancada/carga/bulkinsert.py</code>, que
    achou o que dez testes unitários não achavam — e o primeiro resultado dela
    <em>acusava</em> a queda da conexão de não soltar. O defeito estava no teste:
    o <code>makefile()</code> do Python segura o descritor, então fechar só o
    soquete deixa o servidor sem ver o fim. <strong>Teste que passa por engano é
    pior que teste que falta.</strong></p>
  </div>

  <div class="nota">
    <span class="t">Não é transação, e o documento repete isso alto</span>
    <p><code>BULKINSERT</code> <strong>reserva a tabela; não desfaz nada</strong>.
    Quem ler «exclusiva até concluir» e entender <code>BEGIN</code> vai perder
    dado. <code>docs/SQL.md</code> registra isso junto com as três coisas que o
    analisador de SQL não vai poder tratar como açúcar sintático: é palavra
    reservada; vale para a <strong>sessão</strong>, e não para o comando, então um
    driver que multiplexa conexões quebra a exclusividade sem avisar; e o
    <code>EM_CARGA</code> tem de virar <em>serialization failure</em> no SQLSTATE,
    e não <em>access denied</em>, senão o driver do outro lado desiste em vez de
    repetir.</p>
  </div>

"""
s=s.replace(ancora, novas+ancora)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
