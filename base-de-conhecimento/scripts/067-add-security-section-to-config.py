# Add security section to config
# 27/08 19:25

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''use crate::usuarios::Cadastro;''','''use crate::blacklist::Politica;
use crate::usuarios::Cadastro;''')
s=s.replace('''    /// Usuarios e o poder de cada um sobre cada base.
    pub cadastro: Cadastro,
}''','''    /// Usuarios e o poder de cada um sobre cada base.
    pub cadastro: Cadastro,
    /// Comandos e bases proibidos, e a politica de bloqueio.
    pub politica: Politica,
    /// Arquivo da lista de bloqueio.
    pub blacklist: PathBuf,
}''')
s=s.replace('''            cadastro: Cadastro::default(),
        }
    }
}''','''            cadastro: Cadastro::default(),
            politica: Politica::default(),
            blacklist: PathBuf::from("blacklist.json"),
        }
    }
}''')
s=s.replace('''            cadastro: Cadastro::de_json(j)?,
        })''','''            cadastro: Cadastro::de_json(j)?,
            politica: match j.campo("seguranca") {
                Some(seg) => Politica::de_json(seg),
                None => Politica::default(),
            },
            blacklist: PathBuf::from(
                j.campo("seguranca")
                    .map(|seg| seg.texto_ou("blacklist", "blacklist.json"))
                    .unwrap_or("blacklist.json"),
            ),
        })''')
s=s.replace('''            if c.log_acessos.is_relative() {
                c.log_acessos = dir.join(&c.log_acessos);
            }''','''            if c.log_acessos.is_relative() {
                c.log_acessos = dir.join(&c.log_acessos);
            }
            if c.blacklist.is_relative() {
                c.blacklist = dir.join(&c.blacklist);
            }''')
s=s.replace('''            (
                "usuarios",''','''            (
                "comandos_proibidos",
                Json::Lista(
                    self.politica
                        .comandos_proibidos
                        .iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ),
            (
                "firewall",
                Json::Bool(
                    self.politica
                        .firewall
                        .as_ref()
                        .map(|f| f.ligado)
                        .unwrap_or(false),
                ),
            ),
            (
                "usuarios",''')
s=s.replace('''    #[test]
    fn lista_de_ips_filtra() {''','''    #[test]
    fn le_a_secao_de_seguranca() {
        let txt = r#"{
          "token":"x",
          "seguranca":{
            "comandos_proibidos":["excluir","reindexar"],
            "bases_proibidas":["financeiro"],
            "tentativas_ate_bloquear":3,
            "janela_minutos":5,
            "bloqueio_minutos":120,
            "blacklist":"bl.json",
            "firewall":{"ligado":true,"bloquear":["/sbin/iptables","-s","{ip}"]}
          }
        }"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.politica.comando_proibido("excluir"));
        assert!(c.politica.comando_proibido("REINDEXAR"));
        assert!(!c.politica.comando_proibido("ler"));
        assert!(c.politica.base_proibida("financeiro"));
        assert_eq!(c.politica.tentativas_ate_bloquear, 3);
        assert_eq!(c.politica.bloqueio_minutos, 120);
        assert!(c.politica.firewall.as_ref().unwrap().ligado);
        assert_eq!(c.blacklist, PathBuf::from("bl.json"));
    }

    #[test]
    fn sem_secao_de_seguranca_nada_e_proibido() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.politica.comando_proibido("excluir"));
        assert!(c.politica.firewall.is_none());
        assert_eq!(c.politica.tentativas_ate_bloquear, 5);
    }

    #[test]
    fn lista_de_ips_filtra() {''')
open(p,'w').write(s)
