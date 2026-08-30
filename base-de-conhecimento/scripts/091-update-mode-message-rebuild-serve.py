# Update mode message, rebuild, serve
# 27/08 19:50

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
velho = '''    $("#modo").innerHTML = "Servidor encontrado nesta origem. Informe o token do "
      + "<code>config.json</code> e as suas credenciais.";'''
novo = '''    $("#modo").innerHTML = "Servidor encontrado nesta origem. Informe o token do "
      + "<code>config.json</code> e as suas credenciais.<br>"
      + (podeProvar()
          ? "Login por <b>desafio&#8209;resposta</b>: a senha não sai desta máquina."
          : "Contexto não seguro: o login cai em <b>Base64</b>, que esconde a senha "
            + "de quem olha, não de quem captura. Use HTTPS ou um túnel.");'''
assert s.count(velho)==1
s = s.replace(velho, novo)
open(p,'w').write(s)
