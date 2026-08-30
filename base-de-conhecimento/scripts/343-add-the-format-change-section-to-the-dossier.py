# Add the format-change section to the dossier
# 28/08 11:52

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

SECAO = r'''  <h3>O campo ganhou identidade</h3>

  <p>Até a versão 3 do bloco de esquema, uma coluna era três coisas: nome, tipo
  e se aceita nulo. Uma tela que quisesse mostrar <em>Emissão</em> em vez de
  <code>emissao</code> guardava esse rótulo em outro lugar — e outro lugar é um
  dicionário externo, que se perde, se desatualiza e obriga quem copia os cinco
  arquivos a copiar um sexto.</p>

  <p>Agora cada coluna carrega, dentro do próprio <code>.reg</code>:</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Campo</th><th>Para que serve</th></tr></thead>
      <tbody>
        <tr><td class="dado"><code>id</code></td><td>UUID v7 sorteado na criação e <strong>nunca reaproveitado</strong></td></tr>
        <tr><td class="dado"><code>caption</code></td><td>o rótulo de tela; vazio significa «use o nome»</td></tr>
        <tr><td class="dado"><code>descricao</code></td><td>para que a coluna serve</td></tr>
        <tr><td class="dado"><code>mascara</code></td><td>o PICTURE do Clarion(R): <code>@N-11.2</code>, <code>@D6</code>, <code>@P###-####P</code></td></tr>
      </tbody>
    </table>
  </div>

  <p>O <code>id</code> é o que faz diferença. É por ele que uma tela, um
  relatório ou um mapeamento apontam para a coluna, e por isso <strong>renomear
  o campo não quebra nada</strong>: renomear troca o <code>nome</code>; o
  <code>id</code> segue o mesmo. Sem ele, todo <em>rename</em> seria uma
  caçada por referências em texto.</p>

  <h3>Chave primária, e por que ela não mora no campo</h3>

  <p>Até aqui só havia «índice único», e chave primária é mais do que isso: é a
  identidade da linha, aquilo a que as chaves estrangeiras das outras tabelas
  apontam. Um índice passa a poder ser marcado como primário — só um, sempre
  único, e nenhuma coluna dele aceita nulo, porque uma identidade nula não
  identifica. As três conferências acontecem na montagem do esquema.</p>

  <p>O que <strong>não</strong> foi feito: gravar «é primária» na coluna. Ela
  poderia estar ali, e a tela pediria isso — mas seria uma <em>segunda
  verdade</em> ao lado do índice, e as duas divergiriam no primeiro
  <code>ALTER</code>. As marcas que a tela mostra são derivadas:</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Marca</th><th>De onde sai</th></tr></thead>
      <tbody>
        <tr><td class="dado">primária</td><td>a coluna aparece no índice marcado como primário</td></tr>
        <tr><td class="dado">estrangeira</td><td>a coluna aparece em alguma chave estrangeira</td></tr>
        <tr><td class="dado">composta</td><td>a chave de que ela participa tem mais de uma coluna</td></tr>
      </tbody>
    </table>
  </div>

  <h3>O volume aprendeu a cortar pelo calendário</h3>

  <p>A paginação por quantidade corta a cada <em>N</em> registros, e o endereço
  sai de uma divisão. A partição por período — mensal, bimestral, semestral ou
  anual — corta quando o período de uma coluna de data vira. Isso quebra a
  divisão: dois meses rendem quantidades diferentes.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 300" role="img" aria-label="Na partição por quantidade o volume sai de uma divisão do rowid; na partição por período cada volume grava no próprio cabeçalho o rowid em que começou, e o volume de um rowid sai de uma busca binária nessa tabela de fronteiras">
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="18" font-size="10" opacity=".55" letter-spacing=".08em">POR QUANTIDADE — O VOLUME SAI DE UMA CONTA</text>

          <rect x="16" y="30" width="180" height="42" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="106" y="47" text-anchor="middle" font-size="11">volume 1</text>
          <text x="106" y="63" text-anchor="middle" font-size="10" opacity=".6">rowid 1 … 1000</text>

          <rect x="204" y="30" width="180" height="42" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="294" y="47" text-anchor="middle" font-size="11">volume 2</text>
          <text x="294" y="63" text-anchor="middle" font-size="10" opacity=".6">rowid 1001 … 2000</text>

          <rect x="392" y="30" width="180" height="42" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="482" y="47" text-anchor="middle" font-size="11">volume 3</text>
          <text x="482" y="63" text-anchor="middle" font-size="10" opacity=".6">rowid 2001 … 3000</text>

          <text x="592" y="55" font-size="11" fill="var(--acento)">volume = (rowid−1) ÷ 1000 + 1</text>

          <line x1="16" y1="100" x2="824" y2="100" stroke="currentColor" stroke-width=".8" opacity=".25"/>

          <text x="16" y="126" font-size="10" opacity=".55" letter-spacing=".08em">POR PERÍODO — CADA VOLUME GRAVA ONDE COMEÇOU</text>

          <rect x="16" y="138" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="91" y="156" text-anchor="middle" font-size="11">volume 1</text>
          <text x="91" y="171" text-anchor="middle" font-size="10" opacity=".65">2026-01</text>
          <text x="91" y="186" text-anchor="middle" font-size="10" fill="var(--acento)">começa em 1</text>

          <rect x="174" y="138" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="249" y="156" text-anchor="middle" font-size="11">volume 2</text>
          <text x="249" y="171" text-anchor="middle" font-size="10" opacity=".65">2026-02</text>
          <text x="249" y="186" text-anchor="middle" font-size="10" fill="var(--acento)">começa em 3</text>

          <rect x="332" y="138" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="407" y="156" text-anchor="middle" font-size="11">volume 3</text>
          <text x="407" y="171" text-anchor="middle" font-size="10" opacity=".65">2026-03</text>
          <text x="407" y="186" text-anchor="middle" font-size="10" fill="var(--acento)">começa em 4</text>

          <rect x="490" y="138" width="150" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="565" y="156" text-anchor="middle" font-size="11">volume 4</text>
          <text x="565" y="171" text-anchor="middle" font-size="10" opacity=".65">2026-04</text>
          <text x="565" y="186" text-anchor="middle" font-size="10" fill="var(--acento)">começa em 7</text>

          <text x="660" y="163" font-size="11" fill="var(--acento)">busca binária</text>
          <text x="660" y="179" font-size="10" opacity=".6">nas fronteiras</text>

          <text x="16" y="228" font-size="10.5" opacity=".75">Um lançamento de JANEIRO digitado agora entra no volume 4 — o corrente —, não volta para o 1.</text>
          <text x="16" y="246" font-size="10.5" opacity=".75">A ordem de digitação é sagrada: voltar seria escrever no meio de um arquivo já fechado.</text>
          <text x="16" y="272" font-size="10.5" opacity=".6">Por isso o período de um volume é «o período em que ele abriu», e um volume pode conter</text>
          <text x="16" y="288" font-size="10.5" opacity=".6">linhas de períodos anteriores que chegaram depois.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 20.</b> A tabela de fronteiras não é um índice: são dois
    números por volume, gravados no cabeçalho que cada volume já tem, e lidos
    uma vez na abertura. Volume é coisa que se conta em dezenas — cada um guarda
    <code>registros_por_arquivo</code> linhas —, então a busca custa três ou
    quatro comparações num vetor que já está na memória.</figcaption>
  </figure>

  <p>Três alternativas foram descartadas, e vale dizer por quê. Guardar a tabela
  de fronteiras num <strong>sexto arquivo</strong> quebraria o modelo de cinco.
  Guardá-la <strong>dentro do bloco de esquema</strong> não serve: o bloco é
  seguido pelos dados, e crescer significaria empurrar a tabela inteira. E
  <strong>mandar a linha para o volume do período dela</strong> — o que um
  particionamento de banco relacional faria — exigiria escrever no meio de um
  arquivo fechado, perdendo a ordem de digitação e o endereço contíguo de uma
  vez só.</p>

  <p>O cabeçalho de cada volume já existia e tinha 48 bytes sobrando. Dois
  <code>u64</code> resolveram.</p>

'''

marca = '''  <h3>O que este motor ainda não faz</h3>'''
assert s.count(marca) == 1
s = s.replace(marca, SECAO + marca, 1)
p.write_text(s)
print('secao do formato no dossie')
