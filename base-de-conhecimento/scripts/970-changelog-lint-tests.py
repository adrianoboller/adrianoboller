# CHANGELOG, lint, tests
# 29/08 01:12

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''- **`--example indice-adiado`**'''
novo = '''- **O cabeçalho do `.reg` parou de reserializar o esquema a cada linha.** Toda
  inserção chamava `gravar_cabecalho`, e ele fazia cinco coisas — serializar o
  esquema inteiro, calcular o CRC-32 dele, gravar os 128 bytes de cabeçalho com
  os contadores, gravar o **bloco de esquema outra vez** byte a byte igual, e
  perguntar o tamanho do arquivo. Das cinco, **uma** era necessária.

  O esquema não muda desde que a tabela é criada: passou a ser serializado uma
  vez, no construtor, com o CRC junto; e o caminho quente ganhou um irmão que
  grava só o cabeçalho. O bloco de esquema e o teste de tamanho ficaram onde
  importam, na criação do volume. **Só o `.reg`: 6,8 → 5,3 µs por linha
  (1,27×). Com dois índices: 18,5 → 17,0 µs.** Nenhum byte mudou de lugar no
  disco.

  Achado respondendo a uma pergunta sobre outra coisa — «e se o `.ndx` parasse
  durante a carga?» —, o que é onde essas coisas costumam aparecer.

- **`--example indice-adiado`**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
