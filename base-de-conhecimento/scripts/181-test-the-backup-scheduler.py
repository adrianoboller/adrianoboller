# Test the backup scheduler
# 27/08 21:16

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
teste='''
    #[test]
    fn o_backup_vem_desligado() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.backup.agendado);
        assert!(c.backup.zip, "zip e o padrao quando ligarem");
        assert_eq!(c.backup.manter, 14);
        assert!(!c.backup.hora_de_rodar(1_000_000, 0), "desligado nunca roda");
    }

    #[test]
    fn hora_marcada_dispara_uma_vez_por_dia() {
        let txt = r#"{"token":"x","backup":{"agendado":true,"hora":"03:00"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let dia = 20_000i64 * 86_400_000;

        // 02:59 ainda nao.
        assert!(!c.backup.hora_de_rodar(dia + 2 * 3_600_000 + 59 * 60_000, 0));
        // 03:00 sim, porque nunca rodou.
        let as_tres = dia + 3 * 3_600_000;
        assert!(c.backup.hora_de_rodar(as_tres, 0));
        // 03:01, ja tendo rodado as 03:00: NAO de novo.
        assert!(!c.backup.hora_de_rodar(as_tres + 60_000, as_tres));
        // 23:59 do mesmo dia: ainda nao.
        assert!(!c.backup.hora_de_rodar(dia + 86_340_000, as_tres));
        // 03:00 do dia seguinte: sim.
        assert!(c.backup.hora_de_rodar(as_tres + 86_400_000, as_tres));
    }

    #[test]
    fn sem_hora_marcada_vale_o_intervalo() {
        let txt = r#"{"token":"x","backup":{"agendado":true,"cada_horas":6}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.backup.hora_de_rodar(1_000_000_000, 0), "nunca rodou, roda");
        let t = 1_000_000_000i64;
        assert!(!c.backup.hora_de_rodar(t + 5 * 3_600_000, t));
        assert!(c.backup.hora_de_rodar(t + 6 * 3_600_000, t));
    }

    #[test]
    fn hora_invalida_nao_sobe() {
        for h in ["25:00", "12:60", "meia-noite", "3", "03;00"] {
            let txt = format!(r#"{{"token":"x","backup":{{"agendado":true,"hora":"{h}"}}}}"#);
            assert!(
                Config::de_json(&Json::analisar(&txt).unwrap()).is_err(),
                "{h:?} passou"
            );
        }
        assert_eq!(Backup::minuto_do_dia("03:00"), Some(180));
        assert_eq!(Backup::minuto_do_dia("23:59"), Some(1439));
        assert_eq!(Backup::minuto_do_dia("00:00"), Some(0));
    }
'''
i=s.rindex('\n}\n')
open(p,'w').write(s[:i]+teste+s[i:])
