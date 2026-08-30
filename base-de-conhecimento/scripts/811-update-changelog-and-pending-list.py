# Update changelog and pending list
# 28/08 20:35

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
antigo = """| ◐ | 19 | **Replicação como a do MySQL(R)**, com porta de envio e de retorno | as três portas entram e validam, o desenho está escrito; falta o **`.log` v2 com imagem da linha** |"""
novo = """| ☑️ | 19 | **Replicação como a do MySQL(R)**, com porta de envio e de retorno | **funcionando**: `.log` v2 com a imagem da linha, ops `posicao`/`replicar`/`aplicar`, e o laço da réplica dentro do `phxsqld`. Medido com quatro servidores — master 18.773 linhas/s, réplica 4.273 eventos/s, atraso de 1,3 a 2,1 s, retrato SHA-256 das quatro tabelas idêntico. Falta long-poll, espera crescente e TLS |"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """| ☑️ | 67 | **Botão e menu Tabelas**"""
novo = """| ☑️ | 110 | **Teste de replicação com três servidores espelho** | `bancada/replicacao/`: `montar.py` sobe Master + Slave01/02/03, `medir.py` mede atraso por tipo de escrita, vazão, queda e retomada. Compara um SHA-256 de **cada linha**, com o rowid junto — e não a contagem, que não acharia uma linha que atravessou errada. Cascata Master → Slave01 → Slave03 também medida |
| ◐ | 111 | **A réplica acompanhar a escrita do master** | não acompanha: 4.273 eventos/s contra 18.773 linhas/s. Aplicar decodifica a imagem para `Value` e **reencoda** o payload, em vez de gravar os bytes que vieram. Gravar direto, remendando só os ponteiros dos anexos, é o próximo ganho grande |
| ☑️ | 67 | **Botão e menu Tabelas**"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
