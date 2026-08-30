# Add the deletion-flow figure to the dossier
# 28/08 18:08

import io
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()

alvo = '''  <div class="nota">
    <p><strong>O que o espelho protege, e o que não protege.</strong>'''

novo = '''  <h3>As duas exclusões, e a ordem que é a garantia</h3>

  <p>Toda tabela nasce com a coluna de sistema <code>softdeleted</code>, e por
  isso excluir passou a ser duas coisas diferentes. O <strong>padrão do
  protocolo é o caminho reversível</strong>: um cliente que manda
  <code>excluir</code> sem dizer mais nada está pedindo «tira isto da minha
  lista», e é isso que ele recebe. O irreversível existe, mas se escreve.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 470" role="img" aria-label="Os dois caminhos de uma exclusão. Sem fisico, a linha é marcada na coluna softdeleted e continua inteira no reg, com o motivo indo para o reason. Com fisico, a linha inteira é gravada no trash, o disco confirma, e só então as chaves saem do ndx, os blocos do bin e do memo são liberados e o slot do reg é marcado como livre.">
        <defs>
          <marker id="setaX" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
          <marker id="setaXok" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="var(--ok)"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <rect x="16" y="40" width="104" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="68" y="60" text-anchor="middle" font-size="11">excluir</text>
          <text x="68" y="75" text-anchor="middle" font-size="10" opacity=".6">rowid, motivo</text>

          <path d="M120 62 L146 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaX)"/>

          <rect x="150" y="34" width="116" height="56" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="208" y="56" text-anchor="middle" font-size="11">"fisico"?</text>
          <text x="208" y="72" text-anchor="middle" font-size="9.5" opacity=".55">omitido = não</text>
          <text x="208" y="85" text-anchor="middle" font-size="9.5" opacity=".55">o padrão volta</text>

          <!-- ramo suave: horizontal, porque e o caminho comum -->
          <path d="M266 62 L302 62" stroke="var(--ok)" stroke-width="1.4" marker-end="url(#setaXok)"/>
          <text x="284" y="52" text-anchor="middle" fill="var(--ok)" font-size="9.5">não</text>

          <rect x="306" y="34" width="176" height="56" rx="4" fill="none" stroke="var(--ok)" stroke-width="1.5"/>
          <text x="394" y="54" text-anchor="middle" fill="var(--ok)" font-size="11">marca a coluna</text>
          <text x="394" y="69" text-anchor="middle" fill="var(--ok)" font-size="10.5">softdeleted = sim</text>
          <text x="394" y="83" text-anchor="middle" font-size="9.5" opacity=".55">a linha fica inteira no .reg</text>

          <path d="M482 62 L518 62" stroke="var(--ok)" stroke-width="1.4" marker-end="url(#setaXok)"/>

          <rect x="522" y="34" width="150" height="56" rx="4" fill="none" stroke="var(--pend)" stroke-width="1.5"/>
          <text x="597" y="54" text-anchor="middle" fill="var(--pend)" font-size="11">registra o porquê</text>
          <text x="597" y="69" text-anchor="middle" fill="var(--pend)" font-size="10.5">.reason</text>
          <text x="597" y="83" text-anchor="middle" font-size="9.5" opacity=".55">suave</text>

          <text x="700" y="58" font-size="10.5" fill="var(--ok)">restaurar</text>
          <text x="700" y="73" font-size="9.5" opacity=".6">desfaz</text>

          <!-- ramo fisico: desce, porque e o caminho longo -->
          <path d="M208 90 L208 122" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaX)"/>
          <text x="228" y="110" font-size="9.5" fill="var(--log)">sim</text>

          <rect x="112" y="126" width="192" height="52" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5"/>
          <text x="208" y="145" text-anchor="middle" fill="var(--log)" font-size="11">grava a linha inteira</text>
          <text x="208" y="160" text-anchor="middle" fill="var(--log)" font-size="10.5">.trash</text>
          <text x="208" y="173" text-anchor="middle" font-size="9" opacity=".55">payload + conteúdo dos anexos</text>

          <path d="M208 178 L208 204" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaX)"/>

          <!-- A caixa que e a garantia inteira desta funcionalidade. -->
          <rect x="102" y="208" width="212" height="52" rx="4" fill="none" stroke="var(--acento)" stroke-width="2"/>
          <text x="208" y="228" text-anchor="middle" fill="var(--acento)" font-size="11.5" font-weight="600">o disco confirma</text>
          <text x="208" y="244" text-anchor="middle" font-size="9.5" opacity=".62">só aqui a linha está segura</text>
          <text x="208" y="256" text-anchor="middle" font-size="9" opacity=".55">fsync do .trash</text>

          <path d="M208 260 L208 282" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaX)"/>

          <rect x="112" y="286" width="150" height="48" rx="4" fill="none" stroke="var(--ndx)" stroke-width="1.5"/>
          <text x="187" y="306" text-anchor="middle" fill="var(--ndx)" font-size="11">tira as chaves</text>
          <text x="187" y="321" text-anchor="middle" fill="var(--ndx)" font-size="10.5">.ndx</text>

          <path d="M262 310 L296 310" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaX)"/>

          <rect x="300" y="286" width="170" height="48" rx="4" fill="none" stroke="var(--bin)" stroke-width="1.5"/>
          <text x="385" y="306" text-anchor="middle" fill="var(--bin)" font-size="11">libera os blocos</text>
          <text x="385" y="321" text-anchor="middle" fill="var(--bin)" font-size="10.5">.bin / .memo</text>

          <path d="M470 310 L504 310" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaX)"/>

          <rect x="508" y="286" width="150" height="48" rx="4" fill="none" stroke="var(--reg)" stroke-width="1.5"/>
          <text x="583" y="306" text-anchor="middle" fill="var(--reg)" font-size="11">libera o slot</text>
          <text x="583" y="321" text-anchor="middle" fill="var(--reg)" font-size="10.5">.reg</text>

          <path d="M658 310 L692 310" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaX)"/>

          <rect x="696" y="286" width="128" height="48" rx="4" fill="none" stroke="var(--pend)" stroke-width="1.5"/>
          <text x="760" y="306" text-anchor="middle" fill="var(--pend)" font-size="11">.reason</text>
          <text x="760" y="321" text-anchor="middle" font-size="9.5" opacity=".55">física</text>

          <line x1="16" y1="360" x2="816" y2="360" stroke="currentColor" stroke-width="1" opacity=".3"/>

          <text x="16" y="386" font-size="11.5" opacity=".75" font-weight="600">Três decisões que este desenho carrega:</text>
          <circle cx="26" cy="410" r="3" fill="var(--acento)"/>
          <text x="40" y="414" font-size="11.5">O <tspan font-weight="600">disco confirma antes</tspan> de o slot sair. Guardar depois teria uma janela em que a linha não existe em lugar nenhum.</text>
          <circle cx="26" cy="436" r="3" fill="var(--log)"/>
          <text x="40" y="440" font-size="11.5">O <code>.trash</code> guarda o <tspan font-weight="600">conteúdo</tspan> dos anexos, não os ponteiros — os blocos que ele apontaria acabaram de ser liberados.</text>
          <circle cx="26" cy="462" r="3" fill="var(--ok)"/>
          <text x="40" y="466" font-size="11.5">O caminho <tspan font-weight="600">reversível é o padrão</tspan>: o irreversível não pode ser escolhido por omissão.</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 8.</b> A caixa vermelha grossa é a funcionalidade inteira.
    Entre perder o dado e duplicá-lo, o motor duplica: se a máquina cair entre o
    <em>fsync</em> e a liberação do slot, a linha aparece nos dois lugares — o que se
    resolve olhando. A ordem inversa não tem conserto depois.</figcaption>
  </figure>

  <div class="rolo">
    <table>
      <thead><tr><th>&nbsp;</th><th>Suave (o padrão)</th><th>Física (<code>"fisico": true</code>)</th></tr></thead>
      <tbody>
        <tr><td>O que muda</td><td>um byte da coluna <code>softdeleted</code></td><td>o slot vai a livre, as chaves saem, os blocos são liberados</td></tr>
        <tr><td>A linha</td><td>continua <strong>inteira</strong> no <code>.reg</code>, com os anexos</td><td>vai <strong>inteira</strong> para o <code>.trash</code>, com o conteúdo dos anexos</td></tr>
        <tr><td>Quem enxerga</td><td>ninguém, na varredura comum; <code>"visao": "excluidas"</code> mostra</td><td>ninguém — só a lixeira, e só quem administra</td></tr>
        <tr><td>Volta?</td><td><code>restaurar</code>, no mesmo rowid</td><td>reinserindo, e com <strong>outro</strong> rowid: o <code>.reg</code> não reaproveita slot nem por restauração</td></tr>
        <tr><td>Custo</td><td>uma reescrita de slot</td><td>uma cópia da linha + um <em>fsync</em></td></tr>
      </tbody>
    </table>
  </div>

  <p>O <code>.reason</code> recebe os dois casos, e mais dois: a restauração e o
  expurgo. Ele guarda a frase, quem, quando, e a <strong>identidade da linha em
  texto</strong> — a chave primária, e não o rowid, porque «rowid 4173» não diz
  nada seis meses depois. O expurgo da lixeira é registrado <strong>antes</strong>
  de o dado sair: o motivo tem de sobreviver ao dado.</p>

  <div class="nota">
    <p><strong>A coluna entra no fim da lista, e isso não é estética.</strong>
    Os <em>offsets</em> das colunas do usuário não podem mudar de lugar quando
    ela entra — e a leitura do disco <strong>não</strong> a acrescenta, porque a
    lista de colunas gravada é a verdade inteira. Se a leitura acrescentasse a
    coluna, cada linha de uma tabela anterior passaria a ser lida com os
    <em>offsets</em> deslocados, <strong>e em silêncio</strong>: o CRC do slot
    continuaria batendo, porque os bytes seriam os mesmos — só a interpretação
    mudaria. Há teste que trava exatamente isso.</p>
    <p>O <code>.trash</code> e o <code>.reason</code> exigem
    <strong>administrar</strong>, e a razão está no conteúdo. Quem só tem
    <code>ler</code> perdeu o direito àquela linha no instante em que ela foi
    excluída, e a lixeira devolveria o direito por outra porta. E um motivo de
    exclusão costuma ser mais revelador que o registro: <em>fraude</em>,
    <em>pedido de remoção do titular</em>, <em>duplicidade com o contrato X</em>.
    Os dois <strong>não</strong> são cifrados nem compactados hoje — a proteção
    é a permissão, e no disco vale a do sistema de arquivos.</p>
  </div>

  <div class="nota">
    <p><strong>O que o espelho protege, e o que não protege.</strong>'''

assert alvo in s
s = s.replace(alvo, novo, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
