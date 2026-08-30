# Add the table-management section to the dossier
# 28/08 11:05

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

v = '''        <tr><td>Bloqueios</td><td>quem está barrado, por quê e até quando</td><td><code>bloqueios</code></td></tr>
      </tbody>
    </table>
  </div>
'''
n = '''        <tr><td>Bloqueios</td><td>quem está barrado, por quê e até quando</td><td><code>bloqueios</code></td></tr>
        <tr><td>Tabelas</td><td>as tabelas do banco e as oito operações sobre cada uma</td><td><code>tabelas</code>, <code>esquema</code></td></tr>
        <tr><td>Nova tabela</td><td>colunas, índices, schema e paginação, montados linha a linha</td><td><code>criar_tabela</code></td></tr>
        <tr><td>Partições</td><td>em que volume cada faixa de rowid cai, e o nome do arquivo</td><td><code>esquema</code> + conta</td></tr>
      </tbody>
    </table>
  </div>

  <h3>Gestão de tabelas: o que cada operação toca no disco</h3>

  <p>A tela de tabelas não é um menu de conveniência: cada item ali mexe em um
  conjunto diferente dos cinco arquivos, e é isso que decide o que é reversível
  e o que não é. <em>Reparar índice</em> joga fora um arquivo que sabe
  reconstruir sozinho; <em>Excluir</em> apaga seis e não sabe reconstruir
  nenhum.</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Operação</th><th class="num">.reg</th><th class="num">.ndx</th>
        <th class="num">.bin</th><th class="num">.memo</th><th class="num">.log</th>
        <th class="num">.bkp</th><th>Desfaz?</th></tr></thead>
      <tbody>
        <tr><td class="dado">Estrutura</td><td class="num">lê</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td>não altera</td></tr>
        <tr><td class="dado">Partições</td><td class="num">lê</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td>não altera</td></tr>
        <tr><td class="dado">Editar conteúdo</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td class="num">grava</td><td>linha a linha, pelo diário</td></tr>
        <tr><td class="dado">Nova tabela</td><td class="num">cria</td><td class="num">cria</td><td class="num">cria</td><td class="num">cria</td><td class="num">cria</td><td class="num">—</td><td>excluindo</td></tr>
        <tr><td class="dado">Duplicar</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td class="num">copia</td><td>excluindo a cópia</td></tr>
        <tr><td class="dado">Reparar tabela</td><td class="num">grava</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">grava</td><td>não — repara em cima</td></tr>
        <tr><td class="dado">Reparar índice</td><td class="num">lê</td><td class="num">refaz</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td class="num">—</td><td>refazendo de novo</td></tr>
        <tr><td class="dado">Excluir</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td class="num">apaga</td><td><strong>não</strong></td></tr>
      </tbody>
    </table>
  </div>

  <p>Três coisas que a linha do <em>Excluir</em> obriga. A permissão é
  <code>administrar</code>, não <code>excluir</code>: poder perder uma linha não
  é poder perder a tabela. O servidor exige o nome repetido num campo
  <code>confirmar</code>, e a tela pede que ele seja digitado — o clique errado
  não basta. E o espelho <code>.bkp</code> vai junto: deixá-lo para trás faria a
  tabela «voltar» pela metade se alguém recriasse uma com o mesmo nome.</p>

  <p>A conferência de qual arquivo pertence a qual tabela também não é
  ingênua. Uma tabela paginada tem vários volumes por extensão, então o alvo é
  <code>nome.ext</code> ou <code>nome_NNN.ext</code> com o sufixo todo em
  algarismos — sem isso, excluir <code>precos</code> levaria
  <code>precos_historico</code> junto.</p>

  <h3>Transações: uma tela que diz que não existem</h3>

  <p>O menu <em>Ferramentas</em> tem <em>Gestão de transações</em>, e o que ela
  mostra é a ausência. Não há <code>BEGIN</code>, <code>COMMIT</code> nem
  <code>ROLLBACK</code>, então a tela não traz uma lista de transações abertas:
  uma lista vazia daria a entender que o mecanismo existe e está parado. Ela
  lista o que <em>de fato</em> existe — a inserção que se desfaz sozinha quando
  um índice único recusa a chave, a trava global que serializa as escritas, o
  <code>.log</code> que audita sem guardar a imagem anterior — e o que falta,
  na ordem: journal com a imagem anterior de cada slot, identificador de
  transação na sessão, e o <code>COMMIT</code> que troca o journal por um marco
  no disco.</p>
'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
