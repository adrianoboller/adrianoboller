//! `.pag` -- a instrucao de particao da tabela, em JSON legivel.
//!
//! ```text
//! cadastroClientes.pag
//! ```
//!
//! # O que ele e, e o que ele NAO e
//!
//! Ele e um **descritor**: diz como a tabela esta partida, que arquivo guarda
//! o que, e quanto tem em cada um. Existe para quem esta do lado de fora --
//! uma camada SQL, um ETL, um relatorio, um `ls` -- descobrir isso sem abrir o
//! `.reg` e sem saber ler o bloco de esquema.
//!
//! Ele **nao e fonte de verdade**, e isso e o desenho e nao um detalhe. A
//! verdade mora onde sempre morou: o modo e a coluna de referencia estao no
//! bloco de esquema dentro do `.reg`, e quantas linhas cada balde tem esta no
//! cabecalho de cada volume. O `.pag` e gerado a partir dos dois.
//!
//! A razao e a mesma que impede gravar «e chave primaria» na coluna, e a mesma
//! que impede um arquivo `sequences` com uma segunda copia dos contadores: uma
//! segunda copia e uma segunda verdade, e as duas divergem no primeiro caminho
//! que esquecer de atualizar uma delas. Aqui a divergencia seria pior que o
//! normal -- o `.pag` diz em que ARQUIVO a linha esta.
//!
//! Por isso o motor nunca LE este arquivo para decidir nada. Apagar o `.pag`
//! nao quebra a tabela; regravar resolve.

use std::path::{Path, PathBuf};

use phxsql_core::error::Result;
use phxsql_core::paginacao::{ModoParticao, BALDES};
use phxsql_core::schema::Schema;

pub const EXT_PAG: &str = "pag";

/// Escreve o `.pag` da tabela, trocando o arquivo INTEIRO de uma vez.
///
/// `baldes` vem de `Table::baldes()` -- vazio nos modos que nao sao
/// alfanumericos, e ai o arquivo descreve a particao sem a lista de letras.
///
/// # Por que temporario + `rename`, e nao `fs::write`
///
/// Este arquivo e' regravado a CADA `Table::sincronizar()`, e `fs::write`
/// abre com `O_TRUNC`: entre o zerar e o terminar de escrever, quem le de
/// fora ve um JSON pela metade. Medido em 05/09/2026 nesta maquina: a janela
/// truncada dura **33,2 us por regravacao** (36% dos 93 us da gravacao), e um
/// leitor que insista durante a regravacao pega o arquivo partido em
/// **82,4% das leituras** (482.246 de 585.169). Com a troca por `rename`,
/// **0 de 606.086**.
///
/// Um `.pag` PERDIDO nao custa nada -- ele e' derivado, o motor nunca o le, e
/// o proximo `sincronizar` o regrava. Um `.pag` pela METADE mente para a
/// unica plateia que ele tem, que e' de fora: uma camada SQL, um ETL, um
/// relatorio. Por isso o conserto e' de ATOMICIDADE e nao de durabilidade --
/// **nao ha `fsync` aqui**, e a catraca `TETO_FSYNC_POR_FECHO_V2` continua
/// valendo 8. Sem `fsync` do diretorio, uma queda pode desfazer a troca e
/// deixar o `.pag` ANTERIOR, que e' velho e valido; e' a degradacao certa.
///
/// Medido tambem o custo, e ele nao existe: mediana de seis corridas de
/// 20.000 gravacoes, 92,6 us com `fs::write` contra 76,0 us com
/// temporario + `rename`.
///
/// # O resto que uma queda pode deixar, dito em vez de escondido
///
/// Uma queda dentro da janela de 33 us deixa `<nome>.pag.novo` para tras. O
/// proximo `sincronizar` da tabela o renomeia por cima, entao ele nao se
/// acumula. O que ele NAO faz e' sair no `excluir_tabela`: as dez extensoes de
/// `catalogo.rs` cobrem `.pag` e nao `.pag.novo`. E' decisao, e nao esquecimento
/// -- por o temporario naquela lista o faria aparecer no `arquivos_da_tabela`,
/// que e' a lista que a TELA mostra, e arquivo de trabalho na tela do usuario
/// e' pior que um resto de 3 KiB que so' existe depois de uma queda.
pub fn escrever(
    diretorio: impl AsRef<Path>,
    nome: &str,
    esquema: &Schema,
    baldes: &[u64],
    volumes: &[u32],
) -> Result<PathBuf> {
    let diretorio = diretorio.as_ref();
    let caminho = diretorio.join(format!("{nome}.{EXT_PAG}"));
    let temporario = diretorio.join(format!("{nome}.{EXT_PAG}.novo"));
    let texto = montar(nome, esquema, baldes, volumes);
    // Falhar aqui nao pode deixar o temporario para tras: quem falha uma vez
    // costuma falhar de novo, e o resto viraria permanente.
    if let Err(e) = std::fs::write(&temporario, texto) {
        let _ = std::fs::remove_file(&temporario);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&temporario, &caminho) {
        let _ = std::fs::remove_file(&temporario);
        return Err(e.into());
    }
    Ok(caminho)
}

/// O JSON, montado a mao com o escritor do projeto.
///
/// Indentado de proposito: este arquivo existe para ser LIDO por gente e por
/// ferramenta de fora, e JSON numa linha so serve nem a um nem a outro.
pub fn montar(nome: &str, esquema: &Schema, baldes: &[u64], volumes: &[u32]) -> String {
    let p = esquema.paginacao();
    let modo = p.modo;
    let coluna = modo
        .coluna()
        .and_then(|i| esquema.colunas().get(i))
        .map(|c| c.nome.as_str())
        .unwrap_or("");

    let mut out = String::with_capacity(2048);
    out.push_str("{\n");
    linha(
        &mut out,
        "gerado_por",
        &format!("\"PhxSql {}\"", versao()),
        true,
    );
    linha(
        &mut out,
        "aviso",
        "\"descritor gerado; a verdade esta no bloco de esquema do .reg e nos \
         cabecalhos dos volumes. O motor nao le este arquivo.\"",
        true,
    );
    linha(&mut out, "tabela", &texto(nome), true);
    linha(&mut out, "modo", &texto(modo.nome()), true);
    linha(
        &mut out,
        "particionada",
        if p.ligada() { "true" } else { "false" },
        true,
    );
    linha(&mut out, "coluna_referencia", &texto(coluna), true);
    linha(
        &mut out,
        "registros_por_arquivo",
        &p.registros_por_arquivo.to_string(),
        true,
    );
    linha(&mut out, "max_arquivos", &p.max_arquivos.to_string(), true);

    // A conta que transforma rowid em arquivo. Escrita por extenso porque e
    // exatamente o que quem le este arquivo precisa saber para nao ter de
    // adivinhar -- e porque ela e a mesma nos tres modos.
    linha(
        &mut out,
        "endereco",
        &texto(
            "volume = (rowid - 1) / registros_por_arquivo + 1; \
             slot = (rowid - 1) % registros_por_arquivo + 1",
        ),
        true,
    );

    if let ModoParticao::PorLetra { .. } = modo {
        linha(
            &mut out,
            "ordem_de_leitura",
            &texto(
                "alfabetica, balde a balde. A ordem de digitacao esta na \
                 coluna de sistema rownum, e nao no rowid",
            ),
            true,
        );
        out.push_str("  \"baldes\": [\n");
        let existentes: Vec<u32> = volumes.to_vec();
        for (i, letra) in BALDES.iter().enumerate() {
            let n = i + 1;
            let usados = baldes.get(i).copied().unwrap_or(0);
            let existe = existentes.contains(&(n as u32));
            let primeiro = (n as u64 - 1) * p.registros_por_arquivo + 1;
            out.push_str(&format!(
                "    {{ \"balde\": {n}, \"letra\": {}, \"arquivo\": {}, \
                 \"existe\": {existe}, \"registros\": {usados}, \
                 \"primeiro_rowid\": {primeiro} }}{}\n",
                texto(letra),
                texto(&format!("{nome}_{letra}.reg")),
                if n == BALDES.len() { "" } else { "," },
            ));
        }
        out.push_str("  ],\n");
        let total: u64 = baldes.iter().sum();
        linha(&mut out, "registros", &total.to_string(), false);
    } else {
        out.push_str("  \"volumes\": [");
        for (i, v) in volumes.iter().enumerate() {
            out.push_str(&format!("{}{}", if i == 0 { "" } else { ", " }, v));
        }
        out.push_str("]\n");
    }
    out.push_str("}\n");
    out
}

fn versao() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn linha(out: &mut String, chave: &str, valor: &str, virgula: bool) {
    out.push_str(&format!(
        "  \"{chave}\": {valor}{}\n",
        if virgula { "," } else { "" }
    ));
}

/// Texto entre aspas, com o escape do JSON.
///
/// Nome de tabela vem do usuario, e o `.pag` e lido por ferramenta de fora:
/// uma aspa nao escapada aqui quebraria o parser do outro lado.
fn texto(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::json::Json;
    use phxsql_core::paginacao::Paginacao;
    use phxsql_core::schema::{Column, Schema};
    use phxsql_core::types::ColumnType;

    fn esquema_por_letra() -> Schema {
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            ],
            vec![],
        )
        .unwrap()
        .com_paginacao(Paginacao::por_letra(1_000_000, 1).unwrap())
        .unwrap()
    }

    /// O que sai tem de ser JSON valido -- e quem confere e o analisador do
    /// proprio projeto, e nao o olho.
    #[test]
    fn o_que_sai_e_json_valido() {
        let e = esquema_por_letra();
        let mut baldes = vec![0u64; BALDES.len()];
        baldes[0] = 12; // A
        baldes[18] = 4_000; // S
        let texto = montar("clientes", &e, &baldes, &[1, 19]);
        let j = Json::analisar(&texto).expect("o .pag nao e JSON valido");

        assert_eq!(j.texto_ou("tabela", ""), "clientes");
        assert_eq!(j.texto_ou("modo", ""), "letra");
        assert_eq!(j.texto_ou("coluna_referencia", ""), "nome");
        assert_eq!(j.inteiro_ou("registros", -1), 4_012);

        let lista = j.campo("baldes").and_then(Json::lista).unwrap();
        assert_eq!(lista.len(), 37);
        assert_eq!(lista[0].texto_ou("letra", ""), "A");
        assert_eq!(lista[0].inteiro_ou("registros", -1), 12);
        assert_eq!(lista[0].texto_ou("arquivo", ""), "clientes_A.reg");
        assert_eq!(lista[18].texto_ou("letra", ""), "S");
        assert_eq!(lista[18].inteiro_ou("registros", -1), 4_000);
        assert_eq!(lista[36].texto_ou("letra", ""), "Outros");
        // O balde que nunca recebeu linha aparece, com `existe: false`: a tela
        // precisa saber que ele e previsto e esta vazio, e nao que sumiu.
        assert!(matches!(lista[1].campo("existe"), Some(Json::Bool(false))));
        assert!(matches!(lista[0].campo("existe"), Some(Json::Bool(true))));
    }

    /// Nome hostil nao pode quebrar o JSON de quem le do outro lado.
    #[test]
    fn nome_com_aspas_sai_escapado() {
        let e = esquema_por_letra();
        let texto = montar("ta\"bela\\estranha", &e, &[], &[1]);
        let j = Json::analisar(&texto).expect("aspas quebraram o JSON");
        assert_eq!(j.texto_ou("tabela", ""), "ta\"bela\\estranha");
    }

    #[test]
    fn tabela_sem_particao_tambem_se_descreve() {
        let e = Schema::new(
            "simples",
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![],
        )
        .unwrap();
        let j = Json::analisar(&montar("simples", &e, &[], &[1])).unwrap();
        assert_eq!(j.texto_ou("modo", ""), "quantidade");
        assert!(matches!(j.campo("particionada"), Some(Json::Bool(false))));
        assert!(j.campo("baldes").is_none());
    }
}
