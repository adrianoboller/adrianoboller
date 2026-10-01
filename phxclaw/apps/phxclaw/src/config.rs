//! `phxclaw config mostrar|validar|exemplo|definir`: a porta do terminal para o
//! `config.json`. Chama as MESMAS `vista` e `definir` da rota `/v1/config`.

use anyhow::{Result, bail};
use phxclaw_agent::config::catalogo_do_config::{carga, gerar, por_chave};
use phxclaw_agent::config::{self as cfg, Escopo};
use serde_json::{Map, Value};

const USO: &str = "uso: phxclaw config mostrar [--json] | validar ARQ | exemplo | \
                   definir CHAVE VALOR|--remover [--projeto] [--pasta DIR]";

pub fn comando(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("mostrar" | "show") => mostrar(args),
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
    let escopo = if args.iter().any(|a| a == "--projeto") {
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
