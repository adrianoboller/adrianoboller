//! Firewall de proveniencia pela linha de comando: le alegacoes em JSON (uma por linha) e
//! devolve a decisao de ingestao de cada fonte -- a MESMA funcao `evaluate` que o harvester
//! usa, para que a decisao sobre um repositorio de fora nao seja opiniao de quem o analisa.
//!
//! Entrada (stdin), uma linha por fonte:
//! {"source":"x","license":"permissive|copyleft|proprietary|unknown","license_file":true,
//!  "hash":false,"origin_known":true,"declared_leak":false,"redistribution_prohibited":false}

use phxclaw_provenance_core::{evaluate, IngestionDecision, LicenseClass, ProvenanceClaim};
use std::io::BufRead;

fn campo<'a>(l: &'a str, k: &str) -> Option<&'a str> {
    let i = l.find(&format!("\"{k}\""))? + k.len() + 2;
    let resto = l[i..].trim_start().strip_prefix(':')?.trim_start();
    if let Some(s) = resto.strip_prefix('"') {
        return s.split('"').next();
    }
    resto.split([',', '}']).next().map(str::trim)
}

fn main() {
    let mut negados = 0;
    for linha in std::io::stdin().lock().lines().map_while(Result::ok) {
        if linha.trim().is_empty() {
            continue;
        }
        let b = |k: &str| campo(&linha, k) == Some("true");
        let fonte = campo(&linha, "source").unwrap_or("?");
        let licenca = match campo(&linha, "license") {
            Some("permissive") => LicenseClass::Permissive,
            Some("copyleft") => LicenseClass::Copyleft,
            Some("proprietary") => LicenseClass::Proprietary,
            _ => LicenseClass::Unknown,
        };
        let r = evaluate(&ProvenanceClaim {
            source_name: fonte,
            license_class: licenca,
            local_license_evidence: b("license_file"),
            archive_hash_verified: b("hash"),
            origin_known: b("origin_known"),
            declared_leak: b("declared_leak"),
            redistribution_prohibited: b("redistribution_prohibited"),
        });
        if r.decision == IngestionDecision::Deny {
            negados += 1;
        }
        println!(
            "{:<11} {fonte}  {:?}",
            format!("{:?}", r.decision).to_uppercase(),
            r.reasons
        );
    }
    std::process::exit(if negados > 0 { 3 } else { 0 });
}
