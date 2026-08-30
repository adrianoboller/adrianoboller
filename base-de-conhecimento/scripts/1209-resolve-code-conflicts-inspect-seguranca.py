# Resolve code conflicts; inspect SEGURANCA
# 29/08 21:03

import io
M=">>>>>>> worktree-agent-a7b2760cf4c033d58"

# config.rs: os dois lados acrescentaram campos -- entram os dois
p="phxsql/crates/phxsql-server/src/config.rs"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
novo = """    (
        "cifra",
        &[
            "ligada",
            "senha",
            "senha_env",
            "iteracoes",
            "modo",
            "salto",
            "separador",
        ],
    ),
    ("lgpd", &["alteracoes", "acessos"]),
"""
s=s[:a]+novo+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)

# index.html: fica o texto DELES (que explica a cifra), com a frase de onde se
# marca corrigida -- a op de marcar e a coluna LGPD na Estrutura ja existem
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
deles=s[b+len("\n=======\n"):c]
velho = """       <p>Esta tela <b>lê</b> a marca. Marcar e desmarcar é no cadastro de
       campos da tabela, junto com <code>caption</code>,
       <code>descricao</code> e <code>mascara</code>.</p>
"""
nova = """       <p>Esta tela <b>lê</b> a marca, e quem varre é o servidor — a op
       <code>dados_pessoais</code>, que confere o direito tabela a tabela por
       dentro. Marcar e desmarcar é na aba <b>Estrutura</b> da tabela, na
       coluna LGPD; clique numa linha acima para ir até lá.</p>
"""
assert deles.count(velho)==1
deles=deles.replace(velho,nova)
s=s[:a]+deles+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("config.rs e index.html resolvidos")
