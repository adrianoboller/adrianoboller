# Test the hostile-name predicate
# 27/08 20:18

p='crates/phxsql-store/src/catalogo.rs'
s=open(p).read()
teste = '''
    #[test]
    fn separa_nome_ruim_de_tentativa_de_travessia() {
        // Engano de digitacao: recusado, mas nao e ataque.
        assert!(!nome_hostil("minha tabela!"));
        assert!(!nome_hostil("cadastro*"));
        assert!(!nome_hostil("aspas\\"aqui"));
        assert!(!nome_hostil("cadastroClientes"));
        assert!(!nome_hostil("Comercial"));
        assert!(!nome_hostil("nota.fiscal"));

        // Sondagem: ninguem digita isso por acidente.
        assert!(nome_hostil(".."));
        assert!(nome_hostil("."));
        assert!(nome_hostil("../../etc/passwd"));
        assert!(nome_hostil("..\\\\..\\\\windows"));
        assert!(nome_hostil("/etc"));
        assert!(nome_hostil("C:\\\\dados"));
        assert!(nome_hostil("a/b"));
        assert!(nome_hostil("nome\\u{0}nulo"));
        assert!(nome_hostil("quebra\\nlinha"));
        // Sem barra, mas ainda saindo do lugar.
        assert!(nome_hostil("tabela..oculta"));
    }

    #[test]
    fn tudo_que_e_hostil_tambem_e_invalido() {
        // O contrario nao vale, e e essa a assimetria que interessa.
        for n in ["..", "/etc", "a/b", "C:\\\\x", "quebra\\nlinha"] {
            assert!(nome_hostil(n));
            assert!(validar_nome("tabela", n).is_err(), "{n:?} deveria ser invalido");
        }
    }
'''
i = s.rindex('\n}\n')
open(p,'w').write(s[:i] + teste + s[i:])
