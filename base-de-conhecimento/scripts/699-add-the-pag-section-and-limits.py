# Add the .pag section and limits
# 28/08 19:07

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()
velho='''## 8. Hierarquia: database, schema e tabela'''
novo='''## 8. `.pag` — o descritor de partição

JSON indentado, ao lado dos outros arquivos da tabela. Diz **como a tabela está
partida**, que arquivo guarda o quê, e quanto tem em cada um:

```json
{
  "tabela": "clientes",
  "modo": "letra",
  "coluna_referencia": "nome",
  "registros_por_arquivo": 1000,
  "max_arquivos": 37,
  "endereco": "volume = (rowid - 1) / registros_por_arquivo + 1; …",
  "baldes": [
    { "balde": 1, "letra": "A", "arquivo": "clientes_A.reg",
      "existe": true, "registros": 2, "primeiro_rowid": 1 },
    …
  ]
}
```

Existe para quem está do **lado de fora** — uma camada SQL, um ETL, um
relatório, um `ls` — descobrir isso sem abrir o `.reg` e sem saber ler o bloco
de esquema. A conta do endereço vai escrita por extenso, porque é exatamente o
que quem lê precisa saber para não ter de adivinhar.

**Ele não é fonte de verdade**, e isso é o desenho e não um detalhe. O modo e a
coluna de referência estão no bloco de esquema dentro do `.reg`; quantas linhas
cada balde tem está no cabeçalho de cada volume. O `.pag` é **gerado** a partir
dos dois, na criação e a cada `sincronizar`.

A razão é a mesma que impede gravar «é chave primária» na coluna, e a mesma que
impede um arquivo `sequences` com uma segunda cópia dos contadores: uma segunda
cópia é uma segunda verdade, e as duas divergem no primeiro caminho que
esquecer de atualizar uma delas. Aqui a divergência seria pior que o normal —
o `.pag` diz em que **arquivo** a linha está.

Por isso o motor nunca **lê** este arquivo para decidir nada. Apagar o `.pag`
não quebra a tabela; regravar resolve.

---

## 9. Hierarquia: database, schema e tabela'''
assert velho in s
s=s.replace(velho,novo,1)
for velho_n, novo_n in [("## 9. Reindex","## 10. Reindex"),
                        ("## 10. Identificadores","## 11. Identificadores"),
                        ("## 11. Limites","## 12. Limites"),
                        ("## 12. O que este formato ainda não faz","## 13. O que este formato ainda não faz")]:
    assert velho_n in s, velho_n
    s=s.replace(velho_n, novo_n, 1)

s=s.replace('''| Tamanho de um registro do `.trash` | 4 GiB |''',
'''| Tamanho de um registro do `.trash` | 4 GiB |
| Volumes na partição alfanumérica | 37 (A-Z, 0-9, Outros) — fixo |
| Registros por balde | `registros_por_arquivo`, e é o teto **por letra** |
| Valor máximo de `rownum` | 2⁶⁴ − 1 |''',1)
io.open(p,'w',encoding='utf-8').write(s)
