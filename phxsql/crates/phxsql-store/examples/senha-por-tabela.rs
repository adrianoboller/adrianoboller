//! As tres premissas que decidem se cabe **senha por tabela** no PhxSql.
//!
//! ```bash
//! cargo run --release --example senha-por-tabela -p phxsql-store
//! ```
//!
//! Existe por causa da ordem do dono de 05/09/2026 -- *«medir primeiro,
//! decidir depois»* -- e da regra da casa que a acompanha: **medir a premissa
//! do item vem antes de implementar o item**. Nenhuma linha daqui implementa
//! senha por tabela; todas medem o terreno onde ela moraria.
//!
//! O que ele mede, nesta ordem, porque uma mata a seguinte:
//!
//! 1. **o cache de chaves derivadas com duas senhas** -- a chave do mapa e
//!    `(sal, iteracoes)` e NAO inclui a senha. A pergunta e se isso devolve
//!    chave errada, e quantas derivacoes de PBKDF2 um pedido passaria a pagar;
//! 2. **quanto do dado sai do `.ndx` sem a senha** -- em pares
//!    `(valor, rowid)` reconstruidos dos bytes crus, nao em «vaza / nao vaza»;
//! 3. **o que a replica precisa para aplicar a imagem** -- a faixa inline e a
//!    externa respondem coisas OPOSTAS, e so uma delas esta escrita.
//!
//! Nao mede TEMPO de proposito: o custo de uma derivacao ja esta medido em
//! `docs/SEGURANCA.md` §11.4 (**298 ms** para 210.000 iteracoes) e cronometro
//! aqui brigaria com quem estiver medindo na maquina ao lado. O que se conta
//! aqui e determinístico -- derivacoes, bytes, pares recuperados.
//!
//! A ultima linha e `RESULTADO <json>`, como na `carga`.

use std::path::{Path, PathBuf};

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::cofre;
use phxsql_store::table::Table;
use phxsql_store::Operacao;

/// Iteracoes no piso. O que se conta aqui e QUANTAS derivacoes, nunca quanto
/// cada uma demora -- e 210.000 iteracoes so fariam o medidor demorar.
const RAPIDO: u32 = cofre::ITERACOES_MINIMAS;
const SENHA_A: &str = "a senha da tabela de clientes";
const SENHA_B: &str = "a senha da tabela de folha";

/// Um nome bem reconhecivel: e o que se procura nos bytes do indice.
const SEGREDO: &str = "Fulano de Tal da Silva";
const LINHAS: i64 = 200;

fn base() -> PathBuf {
    std::env::temp_dir().join(format!("phx-senha-por-tabela-{}", std::process::id()))
}

fn esquema(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            // Marcada COMO dado pessoal e indexada -- exatamente o par que a
            // premissa 2 investiga.
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

/// O mesmo esquema com a coluna externa em `Bin`, e nao em `Memo`.
///
/// A troca nao e cosmetica: `Memo` passa por `String::from_utf8` na volta e o
/// texto cifrado quase nunca e UTF-8 valido, entao ele para com erro por
/// ACIDENTE. `Bin` nao tem essa peneira -- e e nele que se ve o que a replica
/// faz de verdade com bytes que ela nao sabe abrir.
fn esquema_bin(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("anexo", ColumnType::Bin).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porNome", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
}

/// O anexo de uma linha: bytes reconheciveis, para saber se o que chegou do
/// outro lado e o conteudo ou o texto cifrado dele.
fn anexo(i: i64) -> Vec<u8> {
    format!("ANEXO-CONFIDENCIAL-{i:04}").into_bytes()
}

fn linha_bin(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("{SEGREDO} {i:04}")),
        Value::Bin(anexo(i)),
    ]
}

/// O sal de 16 bytes do `.reg`, lido do cabecalho como qualquer um que so
/// tenha o arquivo leria. Ele mora em claro: sal nao e segredo.
fn sal_do_reg(d: &Path, nome: &str) -> Vec<u8> {
    let b = std::fs::read(d.join(format!("{nome}.reg"))).unwrap();
    b[136..136 + cofre::SAL_LEN].to_vec()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("{SEGREDO} {i:04}")),
        Value::Memo(format!("anotacao confidencial numero {i}")),
    ]
}

fn bytes_com_extensao(d: &Path, ext: &str) -> Vec<u8> {
    let mut tudo = Vec::new();
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some(ext) {
                tudo.extend_from_slice(&std::fs::read(&p).unwrap_or_default());
            }
        }
    }
    tudo
}

// ---------------------------------------------------------------------------
// Premissa 1 -- o cache de chaves derivadas com duas senhas
// ---------------------------------------------------------------------------

/// Devolve `(erra, derivacoes_com_uma_senha, derivacoes_alternando)`.
fn premissa_1() -> (bool, u64, u64) {
    cofre::desligar();
    let sal = [7u8; cofre::SAL_LEN];

    // (a) A mesma dupla (sal, iteracoes) com DUAS senhas devolve a mesma
    //     chave? Se devolvesse, o cache erraria -- que era o diagnostico a
    //     medir.
    cofre::definir(SENHA_A, RAPIDO).unwrap();
    let ka = cofre::derivar(&sal, RAPIDO, "<medidor>").unwrap();
    cofre::definir(SENHA_B, RAPIDO).unwrap();
    let kb = cofre::derivar(&sal, RAPIDO, "<medidor>").unwrap();
    let erra = ka == kb;

    // (b) Quantas derivacoes um SERVIDOR pagaria. Ele abre e fecha a tabela a
    //     cada pedido, entao o que importa e quantas vezes o PBKDF2 roda em N
    //     aberturas das MESMAS duas tabelas.
    //
    //     Hoje (uma senha do servidor): as duas tabelas ja tem cada uma o seu
    //     sal, entao sao duas derivacoes e nunca mais.
    let sal_clientes = [1u8; cofre::SAL_LEN];
    let sal_folha = [2u8; cofre::SAL_LEN];
    cofre::desligar();
    cofre::definir(SENHA_A, RAPIDO).unwrap();
    let antes = cofre::derivacoes();
    for _ in 0..10 {
        cofre::derivar(&sal_clientes, RAPIDO, "<clientes>").unwrap();
        cofre::derivar(&sal_folha, RAPIDO, "<folha>").unwrap();
    }
    let uma_senha = cofre::derivacoes() - antes;

    //     Com senha POR TABELA pelo caminho que existe hoje -- `definir` antes
    //     de cada abertura --, cada troca limpa o cache inteiro.
    let antes = cofre::derivacoes();
    for _ in 0..10 {
        cofre::definir(SENHA_A, RAPIDO).unwrap();
        cofre::derivar(&sal_clientes, RAPIDO, "<clientes>").unwrap();
        cofre::definir(SENHA_B, RAPIDO).unwrap();
        cofre::derivar(&sal_folha, RAPIDO, "<folha>").unwrap();
    }
    let alternando = cofre::derivacoes() - antes;

    cofre::desligar();
    (erra, uma_senha, alternando)
}

// ---------------------------------------------------------------------------
// Premissa 2 -- quanto do dado sai do `.ndx` sem a senha
// ---------------------------------------------------------------------------

/// As entradas de folha de um indice, lidas dos BYTES CRUS do `.ndx`.
///
/// Nao usa `NdxFile` de proposito: o que se mede e o que alguem consegue com o
/// arquivo copiado e nenhuma chave -- e como nenhuma parte do `.ndx` e
/// cifrada, nenhuma parte dele precisa do motor para ser lida.
fn pares_do_ndx(bytes: &[u8], indice: &str) -> Vec<(String, u64)> {
    let u16le = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]) as usize;
    let u32le = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
    let u64le = |o: usize| u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());

    let cab_len = u16le(10);
    let page_size = u32le(12);
    let qtd_indices = u32le(16);
    let qtd_paginas = u64le(20) as usize;

    // O diretorio de indices vive logo depois do cabecalho, em claro.
    let mut i = cab_len;
    let mut key_len = 0usize;
    for _ in 0..qtd_indices {
        let n = u16le(i);
        let nome = String::from_utf8_lossy(&bytes[i + 2..i + 2 + n]).to_string();
        let kl = u32le(i + 2 + n + 1);
        if nome == indice {
            key_len = kl;
        }
        i += 2 + n + 1 + 4 + 8 + 8;
    }
    if key_len == 0 {
        return Vec::new();
    }

    // Toda pagina de FOLHA, varrida na marra. O tipo esta no byte 0 da pagina.
    const TIPO_FOLHA: u8 = 1;
    let ck = key_len + 8; // chave do usuario + rowid em big-endian
    let mut pares = Vec::new();
    for p in 1..qtd_paginas {
        let ini = p * page_size;
        if ini + page_size > bytes.len() || bytes[ini] != TIPO_FOLHA {
            continue;
        }
        let qtd = u16::from_le_bytes([bytes[ini + 2], bytes[ini + 3]]) as usize;
        for e in 0..qtd {
            let o = ini + 32 + e * ck;
            if o + ck > bytes.len() {
                break;
            }
            let comp = &bytes[o..o + key_len];
            // `keyenc`: [presenca 0x01][bytes ordenaveis]. Para `Str` os bytes
            // ordenaveis sao o proprio texto, com zeros a direita.
            if comp[0] != 0x01 {
                continue;
            }
            let corpo: Vec<u8> = comp[1..].iter().copied().take_while(|b| *b != 0).collect();
            let Ok(texto) = String::from_utf8(corpo) else {
                continue;
            };
            let rowid = u64::from_be_bytes(bytes[o + key_len..o + ck].try_into().unwrap());
            pares.push((texto, rowid));
        }
    }
    pares
}

/// O mesmo esquema SEM indice sobre a coluna marcada.
///
/// E o controle da premissa 2, e ele e o que faz a medicao valer nos dois
/// sentidos: se o `.ndx` da tabela sem esse indice tambem entregasse os
/// valores, o vazamento nao seria do indice e a conclusao estaria errada.
fn esquema_sem_indice_no_nome(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("obs", ColumnType::Memo).com_dado_pessoal(DadoPessoal::Sensivel),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

struct Premissa2 {
    linhas: usize,
    pares: usize,
    valores_certos: usize,
    ndx_bytes: usize,
    reg_bytes: usize,
    reg_vaza: usize,
    memo_vaza: usize,
    /// O controle: ocorrencias do mesmo texto no `.ndx` de uma tabela igual,
    /// sem indice sobre a coluna marcada.
    ndx_sem_indice_vaza: usize,
    ndx_sem_indice_bytes: usize,
}

fn premissa_2(raiz: &Path) -> Premissa2 {
    let d = raiz.join("premissa2");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();

    cofre::desligar();
    cofre::definir(SENHA_A, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&d, esquema("clientes")).unwrap();
        for i in 1..=LINHAS {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    cofre::desligar();

    let ndx = bytes_com_extensao(&d, "ndx");
    let reg = bytes_com_extensao(&d, "reg");
    let memo = bytes_com_extensao(&d, "memo");
    let pares = pares_do_ndx(&ndx, "porNome");

    // Quantos dos pares batem com o que foi gravado de verdade.
    let certos = pares
        .iter()
        .filter(|(texto, rowid)| {
            *rowid >= 1 && *rowid <= LINHAS as u64 && *texto == format!("{SEGREDO} {rowid:04}")
        })
        .count();

    let conta = |palheiro: &[u8], agulha: &[u8]| {
        if agulha.is_empty() {
            return 0;
        }
        palheiro
            .windows(agulha.len())
            .filter(|j| *j == agulha)
            .count()
    };

    // O CONTROLE: a mesma tabela, o mesmo dado, sem o indice sobre `nome`.
    let dc = raiz.join("premissa2-controle");
    let _ = std::fs::remove_dir_all(&dc);
    std::fs::create_dir_all(&dc).unwrap();
    cofre::definir(SENHA_A, RAPIDO).unwrap();
    {
        let mut t = Table::criar(&dc, esquema_sem_indice_no_nome("clientes")).unwrap();
        for i in 1..=LINHAS {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    cofre::desligar();
    let ndx_c = bytes_com_extensao(&dc, "ndx");

    let r = Premissa2 {
        linhas: LINHAS as usize,
        pares: pares.len(),
        valores_certos: certos,
        ndx_bytes: ndx.len(),
        reg_bytes: reg.len(),
        reg_vaza: conta(&reg, SEGREDO.as_bytes()),
        memo_vaza: conta(&memo, b"anotacao confidencial"),
        ndx_sem_indice_vaza: conta(&ndx_c, SEGREDO.as_bytes()),
        ndx_sem_indice_bytes: ndx_c.len(),
    };
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&dc);
    r
}

// ---------------------------------------------------------------------------
// Premissa 3 -- a replica precisa da senha?
// ---------------------------------------------------------------------------

/// Um caso da premissa 3: o rotulo e o que aconteceu na replica.
struct Caso {
    rotulo: &'static str,
    inline_igual: bool,
    externo: String,
    /// Os dois `.reg` sortearam o mesmo sal? E a causa a medir, e nao a supor.
    sal_igual: bool,
}

/// Grava uma linha no `source`, tira a imagem dela, e aplica na `replica`.
///
/// E o caminho EXATO do `sincronizar_replicada` do servidor: ele le o evento
/// do `.log` do source e chama `aplicar_evento` na replica. Aqui os dois
/// servidores viram dois diretorios no mesmo processo, o que troca dois
/// conteineres por uma medicao deterministica -- e o que se mede e a CHAVE,
/// que e do processo e nao da rede.
fn um_caso(
    raiz: &Path,
    rotulo: &'static str,
    senha_source: &str,
    senha_replica: Option<&str>,
) -> Caso {
    let ds = raiz.join(format!("p3-{rotulo}-source"));
    let dr = raiz.join(format!("p3-{rotulo}-replica"));
    for d in [&ds, &dr] {
        let _ = std::fs::remove_dir_all(d);
        std::fs::create_dir_all(d).unwrap();
    }

    // O source grava com a senha dele.
    cofre::desligar();
    cofre::definir(senha_source, RAPIDO).unwrap();
    let mut fonte = Table::criar(&ds, esquema_bin("clientes")).unwrap();
    let rowid = fonte.inserir(&linha_bin(1)).unwrap();
    let imagem = fonte.imagem_da_linha_do_rowid(rowid).unwrap();
    drop(fonte);

    // A replica nasce com a senha DELA -- e o `.reg` dela sorteia o proprio
    // sal, que e o ponto: dois arquivos nunca dividem sal.
    cofre::desligar();
    match senha_replica {
        Some(s) => cofre::definir(s, RAPIDO).unwrap(),
        None => cofre::desligar(),
    }
    let mut replica = Table::criar(&dr, esquema_bin("clientes")).unwrap();
    let aplicado = replica.aplicar_evento(Operacao::Inclusao, rowid, &imagem);

    let inline_igual;
    let externo;
    match aplicado {
        Ok(_) => {
            let lida = replica.ler(rowid).unwrap().unwrap();
            inline_igual = lida[1] == linha_bin(1)[1];
            externo = match &lida[2] {
                Value::Bin(b) if *b == anexo(1) => "aplicou o conteudo certo".to_string(),
                Value::Bin(b) => format!(
                    "GRAVOU {} bytes que NAO sao o conteudo -- e o texto cifrado, sem erro nenhum",
                    b.len()
                ),
                outro => format!("aplicou {outro:?}"),
            };
        }
        Err(e) => {
            // A faixa inline viaja em claro dentro da imagem; se a aplicacao
            // parou, foi o externo. O texto do erro e o achado.
            inline_igual = true;
            externo = format!("recusou: {e}");
        }
    }
    drop(replica);
    // Os sais so se leem com os arquivos ainda no lugar.
    let sal_igual =
        senha_replica.is_some() && sal_do_reg(&ds, "clientes") == sal_do_reg(&dr, "clientes");
    cofre::desligar();
    for d in [&ds, &dr] {
        let _ = std::fs::remove_dir_all(d);
    }
    Caso {
        rotulo,
        inline_igual,
        externo,
        sal_igual,
    }
}

/// A imagem carrega a coluna marcada INLINE em claro? Le os bytes da imagem.
fn inline_em_claro_na_imagem(raiz: &Path) -> (bool, usize) {
    let d = raiz.join("p3-imagem");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    cofre::desligar();
    cofre::definir(SENHA_A, RAPIDO).unwrap();
    let mut t = Table::criar(&d, esquema("clientes")).unwrap();
    let rowid = t.inserir(&linha(1)).unwrap();
    let imagem = t.imagem_da_linha_do_rowid(rowid).unwrap();
    drop(t);
    cofre::desligar();
    let achou = imagem
        .windows(SEGREDO.len())
        .any(|j| j == SEGREDO.as_bytes());
    let _ = std::fs::remove_dir_all(&d);
    (achou, imagem.len())
}

fn main() {
    let raiz = base();
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();

    println!("As tres premissas da senha por tabela");
    println!("=====================================");
    println!();

    // -------------------------------------------------------- premissa 1
    let (erra, uma_senha, alternando) = premissa_1();
    println!("1. O cache de chaves derivadas com DUAS senhas");
    println!(
        "   mesma dupla (sal, iteracoes), senhas diferentes: {}",
        if erra {
            "MESMA chave -- o cache erra"
        } else {
            "chaves DIFERENTES -- o cache nao erra"
        }
    );
    println!("   10 pedidos sobre 2 tabelas, uma senha do servidor: {uma_senha} derivacoes");
    println!("   os mesmos 10 pedidos, senha por tabela via `definir`: {alternando} derivacoes");
    println!(
        "   a 298 ms por derivacao (SEGURANCA.md §11.4), isso e {:.1} s de PBKDF2 \
         contra {:.1} s",
        alternando as f64 * 0.298,
        uma_senha as f64 * 0.298
    );
    println!();

    // -------------------------------------------------------- premissa 2
    let p2 = premissa_2(&raiz);
    println!("2. Quanto do dado sai do `.ndx` sem a senha");
    println!(
        "   {} linhas gravadas com a cifra LIGADA; o `.ndx` tem {} bytes",
        p2.linhas, p2.ndx_bytes
    );
    println!(
        "   pares (valor, rowid) reconstruidos dos bytes crus do `.ndx`: {} de {} ({:.0}%)",
        p2.valores_certos,
        p2.linhas,
        100.0 * p2.valores_certos as f64 / p2.linhas as f64
    );
    println!("   entradas de folha lidas ao todo: {}", p2.pares);
    println!(
        "   o mesmo texto dentro do `.reg` ({} bytes): {} ocorrencias",
        p2.reg_bytes, p2.reg_vaza
    );
    println!(
        "   o texto do memo marcado dentro do `.memo`: {} ocorrencias",
        p2.memo_vaza
    );
    println!(
        "   CONTROLE -- a mesma tabela SEM indice sobre `nome`, `.ndx` de {} bytes: \
         {} ocorrencias do texto",
        p2.ndx_sem_indice_bytes, p2.ndx_sem_indice_vaza
    );
    println!();

    // -------------------------------------------------------- premissa 3
    let (inline_claro, imagem_bytes) = inline_em_claro_na_imagem(&raiz);
    println!("3. O que a replica precisa para aplicar a imagem");
    println!(
        "   a coluna marcada INLINE aparece em claro na imagem de {imagem_bytes} bytes: {}",
        if inline_claro { "SIM" } else { "nao" }
    );
    let casos = [
        um_caso(&raiz, "mesma-senha", SENHA_A, Some(SENHA_A)),
        um_caso(&raiz, "senha-diferente", SENHA_A, Some(SENHA_B)),
        um_caso(&raiz, "replica-sem-cifra", SENHA_A, None),
    ];
    for c in &casos {
        println!(
            "   {:<18} inline igual: {:<5} sal igual: {:<5} externo: {}",
            c.rotulo, c.inline_igual, c.sal_igual, c.externo
        );
    }
    println!();

    let _ = std::fs::remove_dir_all(&raiz);

    let json = format!(
        "{{\"premissa1\":{{\"cache_erra\":{erra},\"derivacoes_uma_senha\":{uma_senha},\
         \"derivacoes_alternando\":{alternando}}},\
         \"premissa2\":{{\"linhas\":{},\"pares_certos\":{},\"pares_lidos\":{},\
         \"ndx_bytes\":{},\"reg_bytes\":{},\"reg_ocorrencias\":{},\"memo_ocorrencias\":{},\
         \"controle_ndx_bytes\":{},\"controle_ndx_ocorrencias\":{}}},\
         \"premissa3\":{{\"inline_em_claro\":{inline_claro},\"imagem_bytes\":{imagem_bytes},\
         \"casos\":[{}]}}}}",
        p2.linhas,
        p2.valores_certos,
        p2.pares,
        p2.ndx_bytes,
        p2.reg_bytes,
        p2.reg_vaza,
        p2.memo_vaza,
        p2.ndx_sem_indice_bytes,
        p2.ndx_sem_indice_vaza,
        casos
            .iter()
            .map(|c| format!(
                "{{\"caso\":\"{}\",\"inline_igual\":{},\"sal_igual\":{},\"externo\":\"{}\"}}",
                c.rotulo,
                c.inline_igual,
                c.sal_igual,
                c.externo.replace('"', "'")
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("RESULTADO {json}");
}
