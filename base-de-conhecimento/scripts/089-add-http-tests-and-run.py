# Add http tests and run
# 27/08 19:47

p='/home/user/adrianoboller/phxsql/crates/phxsql-server/src/http.rs'
s=open(p).read()
teste = '''
    #[test]
    fn a_pagina_servida_tem_esqueleto_e_o_fragmento_nao() {
        assert!(
            !PAGINA.to_lowercase().contains("<!doctype"),
            "o fragmento nao pode trazer esqueleto: ele e publicado como artefato"
        );
        let inteira = montar_pagina();
        assert!(inteira.starts_with("<!doctype html>"));
        assert!(inteira.contains("<html lang=\\"pt-BR\\">"));
        assert!(inteira.contains("<meta charset=\\"utf-8\\">"));
        assert!(inteira.contains(PAGINA), "o fragmento tem de entrar inteiro");
        assert!(inteira.trim_end().ends_with("</html>"));
    }

    #[test]
    fn so_o_html_pode_buscar_a_fonte_da_marca() {
        let pagina = montar_resposta(200, "text/html; charset=utf-8", "x");
        assert!(pagina.contains("https://fonts.googleapis.com"));
        assert!(pagina.contains("font-src https://fonts.gstatic.com"));

        let dados = montar_resposta(200, "application/json; charset=utf-8", "{}");
        assert!(
            !dados.contains("fonts.g"),
            "resposta de dados nao abre excecao para host nenhum"
        );
        assert!(dados.contains("default-src 'none'"));
    }

    #[test]
    fn desafio_atravessa_dois_pedidos_e_vale_uma_vez_so() {
        let mut s = Sessoes::default();
        let id = s.nova("", HORA, T0);
        assert!(s.tomar_desafio(&id).is_none());
        s.guardar_desafio(&id, ("adriano".into(), "nonce123".into(), T0 + 30_000));
        let d = s.tomar_desafio(&id).expect("o desafio deveria estar guardado");
        assert_eq!(d.0, "adriano");
        assert_eq!(d.1, "nonce123");
        assert!(s.tomar_desafio(&id).is_none(), "vale uma vez so");
    }

    #[test]
    fn a_sessao_anonima_ganha_nome_no_login() {
        let mut s = Sessoes::default();
        let id = s.nova("", HORA, T0);
        assert_eq!(s.usar(&id, HORA, T0).as_deref(), Some(""));
        assert!(s.definir_login(&id, "adriano"));
        assert_eq!(s.usar(&id, HORA, T0).as_deref(), Some("adriano"));
        assert!(!s.definir_login("sessao-que-nao-existe", "invasor"));
    }
'''
i = s.rindex('\n}\n')
s = s[:i] + teste + s[i:]
open(p,'w').write(s)
