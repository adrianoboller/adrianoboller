# Update the state table
# 28/08 20:46

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()

antigo = """        <tr><td>Replicação — <code>.log</code> v2 com imagem da linha</td><td><span class="pino pend">desenhada</span></td><td class="num">—</td></tr>"""
novo = """        <tr><td>Replicação Master → Réplica · quatro servidores medidos</td><td><span class="pino ok">pronto</span></td><td class="num">6</td></tr>
        <tr><td><code>.log</code> v2 com a imagem da linha · atrás do interruptor</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Réplica acompanhar a escrita do master · 4.273/s contra 18.773/s</td><td><span class="pino pend">parcial</span></td><td class="num">—</td></tr>
        <tr><td><em>Long-poll</em> e TLS no transporte da replicação</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Carga em lote · 9,6× medido · cinco formatos colados</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Salto por posição com bissecção no <code>rownum</code></td><td><span class="pino ok">pronto</span></td><td class="num">22</td></tr>"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """        <tr><td><code>.log</code> — diário datado das três operações</td><td><span class="pino ok">pronto</span></td><td class="num">9</td></tr>"""
novo = """        <tr><td><code>.log</code> — diário datado das três operações, com a imagem da linha</td><td><span class="pino ok">pronto</span></td><td class="num">9</td></tr>"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    <li><strong>A replicação não transporta evento.</strong> A configuração entra e
    valida, o desenho está na seção 9, e o servidor avisa alto no arranque quando o
    papel não é <code>isolado</code>. Config que promete o que o código não faz é pior
    do que config que falta.</li>"""
novo = """    <li><strong>A réplica não acompanha a escrita do master.</strong> 4.273
    eventos/s contra 18.773 linhas/s: sob carga sustentada ela fica para trás.
    Aplicar decodifica a imagem para valores e <em>reencoda</em> o payload, em vez
    de gravar os bytes que vieram. E o transporte não tem <em>long-poll</em> nem
    TLS: o atraso é o intervalo do laço, e a proteção é o túnel.</li>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
