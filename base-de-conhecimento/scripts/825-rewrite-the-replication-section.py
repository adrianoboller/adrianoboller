# Rewrite the replication section
# 28/08 20:44

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()

# 1. Abertura da secao 9
antigo = """  <h2>Replicação Source → Réplica</h2>

  <p>O <code>.log</code> já é o binlog. A posição também já existe: como o diário é uma
  sequência de eventos de tamanho conhecido, <strong>o evento N é a posição N</strong> —
  o que dispensa inventar um GTID.</p>
"""
novo = """  <h2>Replicação Source → Réplica</h2>

  <p>O <code>.log</code> sempre foi o binlog, e a posição sempre existiu: <strong>o
  evento N é a posição N</strong>, o que dispensa inventar um GTID. Faltava uma
  coisa só — a <strong>imagem da linha</strong> dentro do evento —, e ela entrou.
  Quatro servidores no ar, medidos.</p>

  <div class="kpis">
    <div class="kpi"><div class="v">18.773</div><div class="r">linhas/s no master</div><div class="u">com a imagem no diário</div></div>
    <div class="kpi"><div class="v">4.273</div><div class="r">eventos/s por réplica</div><div class="u">as três em paralelo</div></div>
    <div class="kpi"><div class="v">1,3–2,1 s</div><div class="r">até as três</div><div class="u">laço de 2 s</div></div>
    <div class="kpi"><div class="v">1,0 s</div><div class="r">retomada</div><div class="u">4.000 eventos após queda</div></div>
  </div>
"""
assert antigo in s
s = s.replace(antigo, novo)

# 2. O quadro "o que falta para aplicar" virou "o que a imagem carrega"
antigo = """          <rect x="436" y="218" width="380" height="94" rx="4" fill="none" stroke="currentColor" stroke-width="1" opacity=".45"/>
          <text x="452" y="238" font-size="10.5" opacity=".6">O QUE FALTA PARA APLICAR</text>
          <text x="452" y="258" font-size="11">O evento diz <tspan font-weight="600">que</tspan> o rowid 42 mudou,</text>
          <text x="452" y="274" font-size="11">mas não diz <tspan font-weight="600">para quê</tspan>.</text>
          <text x="452" y="296" font-size="10.5" opacity=".6">.log versão 2: o payload cru do .reg mais os</text>
          <text x="452" y="308" font-size="10.5" opacity=".6">blocos externos que ele referencia.</text>"""
novo = """          <rect x="436" y="218" width="380" height="94" rx="4" fill="none" stroke="var(--ok)" stroke-width="1.2"/>
          <text x="452" y="238" font-size="10.5" fill="var(--ok)">O QUE O EVENTO CARREGA · .log v2</text>
          <text x="452" y="258" font-size="11">o payload <tspan font-weight="600">cru</tspan> do .reg, sem reencodar,</text>
          <text x="452" y="274" font-size="11">e o <tspan font-weight="600">conteúdo</tspan> dos anexos</text>
          <text x="452" y="296" font-size="10.5" opacity=".6">ponteiro do .bin não vale na outra máquina;</text>
          <text x="452" y="308" font-size="10.5" opacity=".6">é a mesma razão do .trash guardar conteúdo</text>"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """          <text x="700" y="122" text-anchor="middle" font-size="10.5" opacity=".7">posição: evento 1 234</text>"""
novo = """          <text x="700" y="122" text-anchor="middle" font-size="10.5" opacity=".7">posição: o .log dela</text>"""
assert antigo in s
s = s.replace(antigo, novo)

# 3. A tabela de paralelo
antigo = """        <tr><td>Row-based binlog</td><td>imagem da linha no evento</td><td><span class="pino pend">falta</span></td></tr>
        <tr><td>Multi-source</td><td>uma conexão por origem</td><td><span class="pino pend">no config</span></td></tr>
      </tbody>
    </table>
  </div>
</section>"""
novo = """        <tr><td>Row-based binlog</td><td>imagem da linha no evento</td><td><span class="pino ok">existe</span></td></tr>
        <tr><td>Multi-source</td><td>uma thread por origem</td><td><span class="pino ok">existe</span></td></tr>
        <tr><td>Replicação em cascata</td><td>uma réplica é origem de outra</td><td><span class="pino ok">existe</span></td></tr>
        <tr><td><code>START/STOP REPLICA</code></td><td>sobe com o servidor, pelo <code>config.json</code></td><td><span class="pino pend">sem comando</span></td></tr>
        <tr><td>Binlog dump (long-poll)</td><td>a réplica pergunta a cada N segundos</td><td><span class="pino pend">falta</span></td></tr>
        <tr><td>TLS no transporte</td><td>depende do túnel</td><td><span class="pino pend">falta</span></td></tr>
      </tbody>
    </table>
  </div>

  <h3>A posição é o diário da própria réplica</h3>

  <p>A réplica <strong>não guarda um arquivo</strong> com «apliquei até aqui». Ela
  conta os eventos do <code>.log</code> dela — e é isso que faz a retomada
  funcionar sem estado extra: matar a réplica no meio de um lote não perde nem
  repete, porque o número que ela usa é o que os arquivos dela dizem, e não o que
  ela lembrava. É a mesma recusa em ter uma segunda verdade que impede o
  <code>.pag</code> de ser lido pelo motor e um arquivo <code>sequences</code> de
  existir.</p>

  <p>Para isso valer, cada evento aplicado tem de gerar <strong>exatamente
  um</strong> evento local. Daí uma decisão que parece severa e não é: uma exclusão
  que não acha o que excluir é tratada como <strong>divergência</strong> e para. Se
  passasse batido, o evento não geraria evento, a posição não andaria, e a
  replicação giraria em falso puxando o mesmo para sempre.</p>

  <h3>O que ela custa, medido</h3>

  <p>Mesma tabela, mesmas 100.000 linhas, só o interruptor mudando:</p>

  <div class="rolo">
    <table>
      <thead><tr><th><code>imagem_da_linha</code></th><th class="num">linhas/s</th><th class="num">bytes por evento</th><th class="num">tamanho do <code>.log</code></th></tr></thead>
      <tbody>
        <tr><td>desligada</td><td class="num">21.740</td><td class="num">44</td><td class="num">4,4 MB</td></tr>
        <tr><td>ligada</td><td class="num">19.531</td><td class="num">223</td><td class="num">22,3 MB</td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>10% mais devagar, e um diário 5,1× maior.</strong> É o preço de a
  réplica receber a linha, e não só o aviso de que ela mudou. Por isso o
  interruptor — que já vem ligado num servidor com <code>papel: source</code>,
  porque um source sem imagem no diário é um source que não replica, e descobrir
  isso pela réplica parada seria o pior jeito de descobrir.</p>

  <div class="nota">
    <span class="t">A senha não fica em claro nem viaja</span>
    <p>A réplica se autentica pelo mesmo <strong>desafio-resposta</strong> do resto
    do protocolo: pede um <em>nonce</em>, calcula o HMAC com a chave derivada e
    manda a prova. No <code>config.json</code> dela mora o <code>senha_hash</code> —
    o mesmo texto que já mora no cadastro de usuários —, e dele sai a chave. Não há
    senha em claro em lugar nenhum, e a réplica não é exceção à regra.</p>
    <p><code>posicao</code> e <code>replicar</code> exigem a permissão
    <strong>replicar</strong>, que é própria: dá para concedê-la a uma réplica sem
    conceder mais nada. <code>aplicar</code> exige <strong>administrar</strong>,
    porque grava com o rowid escolhido e o payload cru, por fora das conferências
    normais — e <strong>não</strong> está na lista de operações de escrita, porque
    uma réplica em <code>somente_leitura</code> precisa aceitar exatamente essa.</p>
  </div>

  <div class="nota" data-tom="aviso">
    <span class="t">A réplica não acompanha a escrita do master</span>
    <p>4.273 eventos/s contra 18.773 linhas/s, com as três competindo pela mesma
    máquina: sob carga sustentada elas ficam para trás. A razão está no caminho, e
    é a mesma que a figura 8 mostra — aplicar decodifica a imagem para valores e
    <strong>reencoda</strong> o payload, em vez de gravar os bytes que vieram.
    Gravar o payload direto, remendando só os ponteiros dos anexos, é o próximo
    ganho grande. Está anotado.</p>
    <p>O <strong>atraso</strong> de 1,3 a 2,1 s é outra coisa: ele é o intervalo do
    laço, e não trabalho. A réplica dorme <code>reconectar_em</code> segundos entre
    uma pergunta e outra. Baixar o intervalo baixa o atraso e sobe o tráfego de
    perguntas em vão; o <em>long-poll</em> — o source segurar a resposta até ter
    novidade — resolveria os dois e ainda não existe.</p>
  </div>

  <p class="fonte">Como refazer: <code>python3 bancada/replicacao/montar.py
  /tmp/phx-replicacao</code> e <code>python3 bancada/replicacao/medir.py
  100000</code>. A bancada não compara «quantas linhas»: compara um SHA-256 de
  <strong>cada linha inteira</strong>, com o <code>rowid</code> e o
  <code>rownum</code> juntos — contar não acharia uma linha que atravessou
  errada.</p>
</section>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
