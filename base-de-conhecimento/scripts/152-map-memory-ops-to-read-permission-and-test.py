# Map memory ops to read permission and test
# 27/08 20:49

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
teste = '''
    #[test]
    fn a_memoria_pede_leitura_e_o_backup_pede_administrar() {
        // Consultar em memoria nao pode exigir mais poder do que ler do disco:
        // e o mesmo dado. Ja o backup e conta de administrador.
        for op in [
            "memoria_carregar",
            "memoria",
            "SelectMemory",
            "selecionar_memoria",
        ] {
            assert_eq!(Atividade::da_operacao(op), Some(Atividade::Ler), "{op}");
        }
        for op in ["backup", "conferir_backup", "memoria_liberar"] {
            assert_eq!(
                Atividade::da_operacao(op),
                Some(Atividade::Administrar),
                "{op}"
            );
        }
        assert_eq!(Atividade::da_operacao("sair"), None);
    }
'''
i = s.rindex('\n}\n')
open(p,'w').write(s[:i] + teste + s[i:])
