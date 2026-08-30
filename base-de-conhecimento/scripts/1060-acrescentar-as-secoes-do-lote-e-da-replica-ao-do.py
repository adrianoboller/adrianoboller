# Acrescentar as secoes do lote e da replica ao dossie
# 29/08 03:58

import io
p='docs/dossie/dossie-phxsql-0.15.html'
s=io.open(p,encoding='utf-8').read()

anc = '''  <h3>A inserção ainda é onde o MySQL(R) ganha — e 2,9&#215; já voltaram</h3>'''
assert s.count(anc)==1

novo = '''  <h3>A construção em lote da B+tree, e o item que ela não salvou</h3>

  <p>O <code>reindexar</code> inseria <strong>chave a chave</strong> — uma descida
  na árvore por chave, que é exatamente o trabalho do caminho de dentro feito de
  novo. Era por isso que adiar o índice numa carga comprava 1,02&#215;: o preço
  não sumia, mudava de lugar e continuava o mesmo.</p>

  <p><code>construir_em_lote</code> não desce a árvore nenhuma vez. Ordena as
  chaves, enche as folhas <em>em sequência</em> e monta os níveis de cima por
  cima dos de baixo. Um milhão de chaves:</p>

  <div class="rolo">
    <table>
      <thead><tr><th></th><th class="num">montar</th><th class="num">páginas</th><th class="num">varrer</th></tr></thead>
      <tbody>
        <tr><td>uma a uma (o <code>reindexar</code> de antes)</td><td class="num">7,72&#8201;s</td><td class="num">6.136</td><td class="num">0,036&#8201;s</td></tr>
        <tr><td><strong>em lote</strong></td><td class="num"><strong>0,31&#8201;s</strong></td><td class="num">5.271</td><td class="num">0,028&#8201;s</td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>23&#215; a 25&#215;</strong>, em duas corridas. Todo
  <code>reindexar</code> e todo <em>reparar índice</em> andam nisso.</p>

  <div class="nota">
    <span class="t">80% de enchimento, e o número é medido</span>
    <p>Encher a folha a 70% é a folga clássica e <strong>não compra nada</strong>:
    inserção aleatória já assenta perto de 69% de ocupação sozinha, que é um
    resultado clássico de B-tree. De 90% para cima a folha fica sem folga —
    crescer 10% aloca mais de dois mil páginas e fica <strong>mais lento</strong>
    do que na árvore mais frouxa, e a varredura mais rápida não paga isso. 80% é
    a ocupação mais densa que ainda absorve 10% de crescimento sem alocar uma
    página.</p>
    <p>E a primeira versão do medidor deu 100% de graça, porque as chaves de
    crescimento entravam <strong>acima</strong> da faixa — e chave maior que
    todas vai sempre para a última folha, então a divisão que o enchimento
    deveria provocar nunca acontecia. <em>Medidor com furo mede o furo.</em></p>
  </div>

  <p>Com o lote pronto, adiar o índice virou item de implementar. <strong>Medi
  antes, e o número o derrubou.</strong> O 1,59&#215; vale para tabela vazia, mas
  o <code>reindexar</code> refaz sobre a tabela <em>inteira</em>: carregando M
  numa tabela de 200.000, o ganho é 1,22&#215; quando M dobra a tabela e vira
  <strong>prejuízo abaixo de M&#8239;&#8776;&#8239;N/3</strong> — 0,86&#215; com
  M=40.000, 0,22&#215; com M=4.000. E cobraria marcar <strong>índice suspenso no
  formato</strong>, cujo defeito é busca respondendo errado em silêncio depois de
  uma queda. Ficou fora com o número na mesa.</p>

  <h3>A réplica: a causa registrada apontava para o lado errado do fio</h3>

  <p>Estava escrito em dois documentos que a réplica ficava para trás porque
  «aplicar decodifica a imagem para <code>Value</code> e <strong>reencoda</strong>
  o payload, em vez de gravar os bytes que vieram». A primeira coisa foi medir a
  acusação — e ela custa <strong>0,35&#8201;µs</strong>:
  <code>aplicar_evento</code> são 16,15&#8201;µs contra 15,80 de uma inserção
  local pura. A réplica media <strong>229&#8201;µs por evento</strong>.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 240" role="img" aria-label="Dos 229 microssegundos por evento, o caminho de CPU dos dois lados custa 24 e o resto estava no source, que varria o diário desde o começo a cada lote, e no laço, que dormia depois de toda rodada">
        <defs>
          <marker id="setaR" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="20" font-size="10" opacity=".55" letter-spacing=".08em">229 µs POR EVENTO — ONDE ELES ESTAVAM</text>

          <rect x="16" y="34" width="88" height="34" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="60" y="50" text-anchor="middle" font-size="10">CPU dos</text>
          <text x="60" y="62" text-anchor="middle" font-size="10">dois lados</text>
          <text x="112" y="55" font-size="11" font-weight="600">24 µs</text>

          <rect x="16" y="82" width="600" height="34" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="30" y="103" font-size="11" fill="var(--acento)">o source varria o diário desde o começo a cada lote</text>
          <text x="624" y="103" font-size="11" font-weight="600" fill="var(--acento)">~180 µs</text>

          <rect x="16" y="130" width="150" height="34" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="30" y="151" font-size="11">o laço dormia</text>
          <text x="174" y="151" font-size="11" font-weight="600">o resto</text>

          <line x1="16" y1="184" x2="824" y2="184" stroke="currentColor" stroke-width="1" opacity=".25"/>

          <text x="16" y="208" font-size="10" opacity=".55" letter-spacing=".08em">A ACUSAÇÃO QUE ESTAVA REGISTRADA</text>
          <rect x="16" y="214" width="10" height="16" rx="2" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="36" y="227" font-size="11">reencodar o payload &#8212; 0,35 µs, e não os 229</text>
        </g>
      </svg>
    </div>
    <figcaption>Servir «500 eventos a partir de P» caminhava pelos P anteriores
    lendo o cabeçalho de cada um: 1,11&#8201;µs por evento com P=0, e
    <strong>72,65</strong> com P=90.000. Linear em P, e portanto quadrático no
    total.</figcaption>
  </figure>

  <p>Alcançar 100.000 eventos de 500 em 500 gastava <strong>4,07&#8201;s só do
  lado de quem serve</strong>, com três réplicas fazendo isso ao mesmo tempo sob
  a trava global do master. Uma <strong>marca de posição</strong> levou a
  <strong>0,09&#8201;s — 45&#215;</strong>.</p>

  <div class="nota">
    <span class="t">A marca é uma dica, e é isso que a torna segura</span>
    <p>Uma marca errada faz a leitura começar no lugar errado e o
    <strong>CRC do evento recusar</strong>, ou cair depois do fim do arquivo e
    devolver vazio. Nenhum dos dois entrega evento errado.</p>
    <p>Ela mora no <strong>servidor</strong>, e não na tabela, porque a tabela é
    aberta e fechada a cada pedido — e são pedidos seguidos que ela serve. E são
    <strong>várias por tabela</strong>: um source atende réplicas em posições
    diferentes, e uma marca só seria empurrada para frente pela mais adiantada e
    nunca serviria às outras. Foi essa correção que levou o número de 7.835 para
    17.450.</p>
  </div>

  <div class="rolo">
    <table>
      <thead><tr><th>na bancada dos quatro servidores</th><th class="num">antes</th><th class="num">agora</th></tr></thead>
      <tbody>
        <tr><td>master, com a imagem no diário</td><td class="num">28.914/s</td><td class="num">34.048/s</td></tr>
        <tr><td><strong>aplicação, por réplica (as três em paralelo)</strong></td><td class="num"><strong>4.273/s</strong></td><td class="num"><strong>17.450/s</strong></td></tr>
        <tr><td>alcançar 100.000 eventos</td><td class="num">18,7&#8201;s</td><td class="num">5,7&#8201;s</td></tr>
        <tr><td>exclusão física até as três</td><td class="num">1.952&#8201;ms</td><td class="num">140&#8201;ms</td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>4,08&#215;.</strong> As três juntas aplicam ~52.000 eventos/s contra
  os 34.048 que o master escreve. E um terceiro achado, menor, saiu no caminho:
  <code>bytes_para_hex</code> fazia um <code>format!</code> — e uma alocação de
  <code>String</code> — <strong>por byte</strong> da imagem. Tabela de dígitos no
  lugar: 3,48 → 0,24&#8201;µs por evento, <strong>14,5&#215;</strong>.</p>

'''
io.open(p,'w',encoding='utf-8').write(s.replace(anc, novo+anc))
print('ok')
