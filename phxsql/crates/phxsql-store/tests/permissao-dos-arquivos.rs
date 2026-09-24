//! A permissao dos arquivos do banco -- pedido 542.
//!
//! Medido com `stat` antes do conserto: na base viva o `.lgpd` era `600`, e o
//! `.reg`, o `.ndx`, o `.memo`, o `.bin`, o `.trash` e o `.reason` eram `644`
//! em diretorio `755`; na copia do backup, TUDO saia `644`. Decisao do papel
//! J (`docs/propostas/pesquisa-rodada-2026-09-24.md` §2): diretorio 0700 e
//! todo arquivo 0600, na base, na copia e na restauracao, qualquer que seja o
//! `umask` -- e a base que ja existe larga ganha um ALERTA, sem recusa e sem
//! `chmod` calado.
//!
//! # Por que cada prova roda num processo filho com `umask 022`
//!
//! Porque a pergunta e «nasce 0600 com o `umask` da instalacao?», e a
//! resposta depende do `umask` do processo -- que um `cargo test` herda de
//! quem o chamou. Um ambiente com `umask 077` faria o `File::create` de antes
//! nascer `0600` tambem, e o teste passaria com o defeito de pe: o que depende
//! do sistema operacional se prova contra o sistema operacional, e com a
//! premissa posta, nao suposta. O filho e este mesmo binario, reexecutado por
//! `sh -c 'umask 022 && exec ...'`, e ele confere a premissa antes de tudo:
//! um `std::fs::write` cru tem de sair `0644`.
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use phxsql_core::schema::IndiceDeTexto;
use phxsql_core::{Column, ColumnType, DadoPessoal, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::backup;
use phxsql_store::restaurar::Preparada;
use phxsql_store::table::Table;
use phxsql_store::Instancia;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const FILHO: &str = "PHX_PERMISSAO_UMASK_022";

/// `true` no processo PAI: ele rodou `nome` num filho com `umask 022`,
/// conferiu que passou, e o teste acaba aqui. `false` no filho, que segue
/// para o corpo da prova.
fn no_filho_com_umask_022(nome: &str) -> bool {
    if std::env::var_os(FILHO).is_some() {
        return false;
    }
    let eu = std::env::current_exe().expect("o binario deste teste");
    let saida = std::process::Command::new("sh")
        .arg("-c")
        .arg("umask 022 && exec \"$0\" --exact \"$1\" --nocapture --test-threads=1")
        .arg(&eu)
        .arg(nome)
        .env(FILHO, "1")
        .output()
        .expect("reexecutar o teste com umask 022");
    let texto = format!(
        "{}\n{}",
        String::from_utf8_lossy(&saida.stdout),
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(
        saida.status.success(),
        "a prova `{nome}` reprovou no filho com umask 022:\n{texto}"
    );
    // Um filtro que nao casasse nada tambem sairia 0: o filho tem de dizer
    // que rodou UM teste.
    assert!(
        texto.contains("1 passed"),
        "o filho nao rodou `{nome}` -- nome errado no filtro?\n{texto}"
    );
    true
}

/// A premissa, conferida no proprio processo: sem o motor, o arquivo nasce
/// legivel por outros. Se nao nascer, o `umask` nao e o da instalacao e a
/// prova passaria por engano.
fn conferir_premissa(d: &Path) {
    let sonda = d.join("sonda-do-umask");
    std::fs::write(&sonda, b"x").unwrap();
    let m = modo(&sonda);
    std::fs::remove_file(&sonda).unwrap();
    assert_eq!(
        m, 0o644,
        "premissa: com umask 022, um std::fs::write cru nasce 644 e nasceu {m:o}"
    );
}

fn modo(p: &Path) -> u32 {
    std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777
}

/// Todo arquivo e todo diretorio debaixo de `raiz` (ela inclusive), com o
/// modo de cada um.
fn tudo_debaixo(raiz: &Path) -> Vec<(PathBuf, bool, u32)> {
    let mut v = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(p) = pilha.pop() {
        let meta = std::fs::symlink_metadata(&p).unwrap();
        v.push((p.clone(), meta.is_dir(), meta.permissions().mode() & 0o777));
        if meta.is_dir() {
            for i in std::fs::read_dir(&p).unwrap().flatten() {
                pilha.push(i.path());
            }
        }
    }
    v.sort();
    v
}

/// Nenhum arquivo fora de 0600 e nenhum diretorio fora de 0700 -- e a lista
/// dos que sairam, para o recado dizer QUAIS.
fn so_do_dono(raiz: &Path) -> Vec<String> {
    tudo_debaixo(raiz)
        .into_iter()
        .filter(|(_, dir, m)| *m != if *dir { 0o700 } else { 0o600 })
        .map(|(p, _, m)| format!("{m:o} {}", p.display()))
        .collect()
}

fn extensoes(raiz: &Path) -> Vec<String> {
    let mut v: Vec<String> = tudo_debaixo(raiz)
        .into_iter()
        .filter(|(_, dir, _)| !dir)
        .filter_map(|(p, _, _)| p.extension().map(|e| e.to_string_lossy().into_owned()))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Todas as familias de arquivo de uma tabela: indice, indice de texto sobre
/// coluna com dado pessoal (o `.lgpd` nasce do acesso registrado), memo.
fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(60))
                .obrigatoria()
                .com_dado_pessoal(DadoPessoal::Pessoal),
            Column::new("obs", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_indices_de_texto(vec![IndiceDeTexto::new("porNome", 1)])
    .unwrap()
}

/// A raiz `dados` (que ainda nao existe) com o database `loja` e uma tabela
/// que tocou todas as familias -- inclusive o `.trash` e o `.reason` do
/// excluir e o `.lgpd` do acesso a dado pessoal.
fn base_cheia(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    let inst = Instancia::nova(&raiz).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let mut t = Table::criar(db.caminho(), esquema()).unwrap();
    for i in 1..=50 {
        t.inserir(&[
            Value::Int(i),
            Value::Str(format!("fulano {i}")),
            Value::Memo(format!("observacao longa {i} ").repeat(40)),
        ])
        .unwrap();
    }
    t.registrar_acesso(1, "id=1", 1).unwrap();
    t.excluir(2).unwrap();
    t.sincronizar().unwrap();
    raiz
}

/// **542: o banco nasce 0600 em diretorio 0700, com `umask 022`.**
#[test]
fn o_banco_nasce_0600_em_diretorio_0700() {
    if no_filho_com_umask_022("o_banco_nasce_0600_em_diretorio_0700") {
        return;
    }
    let d = DirTemp::novo("542-nasce");
    conferir_premissa(&d);
    let raiz = base_cheia(&d);
    let exts = extensoes(&raiz);
    // A prova nao pode passar por nao ter olhado: as familias que o 542
    // mediu em 644 tem de estar la.
    for e in ["reg", "ndx", "memo", "log", "trash", "fts", "lgpd", "json"] {
        assert!(
            exts.iter().any(|x| x == e),
            "a tabela de prova nao criou nenhum .{e} -- a prova nao olharia \
             para ele. Extensoes: {exts:?}"
        );
    }
    let fora = so_do_dono(&raiz);
    assert!(
        fora.is_empty(),
        "arquivos do banco legiveis por outros usuarios da maquina (pedido \
         542):\n{}",
        fora.join("\n")
    );
    assert!(phxsql_store::permissao::permissao_larga(&raiz).is_none());
}

/// **542: a copia do backup, o ZIP e a restauracao ficam 0600/0700.**
///
/// A copia era `File::create` (644, mesmo com a origem 600, medido), e a
/// restauracao, pelo mesmo `File::create`, devolveria o database 644.
#[test]
fn a_copia_do_backup_e_a_restauracao_ficam_so_do_dono() {
    if no_filho_com_umask_022("a_copia_do_backup_e_a_restauracao_ficam_so_do_dono") {
        return;
    }
    let d = DirTemp::novo("542-backup");
    conferir_premissa(&d);
    let raiz = base_cheia(&d);

    let destino = d.join("copias").join("hoje");
    let (r, caminhos) = backup::executar(&raiz, &destino, 1).unwrap();
    backup::sincronizar_copias(&caminhos).unwrap();
    backup::finalizar_manifesto(&destino, 1, &r).unwrap();
    assert!(
        caminhos.len() > 5,
        "o backup copiou {} arquivos",
        caminhos.len()
    );
    let fora = so_do_dono(&destino);
    assert!(
        fora.is_empty(),
        "a copia do backup saiu legivel por outros:\n{}",
        fora.join("\n")
    );

    let (zip, _) = backup::executar_zip(&raiz, &d.join("zips"), "loja", "ana", 1).unwrap();
    backup::finalizar_zip(&zip).unwrap();
    let fora = so_do_dono(&d.join("zips"));
    assert!(
        fora.is_empty(),
        "o zip do backup saiu legivel por outros:\n{}",
        fora.join("\n")
    );

    let p = Preparada::preparar(&zip, &raiz, "").unwrap();
    p.confirmar(&raiz, "loja_restaurada", false).unwrap();
    let restaurada = raiz.join("loja_restaurada");
    assert!(restaurada.join("clientes.reg").is_file());
    let fora = so_do_dono(&restaurada);
    assert!(
        fora.is_empty(),
        "o database restaurado voltou legivel por outros:\n{}",
        fora.join("\n")
    );
}

/// **542: o que o banco REFAZ nasce 0600 -- mesmo por cima de um `0644`.**
///
/// O `mode` do `open` so vale para o arquivo que nasce; o `reindexar` trunca
/// o `.ndx` e o `.fts` no mesmo caminho, e sem o `fchmod` de
/// `recriar_do_banco` o arquivo refeito numa base antiga herdaria o `0644` --
/// com a arvore inteira nova dentro. O `.reg`, que o `reindexar` so le, fica
/// como estava: o que o banco nao refaz, ele nao aperta.
#[test]
fn o_que_o_banco_refaz_nasce_0600_mesmo_por_cima_de_0644() {
    if no_filho_com_umask_022("o_que_o_banco_refaz_nasce_0600_mesmo_por_cima_de_0644") {
        return;
    }
    let d = DirTemp::novo("542-refaz");
    conferir_premissa(&d);
    let raiz = base_cheia(&d);
    let db = raiz.join("loja");
    for (p, dir, _) in tudo_debaixo(&raiz) {
        let m = if dir { 0o755 } else { 0o644 };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m)).unwrap();
    }
    let mut t = Table::abrir(&db, "clientes").unwrap();
    t.reindexar().unwrap();
    drop(t);
    for arq in ["clientes.ndx", "clientes.fts"] {
        assert_eq!(
            modo(&db.join(arq)),
            0o600,
            "{arq}: refeito pelo reindexar, herdou o modo do arquivo antigo"
        );
    }
    assert_eq!(
        modo(&db.join("clientes.reg")),
        0o644,
        "o reindexar apertou o .reg, que ele so le"
    );
}

/// **542: a base ANTIGA, larga, abre, grava e alerta -- sem `chmod` calado.**
///
/// E o comportamento velho que a guarda nova nao pode quebrar: apertar
/// sozinho tiraria o acesso de quem hoje le por grupo. E o outro lado do
/// laco: a base fechada nao alerta, senao o alerta vira ruido que ninguem le.
#[test]
fn a_base_antiga_aberta_abre_grava_e_alerta() {
    if no_filho_com_umask_022("a_base_antiga_aberta_abre_grava_e_alerta") {
        return;
    }
    let d = DirTemp::novo("542-antiga");
    conferir_premissa(&d);
    let raiz = base_cheia(&d);
    let db = raiz.join("loja");
    assert!(phxsql_store::permissao::permissao_larga(&raiz).is_none());

    // A base de antes do 542: 644 em 755, como o `stat` mediu.
    for (p, dir, _) in tudo_debaixo(&raiz) {
        let m = if dir { 0o755 } else { 0o644 };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m)).unwrap();
    }
    let aviso =
        phxsql_store::permissao::permissao_larga(&raiz).expect("a base 644/755 tinha de alertar");
    assert!(aviso.contains("chmod -R go-rwx"), "{aviso}");
    assert!(aviso.contains(&raiz.display().to_string()), "{aviso}");

    let mut t = Table::abrir(&db, "clientes").unwrap();
    t.inserir(&[
        Value::Int(900),
        Value::Str("depois do 542".into()),
        Value::Null,
    ])
    .unwrap();
    t.sincronizar().unwrap();
    drop(t);
    for arq in ["clientes.reg", "clientes.ndx", "clientes.log"] {
        assert_eq!(
            modo(&db.join(arq)),
            0o644,
            "{arq}: a abertura apertou calada um arquivo da base antiga"
        );
    }
    assert_eq!(modo(&db), 0o755, "o diretorio da base antiga foi apertado");

    // Fechada pelo operador, o alerta some.
    for (p, dir, _) in tudo_debaixo(&raiz) {
        let m = if dir { 0o700 } else { 0o600 };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m)).unwrap();
    }
    assert_eq!(phxsql_store::permissao::permissao_larga(&raiz), None);
}

/// Uma vitima FORA do banco, 0644 e com conteudo proprio -- para conferir
/// depois que nada dela mudou.
fn vitima(d: &Path) -> (PathBuf, Vec<u8>) {
    let v = d.join("vitima-fora-do-banco.txt");
    std::fs::write(&v, b"conteudo que so o dono dele escreve\n").unwrap();
    std::fs::set_permissions(&v, std::fs::Permissions::from_mode(0o644)).unwrap();
    let conteudo = std::fs::read(&v).unwrap();
    (v, conteudo)
}

/// Os modos de todo nome debaixo de `raiz` que NAO e link: o link tem modo
/// 777 dele, e mexer nele mexeria no alvo.
fn dar_modo_sem_links(raiz: &Path, dir: u32, arq: u32) {
    for (p, e_dir, _) in tudo_debaixo(raiz) {
        if std::fs::symlink_metadata(&p)
            .unwrap()
            .file_type()
            .is_symlink()
        {
            continue;
        }
        let m = if e_dir { dir } else { arq };
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(m)).unwrap();
    }
}

/// **Revisao SEC do 542, achado 1: o backup NAO atravessa o link plantado no
/// destino.**
///
/// O destino do backup e lugar onde outros escrevem (disco de rede, USB). Com
/// `dest/loja/clientes.reg -> vitima` plantado entre duas corridas, o motor de
/// antes gravava o `.reg` NA vitima e a apertava para 0600 (medido pela SEC
/// contra o `phxsqld`: `0o644 -> 0o600`, conteudo `PHXREG`). Agora a corrida
/// recusa dizendo o nome, e a vitima fica com o conteudo e o modo dela. E o
/// comportamento velho que tem de seguir: a corrida por cima de um destino
/// POVOADO de arquivos regulares passa, e o refeito sai 0600.
#[test]
fn o_backup_nao_atravessa_o_link_plantado_no_destino() {
    let d = DirTemp::novo("542-sec-link-no-backup");
    let raiz = base_cheia(&d);
    let destino = d.join("copias");
    for vez in 1..=2 {
        let (r, c) = backup::executar(&raiz, &destino, vez).unwrap();
        backup::sincronizar_copias(&c).unwrap();
        backup::finalizar_manifesto(&destino, vez, &r).unwrap();
    }
    let plantado = destino.join("loja").join("clientes.reg");
    assert_eq!(modo(&plantado), 0o600, "o reuso por cima do povoado");

    let (v, conteudo) = vitima(&d);
    std::fs::remove_file(&plantado).unwrap();
    std::os::unix::fs::symlink(&v, &plantado).unwrap();

    let corrida = backup::executar(&raiz, &destino, 3).map(|_| ());
    assert_eq!(
        std::fs::read(&v).unwrap(),
        conteudo,
        "o backup gravou ATRAVES do link plantado no destino: a vitima fora do \
         banco recebeu o .reg"
    );
    assert_eq!(
        modo(&v),
        0o644,
        "o backup apertou para 0600 um arquivo FORA do banco, pelo link plantado"
    );
    let e = corrida.expect_err("a corrida tinha de recusar o nome que e um link");
    assert!(e.to_string().contains("link simbolico"), "{e}");
    assert!(
        std::fs::symlink_metadata(&plantado)
            .unwrap()
            .file_type()
            .is_symlink(),
        "o motor apagou o que estava no nome em vez de recusar"
    );
}

/// O irmao no motor, sem o backup em volta: `copiar_do_banco` -- o que a
/// restauracao usa quando o `rename` nao serve, e o que duplica tabela -- e
/// `escrever_do_banco`, com o link no destino e com o link PENDURADO (que o
/// `O_CREAT` seguiria, e criaria o alvo do outro lado). E o caso comum, que
/// continua: nome livre nasce, arquivo regular se refaz.
#[test]
fn o_motor_nao_atravessa_link_nem_pendurado() {
    let d = DirTemp::novo("542-sec-link-no-motor");
    let (v, conteudo) = vitima(&d);
    let origem = d.join("origem.reg");
    std::fs::write(&origem, b"PHXREG\0\0 o dado").unwrap();

    let no_link = d.join("copia.reg");
    std::os::unix::fs::symlink(&v, &no_link).unwrap();
    assert!(phxsql_store::permissao::copiar_do_banco(&origem, &no_link).is_err());
    assert!(phxsql_store::permissao::escrever_do_banco(&no_link, b"x").is_err());
    assert_eq!(
        std::fs::read(&v).unwrap(),
        conteudo,
        "a vitima mudou de conteudo"
    );
    assert_eq!(modo(&v), 0o644, "a vitima mudou de modo");

    let alvo_novo = d.join("nao-existia.txt");
    let pendurado = d.join("pendurado.reg");
    std::os::unix::fs::symlink(&alvo_novo, &pendurado).unwrap();
    assert!(phxsql_store::permissao::escrever_do_banco(&pendurado, b"x").is_err());
    assert!(
        !alvo_novo.exists(),
        "o link pendurado fez o motor CRIAR um arquivo fora do banco"
    );

    let livre = d.join("livre.reg");
    phxsql_store::permissao::copiar_do_banco(&origem, &livre).unwrap();
    std::fs::set_permissions(&livre, std::fs::Permissions::from_mode(0o644)).unwrap();
    phxsql_store::permissao::copiar_do_banco(&origem, &livre).unwrap();
    assert_eq!(
        std::fs::read(&livre).unwrap(),
        std::fs::read(&origem).unwrap()
    );
    assert_eq!(modo(&livre), 0o600, "o regular refeito nao saiu 0600");
}

/// **O que nao e arquivo regular recusa sem ABRIR -- uma FIFO plantada.**
///
/// O `open` de escrita numa FIFO fica parado esperando um leitor, e o backup
/// roda com a trava de dados na mao: uma FIFO no destino pararia o servidor
/// inteiro. O `lstat` antes de abrir (o passo 2 do motor) recusa na hora. A
/// chamada roda numa thread com prazo, para o defeito reposto reprovar em 5 s
/// em vez de pendurar a bateria.
#[test]
fn o_motor_recusa_fifo_sem_ficar_parado() {
    let d = DirTemp::novo("542-sec-fifo");
    let fifo = d.join("clientes.reg");
    let criada = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !criada {
        eprintln!("sem `mkfifo` nesta maquina -- prova pulada");
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let alvo = fifo.clone();
    std::thread::spawn(move || {
        let _ = tx.send(phxsql_store::permissao::escrever_do_banco(&alvo, b"x").is_err());
    });
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(recusou) => assert!(recusou, "o motor gravou numa FIFO"),
        Err(_) => panic!(
            "o motor ficou parado 5 s abrindo uma FIFO plantada no nome: com a \
             trava de dados na mao, e o servidor inteiro parado"
        ),
    }
}

/// **Revisao SEC do 542, achado 2: a base alcancada por um LINK tambem
/// alerta.**
///
/// `config.base` apontando para um link e a instalacao comum
/// (`/var/lib/phxsql -> /mnt/dados`), e o `lstat` do topo calava o alerta
/// inteiro (medido pela SEC contra o `phxsqld`: 1 alerta pelo caminho real, 0
/// pelo link). Os dois sentidos: a base larga alerta pelo link como alerta
/// pelo caminho real; a base fechada nao alerta por nenhum dos dois -- nem
/// com um link DENTRO dela apontando para um 0644 de fora, que continua sem
/// contar: o alvo dele nao e do banco.
#[test]
fn a_base_por_link_simbolico_tambem_alerta() {
    let d = DirTemp::novo("542-sec-base-por-link");
    let raiz = base_cheia(&d);
    let por_link = d.join("base-pelo-link");
    std::os::unix::fs::symlink(&raiz, &por_link).unwrap();
    let (v, _) = vitima(&d);
    std::os::unix::fs::symlink(&v, raiz.join("loja").join("link-para-fora")).unwrap();

    dar_modo_sem_links(&raiz, 0o700, 0o600);
    assert_eq!(phxsql_store::permissao::permissao_larga(&raiz), None);
    assert_eq!(
        phxsql_store::permissao::permissao_larga(&por_link),
        None,
        "a base fechada alertou pelo link -- o link DENTRO da arvore, para um \
         0644 de fora, contou"
    );

    dar_modo_sem_links(&raiz, 0o755, 0o644);
    assert!(
        phxsql_store::permissao::permissao_larga(&raiz).is_some(),
        "premissa: pelo caminho real a base 644/755 alerta"
    );
    assert!(
        phxsql_store::permissao::permissao_larga(&por_link).is_some(),
        "a mesma base 644/755, alcancada por um link simbolico, nao alertou"
    );
}
