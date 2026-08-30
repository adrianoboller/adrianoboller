# Add replication config tests and honest startup notice
# 27/08 20:56

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
teste='''
    #[test]
    fn le_a_porta_de_replicacao() {
        let txt = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","escuta":"0.0.0.0:5010"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.escuta, "0.0.0.0:5010");
        assert_eq!(c.replicacao.endereco().unwrap().port(), 5010);
        c.validar().unwrap();
    }

    #[test]
    fn a_replicacao_nao_pode_roubar_a_porta_de_dados_nem_a_da_web() {
        let mesma = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","escuta":"127.0.0.1:5000"}}"#;
        assert!(Config::de_json(&Json::analisar(mesma).unwrap())
            .unwrap()
            .validar()
            .is_err());

        let contra_web = r#"{"token":"x","bind":"127.0.0.1:5000",
          "web":{"ligado":true,"bind":"127.0.0.1:5001"},
          "replicacao":{"papel":"source","escuta":"127.0.0.1:5001"}}"#;
        assert!(Config::de_json(&Json::analisar(contra_web).unwrap())
            .unwrap()
            .validar()
            .is_err());
    }

    #[test]
    fn sem_escuta_a_replicacao_usa_a_porta_de_dados() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(c.replicacao.escuta.is_empty());
        c.validar().unwrap();
    }

    #[test]
    fn a_lista_de_servidores_da_web_e_exata() {
        let txt = r#"{"token":"x","web":{"servidores":["10.1.1.5:5000","curitiba:5000"]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.alcanca_outro_servidor());
        assert!(c.web.servidor_permitido("10.1.1.5:5000"));
        assert!(c.web.servidor_permitido(" curitiba:5000 "));
        // Sem porta, com outra porta, ou vazio: nao entra.
        assert!(!c.web.servidor_permitido("10.1.1.5"));
        assert!(!c.web.servidor_permitido("10.1.1.5:5001"));
        assert!(!c.web.servidor_permitido(""));

        let fechado = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!fechado.web.alcanca_outro_servidor());
        assert!(!fechado.web.servidor_permitido("qualquer:5000"));
    }
'''
i=s.rindex('\n}\n')
open(p,'w').write(s[:i]+teste+s[i:])
