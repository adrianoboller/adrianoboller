# Fix the four stale citations
# 29/08 18:34

import pathlib
p = pathlib.Path("docs/CASSANDRA.md"); t = p.read_text()
subs = [
("| Inserção local, 2 índices, esquema da bancada (`+Decimal(15,2)`, `+Date`) | **16,61 µs/linha** (§4.8) |",
 "| Inserção local, 2 índices, esquema da bancada (`+Decimal(15,2)`, `+Date`) | **7,92 µs/linha** (§4.8) — o esquema custa ~5% a mais que o simples, não 2,2× |"),

("""do `DESEMPENHO.md` (§4.8): depois do *write-back*, o custo dominante da nossa
inserção passou a ser `montar_payload` + `codificar_chave`, e duas colunas
(`Decimal(15,2)` e `Date`) levam a inserção de 7,50 para 16,61 µs. O Cassandra""",
 """do `DESEMPENHO.md` (§4.8): depois do *write-back*, o custo dominante da nossa
inserção passou a ser `montar_payload` + `codificar_chave`. Duas colunas
(`Decimal(15,2)` e `Date`) custam ~0,4 µs a mais — 7,50 para 7,92 µs, e não os
16,61 que este documento citou antes de o §4.8 derrubar o número. O Cassandra"""),

("""O §4.8 mediu: depois do *write-back*, `.reg` + `.log` viraram **60,8%** do
tempo e os dois índices **29,4%**; e o mesmo código, com o esquema da bancada,
sai de 7,50 para **16,61 µs** por causa de **duas colunas** (`Decimal(15,2)` e
`Date`). O custo dominante é a **codificação da linha**.""",
 """O §4.8 mediu: depois do *write-back*, `.reg` + `.log` viraram **60,8%** do
tempo e os dois índices **29,4%**; o mesmo código com o esquema da bancada sai
de 7,50 para **7,92 µs** — as duas colunas custam ~5%, não 2,2×. (Este
documento chegou a citar 16,61 µs em quatro lugares; era um **binário velho**,
e o §4.8 conta a derrubada.) O custo dominante é a **codificação da linha**."""),

("""inserção local custa **7,50 µs** (esquema simples) ou **16,61 µs** (esquema da
bancada), medidos.""",
 """inserção local custa **7,50 µs** (esquema simples) ou **7,92 µs** (esquema da
bancada), medidos."""),
]
for velho, novo in subs:
    assert velho in t, velho[:70]
    t = t.replace(velho, novo, 1)
p.write_text(t); print("quatro citacoes corrigidas")
