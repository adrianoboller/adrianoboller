//! Quais tabelas tem coluna marcada como dado pessoal e mesmo assim nasceram
//! em CLARO no disco.
//!
//! ```bash
//! cargo run --release --example tabela-marcada-nasceu-em-claro -- <dir-do-banco> [...]
//! ```
//!
//! # Por que ele existe
//!
//! Medido em 05/09/2026: uma tabela cujas UNICAS colunas marcadas sao
//! EXTERNAS (`Memo`/`Bin`) nasce em claro mesmo com o cofre ligado, e o texto
//! sigiloso vai legivel para o `.memo`. A cadeia esta na guarda vermelha
//! `coluna_externa_marcada_sozinha_nao_pode_ir_em_claro`
//! (`tests/cifra-dos-dados.rs`) e na cognicao do dia.
//!
//! O achado veio com um buraco nomeado junto: **ninguem sabia quantas tabelas
//! estao nesse estado**, e sem esse numero o alcance do defeito era suposicao.
//! Este conferidor responde a pergunta -- e responde tambem depois do
//! conserto, que e quando ele passa a valer como catraca.
//!
//! # Como ele decide, e o que ele NAO faz
//!
//! Duas leituras, e nenhuma adivinhacao:
//!
//! - **nasceu cifrada?** sai da VERSAO do `.reg`, nos bytes 8..10 do
//!   cabecalho: 4 e o volume em claro, 5 e o cifrado. E o proprio comentario
//!   do `reg.rs` que diz por que ela e o discriminador -- *«a versao no byte 8
//!   e a unica coisa que se le antes de decidir quantos bytes ler»*;
//! - **tem coluna marcada?** sai do esquema, pelo caminho oficial
//!   (`Table::abrir` e `esquema().colunas_pessoais()`). Reimplementar o
//!   desserializador aqui abriria a porta para os dois divergirem.
//!
//! Ele NAO conserta, NAO abre a tabela para escrita e NAO precisa da senha:
//! a pergunta «nasceu cifrada?» se responde sem chave nenhuma, que e
//! exatamente o que a torna util em auditoria.
//!
//! Sai 1 quando achou alguma tabela nesse estado, para servir de portao.

use std::path::{Path, PathBuf};

use phxsql_core::schema::Schema;

/// A versao e o esquema, lidos do cabecalho do `.reg` SEM CHAVE NENHUMA.
///
/// # Por que sem chave, e por que isso e legitimo
///
/// Porque o esquema NAO e protegido, e o `reg.rs` diz isso na lista do que
/// continua em claro na versao 5: *«o rowid, a versao da linha, o status do
/// slot e o esquema -- inclusive o NOME da coluna marcada»*. Uma auditoria que
/// exigisse a senha do banco para responder «esta tabela protege o que
/// declarou?» seria uma auditoria que so quem tem o segredo pode fazer.
///
/// A primeira versao deste conferidor abria a tabela inteira (`Table::abrir`)
/// e falhou em 8 de 8 -- os DIARIOS nascem cifrados sempre que o cofre esta
/// ligado, mesmo quando o `.reg` nasce em claro, entao abrir a tabela exigia a
/// chave para responder uma pergunta que nao precisa dela.
///
/// Layout, do cabecalho do `reg.rs`: bytes 8..10 a versao (4 em claro, 5
/// cifrado), byte 52 o `schema_len`, e o bloco do esquema logo apos o
/// cabecalho -- 128 bytes na versao 4, 192 na 5.
fn versao_e_esquema(caminho: &Path) -> Option<(u16, Result<Schema, String>)> {
    let bytes = std::fs::read(caminho).ok()?;
    if bytes.len() < 56 || &bytes[0..8] != phxsql_store::reg::MAGIC_REG {
        return None;
    }
    let versao = u16::from_le_bytes([bytes[8], bytes[9]]);
    let cab_len = if versao >= 5 { 192usize } else { 128 };
    let schema_len = u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]) as usize;
    let fim = cab_len + schema_len;
    if bytes.len() < fim {
        return Some((versao, Err("bloco de esquema truncado".into())));
    }
    Some((
        versao,
        Schema::desserializar(&bytes[cab_len..fim]).map_err(|e| e.to_string()),
    ))
}

/// O nome da tabela a partir do arquivo: `clientes_003.reg` -> `clientes`.
///
/// O corte no sublinhado seguido de digitos e o mesmo criterio do somador de
/// disco da bancada: `pedidos_001` e a mesma tabela que `pedidos`, e
/// `pedidos2` nao e.
fn nome_da_tabela(arquivo: &str) -> String {
    let base = arquivo.trim_end_matches(".reg");
    match base.rsplit_once('_') {
        Some((cabeca, cauda)) if !cauda.is_empty() && cauda.chars().all(|c| c.is_ascii_digit()) => {
            cabeca.to_string()
        }
        _ => base.to_string(),
    }
}

fn main() {
    let dirs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if dirs.is_empty() {
        eprintln!("uso: tabela-marcada-nasceu-em-claro <dir-do-banco> [...]");
        std::process::exit(2);
    }

    let mut vistas = 0usize;
    let mut marcadas = 0usize;
    let mut em_claro: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut nao_conferidas: Vec<(String, String, String)> = Vec::new();

    for dir in &dirs {
        let Ok(entradas) = std::fs::read_dir(dir) else {
            eprintln!("nao consegui ler {}", dir.display());
            continue;
        };
        let mut nomes: Vec<String> = entradas
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.ends_with(".reg").then(|| nome_da_tabela(&n))
            })
            .collect();
        nomes.sort();
        nomes.dedup();

        for nome in nomes {
            vistas += 1;
            // O volume 1 basta: todo volume carrega o cabecalho completo, e a
            // cifra e decidida na CRIACAO da tabela, uma vez so.
            let v1 = dir.join(format!("{nome}.reg"));
            let v1 = if v1.exists() {
                v1
            } else {
                dir.join(format!("{nome}_001.reg"))
            };
            let Some((versao, esquema)) = versao_e_esquema(&v1) else {
                nao_conferidas.push((
                    dir.display().to_string(),
                    nome.clone(),
                    "nao e um `.reg` desta casa (magic ou tamanho)".into(),
                ));
                continue;
            };
            let esquema = match esquema {
                Ok(e) => e,
                Err(e) => {
                    nao_conferidas.push((dir.display().to_string(), nome.clone(), e));
                    continue;
                }
            };
            let pessoais: Vec<String> = esquema
                .colunas_pessoais()
                .iter()
                .map(|(_, c)| {
                    format!(
                        "{} ({})",
                        c.nome,
                        if c.ty.externo() { "externa" } else { "inline" }
                    )
                })
                .collect();
            if pessoais.is_empty() {
                continue;
            }
            marcadas += 1;
            if versao == 4 {
                em_claro.push((dir.display().to_string(), nome.clone(), pessoais));
            }
        }
    }

    println!("{vistas} tabela(s) vistas, {marcadas} com coluna marcada como dado pessoal");

    // O que nao se conseguiu ler aparece ANTES do veredito, e muda o codigo de
    // saida: «nao consegui olhar» nunca pode ser lido como «esta limpo».
    if !nao_conferidas.is_empty() {
        println!(
            "\n{} NAO CONFERIDA(S) -- o veredito abaixo NAO as cobre:",
            nao_conferidas.len()
        );
        for (dir, nome, erro) in &nao_conferidas {
            println!("  {dir}/{nome}: {erro}");
        }
    }

    if em_claro.is_empty() {
        if nao_conferidas.is_empty() {
            println!("nenhuma tabela marcada nasceu em claro.");
            return;
        }
        println!("\nNenhuma das CONFERIDAS nasceu em claro.");
        std::process::exit(1);
    }
    println!("\n{} NASCERAM EM CLARO com coluna marcada:", em_claro.len());
    for (dir, nome, cols) in &em_claro {
        println!("  {dir}/{nome}");
        for c in cols {
            println!("      marcada: {c}");
        }
    }
    println!(
        "\nUma tabela so nasce cifrada quando ha coluna marcada INLINE: a\n\
         condicao que liga a cifra ignora as externas (`faixas_pessoais`,\n\
         reg.rs). Ver a guarda `coluna_externa_marcada_sozinha_nao_pode_ir_em_claro`."
    );
    std::process::exit(1);
}
