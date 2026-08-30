# Update remaining numbers and the CHANGELOG
# 29/08 01:58

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''- **`--example custo-do-log`**, que decompõe o bloco `.reg` + `.log` que este
  documento registrava como não decomposto. O diário custa **1,22 µs por
  evento (7,2% de uma inserção)**, ou 2,24 com a imagem da linha; a reescrita
  do cabeçalho, sozinha, custa 0,41. Responde «dá para guardar o diário em
  memória?»: segurar os eventos compraria 7,2% e trocaria uma garantia
  irreconstruível; parar de reescrever o cabeçalho compra 2,4% sem buffer
  nenhum.'''
novo = '''- **O `.log` deixou de atrasar o `.reg`.** O diário fazia **duas escritas por
  evento**: os 44 bytes do evento, e os 64 do cabeçalho com `fim` e
  `qtd_eventos`. O evento tem de ir na hora; o cabeçalho é um contador, e a
  leitura sabe recalculá-lo varrendo os próprios eventos. Ele passou a ir no
  `sincronizar`: **1,22 → 0,67 µs por evento (1,82×)**, e a inserção completa
  com dois índices de **17,0 para 15,9 µs**.

  **O evento continua indo para o arquivo dentro da inserção** — o que ficou
  para depois foi só o contador. O que isso pediu foi um caminho de reparo:
  uma queda antes do `sincronizar` deixaria o cabeçalho atrasado, e a próxima
  gravação escreveria **por cima** dos eventos já gravados — evento destruído,
  não invisível. Então `abrir` varre para a frente a partir do `fim` gravado,
  validando cada evento pelo CRC que ele já carrega, e para no primeiro que não
  confere. Quatro testes travam isso; o que mais importa é
  `depois_da_cura_o_novo_evento_nao_sobrescreve`.

  **Segurar os eventos em RAM continua fora**, e a razão não é de tamanho (4,2%)
  e sim de natureza: índice perdido se reconstrói do `.reg`; evento perdido não
  se reconstrói — ele é a história e é a posição de que a replicação depende.

- **`--example custo-do-log`**, que decompôs o bloco `.reg` + `.log` que este
  documento registrava como não decomposto — e foi ele que apontou onde estava
  a escrita de sobra.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
