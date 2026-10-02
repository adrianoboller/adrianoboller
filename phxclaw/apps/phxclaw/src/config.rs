//! `phxclaw config mostrar|validar|exemplo|definir|perfil|sincronizar`: a porta do
//! terminal para o `config.json`. Chama as MESMAS `vista`, `definir` e `perfil` da rota
//! `/v1/config`, e a mesma `sincronizar` da ponte.

use anyhow::{Result, bail};
use phxclaw_agent::config::catalogo_do_config::{carga, gerar, por_chave};
use phxclaw_agent::config::{self as cfg, AcaoDePerfil, Escopo};
use serde_json::{Map, Value};

const USO: &str = "uso: phxclaw config mostrar [--json] | validar ARQ | exemplo | \
                   definir CHAVE VALOR|--remover [--projeto|--perfil NOME] | \
                   perfil listar|usar NOME|nenhum|criar NOME [--copiar-base] | \
                   sincronizar enviar|receber --ponte URL [--forcar] [--pasta DIR]";

pub fn comando(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("mostrar" | "show") => mostrar(args),
        Some("perfil" | "profile") => perfil(args),
        Some("sincronizar" | "sync") => sincronizar(args),
        Some("validar" | "validate") => {
            let Some(arq) = args.get(1) else {
                bail!("{USO}")
            };
            match carga::ler_arquivo(std::path::Path::new(arq)) {
                Ok(a) => {
                    println!(
                        "{arq}: valido (revisao {}, {} chave(s))",
                        a.revisao,
                        a.valores.len()
                    );
                    Ok(())
                }
                Err(e) => bail!("{arq}: invalido\n{}", carga::em_texto(&e)),
            }
        }
        Some("exemplo" | "example") => {
            println!("{}", serde_json::to_string_pretty(&gerar::exemplo())?);
            Ok(())
        }
        Some("definir" | "set") => definir(args),
        _ => bail!("{USO}"),
    }
}

fn mostrar(args: &[String]) -> Result<()> {
    let p = super::pasta(args);
    let v = cfg::vista(&p).map_err(|e| anyhow::anyhow!("{}", carga::em_texto(&e)))?;
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    println!("revisao {}  pasta {}", v["revisao"], v["arquivos"]["pasta"]);
    match (
        &v["arquivos"]["projeto"],
        &v["arquivos"]["projeto_ignorado"],
    ) {
        (Value::String(p), _) => println!("projeto {p}"),
        (_, Value::String(m)) => println!("aviso: {m}"),
        _ => {}
    }
    for c in v["chaves"].as_array().into_iter().flatten() {
        let valor = if c["segredo"] == true {
            if c["origem"] == "ambiente" {
                "(no ambiente)".to_string()
            } else if c["segredo_presente"] == true {
                "(no broker)".to_string()
            } else {
                "(ausente)".to_string()
            }
        } else {
            match &c["valor"] {
                Value::Null => "-".to_string(),
                Value::String(s) => s.clone(),
                outro => outro.to_string(),
            }
        };
        println!(
            "{:<38} {:<9} {}",
            c["chave"].as_str().unwrap_or(""),
            c["origem"].as_str().unwrap_or(""),
            valor
        );
    }
    Ok(())
}

fn definir(args: &[String]) -> Result<()> {
    let (Some(chave), Some(bruto)) = (args.get(1), args.get(2)) else {
        bail!("{USO}")
    };
    let Some(c) = por_chave(chave) else {
        let dica = carga::sugestao(chave)
            .map(|s| format!("; quis dizer `{s}`?"))
            .unwrap_or_default();
        bail!("{chave}: chave desconhecida{dica}");
    };
    let valor = if bruto == "--remover" {
        Value::Null
    } else {
        carga::do_texto(c, bruto).map_err(|m| anyhow::anyhow!("{chave}: {m}"))?
    };
    let escopo = if let Some(nome) = super::opcao(args, "--perfil") {
        Escopo::Perfil(nome)
    } else if args.iter().any(|a| a == "--projeto") {
        Escopo::Projeto
    } else {
        Escopo::Pasta
    };
    let mut m = Map::new();
    m.insert(chave.clone(), valor);
    match cfg::definir(&super::pasta(args), escopo, &m, None) {
        Ok(n) => {
            println!("{chave}: gravado (revisao {n})");
            Ok(())
        }
        Err(r) => bail!("{r}"),
    }
}

/// `config perfil listar | usar NOME | nenhum | criar NOME [--copiar-base]`.
fn perfil(args: &[String]) -> Result<()> {
    let pasta = super::pasta(args);
    let acao = match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("listar" | "list") | None, _) => {
            let v = cfg::perfis(&pasta).map_err(|e| anyhow::anyhow!("{}", carga::em_texto(&e)))?;
            match (&v["ativo"], &v["origem_do_ativo"]) {
                (Value::String(a), Value::String(o)) => println!("ativo: {a} ({o})"),
                _ => println!("ativo: (nenhum)"),
            }
            for p in v["lista"].as_array().into_iter().flatten() {
                println!(
                    "{:<24} {} chave(s){}",
                    p["nome"].as_str().unwrap_or(""),
                    p["chaves"].as_object().map(|m| m.len()).unwrap_or(0),
                    if p["ativo"] == true { "  *" } else { "" }
                );
            }
            return Ok(());
        }
        (Some("usar" | "use"), Some(nome)) => AcaoDePerfil::Usar(Some(nome.clone())),
        (Some("nenhum" | "none"), _) => AcaoDePerfil::Usar(None),
        (Some("criar" | "create"), Some(nome)) => AcaoDePerfil::Criar {
            nome: nome.clone(),
            copiar_base: args.iter().any(|a| a == "--copiar-base"),
        },
        _ => bail!("{USO}"),
    };
    let rotulo = match &acao {
        AcaoDePerfil::Usar(Some(n)) => format!("perfil {n} ativo"),
        AcaoDePerfil::Usar(None) => "nenhum perfil ativo".to_string(),
        AcaoDePerfil::Criar { nome, .. } => format!("perfil {nome} criado"),
    };
    match cfg::perfil(&pasta, acao, None) {
        Ok(n) => {
            println!("{rotulo} (revisao {n})");
            Ok(())
        }
        Err(r) => bail!("{r}"),
    }
}

/// `config sincronizar enviar|receber --ponte URL [--forcar]`: pela ponte, com o token da
/// ponte (ambiente, senao o broker da pasta).
fn sincronizar(args: &[String]) -> Result<()> {
    use phxclaw_agent::sincronizar::{Decisao, Ponte, enviar, receber};
    let pasta = super::pasta(args);
    let Some(url) = super::opcao(args, "--ponte") else {
        bail!("{USO}\n(falta --ponte URL: o http:// da `phxclaw ponte`)");
    };
    let token = phxclaw_agent::chaves::PONTE
        .do_ambiente_ou_broker(&pasta)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} nao esta no ambiente nem no broker: rode `phxclaw {}`",
                phxclaw_agent::chaves::PONTE.variaveis()[0],
                phxclaw_agent::chaves::PONTE.comando
            )
        })?;
    let ponte = Ponte { url, token };
    let forcar = args.iter().any(|a| a == "--forcar");
    let (verbo, r) = match args.get(1).map(String::as_str) {
        Some("enviar" | "push") => (
            "enviado",
            super::runtime()?.block_on(enviar(&pasta, &ponte, forcar)),
        ),
        Some("receber" | "pull") => (
            "recebido",
            super::runtime()?.block_on(receber(&pasta, &ponte, forcar)),
        ),
        _ => bail!("{USO}"),
    };
    let r = r.map_err(anyhow::Error::msg)?;
    match r.decisao {
        Decisao::Iguais => println!(
            "nada a fazer: os dois lados ja estao em {}",
            r.revisao_local
        ),
        Decisao::Seguir => println!(
            "{verbo}: local {} | remoto {}",
            r.revisao_local, r.revisao_remota
        ),
        Decisao::Conflito { local, remoto } if forcar => println!(
            "{verbo} a forca (conflito ignorado): local {local} -> {} | remoto {remoto} -> {}",
            r.revisao_local, r.revisao_remota
        ),
        Decisao::Conflito { local, remoto } => bail!(
            "conflito: os dois lados mudaram desde a ultima sincronizacao\n  local  {local}\n  remoto {remoto}\nrepita com --forcar para sobrepor o lado de destino"
        ),
    }
    Ok(())
}
