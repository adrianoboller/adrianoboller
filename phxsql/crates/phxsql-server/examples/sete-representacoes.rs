//! As sete representacoes do dado de uma coluna MARCADA, varridas em disco.
//!
//! ```bash
//! CARGO_BUILD_JOBS=2 cargo run --example sete-representacoes -p phxsql-server --offline
//! ```
//!
//! # Por que existe
//!
//! O pedido 339(b) mapeou onde o valor de uma coluna `DadoPessoal` aparece
//! (filhos 340 a 347) com uma sonda que morava fora do repositorio; o 669 a
//! trouxe para ca. Script que resolveu algo nao morre com a sessao.
//!
//! # As sete, e o controle
//!
//! 1. `.fts` (indice de texto)       4. backup (`backup::executar`)
//! 2. imagem de replicacao           5. exportacao (`Planilha`)
//! 3. lixeira + trilha (`.trash`,    6. `.reason` (motivo da exclusao)
//!    `.lgpd`)                       7. `.pag` (particao por letra)
//!
//! O CONTROLE vem antes do veredito: uma tabela GEMEA, mesma carga com o
//! cofre DESLIGADO, TEM de sair «claro» no `.ndx` e no `.reg`, e o `.reg` da
//! cifrada TEM de sair «cifrado». Sonda que nao acha o caso conhecido nao vale
//! para os desconhecidos -- foi assim que a primeira rodada da sonda original
//! errou. Se o controle falhar, o exemplo sai 2.
//!
//! Ate 09/10/2026 o controle positivo era o `.ndx` da propria tabela cifrada,
//! que vazava por decisao registrada. O pedido 339 (achado 2) selou a pagina
//! dele, e o controle passou para a gemea em claro: o `.ndx` virou a linha 0,
//! uma representacao medida como as outras.
//!
//! # Limites, ditos
//!
//! - A exportacao do servidor (`op_exportar`) so se alcanca pelo soquete; aqui
//!   se exerce o gerador (`exportar::Planilha`) que ela chama, sobre as linhas
//!   que `Table::varrer` devolve. Sai «claro» POR DESENHO: quem pede o
//!   arquivo e quem tem a chave, e a planilha e o produto.
//! - O `.pag` sobre coluna marcada e RECUSADO na declaracao do esquema
//!   (`coluna_que_o_rowid_revela`, pedido 358); a linha mostra a recusa, nao
//!   um arquivo.
//! - A sonda procura bytes. «cifrado» quer dizer «a sonda nao esta nos bytes»,
//!   nao prova de sigilo contra quem tem a senha.

use std::path::{Path, PathBuf};

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, IndiceDeTexto, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_server::exportar::{Formato, Planilha};
use phxsql_store::backup;
use phxsql_store::cofre;
use phxsql_store::table::Table;

/// Minuscula e sem acento: o `.fts` dobra o termo, e procurar a grafia crua
/// acharia «cifrado» num arquivo vazando (o mesmo cuidado do teste do 340).
const SONDA: &str = "fulanodetalzinho";

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            // O indice sobre a coluna marcada: o controle positivo.
            IndexDef::new("porNome", vec![IndexColumn::asc(1)]),
        ],
    )
    .expect("esquema")
    .com_indices_de_texto(vec![IndiceDeTexto::new("porNomeTexto", 1)])
    .expect("indice de texto")
}

fn contem(bytes: &[u8], agulha: &[u8]) -> bool {
    !agulha.is_empty() && bytes.windows(agulha.len()).any(|j| j == agulha)
}

fn veredito(achou: bool) -> &'static str {
    if achou {
        "claro"
    } else {
        "cifrado"
    }
}

/// Junta os bytes de todo arquivo da pasta (recursivo) cujo nome casa o filtro.
fn bytes_de(pasta: &Path, filtro: &dyn Fn(&str) -> bool) -> Vec<u8> {
    let mut tudo = Vec::new();
    let mut pilha: Vec<PathBuf> = vec![pasta.to_path_buf()];
    while let Some(p) = pilha.pop() {
        let Ok(it) = std::fs::read_dir(&p) else {
            continue;
        };
        for e in it.flatten() {
            let c = e.path();
            if c.is_dir() {
                pilha.push(c);
            } else if let Some(n) = c.file_name().and_then(|n| n.to_str()) {
                if filtro(n) {
                    tudo.extend(std::fs::read(&c).unwrap_or_default());
                }
            }
        }
    }
    tudo
}

fn com_extensao(ext: &'static str) -> impl Fn(&str) -> bool {
    move |n: &str| n.ends_with(&format!(".{ext}"))
}

fn main() {
    let dir = std::env::temp_dir().join(format!("sete-representacoes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let raiz = dir.join("raiz");
    let dados = raiz.join("banco");
    let copia = dir.join("copia");
    std::fs::create_dir_all(&dados).expect("diretorio");

    // A gemea em claro: o controle positivo EM DISCO. Mesma carga, cofre
    // desligado -- se a sonda nao achar o nome aqui, ela nao acha nada.
    cofre::desligar();
    let gemea = dir.join("gemea");
    std::fs::create_dir_all(&gemea).expect("diretorio");
    {
        let mut g = Table::criar(&gemea, esquema()).expect("criar gemea");
        for i in 1..=3i64 {
            g.inserir(&[Value::Int(i), Value::Str(format!("{SONDA}{i:04}"))])
                .expect("inserir gemea");
        }
        g.sincronizar().expect("sincronizar gemea");
    }

    cofre::definir("sonda das sete representacoes", cofre::ITERACOES_MINIMAS).expect("cofre");

    // A trilha e opt-in: sem isto o `.lgpd` nem nasce e a linha 3 da saida
    // diria «cifrado» por ausencia.
    phxsql_store::trilha::definir(true, false);

    // Espelhada: so assim o `.bkp` existe para ser sondado.
    let mut t = Table::criar_espelhada(&dados, esquema())
        .expect("criar")
        .com_imagem_no_diario(true);
    for i in 1..=3i64 {
        t.inserir(&[Value::Int(i), Value::Str(format!("{SONDA}{i:04}"))])
            .expect("inserir");
    }
    // Uma alteracao da coluna marcada: e ela que a trilha registra.
    t.atualizar(2, &[Value::Int(2), Value::Str(format!("{SONDA}9999"))])
        .expect("atualizar");
    // A linha 3 vai para a lixeira, com motivo: enche `.trash`, `.reason`
    // e a trilha com o payload daquela linha.
    t.excluir_suave(3, "sonda do pedido 669").expect("excluir");
    t.sincronizar().expect("sincronizar");

    // Imagem de replicacao: a do diario (`imagem_da_linha_do_rowid`) e a que
    // sai pelo fio (`imagem_para_o_fio`), que o 342 trata de forma distinta.
    let imagem = t.imagem_da_linha_do_rowid(1).expect("imagem");
    let imagem_fio = t.imagem_para_o_fio(&imagem).expect("imagem para o fio");
    let linhas: Vec<Vec<Value>> = t
        .varrer()
        .expect("varrer")
        .into_iter()
        .map(|(_, l)| l)
        .collect();
    let planilha = Planilha {
        titulo: "sonda".into(),
        subtitulo: String::new(),
        colunas: Planilha::do_esquema(t.esquema(), "sonda"),
        linhas: &linhas,
    };
    let csv = planilha.gerar(Formato::Csv).expect("csv");
    drop(t);

    // Backup: copia a pasta que contem o banco, como o servidor faz.
    backup::executar(&raiz, &copia, 0).expect("backup");

    let agulha = SONDA.as_bytes();
    let tem = |ext: &'static str| contem(&bytes_de(&dados, &com_extensao(ext)), agulha);
    // Arquivo que nao existe nao prova nada: «cifrado» por ausencia seria o
    // teste que passa por engano. Os arquivos sondados tem de ter bytes.
    for ext in ["reg", "ndx", "fts", "trash", "lgpd", "reason", "log"] {
        let n = bytes_de(&dados, &com_extensao(ext)).len();
        assert!(
            n > 0,
            ".{ext} nao existe ou esta vazio: a sonda nao mediu nada"
        );
    }

    // .pag: a particao por letra sobre a coluna marcada.
    let pag = match esquema().com_paginacao(Paginacao::por_letra(100, 1).expect("paginacao")) {
        Err(e) => format!("recusado na declaracao ({e})"),
        Ok(s) => match s.conferir_oraculo_do_rowid(s.paginacao().modo) {
            Err(e) => format!("recusado na declaracao ({e})"),
            Ok(()) => "ACEITO -- o nome do arquivo vaza o primeiro caractere".into(),
        },
    };

    let na_gemea = |ext: &'static str| contem(&bytes_de(&gemea, &com_extensao(ext)), agulha);
    let ctrl_gemea_ndx = na_gemea("ndx");
    let ctrl_gemea_reg = na_gemea("reg");
    let ctrl_reg = tem("reg");
    println!(
        "controle   .ndx  {:8}  (gemea sem cofre; esperado: claro)",
        veredito(ctrl_gemea_ndx)
    );
    println!(
        "controle   .reg  {:8}  (gemea sem cofre; esperado: claro)",
        veredito(ctrl_gemea_reg)
    );
    println!(
        "controle   .reg  {:8}  (esperado: cifrado)",
        veredito(ctrl_reg)
    );
    println!("0 .ndx           {}", veredito(tem("ndx")));
    println!(
        "1 .fts           {}",
        veredito(tem(phxsql_store::fts::EXT_FTS))
    );
    println!(
        "2 imagem         {} (em memoria)  {} (.log)  {} (fio, por desenho)",
        veredito(contem(&imagem, agulha)),
        veredito(tem("log")),
        veredito(contem(&imagem_fio, agulha))
    );
    println!(
        "3 lixeira/trilha {} (.trash)  {} (.lgpd)",
        veredito(tem(phxsql_store::lixeira::EXT_TRASH)),
        veredito(tem(phxsql_store::trilha::EXT_LGPD))
    );
    println!(
        "4 backup         {} (.ndx copiado)  {} (.fts)  {} (.reg)  {} (.bkp)",
        veredito(contem(&bytes_de(&copia, &com_extensao("ndx")), agulha)),
        veredito(contem(&bytes_de(&copia, &com_extensao("fts")), agulha)),
        veredito(contem(&bytes_de(&copia, &com_extensao("reg")), agulha)),
        veredito(contem(&bytes_de(&copia, &com_extensao("bkp")), agulha)),
    );
    println!(
        "5 exportacao     {} (csv; claro por desenho)",
        veredito(contem(&csv, agulha))
    );
    println!(
        "6 .reason        {}",
        veredito(tem(phxsql_store::motivo::EXT_REASON))
    );
    println!("7 .pag           {pag}");

    cofre::desligar();
    let _ = std::fs::remove_dir_all(&dir);
    if !ctrl_gemea_ndx || !ctrl_gemea_reg || ctrl_reg {
        eprintln!("CONTROLE FALHOU: a sonda nao reproduziu o caso conhecido; o veredito nao vale");
        std::process::exit(2);
    }
}
