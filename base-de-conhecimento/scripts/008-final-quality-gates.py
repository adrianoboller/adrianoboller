# Final quality gates
# 27/08 18:00

p='README.md'
s=open(p).read()
s=s.replace("Na biblioteca:","Na biblioteca (este trecho e o `crates/phxsql-store/examples/basico.rs`,\nque compila e roda com `cargo run --example basico`):")
s=s.replace("""  phxsql-cli/      a ferramenta de linha de comando
docs/
  FORMATO.md       especificação byte a byte dos quatro arquivos
```""","""  phxsql-cli/      a ferramenta de linha de comando
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos quatro arquivos
```""")
open(p,'w').write(s)
