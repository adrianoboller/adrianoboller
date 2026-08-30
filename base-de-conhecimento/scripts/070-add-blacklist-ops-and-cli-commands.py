# Add blacklist ops and CLI commands
# 27/08 19:27

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
s=s.replace('''            "ping" | "login" | "quem_sou" => return None,''','''            "ping" | "login" | "desafio" | "quem_sou" => return None,''')
s=s.replace('''            "acessos" | "ips" | "config" | "usuarios" => Atividade::Administrar,''','''            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {
                Atividade::Administrar
            }''')
s=s.replace('''        assert_eq!(Atividade::da_operacao("ips"), Some(Atividade::Administrar));''','''        assert_eq!(Atividade::da_operacao("ips"), Some(Atividade::Administrar));
        assert_eq!(Atividade::da_operacao("desafio"), None);
        assert_eq!(
            Atividade::da_operacao("bloqueios"),
            Some(Atividade::Administrar)
        );
        assert_eq!(
            Atividade::da_operacao("desbloquear"),
            Some(Atividade::Administrar)
        );''')
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''            "ips" => self.op_ips(),''','''            "ips" => self.op_ips(),
            "bloqueios" => self.op_bloqueios(),
            "desbloquear" => self.op_desbloquear(p),''')
s=s.replace('''    fn op_bancos(&self) -> Result<Json> {''','''    fn op_bloqueios(&self) -> Result<Json> {
        let lista = self.lista_negra.lock().map_err(|_| trava_envenenada())?;
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            (
                "arquivo",
                Json::texto_de(lista.caminho().display().to_string()),
            ),
            (
                "ativos",
                Json::Lista(
                    lista
                        .ativos(agora)
                        .into_iter()
                        .map(|b| b.para_json())
                        .collect(),
                ),
            ),
        ]))
    }

    fn op_desbloquear(&self, p: &Json) -> Result<Json> {
        let ip = p.texto_ou("ip", "").trim().to_string();
        if ip.is_empty() {
            return Err(PhxError::Esquema("informe \\"ip\\"".into()));
        }
        let mut lista = self.lista_negra.lock().map_err(|_| trava_envenenada())?;
        let tinha = lista.desbloquear(&ip, &self.config.politica)?;
        Ok(Json::objeto(vec![
            ("ip", Json::texto_de(&ip)),
            ("estava_bloqueado", Json::Bool(tinha)),
        ]))
    }

    fn op_bancos(&self) -> Result<Json> {''')
open(p,'w').write(s)

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''  phxsqld --usuarios [--config <c>] lista o cadastro e o poder de cada um''','''  phxsqld --usuarios [--config <c>] lista o cadastro e o poder de cada um
  phxsqld --bloqueios [--config <c>]      lista os IPs bloqueados
  phxsqld --desbloquear <ip> [--config c] tira um IP da lista de bloqueio''')
s=s.replace('''//! phxsqld --usuarios [--config c]  lista o cadastro e o poder de cada um
//! ```''','''//! phxsqld --usuarios [--config c]  lista o cadastro e o poder de cada um
//! phxsqld --bloqueios              lista os IPs bloqueados
//! phxsqld --desbloquear <ip>       tira um IP da lista
//! ```''')
s=s.replace('''    if args.iter().any(|a| a == "--usuarios") {''','''    if let Some(i) = args.iter().position(|a| a == "--desbloquear") {
        let ip = match args.get(i + 1).filter(|a| !a.starts_with("--")) {
            Some(a) => a.clone(),
            None => {
                eprintln!("informe o IP: phxsqld --desbloquear 203.0.113.9");
                return ExitCode::FAILURE;
            }
        };
        return match phxsql_server::Blacklist::abrir(&config.blacklist)
            .and_then(|mut bl| bl.desbloquear(&ip, &config.politica))
        {
            Ok(true) => {
                println!("{ip} desbloqueado");
                ExitCode::SUCCESS
            }
            Ok(false) => {
                println!("{ip} nao estava na lista");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if args.iter().any(|a| a == "--bloqueios") {
        let bl = match phxsql_server::Blacklist::abrir(&config.blacklist) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("erro ao ler a lista: {e}");
                return ExitCode::FAILURE;
            }
        };
        let agora = phxsql_server::agora_ms();
        let ativos = bl.ativos(agora);
        if ativos.is_empty() {
            println!("nenhum IP bloqueado em {}", bl.caminho().display());
            return ExitCode::SUCCESS;
        }
        println!(
            "{:<40} {:<23} {:<23} {:>5}  {:<8}  motivo",
            "ip", "desde", "ate", "tent", "firewall"
        );
        for b in ativos {
            println!(
                "{:<40} {:<23} {:<23} {:>5}  {:<8}  {} ({})",
                b.ip,
                b.desde(),
                b.ate(),
                b.tentativas,
                if b.firewall { "sim" } else { "nao" },
                b.motivo,
                b.comando
            );
        }
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--usuarios") {''')
open(p,'w').write(s)
