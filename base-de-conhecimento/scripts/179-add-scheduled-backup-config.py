# Add scheduled backup config
# 27/08 21:15

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
bloco = '''/// Backup agendado.
///
/// Vem desligado. Backup que roda sozinho num destino que ninguem conferiu e
/// backup que enche o disco e para -- ligar e uma decisao, com um destino
/// escolhido de proposito.
#[derive(Debug, Clone)]
pub struct Backup {
    pub agendado: bool,
    /// Pasta onde os arquivos caem.
    pub destino: PathBuf,
    /// Hora do dia, "HH:MM". Vazia = usa `cada_horas`.
    pub hora: String,
    /// Intervalo em horas, quando nao ha hora marcada.
    pub cada_horas: u64,
    /// Um ZIP unico (padrao) ou a arvore de diretorios.
    pub zip: bool,
    /// Qual database copiar. Vazio = todos.
    pub database: String,
    /// Nome que entra no arquivo, no lugar do usuario.
    pub admin: String,
    /// Quantos arquivos guardar. Zero = nao apaga nada.
    pub manter: usize,
}

impl Default for Backup {
    fn default() -> Self {
        Backup {
            agendado: false,
            destino: PathBuf::from("backups"),
            hora: String::new(),
            cada_horas: 24,
            zip: true,
            database: String::new(),
            admin: "agendado".into(),
            manter: 14,
        }
    }
}

impl Backup {
    fn de_json(j: &Json) -> Result<Backup> {
        let padrao = Backup::default();
        let Some(b) = j.campo("backup") else {
            return Ok(padrao);
        };
        let hora = b.texto_ou("hora", "").trim().to_string();
        if !hora.is_empty() && Backup::minuto_do_dia(&hora).is_none() {
            return Err(PhxError::Esquema(format!(
                "backup.hora invalida: {hora:?} (use \\"HH:MM\\", 24 horas)"
            )));
        }
        Ok(Backup {
            agendado: b.booleano_ou("agendado", false),
            destino: PathBuf::from(b.texto_ou("destino", "backups")),
            hora,
            cada_horas: b.inteiro_ou("cada_horas", padrao.cada_horas as i64).max(1) as u64,
            zip: b.booleano_ou("zip", true),
            database: b.texto_ou("database", "").trim().to_string(),
            admin: b.texto_ou("admin", "agendado").trim().to_string(),
            manter: b.inteiro_ou("manter", padrao.manter as i64).max(0) as usize,
        })
    }

    /// "HH:MM" em minutos desde a meia-noite. `None` se nao for hora.
    pub fn minuto_do_dia(hora: &str) -> Option<u64> {
        let (h, m) = hora.split_once(':')?;
        let h: u64 = h.trim().parse().ok()?;
        let m: u64 = m.trim().parse().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        Some(h * 60 + m)
    }

    /// Ja passou da hora de rodar de novo?
    ///
    /// `ultimo_ms` e zero quando nunca rodou. Com hora marcada, dispara quando
    /// o minuto do dia chega e ainda nao rodou hoje -- e nao a cada minuto
    /// depois disso.
    pub fn hora_de_rodar(&self, agora_ms: i64, ultimo_ms: i64) -> bool {
        if !self.agendado {
            return false;
        }
        match Backup::minuto_do_dia(&self.hora) {
            Some(alvo) => {
                let minuto_agora = (agora_ms.rem_euclid(86_400_000) / 60_000) as u64;
                let dia_agora = agora_ms.div_euclid(86_400_000);
                let dia_ultimo = ultimo_ms.div_euclid(86_400_000);
                minuto_agora >= alvo && (ultimo_ms == 0 || dia_agora > dia_ultimo)
            }
            None => {
                let intervalo = self.cada_horas as i64 * 3_600_000;
                ultimo_ms == 0 || agora_ms - ultimo_ms >= intervalo
            }
        }
    }
}

'''
s=s.replace('/// Interface web: um servidor HTTP separado', bloco + '/// Interface web: um servidor HTTP separado')
s=s.replace('''    /// Interface web.
    pub web: Web,
}''','''    /// Interface web.
    pub web: Web,
    /// Backup agendado.
    pub backup: Backup,
}''')
s=s.replace('''            web: Web::default(),
        }''','''            web: Web::default(),
            backup: Backup::default(),
        }''')
s=s.replace('''            web: Web::de_json(j),
        })''','''            web: Web::de_json(j),
            backup: Backup::de_json(j)?,
        })''')
# caminho relativo do backup vale a partir do config.json
s=s.replace('''            if c.blacklist.is_relative() {
                c.blacklist = dir.join(&c.blacklist);
            }''','''            if c.blacklist.is_relative() {
                c.blacklist = dir.join(&c.blacklist);
            }
            if c.backup.destino.is_relative() {
                c.backup.destino = dir.join(&c.backup.destino);
            }''')
open(p,'w').write(s)
