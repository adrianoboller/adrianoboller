# Redraw the write-flow figure
# 28/08 20:39

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()

inicio = s.index('      <svg viewBox="0 0 840 450" role="img" aria-label="Caminho de uma inserção')
fim = s.index('    <figcaption><b>Figura 8.</b>')
fim_fig = s.index('</figure>', fim)

novo_svg = '''      <svg viewBox="0 0 840 560" role="img" aria-label="Caminho de uma inserção: o motor completa as colunas de sistema e numera a linha, confere a unicidade, grava os blobs no bin e no memo, anexa o slot ao reg e o espelha no bkp quando o espelho está ligado, insere as chaves nos índices do ndx, registra o evento no log — com a imagem da linha dentro quando a replicação está ligada —, e só então a janela de durabilidade decide quando tudo isso chega ao disco; se um índice falhar, tudo é desfeito.">
        <defs>
          <marker id="setaE" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
          <marker id="setaErro" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <!-- ================= linha de cima: da aplicacao ate o .reg ========= -->

          <rect x="16" y="34" width="104" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="68" y="56" text-anchor="middle" font-size="11">valores</text>
          <text x="68" y="71" text-anchor="middle" font-size="10" opacity=".6">da aplicação</text>
          <text x="68" y="84" text-anchor="middle" font-size="9" opacity=".5">ou de um lote</text>

          <path d="M120 62 L152 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="156" y="34" width="146" height="56" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="229" y="54" text-anchor="middle" fill="var(--acento)" font-size="11">completa e numera</text>
          <text x="229" y="69" text-anchor="middle" font-size="9.5" opacity=".62">softdeleted = não</text>
          <text x="229" y="82" text-anchor="middle" font-size="9.5" opacity=".62">rownum = o próximo</text>

          <path d="M302 62 L334 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="338" y="34" width="134" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="405" y="54" text-anchor="middle" font-size="11">confere chave</text>
          <text x="405" y="69" text-anchor="middle" font-size="11">única</text>
          <text x="405" y="83" text-anchor="middle" font-size="9" opacity=".55">antes de tocar em disco</text>

          <path d="M472 62 L504 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="508" y="34" width="126" height="56" rx="4" fill="none" stroke="var(--bin)" stroke-width="1.5"/>
          <text x="571" y="54" text-anchor="middle" fill="var(--bin)" font-size="11">grava blobs</text>
          <text x="571" y="69" text-anchor="middle" fill="var(--bin)" font-size="10.5">.bin / .memo</text>
          <text x="571" y="83" text-anchor="middle" font-size="9" opacity=".55">devolve ponteiros</text>

          <path d="M634 62 L666 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="670" y="34" width="126" height="56" rx="4" fill="none" stroke="var(--reg)" stroke-width="1.5"/>
          <text x="733" y="54" text-anchor="middle" fill="var(--reg)" font-size="11">anexa no fim</text>
          <text x="733" y="69" text-anchor="middle" fill="var(--reg)" font-size="10.5">.reg</text>
          <text x="733" y="83" text-anchor="middle" font-size="9" opacity=".55">rowid = slots + 1</text>

          <!-- O espelho: uma segunda escrita do MESMO slot, no mesmo instante. -->
          <path d="M796 62 L816 62 L816 126" fill="none" stroke="var(--pend)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaE)"/>
          <rect x="700" y="130" width="116" height="46" rx="4" fill="none" stroke="var(--pend)" stroke-width="1.4" stroke-dasharray="5 3"/>
          <text x="758" y="148" text-anchor="middle" fill="var(--pend)" font-size="11">espelha o slot</text>
          <text x="758" y="162" text-anchor="middle" fill="var(--pend)" font-size="10.5">.bkp</text>
          <text x="758" y="173" text-anchor="middle" font-size="9" opacity=".55">só quando ligado</text>

          <!-- ================= linha de baixo: do .reg ate a resposta ========= -->

          <path d="M733 90 L733 108 L560 108 L560 126" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="497" y="130" width="126" height="56" rx="4" fill="none" stroke="var(--ndx)" stroke-width="1.5"/>
          <text x="560" y="150" text-anchor="middle" fill="var(--ndx)" font-size="11">insere chaves</text>
          <text x="560" y="165" text-anchor="middle" fill="var(--ndx)" font-size="10.5">.ndx</text>
          <text x="560" y="179" text-anchor="middle" font-size="9" opacity=".55">uma por índice</text>

          <path d="M497 158 L465 158" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="335" y="130" width="126" height="56" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5"/>
          <text x="398" y="150" text-anchor="middle" fill="var(--log)" font-size="11">registra evento</text>
          <text x="398" y="165" text-anchor="middle" fill="var(--log)" font-size="10.5">.log</text>
          <text x="398" y="179" text-anchor="middle" font-size="9" opacity=".55">44 bytes + carimbo em ms</text>

          <!-- A imagem da linha: o que transformou o diario em binlog de verdade. -->
          <path d="M398 186 L398 210" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaE)"/>
          <rect x="300" y="214" width="196" height="58" rx="4" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3"/>
          <text x="398" y="233" text-anchor="middle" fill="var(--log)" font-size="11">a imagem da linha</text>
          <text x="398" y="247" text-anchor="middle" font-size="9" opacity=".62">payload cru + conteúdo dos anexos</text>
          <text x="398" y="260" text-anchor="middle" font-size="9" opacity=".62">só com a replicação ligada</text>
          <text x="398" y="270" text-anchor="middle" font-size="8.5" opacity=".5">44 → 223 bytes por evento</text>

          <path d="M335 158 L303 158" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>

          <rect x="140" y="132" width="160" height="52" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.4"/>
          <text x="220" y="152" text-anchor="middle" fill="var(--acento)" font-size="11">janela de durabilidade</text>
          <text x="220" y="167" text-anchor="middle" font-size="9" opacity=".6">agora · por lote · sistema</text>
          <text x="220" y="179" text-anchor="middle" font-size="9" opacity=".5">um lote inteiro = um fsync</text>

          <path d="M220 184 L220 210" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>
          <rect x="150" y="214" width="140" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="220" y="241" text-anchor="middle" font-size="11">devolve o rowid</text>

          <!-- ================= o caminho do erro ============================== -->

          <path d="M623 158 L660 158 L660 240 L580 240" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3" marker-end="url(#setaErro)"/>
          <text x="672" y="204" fill="var(--log)" font-size="10">se um índice</text>
          <text x="672" y="217" fill="var(--log)" font-size="10">falhar</text>

          <rect x="396" y="214" width="180" height="52" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5" stroke-dasharray="5 3" opacity="0"/>
          <rect x="516" y="284" width="180" height="52" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5" stroke-dasharray="5 3"/>
          <text x="606" y="304" text-anchor="middle" fill="var(--log)" font-size="11">desfaz tudo</text>
          <text x="606" y="319" text-anchor="middle" font-size="9" opacity=".62">tira as chaves já postas,</text>
          <text x="606" y="331" text-anchor="middle" font-size="9" opacity=".62">exclui o slot, libera os blocos</text>
          <path d="M580 240 L560 240 L560 284" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3"/>

          <!-- ================= o descritor, que nao entra por linha =========== -->

          <rect x="16" y="284" width="220" height="58" rx="4" fill="none" stroke="currentColor" stroke-width="1.2" opacity=".5"/>
          <text x="126" y="303" text-anchor="middle" font-size="10.5" opacity=".75">.pag — o descritor</text>
          <text x="126" y="317" text-anchor="middle" font-size="9" opacity=".55">regravado ao criar ou repartir,</text>
          <text x="126" y="329" text-anchor="middle" font-size="9" opacity=".55">nunca por linha. O motor não o lê</text>

          <line x1="16" y1="368" x2="816" y2="368" stroke="currentColor" stroke-width="1" opacity=".3"/>

          <text x="16" y="394" font-size="11.5" opacity=".75" font-weight="600">Quatro regras que o desenho impõe:</text>
          <circle cx="26" cy="418" r="3" fill="var(--acento)"/>
          <text x="40" y="422" font-size="11.5">A unicidade é conferida <tspan font-weight="600">antes</tspan> de qualquer escrita — uma recusa não deixa slot fantasma.</text>
          <circle cx="26" cy="444" r="3" fill="var(--acento)"/>
          <text x="40" y="448" font-size="11.5">O <tspan font-weight="600">rownum sai do motor</tspan>, nunca de quem chama: um número escolhido seria uma ordem inventada.</text>
          <circle cx="26" cy="470" r="3" fill="var(--log)"/>
          <text x="40" y="474" font-size="11.5">Operação recusada <tspan font-weight="600">não vira evento</tspan>: o diário registra o que aconteceu, não o que foi tentado.</text>
          <circle cx="26" cy="496" r="3" fill="var(--pend)"/>
          <text x="40" y="500" font-size="11.5">O espelho é escrito <tspan font-weight="600">no mesmo instante</tspan> que o principal — não é uma cópia feita depois.</text>

          <text x="16" y="530" font-size="10.5" opacity=".55">Alterar segue o mesmo caminho: herda o rownum e a marca, remove a chave antiga só quando ela mudou, e libera os blocos antigos no fim.</text>
          <text x="16" y="548" font-size="10.5" opacity=".55">Excluir tem dois caminhos, e o padrão é o reversível — a figura seguinte mostra os dois.</text>
        </g>
      </svg>
'''

nova_legenda = '''    <figcaption><b>Figura 8.</b> Sete arquivos, e três deles não são escritos por
    linha: o <code>.bkp</code> só quando o espelho está ligado, a imagem no
    <code>.log</code> só quando a replicação está, e o <code>.pag</code> nunca —
    ele é regravado quando a tabela nasce ou muda de partição. A aresta tracejada
    em vermelho é a que importa: sem transações ainda, o desfazer explícito é o
    que impede uma falha de índice de deixar a tabela inconsistente.</figcaption>
'''

s = s[:inicio] + novo_svg + '    </div>\n' + nova_legenda + s[s.index('  </figure>', fim):]
p.write_text(s)
print("ok")
