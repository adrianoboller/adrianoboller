# Add the grouping and pivot section
# 28/08 13:38

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

SECAO = r'''  <h3>Agrupar arrastando: o que o Janus(R) e o DevExpress(R) fazem</h3>

  <p>Arraste um cabeçalho para a faixa de cima e a grade agrupa por ele. Arraste
  outro e vira dois níveis; arraste a pastilha para trocar a ordem dos níveis. A
  seta na pastilha inverte crescente e decrescente daquele nível — agrupar por
  mês quase sempre quer o mais recente em cima.</p>

  <p>A direção é guardada <strong>por campo</strong>, e não por posição. Parece
  detalhe e não é: guardada por posição, arrastar uma pastilha para outro lugar
  viraria a ordem de quem ficou no lugar dela.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Onde</th><th>O que mostra</th></tr></thead>
      <tbody>
        <tr><td class="dado">cabeçalho do grupo</td><td>o valor, quantas linhas, e os agregados em linha</td></tr>
        <tr><td class="dado">rodapé do grupo</td><td>o mesmo total, alinhado <strong>na coluna</strong></td></tr>
        <tr><td class="dado">total geral</td><td>o conjunto filtrado inteiro, preso embaixo</td></tr>
      </tbody>
    </table>
  </div>

  <p>O rodapé repete o que o cabeçalho já diz, e existe por causa da rolagem:
  num grupo de trinta linhas o cabeçalho já saiu da tela quando o total
  interessa. E ele alinha o número <em>na coluna</em> em vez de numa tira de
  texto, porque é assim que se compara um total com os valores acima dele.</p>

  <p>O <strong>total geral</strong> é sobre o conjunto filtrado, não sobre a
  página. Um rodapé que muda ao virar de página não é total de nada.</p>

  <p>O agregador de cada coluna cicla no clique da pastilha do cabeçalho — soma,
  média, contagem, mínimo, máximo —, e o esquema só decide o <em>padrão</em>:
  colunas inteiras e decimais nascem somando, <code>Sequence</code> não, porque
  somar um contador não significa nada.</p>

  <h3>Tabela dinâmica: por que o cruzamento é somado aqui</h3>

  <p>Um pivot <em>resume</em>. Cem mil linhas viram uma grade de vinte por doze,
  e o que atravessa a rede é o resumo. Mandar as cem mil para o navegador somar
  seria pagar o transporte do que vai ser jogado fora — por isso a tabulação
  cruzada é uma operação do protocolo, e não código de tela.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 300" role="img" aria-label="A tabela de consulta é lida uma vez para um mapa em memória; a tabela de fatos é varrida linha a linha, cada linha é enriquecida pelo mapa e cai numa célula da grade, que já sai somada">
        <defs>
          <marker id="setaPv" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="18" font-size="10" opacity=".55" letter-spacing=".08em">UMA VEZ, NA ABERTURA</text>
          <rect x="16" y="28" width="150" height="46" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="91" y="47" text-anchor="middle" font-size="11">clientes</text>
          <text x="91" y="63" text-anchor="middle" font-size="10" opacity=".6">a consulta</text>

          <path d="M170 51 L212 51" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaPv)"/>

          <rect x="216" y="28" width="160" height="46" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.6"/>
          <text x="296" y="47" text-anchor="middle" fill="var(--acento)" font-size="11">mapa em memória</text>
          <text x="296" y="63" text-anchor="middle" font-size="10" opacity=".6">id → a linha inteira</text>

          <line x1="16" y1="100" x2="824" y2="100" stroke="currentColor" stroke-width=".8" opacity=".25"/>

          <text x="16" y="126" font-size="10" opacity=".55" letter-spacing=".08em">UMA VEZ POR LINHA DE FATO</text>
          <rect x="16" y="136" width="150" height="46" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="91" y="155" text-anchor="middle" font-size="11">vendas</text>
          <text x="91" y="171" text-anchor="middle" font-size="10" opacity=".6">os fatos</text>

          <path d="M170 159 L212 159" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaPv)"/>

          <rect x="216" y="136" width="160" height="46" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="296" y="155" text-anchor="middle" font-size="11">acesso ao mapa</text>
          <text x="296" y="171" text-anchor="middle" font-size="10" opacity=".6">traz a cidade</text>

          <path d="M380 159 L422 159" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaPv)"/>

          <rect x="426" y="136" width="170" height="46" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="511" y="155" text-anchor="middle" font-size="11">célula (linha, coluna)</text>
          <text x="511" y="171" text-anchor="middle" font-size="10" opacity=".6">soma acumula ali</text>

          <path d="M600 159 L642 159" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaPv)"/>

          <rect x="646" y="130" width="160" height="58" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.6"/>
          <text x="726" y="152" text-anchor="middle" fill="var(--acento)" font-size="11">a grade</text>
          <text x="726" y="168" text-anchor="middle" font-size="10" opacity=".6">20 × 12 células</text>
          <text x="726" y="181" text-anchor="middle" font-size="10" opacity=".6">é só isto que trafega</text>

          <text x="16" y="224" font-size="10.5" opacity=".75">A alternativa ingênua seria uma busca no índice por linha de venda: uma descida na árvore por linha.</text>
          <text x="16" y="242" font-size="10.5" opacity=".75">Para a forma de dado que um pivot cruza — muitos fatos, poucas dimensões — ler a dimensão uma vez ganha.</text>
          <text x="16" y="268" font-size="10.5" opacity=".6">Dinheiro soma no domínio inteiro escalado, e só vira texto na saída. A média divide uma vez, no fim.</text>
          <text x="16" y="284" font-size="10.5" opacity=".6">Célula vazia quer dizer «nenhuma linha caiu ali» — zero seria «somou e deu nada».</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 18.</b> O <em>hash join</em>: a tabela de consulta é
    lida uma vez, a de fatos linha a linha. O teto de 500.000 linhas na consulta
    existe porque ela cabe na memória inteira — e quando estoura, o erro diz
    isso em vez de ficar lento.</figcaption>
  </figure>

  <p>O assistente pergunta as tabelas envolvidas em três passos. No primeiro,
  a tabela dos fatos e as de consulta — e, quando o esquema declara chave
  estrangeira, as junções aparecem <strong>propostas</strong>, porque a
  informação de por qual coluna ligar já está gravada no <code>.reg</code>. No
  segundo, os campos vão para Linhas, Colunas e Valores arrastando; campo de
  data ganha granularidade, porque cruzar venda por <em>dia</em> daria uma coluna
  por dia do ano. No terceiro, a grade com total por linha, por coluna e
  geral.</p>

  <p><strong>Os totais fecham nas duas direções</strong>, e há teste que
  confere: a soma das células de uma linha bate com o total dela, e a soma dos
  totais de coluna bate com o total geral. Um pivot que não fecha não serve para
  nada.</p>

'''
marca = '''  <h3>E a grade, que veio pronta</h3>'''
assert s.count(marca) == 1
s = s.replace(marca, SECAO + marca, 1)
p.write_text(s)
print('secao do agrupamento e do pivot')
