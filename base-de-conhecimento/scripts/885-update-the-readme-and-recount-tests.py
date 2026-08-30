# Update the README and recount tests
# 28/08 23:10

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
antigo = """## Replicação: Master e espelhos"""
novo = """## Profiler: o que chega, antes de virar dado

O ponto de captura é uma linha depois do `read_line` e uma antes do despacho —
**nada foi gravado ainda**. Por isso o pedido que *trava* aparece na lista como
«em curso», que é justamente o que se quer achar.

```json
{"op":"profiler_ligar","database":"Comercial","so_escrita":true,
 "arquivo":"/var/log/phxsql-monitor.txt"}
```

Filtra por banco, usuário, operação e só-escrita; grava num `.txt` no caminho
escolhido. **A senha não passa por aqui**: o pedido é *analisado* e os campos
sensíveis viram `"***"` antes de encostar na memória ou no arquivo — nunca
recortado, porque recortar depende de o pedido estar escrito de um jeito.

## Rodar em contêiner

```bash
docker build -t phxsql .
docker compose up -d     # um master e duas réplicas, em portas diferentes
```

A imagem final é **`scratch`** — sem shell, sem gerenciador de pacotes, só o
binário. Só é possível porque não há dependência externa nenhuma: com o alvo
musl o servidor sai `static-pie` com **3,4 MB**.

## Replicação: Master e espelhos"""
assert antigo in s
s = s.replace(antigo, novo)
s = s.replace("""**370 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 207), **574 no projeto inteiro**""",
"""**374 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 211), **587 no projeto inteiro**""")
p.write_text(s)
