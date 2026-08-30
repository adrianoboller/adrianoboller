# Add the header finding to the dossier
# 29/08 01:14

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
alvo = '''  <div class="nota">
    <span class="t">O pedido pedia outra coisa, e a medição mandou</span>'''
novo = '''  <h3>E o cabeçalho que reserializava o esquema a cada linha</h3>

  <p>Achado respondendo a uma pergunta sobre outra coisa — «e se o
  <code>.ndx</code> parasse durante a carga?» —, que é onde essas coisas costumam
  aparecer. Toda inserção chamava <code>gravar_cabecalho</code>, e ele fazia
  <strong>cinco coisas, das quais uma era necessária</strong>:</p>

  <div class="rolo">
    <table>
      <thead><tr><th class="num">#</th><th>o que fazia por linha inserida</th><th>precisa?</th></tr></thead>
      <tbody>
        <tr><td class="num">1</td><td>serializar o <strong>esquema inteiro</strong></td><td><span class="pino nao">não</span></td></tr>
        <tr><td class="num">2</td><td>calcular o <strong>CRC-32</strong> desse bloco</td><td><span class="pino nao">não</span></td></tr>
        <tr><td class="num">3</td><td>gravar os 128 bytes de cabeçalho, com os contadores</td><td><span class="pino ok">sim</span></td></tr>
        <tr><td class="num">4</td><td>gravar o <strong>bloco de esquema outra vez</strong>, byte a byte igual</td><td><span class="pino nao">não</span></td></tr>
        <tr><td class="num">5</td><td>perguntar o <strong>tamanho do arquivo</strong></td><td><span class="pino nao">não</span></td></tr>
      </tbody>
    </table>
  </div>

  <p>O esquema é imutável depois que a tabela nasce. Ele passou a ser serializado
  uma vez, no construtor, com o CRC junto; e o caminho quente ganhou um irmão que
  grava <em>só</em> o cabeçalho. O bloco de esquema e o teste de tamanho ficaram
  onde importam: na criação do volume. <strong>Só o <code>.reg</code>: 6,8 → 5,3
  µs por linha (1,27×); com dois índices, 18,5 → 17,0</strong>. Nenhum byte mudou
  de lugar no disco.</p>

  <div class="nota">
    <span class="t">O pedido pedia outra coisa, e a medição mandou</span>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
