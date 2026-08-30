# Add the pagination section to the dossier
# 28/08 19:10

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()

# renumera figuras 9..24 -> 10..25
for n in range(24, 8, -1):
    velho = f"<b>Figura {n}.</b>"
    assert velho in s, velho
    s = s.replace(velho, f"<b>Figura {n+1}.</b>", 1)

velho='''  <code>u64</code> resolveram.</p>
</section>

<!-- ============================= 06 ============================= -->'''
novo='''  <code>u64</code> resolveram.</p>

  <h3>A partição alfanumérica: um arquivo por letra</h3>

  <p>Um terceiro modo, e o que ele muda é maior que os outros dois. A linha não
  vai para o volume corrente: vai para o <strong>volume dela</strong>.</p>

  <div class="rolo">
<pre><code>cadastroClientes_A.reg   cadastroClientes_0.reg
cadastroClientes_B.reg   cadastroClientes_1.reg    cadastroClientes_Outros.reg
…                        …
cadastroClientes_Z.reg   cadastroClientes_9.reg</code></pre>
  </div>

  <p>São <strong>37 volumes fixos</strong> — A a Z, 0 a 9 e <code>Outros</code>
  — e o rowid é <em>atribuído</em> assim:</p>

  <div class="rolo">
<pre><code>rowid = (balde − 1) × registros_por_arquivo + slot_no_balde</code></pre>
  </div>

  <p>Que é a <strong>inversa exata</strong> da conta que o
  <code>localizar</code> já fazia. E é isso que faz o desenho funcionar:
  <strong>nenhum caminho de leitura mudou</strong> — <code>localizar</code>
  continua devolvendo (volume, offset) por divisão, o <code>.ndx</code> continua
  guardando rowid sem saber que balde existe, e o espelho também não muda. Só a
  <em>atribuição</em> mudou.</p>

  <div class="nota">
    <p><strong>A ordem de digitação não se perde — ela muda de campo.</strong>
    Esta é a decisão que a regra da casa exigia discutir. O que se perde é o
    rowid ser crescente na ordem de chegada: com os baldes, o rowid diz em que
    <em>arquivo</em> a linha está, e duas linhas digitadas em seguida caem em
    arquivos diferentes. Dentro de cada volume a ordem continua sendo a de
    digitação, e slot excluído continua sem ser reaproveitado.</p>
    <p>A ordem global fica na coluna de sistema <code>rownum</code>, que existe
    em toda tabela. <strong>Sem ela este modo seria uma quebra da regra; com
    ela, é uma troca de campo</strong> — e foi por isso que as duas coisas
    entraram juntas.</p>
  </div>

  <p>Três consequências que quem escolhe este modo precisa saber. O
  <strong>teto passa a ser por letra</strong>: num cadastro brasileiro o
  <code>_S</code> tem dez vezes o <code>_K</code>, e quem enche primeiro derruba
  a inserção daquela letra com as outras 36 ainda com espaço — por isso o erro
  diz <em>qual</em> balde encheu. <strong>Alterar a coluna de referência é
  recusado</strong>, porque mudaria o arquivo em que a linha mora e com ele o
  rowid, que é a identidade dela em todo índice. E <strong>só o
  <code>.reg</code> leva a letra no nome</strong>: um
  <code>Clientes_B.log</code> se leria como «o diário do balde B», e o diário é
  da tabela inteira.</p>

  <p>Ao lado da tabela nasce o <code>.pag</code>, um JSON que descreve a
  partição — o modo, a coluna de referência, a conta do endereço por extenso, e
  o que cada balde tem — para quem está do lado de fora ler sem abrir o
  <code>.reg</code>. Ele é <strong>gerado, e o motor nunca o lê</strong>: uma
  segunda cópia seria uma segunda verdade, e aqui a divergência diria em que
  <em>arquivo</em> a linha está.</p>
</section>

<!-- ============================= 05b ============================ -->
<section id="s5b">
  <div class="rotulo"><span class="num">05</span><span class="traco"></span></div>
  <h2>Paginação: o cursor sai de graça</h2>

  <p>Num motor relacional, pular para o meio de uma tabela grande exige um
  índice: a ordem lógica não tem nada a ver com a posição física, e o
  <code>OFFSET</code> não tem escolha senão contar e descartar. Aqui a ordem
  lógica <strong>é</strong> a posição física. Continuar depois do rowid 500.000
  não é procurar: é uma conta.</p>

  <p>Só que o servidor não fazia essa conta. Ele chamava a varredura, que
  <strong>decodifica cada linha da tabela com os anexos do <code>.bin</code> e
  do <code>.memo</code></strong>, montava tudo em memória, e jogava fora tudo
  menos as primeiras <code>max</code>. Pedir 200 linhas de 800 mil custava 800
  mil decodificações e 800 mil leituras de blob, para descartar 799.800.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 330" role="img" aria-label="Comparação medida: o custo por posição cresce com a tabela — 181 ms em 100 mil linhas, 749 ms em 400 mil, 3.176 ms em 800 mil — enquanto o custo pelo cursor fica no chão nos três tamanhos.">
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">
          <line x1="90" y1="30" x2="90" y2="230" stroke="currentColor" stroke-width="1.2" opacity=".5"/>
          <line x1="90" y1="230" x2="800" y2="230" stroke="currentColor" stroke-width="1.2" opacity=".5"/>

          <text x="82" y="34" text-anchor="end" font-size="10" opacity=".6">3.200 ms</text>
          <text x="82" y="234" text-anchor="end" font-size="10" opacity=".6">0</text>

          <!-- por posicao: 181, 749, 3176 sobre 3200 -->
          <rect x="150" y="219" width="70" height="11" fill="var(--log)" opacity=".85"/>
          <rect x="370" y="183" width="70" height="47" fill="var(--log)" opacity=".85"/>
          <rect x="590" y="32" width="70" height="198" fill="var(--log)" opacity=".85"/>
          <text x="185" y="213" text-anchor="middle" font-size="10.5" fill="var(--log)">181</text>
          <text x="405" y="177" text-anchor="middle" font-size="10.5" fill="var(--log)">749</text>
          <text x="625" y="26" text-anchor="middle" font-size="11" fill="var(--log)" font-weight="600">3.176</text>

          <!-- pelo cursor: nao mensuravel nos tres -->
          <rect x="230" y="227" width="70" height="3" fill="var(--ok)"/>
          <rect x="450" y="227" width="70" height="3" fill="var(--ok)"/>
          <rect x="670" y="227" width="70" height="3" fill="var(--ok)"/>
          <text x="265" y="221" text-anchor="middle" font-size="10" fill="var(--ok)">0</text>
          <text x="485" y="221" text-anchor="middle" font-size="10" fill="var(--ok)">0</text>
          <text x="705" y="221" text-anchor="middle" font-size="10" fill="var(--ok)">0</text>

          <text x="225" y="248" text-anchor="middle" font-size="11">100 mil</text>
          <text x="445" y="248" text-anchor="middle" font-size="11">400 mil</text>
          <text x="665" y="248" text-anchor="middle" font-size="11">800 mil</text>
          <text x="445" y="266" text-anchor="middle" font-size="10" opacity=".55">linhas na tabela · a MESMA página de 200</text>

          <rect x="150" y="284" width="12" height="9" fill="var(--log)" opacity=".85"/>
          <text x="170" y="293" font-size="11">por posição — o custo é o da TABELA</text>
          <rect x="470" y="284" width="12" height="9" fill="var(--ok)"/>
          <text x="490" y="293" font-size="11">pelo cursor — o custo é o da PÁGINA</text>

          <text x="150" y="316" font-size="10.5" opacity=".55">Medido com o exemplo custo-da-pagina, três tamanhos, a mesma página. O cursor não deu tempo mensurável em nenhum.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 9.</b> A barra vermelha é a mesma pergunta feita do jeito
    caro. Ela cresce mais que linearmente porque, além de ler tudo, precisa
    <em>caber</em> tudo na memória.</figcaption>
  </figure>

  <p>O cursor é o <code>rowid</code> onde a página parou, e a resposta devolve o
  par pronto — <code>cursor_inicio</code>, <code>cursor_fim</code>,
  <code>ha_mais</code>, <code>ha_antes</code>. O <code>ha_mais</code> sai de
  <strong>uma</strong> leitura além do teto, e não de contar a tabela: contar
  para escrever «página 3 de 40» é o item mais caro da tela numa tabela grande,
  e é o que ninguém lê.</p>

  <h3>O que estava embaixo do problema, e era maior</h3>

  <div class="nota">
    <p><strong>O servidor nunca ligava <code>TCP_NODELAY</code>.</strong> Só o
    cliente do DbLink ligava. O algoritmo de Nagle segurava cada resposta por
    até 40 ms esperando mais bytes para encher um pacote — e nunca vinham,
    porque a resposta tinha acabado.</p>
    <p>Medido na porta de dados, tabela de 20.000 linhas: <strong>1 ms de
    servidor e 44 ms de relógio</strong>. Depois de uma linha:
    <strong>1,3 ms</strong>. Trinta e três vezes, valendo para toda operação do
    protocolo e todo clique da tela.</p>
    <p>Achado medindo o relógio contra o <code>ms</code> que a própria resposta
    declara. Ler o código não acharia: não há nada errado escrito — há uma
    linha que não estava lá.</p>
  </div>

  <h3>A coluna que sustenta o cursor</h3>

  <p>Toda tabela ganhou <code>rownum</code>: o número de ordem de chegada da
  linha. O motor preenche, não se escreve à mão, e <strong>nunca reaproveita
  número</strong> — se reaproveitasse, uma linha nova apareceria <em>atrás</em>
  de um cursor parado numa página, e a paginação passaria a pular registro sem
  avisar. Alterar a linha não renumera.</p>

  <p>E como o <code>rownum</code> cresce com o <code>rowid</code> — o
  <code>.reg</code> guarda as linhas na ordem de chegada —, achar a linha de
  número 500.000 num milhão é uma <strong>bissecção</strong>: vinte leituras,
  sem índice nenhum a manter. É o mesmo motivo de o endereço sair de uma conta.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>&nbsp;</th><th><code>rowid</code></th><th><code>rownum</code></th></tr></thead>
      <tbody>
        <tr><td>O que é</td><td>a posição física</td><td>a ordem de chegada</td></tr>
        <tr><td>Quem escolhe</td><td>ninguém — sai da conta</td><td>ninguém — sai do contador</td></tr>
        <tr><td>Nas partições por quantidade e período</td><td colspan="2">são o mesmo número</td></tr>
        <tr><td>Na partição alfanumérica</td><td>diz o <b>arquivo</b></td><td>diz <b>quando chegou</b></td></tr>
        <tr><td>Reaproveita?</td><td>nunca</td><td>nunca</td></tr>
      </tbody>
    </table>
  </div>

  <p class="leg">O que ainda não existe: <strong>salto para «a página 500»</strong>.
  O cursor sabe ir e voltar uma página; ir direto a um ponto exigiria contar a
  tabela — que é justamente o que foi removido. Quem precisa de um ponto certo
  usa o <code>rownum</code> com a bissecção. E por <em>índice</em> o cursor não
  vale: ali os rowids vêm na ordem da chave, e «depois do rowid X» não quer
  dizer nada — a resposta declara que paginou por posição.</p>
</section>

<!-- ============================= 06 ============================= -->'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
