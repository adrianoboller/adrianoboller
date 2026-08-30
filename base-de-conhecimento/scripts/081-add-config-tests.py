# Add config tests
# 27/08 19:43

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
teste = '''
    #[test]
    fn a_interface_web_vem_desligada_e_presa_ao_proprio_computador() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.web.ligado);
        assert_eq!(c.web.bind, "127.0.0.1:5001");
        assert_eq!(c.web.sessao_minutos, 60);
        assert_eq!(c.web.sessao_ms(), 3_600_000);
    }

    #[test]
    fn le_a_secao_web() {
        let txt = r#"{"token":"x","web":{"ligado":true,"bind":"0.0.0.0:8080","sessao_minutos":15}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.ligado);
        assert_eq!(c.web.bind, "0.0.0.0:8080");
        assert_eq!(c.web.sessao_ms(), 900_000);
        c.validar().unwrap();
    }

    #[test]
    fn a_web_nao_pode_roubar_a_porta_de_dados() {
        let txt = r#"{"token":"x","bind":"127.0.0.1:5000","web":{"ligado":true,"bind":"127.0.0.1:5000"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.validar().is_err());
    }

    #[test]
    fn web_desligada_nao_valida_o_endereco() {
        // Um bind ruim numa interface desligada nao impede o servidor de subir.
        let txt = r#"{"token":"x","web":{"bind":"isso nao e endereco"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        c.validar().unwrap();
    }
'''
i = s.rindex('\n}\n')  # fim do mod tests
s = s[:i] + teste + s[i:]
open(p,'w').write(s)
print("testes ok")
