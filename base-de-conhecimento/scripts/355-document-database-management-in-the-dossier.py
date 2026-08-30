# Document database management in the dossier
# 28/08 13:12

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

SECAO = r'''  <h3>Gerir o banco: quinze itens, e quatro deles apagados</h3>

  <p>A tela de tabelas trata de <em>uma</em> tabela. A de gerir banco trata do
  database inteiro, e junta em quatro grupos tudo que se faz sobre ele: dados e
  catálogo, configuração e acesso, operação, e — separado, com o nome que
  merece — <strong>ainda não existe</strong>.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Grupo</th><th>Itens</th></tr></thead>
      <tbody>
        <tr><td class="dado">Dados e catálogo</td><td>Tabelas · SysTables · SysColumns · Copiar tabela</td></tr>
        <tr><td class="dado">Configuração e acesso</td><td>Configurações do banco · Diretivas de acesso · Editor de menu</td></tr>
        <tr><td class="dado">Operação</td><td>Conexões · Arquivos bloqueados · Transações · Backup e restauração</td></tr>
        <tr><td class="dado">Ainda não existe</td><td><em>Triggers · Procedures · Jobs · Modo exclusivo</em></td></tr>
      </tbody>
    </table>
  </div>

  <p>Os quatro últimos ficam ali <strong>apagados</strong>, e clicar num deles
  abre uma tela que diz o que falta e de que depende — não um aviso genérico.
  Sumir com eles esconderia o roteiro; ligá-los a um «em breve» seria fingir.
  Triggers e procedures esperam uma decisão que não é técnica: <em>em que
  linguagem o gatilho é escrito</em>. Jobs é o mais barato dos três — o
  agendador do backup já é o desenho. Modo exclusivo depende da trava por
  tabela, que é o mesmo trabalho da concorrência fina.</p>

  <h3>SysTables e SysColumns: o catálogo vira dado</h3>

  <p>O mesmo que a tela de gestão mostra, em forma de tabela — para quem quer
  <em>consultar</em> o catálogo em vez de olhar para ele. <code>SysTables</code>
  dá uma linha por tabela com registros, slots, chave primária, bytes por linha,
  partição e volumes. <code>SysColumns</code> é o dicionário de dados: cada
  campo com <code>id</code>, caption, descrição, máscara, tipo, tamanho e o
  papel na chave.</p>

  <p>Uma tabela ilegível não derruba o catálogo: ela vira uma linha que diz que
  está ilegível — que é exatamente a informação que alguém foi procurar ali.</p>

  <h3>As telas de configuração leem, e não gravam</h3>

  <p>São três, e o que separa uma da outra é o <em>alcance</em>: o servidor
  inteiro (o <code>config.json</code>), um database, e quem entra. Cada campo
  aparece com o nome que tem no arquivo, o valor que está valendo agora, e para
  que serve.</p>

  <p>Nenhuma delas grava, e isso é escolha. Gravar o <code>config.json</code>
  pela porta web significaria que uma sessão roubada consegue <strong>abrir o
  firewall, esvaziar a lista de comandos proibidos e criar um
  supervisor</strong>. Criar e alterar usuário tem o mesmo problema, com
  credencial no meio do caminho. O que falta para quem administra é saber
  <em>qual campo mexer</em> — e isso a tela dá.</p>

  <p>A tela de usuários não traz o hash da senha porque o servidor não o
  devolve, em operação nenhuma. Há teste que falha se a ficha vazar.</p>

  <h3>Os seis portões, na ordem em que fecham</h3>

  <p>A tela de diretivas do banco mostra o caminho que um pedido percorre antes
  de tocar em dado, e o que está valendo em cada ponto:</p>

  <div class="rolo">
    <table>
      <thead><tr><th class="num">#</th><th>Portão</th><th>O que recusa</th></tr></thead>
      <tbody>
        <tr><td class="num dado">1</td><td class="dado">IP permitido</td><td>quem não está na lista nem chega a falar</td></tr>
        <tr><td class="num dado">2</td><td class="dado">lista de bloqueio</td><td>IP barrado por tentativa anterior</td></tr>
        <tr><td class="num dado">3</td><td class="dado">comando proibido</td><td>recusado antes de tocar os dados</td></tr>
        <tr><td class="num dado">4</td><td class="dado">base proibida</td><td>o database inteiro fora do alcance</td></tr>
        <tr><td class="num dado">5</td><td class="dado">somente leitura</td><td>toda operação de escrita</td></tr>
        <tr><td class="num dado">6</td><td class="dado">permissão do usuário</td><td>a atividade contra a base, pelas três regras</td></tr>
      </tbody>
    </table>
  </div>

  <p>Ela também resolve, por usuário, <em>como</em> a permissão chegou ao
  resultado: se veio da base listada, do <code>"*"</code>, de ser supervisor, ou
  se negou porque não havia nenhum dos dois. Ver a regra aplicada vale mais do
  que ler a regra.</p>

  <h3>O editor de menu mora no navegador, não no servidor</h3>

  <p>Ele troca o nome exibido de qualquer um dos 82 rótulos da barra e dos
  menus. O que muda é o texto: <strong>a letra do atalho continua a de
  fábrica</strong>, senão renomear um menu mudaria a tecla debaixo do dedo de
  quem já aprendeu.</p>

  <p>A preferência fica no <code>localStorage</code> de quem mexeu, e não no
  <code>config.json</code>. É preferência de quem opera, não política do banco:
  dois operadores podem querer nomes diferentes, e nenhum dos dois deveria mudar
  a tela do outro.</p>

'''
marca = '''  <h3>Transações: uma tela que diz que não existem</h3>'''
assert s.count(marca) == 1
s = s.replace(marca, SECAO + marca, 1)
p.write_text(s)
print('secao da gestao do banco')
