# Update CLI usage; check bulk docs
# 28/08 20:00

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
antigo = """    phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>]
        Mostra as linhas. Sem --indice, na ordem de digitacao (direto do
        .reg). Com --indice, na ordem daquele indice. --max 0 mostra tudo."""
novo = """    phxsql listar    <dir> <tabela> [--indice <nome>] [--max <n>] [--pular <n>]
        Mostra as linhas. Sem --indice, na ordem de digitacao (direto do
        .reg). Com --indice, na ordem daquele indice. --max 0 mostra tudo.
        --pular salta para a posicao N; numa tabela intacta o salto e uma
        bisseccao pelo rownum, e o rodape diz por onde foi. O teto entra na
        LEITURA: mostrar vinte linhas de uma tabela de 200.000 le vinte."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
