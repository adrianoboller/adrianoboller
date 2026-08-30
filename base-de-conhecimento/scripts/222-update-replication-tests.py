# Update replication tests
# 27/08 21:49

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''        let txt = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","escuta":"0.0.0.0:5010"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.escuta, "0.0.0.0:5010");
        assert_eq!(c.replicacao.endereco().unwrap().port(), 5010);
        c.validar().unwrap();''','''        // Nome novo: envio e retorno separados.
        let txt = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","envio":"0.0.0.0:5010","retorno":"0.0.0.0:5011"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.endereco_envio().unwrap().port(), 5010);
        assert_eq!(c.replicacao.endereco_retorno().unwrap().port(), 5011);
        assert_eq!(c.replicacao.portas().len(), 2);
        c.validar().unwrap();

        // Nome antigo "escuta" continua valendo como envio: config que ja
        // existe nao pode parar de subir so porque o campo foi renomeado.
        let velho = r#"{"token":"x","bind":"0.0.0.0:5000",
          "replicacao":{"papel":"source","escuta":"0.0.0.0:5010"}}"#;
        let c = Config::de_json(&Json::analisar(velho).unwrap()).unwrap();
        assert_eq!(c.replicacao.envio, "0.0.0.0:5010");
        assert!(c.replicacao.retorno.is_empty(), "sem retorno = volta pelo envio");
        c.validar().unwrap();''')
s=s.replace('''        let mesma = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","escuta":"127.0.0.1:5000"}}"#;''',
'''        let mesma = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","envio":"127.0.0.1:5000"}}"#;''')
s=s.replace('''        let contra_web = r#"{"token":"x","bind":"127.0.0.1:5000",
          "web":{"ligado":true,"bind":"127.0.0.1:5001"},
          "replicacao":{"papel":"source","escuta":"127.0.0.1:5001"}}"#;
        assert!(Config::de_json(&Json::analisar(contra_web).unwrap())
            .unwrap()
            .validar()
            .is_err());''','''        let contra_web = r#"{"token":"x","bind":"127.0.0.1:5000",
          "web":{"ligado":true,"bind":"127.0.0.1:5001"},
          "replicacao":{"papel":"source","envio":"127.0.0.1:5001"}}"#;
        assert!(Config::de_json(&Json::analisar(contra_web).unwrap())
            .unwrap()
            .validar()
            .is_err());

        // E o envio contra o proprio retorno.
        let uma_contra_outra = r#"{"token":"x","bind":"127.0.0.1:5000",
          "replicacao":{"papel":"source","envio":"127.0.0.1:5010","retorno":"127.0.0.1:5010"}}"#;
        assert!(Config::de_json(&Json::analisar(uma_contra_outra).unwrap())
            .unwrap()
            .validar()
            .is_err());''')
s=s.replace('''        assert!(c.replicacao.escuta.is_empty());''',
'''        assert!(c.replicacao.envio.is_empty());
        assert!(c.replicacao.retorno.is_empty());
        assert!(c.replicacao.portas().is_empty());''')
open(p,'w').write(s)
