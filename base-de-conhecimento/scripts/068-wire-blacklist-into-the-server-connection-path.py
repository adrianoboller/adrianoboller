# Wire blacklist into the server connection path
# 27/08 19:26

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

s=s.replace('''use crate::acesso::{Acesso, LogAcessos};
use crate::config::Config;''','''use crate::acesso::{Acesso, LogAcessos};
use crate::blacklist::Blacklist;
use crate::config::Config;''')

s=s.replace('''#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
}''','''#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
    /// Desafio em aberto: (usuario, nonce do servidor, quando expira).
    /// Vale uma vez so -- e consumido no login, dando certo ou errado.
    desafio: Option<(String, String, i64)>,
}''')

s=s.replace('''    log: Mutex<LogAcessos>,
    conexoes: AtomicUsize,
}''','''    log: Mutex<LogAcessos>,
    lista_negra: Mutex<Blacklist>,
    conexoes: AtomicUsize,
}''')

s=s.replace('''        let log = LogAcessos::abrir(&config.log_acessos)?;
        Ok(Arc::new(Servidor {
            config,
            dados: Mutex::new(instancia),
            log: Mutex::new(log),
            conexoes: AtomicUsize::new(0),
        }))''','''        let log = LogAcessos::abrir(&config.log_acessos)?;
        let lista_negra = Blacklist::abrir(&config.blacklist)?;
        Ok(Arc::new(Servidor {
            config,
            dados: Mutex::new(instancia),
            log: Mutex::new(log),
            lista_negra: Mutex::new(lista_negra),
            conexoes: AtomicUsize::new(0),
        }))''')

s=s.replace('''        eprintln!("log de acessos: {}", self.config.log_acessos.display());''','''        eprintln!("log de acessos: {}", self.config.log_acessos.display());
        eprintln!("lista de bloqueio: {}", self.config.blacklist.display());
        if !self.config.politica.comandos_proibidos.is_empty() {
            eprintln!(
                "comandos proibidos: {}",
                self.config.politica.comandos_proibidos.join(", ")
            );
        }
        if let Some(fw) = &self.config.politica.firewall {
            eprintln!(
                "firewall: {}",
                if fw.ligado {
                    "ligado -- IP bloqueado vira regra no sistema"
                } else {
                    "desligado -- o bloqueio vale so dentro do servidor"
                }
            );
        }''')

# ---- bloqueio na entrada ----
s=s.replace('''        let permitido = self.config.ip_permitido(&ip);''','''        // Antes de qualquer coisa: quem esta na lista de bloqueio nao entra.
        let agora = crate::agora_ms();
        let bloqueado = {
            let mut lista = match self.lista_negra.lock() {
                Ok(l) => l,
                Err(_) => return,
            };
            let _ = lista.limpar_vencidos(agora, &self.config.politica);
            lista.bloqueado(&ip, agora).map(|b| {
                format!(
                    "bloqueado desde {} ate {} por {} ({})",
                    b.desde(),
                    b.ate(),
                    b.motivo,
                    b.comando
                )
            })
        };
        if let Some(motivo) = bloqueado {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.clone(),
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(motivo.clone()),
            });
            let escrita = fluxo.try_clone();
            if let Ok(mut saida) = escrita {
                let _ = writeln!(saida, "{}", resposta_erro("conexao", &motivo, 0).escrever());
            }
            return;
        }

        let permitido = self.config.ip_permitido(&ip);''')

s=s.replace('''        if !permitido {
            self.anotar(&Acesso {''','''        if !permitido {
            self.violacao_leve(&ip, "conexao", "ip fora da lista de permitidos");
            self.anotar(&Acesso {''')

s=s.replace('''            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao);''','''            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);''')

# ---- registro de violacoes ----
s=s.replace('''    fn anotar(&self, acesso: &Acesso) {''','''    /// Violacao grave: bloqueia na hora e avisa no log.
    fn violacao_grave(&self, ip: &str, comando: &str, motivo: &str) {
        if let Ok(mut lista) = self.lista_negra.lock() {
            let (b, aviso) = lista.violacao_grave(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            );
            eprintln!(
                "BLOQUEADO {ip} ate {} -- {} ({})",
                b.ate(),
                b.motivo,
                b.comando
            );
            if let Some(a) = aviso {
                eprintln!("AVISO: {a}");
            }
        }
    }

    /// Tentativa leve: conta, e bloqueia se passar do limite na janela.
    fn violacao_leve(&self, ip: &str, comando: &str, motivo: &str) {
        if let Ok(mut lista) = self.lista_negra.lock() {
            if let Some((b, aviso)) = lista.tentativa_leve(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            ) {
                eprintln!(
                    "BLOQUEADO {ip} ate {} -- {} apos {} tentativas",
                    b.ate(),
                    b.motivo,
                    b.tentativas
                );
                if let Some(a) = aviso {
                    eprintln!("AVISO: {a}");
                }
            }
        }
    }

    fn anotar(&self, acesso: &Acesso) {''')
open(p,'w').write(s)
print("servidor: bloqueio na entrada e registro de violacoes")
