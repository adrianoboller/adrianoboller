# Add the mirror and durability window to the write flow
# 28/08 17:02

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()

# --- a caixa do espelho entra no fluxo, e a janela de durabilidade tambem
a='''          <path d="M626 62 L662 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>'''
b='''          <!-- O espelho: uma segunda escrita do MESMO slot, no mesmo instante.
               Faltava no desenho, e por isso o .bkp parecia um arquivo que
               alguem preenche depois. Nao e: ele e escrito JUNTO. -->
          <path d="M563 90 L563 116" stroke="var(--pend)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaE)"/>
          <rect x="500" y="120" width="126" height="46" rx="4" fill="none" stroke="var(--pend)" stroke-width="1.4" stroke-dasharray="5 3"/>
          <text x="563" y="139" text-anchor="middle" fill="var(--pend)" font-size="11">espelha o slot</text>
          <text x="563" y="153" text-anchor="middle" fill="var(--pend)" font-size="10.5">.bkp</text>
          <text x="563" y="164" text-anchor="middle" font-size="9" opacity=".55">só quando ligado</text>

          <path d="M626 62 L662 62" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>'''
assert a in s; s=s.replace(a,b,1)

# a caixa "devolve o rowid" muda de lugar para nao colidir com a do espelho
a='''          <path d="M666 158 L604 158" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>
          <rect x="470" y="136" width="130" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="535" y="163" text-anchor="middle" font-size="11">devolve o rowid</text>'''
b='''          <path d="M729 184 L729 206" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>
          <rect x="646" y="210" width="166" height="46" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.4"/>
          <text x="729" y="228" text-anchor="middle" fill="var(--acento)" font-size="11">janela de durabilidade</text>
          <text x="729" y="243" text-anchor="middle" font-size="9.5" opacity=".6">agora · por lote · sistema</text>

          <path d="M646 233 L560 233" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaE)"/>
          <rect x="426" y="211" width="130" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="491" y="238" text-anchor="middle" font-size="11">devolve o rowid</text>'''
assert a in s; s=s.replace(a,b,1)

# a regua e as regras descem, e a terceira regra entra
a='''          <line x1="16" y1="212" x2="816" y2="212" stroke="currentColor" stroke-width="1" opacity=".3"/>

          <text x="16" y="238" font-size="11.5" opacity=".75" font-weight="600">Duas regras que o desenho impõe:</text>
          <circle cx="26" cy="262" r="3" fill="var(--acento)"/>
          <text x="40" y="266" font-size="11.5">A unicidade é conferida <tspan font-weight="600">antes</tspan> de qualquer escrita — uma recusa não deixa slot fantasma.</text>
          <circle cx="26" cy="288" r="3" fill="var(--acento)"/>
          <text x="40" y="292" font-size="11.5">Operação recusada <tspan font-weight="600">não vira evento</tspan>: o diário registra o que aconteceu, não o que foi tentado.</text>

          <text x="16" y="330" font-size="11" opacity=".55">Alterar segue o mesmo caminho, mas remove a chave antiga do índice só quando ela mudou, e libera os blocos externos antigos no fim.</text>
          <text x="16" y="348" font-size="11" opacity=".55">Excluir tira as chaves, marca os blocos como mortos e marca o slot como livre — sem nunca reaproveitá-lo.</text>'''
b='''          <line x1="16" y1="278" x2="816" y2="278" stroke="currentColor" stroke-width="1" opacity=".3"/>

          <text x="16" y="304" font-size="11.5" opacity=".75" font-weight="600">Três regras que o desenho impõe:</text>
          <circle cx="26" cy="328" r="3" fill="var(--acento)"/>
          <text x="40" y="332" font-size="11.5">A unicidade é conferida <tspan font-weight="600">antes</tspan> de qualquer escrita — uma recusa não deixa slot fantasma.</text>
          <circle cx="26" cy="354" r="3" fill="var(--acento)"/>
          <text x="40" y="358" font-size="11.5">Operação recusada <tspan font-weight="600">não vira evento</tspan>: o diário registra o que aconteceu, não o que foi tentado.</text>
          <circle cx="26" cy="380" r="3" fill="var(--pend)"/>
          <text x="40" y="384" font-size="11.5">O espelho é escrito <tspan font-weight="600">no mesmo instante</tspan> que o principal — não é uma cópia feita depois.</text>

          <text x="16" y="418" font-size="11" opacity=".55">Alterar segue o mesmo caminho, mas remove a chave antiga do índice só quando ela mudou, e libera os blocos externos antigos no fim.</text>
          <text x="16" y="436" font-size="11" opacity=".55">Excluir tira as chaves, marca os blocos como mortos e marca o slot como livre — sem nunca reaproveitá-lo.</text>'''
assert a in s; s=s.replace(a,b,1)

s=s.replace('<svg viewBox="0 0 840 360" role="img" aria-label="Caminho de uma inserção: checa unicidade, grava blobs, monta o payload, anexa ao reg, insere as chaves nos índices e registra o evento no log; se um índice falhar, tudo é desfeito">',
            '<svg viewBox="0 0 840 450" role="img" aria-label="Caminho de uma inserção: confere a unicidade, grava os blobs no bin e no memo, anexa o slot ao reg e o espelha no bkp quando o espelho está ligado, insere as chaves nos índices do ndx, registra o evento no log, e só então a janela de durabilidade decide quando tudo isso chega ao disco; se um índice falhar, tudo é desfeito.">',1)

a='''    <figcaption><b>Figura 7.</b> A aresta tracejada em vermelho é a que importa: sem
    transações ainda, o desfazer explícito é o que impede uma falha de índice de deixar
    a tabela inconsistente.</figcaption>
  </figure>
</section>'''
b='''    <figcaption><b>Figura 7.</b> A aresta tracejada em vermelho é a que importa: sem
    transações ainda, o desfazer explícito é o que impede uma falha de índice de deixar
    a tabela inconsistente. A caixa âmbar do <code>.bkp</code> só existe quando o
    espelho está ligado — e ela estava faltando neste desenho.</figcaption>
  </figure>

  <h3>O sexto arquivo: o espelho <code>.bkp</code></h3>

  <p>A tabela é <strong>cinco</strong> arquivos, e um sexto opcional. O
  <code>.bkp</code> é um clone byte a byte do <code>.reg</code>, volume por
  volume, e o desenho acima estava incompleto sem ele: <strong>ele é escrito no
  mesmo instante que o principal</strong>, e não copiado depois. Custa uma
  escrita a mais por gravação e o dobro do espaço do <code>.reg</code>.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Momento</th><th>O que acontece com o espelho</th></tr></thead>
      <tbody>
        <tr><td>Ao ligar</td><td>o <code>.reg</code> inteiro é copiado — mas <strong>só se o tamanho não bater</strong>. Copiar por cima de um espelho que já existe seria destruir a cópia justamente quando ela é necessária, e um teste pegou exatamente isso: estragar o principal e religar o espelho não pode apagar a única cópia boa</td></tr>
        <tr><td>Ao gravar</td><td>o mesmo slot vai para os dois arquivos, no mesmo <em>offset</em></td></tr>
        <tr><td>Ao ler</td><td>o espelho <strong>não é lido</strong>. Só quando o slot principal falha: o CRC não bate, ou o byte de status não é nem livre nem ativo</td></tr>
        <tr><td>Ao reparar</td><td>a varredura percorre todos os slots. Onde o principal quebrou e o espelho está bom, o principal é reescrito; onde o principal está bom e o <em>espelho</em> quebrou, o espelho é reescrito</td></tr>
      </tbody>
    </table>
  </div>

  <p>A tabela conta quantas leituras o espelho salvou desde que foi aberta. É o
  número que diz se ele está pagando o custo — e um número que sobe sozinho é
  disco morrendo, não espelho funcionando.</p>

  <div class="nota">
    <p><strong>O que o espelho protege, e o que não protege.</strong> Ele
    protege contra o dado ficar <em>ruim</em>: bit trocado, escrita cortada no
    meio, setor com defeito. Não protege contra o disco <em>morrer</em> — os
    dois arquivos moram no mesmo lugar. Para isso existe o backup, que é outra
    coisa e vai para outro lugar.</p>
    <p>Um detalhe que já custou um defeito: enquanto qualquer valor diferente
    de «ativo» era tratado como «excluído», <strong>um único bit trocado no
    cabeçalho do slot apagava o registro em silêncio</strong> — a leitura
    devolvia «não existe» sem erro, e o reparo considerava o slot bom e nunca
    ia buscar a cópia no espelho, que estava lá, inteira. Hoje o byte de status
    só pode ser livre ou ativo; qualquer outra coisa é corrupção, e corrupção
    manda ler o espelho.</p>
  </div>
</section>'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
