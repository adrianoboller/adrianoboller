# Add the conflict section to the dossier
# 29/08 00:40

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
alvo = '''    manda ler o espelho.</p>
  </div>
</section>

<!-- ============================= 08 ============================= -->'''
novo = '''    manda ler o espelho.</p>
  </div>

  <h3>A janela de conflito de escrita</h3>

  <p>Alguém abre a ficha do registro 42 às 9h02, sai para o café, volta às 9h11 e
  salva. Entre uma coisa e outra, outra pessoa gravou a mesma linha. Sem conferência,
  o segundo «salvar» apaga o trabalho do primeiro — <strong>sem erro, sem registro,
  sem ninguém perceber</strong> até faltar o dado.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 250" role="img" aria-label="Duas sessões leem o registro na versão 3; a primeira grava e a versão passa a 4; a segunda chega com a versão 3 e é recusada com o erro 3004">
        <defs>
          <marker id="setaCf" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
          <marker id="setaCfN" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
          </marker>
        </defs>

        <text x="20" y="26" font-size="11" fill="currentColor" opacity=".62">SESSÃO A</text>
        <text x="20" y="150" font-size="11" fill="currentColor" opacity=".62">SESSÃO B</text>

        <line x1="20" y1="118" x2="820" y2="118" stroke="currentColor" opacity=".18" stroke-dasharray="4 4"/>

        <rect x="90" y="38" width="150" height="42" rx="7" fill="none" stroke="currentColor" opacity=".55"/>
        <text x="165" y="56" text-anchor="middle" font-size="12" fill="currentColor">ler, com_versao</text>
        <text x="165" y="71" text-anchor="middle" font-size="12" fill="currentColor" opacity=".7">versão 3</text>

        <rect x="90" y="162" width="150" height="42" rx="7" fill="none" stroke="currentColor" opacity=".55"/>
        <text x="165" y="180" text-anchor="middle" font-size="12" fill="currentColor">ler, com_versao</text>
        <text x="165" y="195" text-anchor="middle" font-size="12" fill="currentColor" opacity=".7">versão 3</text>

        <rect x="360" y="38" width="170" height="42" rx="7" fill="none" stroke="var(--acento)"/>
        <text x="445" y="56" text-anchor="middle" font-size="12" fill="var(--acento)">atualizar, versão 3</text>
        <text x="445" y="71" text-anchor="middle" font-size="12" fill="var(--acento)" opacity=".8">gravou → versão 4</text>

        <rect x="360" y="162" width="170" height="42" rx="7" fill="none" stroke="var(--log)"/>
        <text x="445" y="180" text-anchor="middle" font-size="12" fill="var(--log)">atualizar, versão 3</text>
        <text x="445" y="195" text-anchor="middle" font-size="12" fill="var(--log)" opacity=".85">3004 CONFLITO</text>

        <rect x="650" y="162" width="170" height="42" rx="7" fill="none" stroke="currentColor" opacity=".55"/>
        <text x="735" y="180" text-anchor="middle" font-size="12" fill="currentColor">a janela das</text>
        <text x="735" y="195" text-anchor="middle" font-size="12" fill="currentColor">três colunas</text>

        <line x1="240" y1="59" x2="356" y2="59" stroke="currentColor" opacity=".55" marker-end="url(#setaCf)"/>
        <line x1="240" y1="183" x2="356" y2="183" stroke="currentColor" opacity=".55" marker-end="url(#setaCf)"/>
        <line x1="530" y1="183" x2="646" y2="183" stroke="var(--log)" marker-end="url(#setaCfN)"/>

        <text x="445" y="112" text-anchor="middle" font-size="11" fill="currentColor" opacity=".6">a versão do slot do .reg sobe a cada regravação</text>
      </svg>
    </div>
    <figcaption>A peça já estava no formato: cada slot do <code>.reg</code> guarda uma
    versão desde a v1, e ninguém a usava. Conferir custa 24 bytes — o cabeçalho do
    slot, não a linha.</figcaption>
  </figure>

  <p>Quem lê com <code>"com_versao": true</code> recebe a versão junto e a manda de
  volta no <code>atualizar</code>. Se ela não for mais a atual, a gravação é recusada
  com o erro <strong>3004 <code>CONFLITO</code></strong>. Vale também no
  <code>excluir</code> e no <code>restaurar</code> — apagar uma linha que outra pessoa
  acabou de alterar é a mesma janela. E <strong>excluída de vez também é conflito</strong>,
  e não «não encontrado»: quem leu a linha há um minuto precisa saber que ela foi
  apagada, e não que o rowid nunca existiu.</p>

  <div class="nota">
    <span class="t">Não é trava, e a conferência é pedida</span>
    <p>Travar a linha na leitura resolveria o mesmo problema e criaria dois piores: a
    linha fica presa quando alguém fecha o navegador com a ficha aberta, e duas sessões
    que travam em ordem trocada se abraçam. O contador não prende nada — só recusa a
    segunda gravação.</p>
    <p>E a conferência é <strong>pedida, não imposta</strong>: quem manda
    <code>"versao"</code> ganha a garantia, quem não manda continua com a última
    gravação vencendo. Imposta, todo cliente escrito antes desta versão pararia de
    gravar de um dia para o outro, recebendo um erro que não sabe tratar. A interface
    web manda sempre, porque é ali que existe gente — e a janela de minutos entre abrir
    a ficha e clicar em salvar.</p>
  </div>

  <p>Na tela, o conflito abre as <strong>três colunas</strong> que o HFSQL(R)
  mostra — «valor anterior», «o outro escreveu», «você escreve» — e vai um passo além
  dele: <strong>já vem marcado quem mexeu em cada coluna</strong>. A que você digitou
  fica com o seu valor, a que só o outro mudou fica com o dele; dois que editaram
  campos diferentes da mesma linha saem dali com os dois trabalhos preservados, sem
  escolher nada. Marcar tudo como «o meu» por omissão desfaria em silêncio o trabalho
  do outro nas colunas que você nem tocou — o mesmo estrago de antes, com mais
  cliques.</p>
</section>

<!-- ============================= 08 ============================= -->'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
