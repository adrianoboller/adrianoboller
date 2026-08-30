# Add Nivel tests
# 27/08 21:10

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
s=s.replace('''                // O root e supervisor por definicao, diga o que disser o arquivo.
                u.supervisor = true;
                u.ativo = true;''','''                // O root e supervisor por definicao, diga o que disser o arquivo.
                u.supervisor = true;
                u.nivel = Nivel::Admin;
                u.ativo = true;''')
# supervisor implica admin, para a ficha nao mentir
s=s.replace('''        let nivel = Nivel::de_texto(j.texto_ou("nivel", ""))?;''',
'''        // supervisor e um admin de todas as bases -- e a forma antiga de
        // dizer a mesma coisa. Mantida, e agora ela ACERTA o nivel, para a
        // ficha nao dizer "leitor" de quem pode tudo.
        let supervisor = j.booleano_ou("supervisor", false);
        let nivel = if supervisor {
            Nivel::Admin
        } else {
            Nivel::de_texto(j.texto_ou("nivel", ""))?
        };''')
s=s.replace('''            supervisor: j.booleano_ou("supervisor", false),
            ativo: j.booleano_ou("ativo", true),
            nivel,''','''            supervisor,
            ativo: j.booleano_ou("ativo", true),
            nivel,''')
open(p,'w').write(s)
