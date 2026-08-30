# Add Nivel tests
# 27/08 21:10

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
teste='''
    #[test]
    fn cada_nivel_contem_o_anterior() {
        let leitor = Nivel::Leitor.permissoes();
        let operador = Nivel::Operador.permissoes();
        let dono = Nivel::Dono.permissoes();
        let admin = Nivel::Admin.permissoes();

        for a in Atividade::TODAS {
            if leitor.pode(a) {
                assert!(operador.pode(a), "operador perdeu {}", a.nome());
            }
            if operador.pode(a) {
                assert!(dono.pode(a), "dono perdeu {}", a.nome());
            }
            if dono.pode(a) {
                assert!(admin.pode(a), "admin perdeu {}", a.nome());
            }
        }

        // E cada um acrescenta alguma coisa de verdade.
        assert!(!leitor.inserir && operador.inserir);
        assert!(!operador.criar && dono.criar);
        assert!(!dono.administrar && admin.administrar);
        // Leitor le, e so.
        assert!(leitor.ler && leitor.diario && leitor.verificar);
        assert!(!leitor.excluir && !leitor.reindexar && !leitor.replicar);
    }

    #[test]
    fn o_nivel_vale_onde_nao_ha_regra_de_base() {
        let txt = format!(
            r#"{{"usuarios":[{{
                 "login":"ana","senha_hash":"{}","nivel":"operador",
                 "bases":{{"Financeiro":{{}}}}
               }}]}}"#,
            senha::gerar_hash("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let ana = c.por_login("ana").unwrap();
        assert_eq!(ana.nivel, Nivel::Operador);
        // Onde nao ha regra, vale o nivel.
        assert!(ana.pode("Comercial", Atividade::Inserir));
        assert!(ana.pode("Comercial", Atividade::Ler));
        assert!(!ana.pode("Comercial", Atividade::Criar));
        // Onde HA regra, a regra manda -- mesmo para tirar poder.
        assert!(!ana.pode("Financeiro", Atividade::Ler));
        assert!(!ana.pode("Financeiro", Atividade::Inserir));
    }

    #[test]
    fn sem_nivel_o_padrao_e_o_menor() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ze","senha_hash":"{}"}}]}}"#,
            senha::gerar_hash("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let ze = c.por_login("ze").unwrap();
        assert_eq!(ze.nivel, Nivel::Leitor);
        assert!(!ze.e_admin());
        assert!(ze.pode("Qualquer", Atividade::Ler));
        assert!(!ze.pode("Qualquer", Atividade::Excluir));
    }

    #[test]
    fn nivel_admin_e_supervisor_dizem_a_mesma_coisa() {
        let txt = format!(
            r#"{{"usuarios":[
                 {{"login":"a","senha_hash":"{h}","nivel":"admin"}},
                 {{"login":"b","senha_hash":"{h}","supervisor":true}}
               ]}}"#,
            h = senha::gerar_hash("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let a = c.por_login("a").unwrap();
        let b = c.por_login("b").unwrap();
        assert!(a.e_admin() && b.e_admin());
        // A ficha do supervisor nao pode dizer "leitor" de quem pode tudo.
        assert_eq!(b.nivel, Nivel::Admin);
        for at in Atividade::TODAS {
            assert_eq!(
                a.pode("Comercial", at),
                b.pode("Comercial", at),
                "divergiram em {}",
                at.nome()
            );
        }
    }

    #[test]
    fn nivel_desconhecido_nao_sobe() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"x","senha_hash":"{}","nivel":"chefao"}}]}}"#,
            senha::gerar_hash("x")
        );
        assert!(Cadastro::de_json(&Json::analisar(&txt).unwrap()).is_err());
    }

    #[test]
    fn os_apelidos_de_nivel_valem() {
        assert_eq!(Nivel::de_texto("ADMIN").unwrap(), Nivel::Admin);
        assert_eq!(Nivel::de_texto(" dba ").unwrap(), Nivel::Admin);
        assert_eq!(Nivel::de_texto("consulta").unwrap(), Nivel::Leitor);
        assert_eq!(Nivel::de_texto("").unwrap(), Nivel::Leitor);
        assert_eq!(Nivel::de_texto("owner").unwrap(), Nivel::Dono);
    }
'''
i=s.rindex('\n}\n')
open(p,'w').write(s[:i]+teste+s[i:])
