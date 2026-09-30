//! O backup so escreve no que e dele, no destino -- pedidos 568, 569 e 570.
//!
//! O destino do backup e lugar onde outros escrevem (disco de rede, USB,
//! pasta aberta). Os tres achados da re-checagem SEC do 542 sao tres jeitos
//! de um terceiro fazer o backup escrever onde nao devia, e cada prova aqui
//! monta o jeito CONTRA O SISTEMA OPERACIONAL -- link de verdade, `chown` de
//! verdade, FIFO de verdade --, porque a pergunta e o que o nucleo faz com o
//! `open`, e isso nao se simula.
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use phxsql_store::backup;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Uma raiz com dois databases, cada um com o seu `c.reg` de conteudo
/// proprio -- o `rh` e o database VIVO que o link do 568 alcanca.
fn raiz_com_dois(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    for (db, conteudo) in [
        ("loja", "PHXREG loja: 1 registro"),
        ("rh", "PHXREG rh: 2 registros"),
    ] {
        std::fs::create_dir_all(raiz.join(db)).unwrap();
        std::fs::write(raiz.join(db).join("c.reg"), conteudo).unwrap();
    }
    raiz
}

/// **Pedido 568: link simbolico numa pasta do MEIO do destino.**
///
/// Com `copias/loja -> dados/rh`, o `create_dir_all` aceitava o link como
/// pasta que ja existe e o `c.reg` da `loja` caia POR CIMA do `c.reg` vivo do
/// `rh` (medido pela SEC: o `varrer` de `rh` caiu de 2 para 1 registro). O
/// conserto do 542 so olhava o ULTIMO nome. Agora a corrida recusa dizendo o
/// nome, o `rh` fica intacto e o link fica onde estava.
#[test]
fn o_backup_nao_atravessa_link_numa_pasta_do_meio() {
    let d = DirTemp::novo("568-link-no-meio");
    let raiz = raiz_com_dois(&d);
    let vivo = raiz.join("rh").join("c.reg");
    let antes = std::fs::read(&vivo).unwrap();
    let destino = d.join("copias");
    std::fs::create_dir_all(&destino).unwrap();
    std::os::unix::fs::symlink(raiz.join("rh"), destino.join("loja")).unwrap();

    let corrida = backup::executar(&raiz, &destino, 1).map(|_| ());
    assert_eq!(
        std::fs::read(&vivo).unwrap(),
        antes,
        "o backup gravou ATRAVES do link na pasta do meio: o c.reg vivo do \
         database rh virou a copia do c.reg da loja"
    );
    let e = corrida.expect_err("a corrida tinha de recusar a pasta que e um link");
    assert!(e.to_string().contains("link simbolico"), "{e}");
    assert!(
        e.to_string()
            .contains(&*destino.join("loja").to_string_lossy()),
        "a recusa tem de nomear o caminho real, nao o /proc: {e}"
    );
    assert!(
        std::fs::symlink_metadata(destino.join("loja"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "o backup apagou o que estava no nome em vez de recusar"
    );

    // O comportamento velho que tem de seguir: pasta de verdade no meio,
    // reaproveitada de uma corrida anterior, passa.
    std::fs::remove_file(destino.join("loja")).unwrap();
    for vez in 2..=3 {
        let (r, c) = backup::executar(&raiz, &destino, vez).unwrap();
        backup::concluir(&destino, vez, &r, &c).unwrap();
    }
    assert!(backup::conferir(&destino).unwrap().ok());
}

/// Planta `nome` como arquivo de OUTRO dono (uid 1234, modo 0666), com
/// conteudo do terceiro, e devolve o descritor aberto nele -- e por ele que a
/// prova le o inode do terceiro depois, mesmo que o nome tenha mudado.
/// `None` se o `chown` nao e permitido (a suite nao roda como root).
fn plantar_de_outro_dono(nome: &Path) -> Option<std::fs::File> {
    std::fs::write(nome, b"do terceiro").unwrap();
    std::fs::set_permissions(nome, std::os::unix::fs::PermissionsExt::from_mode(0o666)).unwrap();
    if std::os::unix::fs::chown(nome, Some(1234), Some(1234)).is_err() {
        eprintln!("sem permissao de chown (a suite nao roda como root) -- parte pulada");
        return None;
    }
    Some(std::fs::File::open(nome).unwrap())
}

fn conteudo(mut f: &std::fs::File) -> Vec<u8> {
    use std::io::Seek;
    f.rewind().unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).unwrap();
    v
}

/// O uid com que este processo cria arquivo: o dono de um que ele criou.
fn meu_uid(d: &Path) -> u32 {
    let p = d.join("sonda-do-uid");
    std::fs::write(&p, b"").unwrap();
    std::fs::metadata(&p).unwrap().uid()
}

/// **Pedido 569: arquivo regular de OUTRO dono ja no destino -- na pasta, no
/// ZIP e pelo link fisico.**
///
/// Sem link nenhum: o motor do 542 truncava e reescrevia o inode que estava
/// no nome, e o terceiro que o plantou ficava dono de uma copia do banco
/// (medido pela SEC: uid 1234, modo 0600, conteudo do `c.reg`). No ZIP -- o
/// padrao do backup agendado -- bastava plantar o `.part` do minuto. O
/// conserto nao recusa: o nome alheio sai, e o nosso nasce. As provas leem o
/// inode do terceiro pelo descritor que o plantou: tem de continuar com o
/// conteudo dele, e o nome tem de ser nosso.
///
/// A parte do dono pede `chown` e roda porque a suite roda como root neste
/// conteiner; a do link fisico (`nlink > 1`) nao pede nada e roda sempre.
#[test]
fn o_backup_nao_escreve_no_arquivo_de_outro_dono() {
    let d = DirTemp::novo("569-outro-dono");
    let raiz = raiz_com_dois(&d);
    let eu = meu_uid(&d);

    // Na PASTA.
    let destino = d.join("copias");
    std::fs::create_dir_all(destino.join("loja")).unwrap();
    let plantado = destino.join("loja").join("c.reg");
    if let Some(terceiro) = plantar_de_outro_dono(&plantado) {
        let (r, c) = backup::executar(&raiz, &destino, 1).unwrap();
        backup::concluir(&destino, 1, &r, &c).unwrap();
        assert_eq!(
            conteudo(&terceiro),
            b"do terceiro",
            "o backup escreveu a copia do banco DENTRO do arquivo do terceiro"
        );
        let m = std::fs::metadata(&plantado).unwrap();
        assert_eq!(m.uid(), eu, "a copia ficou com o dono de quem a plantou");
        assert_eq!(m.mode() & 0o777, 0o600);
        assert!(backup::conferir(&destino).unwrap().ok());
    }

    // No ZIP: o `.part` do minuto, plantado antes.
    let pasta = d.join("zips");
    std::fs::create_dir_all(&pasta).unwrap();
    let quando = 1_787_000_000_000;
    let nome = backup::nome_do_zip("loja", "ana", quando);
    let part = pasta.join(format!("{nome}.part"));
    if let Some(terceiro) = plantar_de_outro_dono(&part) {
        let (zip, _) = backup::executar_zip(&raiz, &pasta, "loja", "ana", quando).unwrap();
        backup::finalizar_zip(&zip).unwrap();
        assert_eq!(
            conteudo(&terceiro),
            b"do terceiro",
            "o backup escreveu o ZIP DENTRO do .part do terceiro"
        );
        let m = std::fs::metadata(pasta.join(&nome)).unwrap();
        assert_eq!(
            m.uid(),
            eu,
            "o .zip final e o arquivo que o terceiro plantou"
        );
        assert_eq!(m.mode() & 0o777, 0o600);
    }

    // Pelo LINK FISICO: o nome no destino e outro nome de um arquivo de
    // fora. Nao pede root.
    let destino2 = d.join("copias-2");
    std::fs::create_dir_all(destino2.join("loja")).unwrap();
    let de_fora = d.join("arquivo-de-fora.txt");
    std::fs::write(&de_fora, b"conteudo de fora").unwrap();
    std::fs::hard_link(&de_fora, destino2.join("loja").join("c.reg")).unwrap();
    let (r, c) = backup::executar(&raiz, &destino2, 2).unwrap();
    backup::concluir(&destino2, 2, &r, &c).unwrap();
    assert_eq!(
        std::fs::read(&de_fora).unwrap(),
        b"conteudo de fora",
        "o backup escreveu pelo link fisico: o arquivo de fora virou a copia"
    );
    assert!(backup::conferir(&destino2).unwrap().ok());

    // O reuso do que ja e nosso segue no MESMO inode (truncado, nao trocado).
    let nosso = destino2.join("loja").join("c.reg");
    let ino = std::fs::metadata(&nosso).unwrap().ino();
    let (r, c) = backup::executar(&raiz, &destino2, 3).unwrap();
    backup::concluir(&destino2, 3, &r, &c).unwrap();
    assert_eq!(std::fs::metadata(&nosso).unwrap().ino(), ino);
}

/// **Pedido 570: o nome trocado por um link para FIFO entre o `lstat` e o
/// `open`.**
///
/// A janela e de microssegundos, e o juiz a mediu 1 vez em 12 corridas. A
/// prova a alarga: uma thread troca o nome sem parar, por `rename` atomico,
/// entre um arquivo regular e um link para uma FIFO sem leitor, enquanto a
/// outra refaz o arquivo pelo motor centenas de vezes. Com o `open` que
/// segue link e espera leitor, uma das voltas cai na FIFO e PARA -- com a
/// trava de dados na mao, o servidor inteiro parado. O prazo de 20 s
/// transforma a parada em reprovacao, e o leitor aberto no fim solta a
/// thread presa para a suite nao ficar pendurada.
/// Voltas de cada lado da prova do 570: o bastante para a janela aparecer
/// (com o defeito reposto a guarda cai logo), sem martelar o disco.
const TETO_DE_VOLTAS: u32 = 20_000;

#[test]
fn a_fifo_trocada_na_janela_nao_para_o_motor() {
    let d = DirTemp::novo("570-fifo-na-janela");
    let fifo = d.join("fifo");
    let criada = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !criada {
        eprintln!("sem `mkfifo` nesta maquina -- prova pulada");
        return;
    }
    let nome = d.join("c.reg");
    std::fs::write(&nome, b"x").unwrap();
    let parar = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let trocador = {
        let (d, nome, fifo, parar) = (d.to_path_buf(), nome.clone(), fifo.clone(), parar.clone());
        std::thread::spawn(move || {
            let (regular, link) = (d.join("regular.tmp"), d.join("link.tmp"));
            // Teto de voltas: sem ele o laco fazia ~500 mil trocas em 4 s, e
            // duas corridas da suite inteira coincidiram com o conteiner
            // reiniciando (30/09/2026, nao medido que foi ele). A janela cai
            // nas primeiras voltas com o defeito reposto; o teto nao a fecha.
            let mut n = 0u32;
            while !parar.load(std::sync::atomic::Ordering::Relaxed) && n < TETO_DE_VOLTAS {
                n += 1;
                let _ = std::fs::write(&regular, b"x");
                let _ = std::fs::rename(&regular, &nome);
                let _ = std::fs::remove_file(&link);
                let _ = std::os::unix::fs::symlink(&fifo, &link);
                let _ = std::fs::rename(&link, &nome);
            }
        })
    };

    let (tx, rx) = std::sync::mpsc::channel();
    {
        let nome = nome.clone();
        std::thread::spawn(move || {
            let inicio = std::time::Instant::now();
            let (mut voltas, mut recusas) = (0u32, 0u32);
            while inicio.elapsed() < Duration::from_secs(4) && voltas < TETO_DE_VOLTAS {
                voltas += 1;
                if phxsql_store::permissao::escrever_do_banco(&nome, b"dado do banco").is_err() {
                    recusas += 1;
                }
            }
            let _ = tx.send((voltas, recusas));
        });
    }
    let medido = rx.recv_timeout(Duration::from_secs(20));
    parar.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = trocador.join();
    if medido.is_err() {
        // Solta a thread presa no `open` da FIFO: abrir para ler a destrava.
        let _ = std::fs::File::open(&fifo);
    }
    let (voltas, recusas) = medido.expect(
        "o motor ficou PARADO abrindo a FIFO trocada no nome entre o lstat e o \
         open: com a trava de dados na mao, o servidor inteiro parado",
    );
    eprintln!("570: {voltas} voltas, {recusas} recusas na janela");
    assert!(
        voltas > 100,
        "a prova rodou poucas voltas para alcancar a janela: {voltas}"
    );
}

/// Uma raiz com UMA pasta a mais de fundura (`loja/sub/c.reg`), para a
/// corrida criar duas pastas encaixadas no destino -- a de fora e a de dentro.
fn raiz_funda(d: &Path) -> PathBuf {
    let raiz = d.join("dados");
    std::fs::create_dir_all(raiz.join("loja/sub")).unwrap();
    std::fs::write(raiz.join("loja/sub/c.reg"), b"PHXREG loja").unwrap();
    raiz
}

/// Um destino que JA existe com um diretorio no nome `backup.json`: o
/// manifesto recusa de verdade no fim, e a faxina do 576 roda.
fn destino_que_recusa_o_manifesto(d: &Path) -> PathBuf {
    let destino = d.join("copias");
    std::fs::create_dir_all(destino.join(backup::MANIFESTO)).unwrap();
    std::fs::write(destino.join(backup::MANIFESTO).join("dentro"), b"alheio").unwrap();
    destino
}

/// **Pedido 593: o `fsync` da pasta cai no descritor que a corrida abriu, e
/// nao no que estiver no nome.**
///
/// Entre a escrita (sob a trava) e o `fsync` (fora dela), a pasta `loja` do
/// destino sai do lugar e um link entra no nome. Pelo nome, o `open` seguia o
/// link: para a pasta de outro, sincronizava a do outro e respondia Ok sobre
/// a nossa, que nunca sincronizava. Aqui o link aponta para o nada, que e o
/// jeito de o `open` pelo nome APARECER sem `strace` -- recusa com `ENOENT`,
/// e o backup inteiro falhava. Pelo descritor, o nome nem se resolve.
#[test]
fn o_fsync_da_pasta_nao_segue_o_link_posto_no_lugar() {
    let d = DirTemp::novo("593-fsync-pelo-descritor");
    let raiz = raiz_com_dois(&d);
    let destino = d.join("copias");
    let (r, c) = backup::executar(&raiz, &destino, 1).unwrap();
    std::fs::rename(destino.join("loja"), d.join("loja-movida")).unwrap();
    std::os::unix::fs::symlink(d.join("nao-existe"), destino.join("loja")).unwrap();

    let feito = backup::concluir(&destino, 1, &r, &c);
    assert!(
        feito.is_ok(),
        "o fsync da pasta abriu o NOME, e seguiu o link posto no lugar da \
         pasta que a corrida escreveu: {feito:?}"
    );
    assert!(
        std::fs::symlink_metadata(destino.join("loja"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "o backup mexeu no link em vez de ignora-lo"
    );
}

/// **Pedido 593: a faxina nao atravessa um link posto numa pasta do meio.**
///
/// A corrida cria `copias/loja/sub` e o manifesto recusa. Antes da faxina,
/// `copias/loja` sai do lugar e vira um link para a pasta de outro, que tem
/// uma `sub` vazia. Pelo nome real, `remove_dir("copias/loja/sub")` seguia o
/// link do meio e apagava a `sub` do outro. Pelo descritor da mae, a faxina
/// remove a NOSSA `sub` -- a que foi movida junto -- e a do outro fica.
#[test]
fn a_faxina_nao_atravessa_link_na_pasta_do_meio() {
    let d = DirTemp::novo("593-faxina-link-no-meio");
    let raiz = raiz_funda(&d);
    let destino = destino_que_recusa_o_manifesto(&d);
    let (r, c) = backup::executar(&raiz, &destino, 1).unwrap();
    let alheia = d.join("alheia");
    std::fs::create_dir_all(alheia.join("sub")).unwrap();
    std::fs::rename(destino.join("loja"), d.join("loja-movida")).unwrap();
    std::os::unix::fs::symlink(&alheia, destino.join("loja")).unwrap();

    backup::concluir(&destino, 1, &r, &c).expect_err("a premissa: o manifesto recusa");
    assert!(
        alheia.join("sub").is_dir(),
        "a faxina seguiu o link da pasta do meio e apagou a pasta VAZIA de \
         outro do lado de la"
    );
    assert!(
        std::fs::symlink_metadata(d.join("loja-movida/sub")).is_err(),
        "a pasta que a corrida criou nao saiu pelo descritor da mae"
    );
}

/// **Pedido 593: a faxina confere que a pasta no nome e a que nasceu.**
///
/// Sem link nenhum: a nossa `sub` sai do lugar e uma pasta vazia de outro
/// entra no nome dela por `rename`. Pelo nome, o `remove_dir` apagava a do
/// outro (vazia, o `rmdir` aceita). Com o dev/inode anotado ao nascer, a do
/// outro fica.
#[test]
fn a_faxina_nao_remove_a_pasta_vazia_trocada_no_nome() {
    let d = DirTemp::novo("593-faxina-troca-no-nome");
    let raiz = raiz_funda(&d);
    let destino = destino_que_recusa_o_manifesto(&d);
    let (r, c) = backup::executar(&raiz, &destino, 1).unwrap();
    std::fs::rename(destino.join("loja/sub"), d.join("sub-movida")).unwrap();
    std::fs::create_dir_all(d.join("de-outro")).unwrap();
    std::fs::rename(d.join("de-outro"), destino.join("loja/sub")).unwrap();

    backup::concluir(&destino, 1, &r, &c).expect_err("a premissa: o manifesto recusa");
    assert!(
        destino.join("loja/sub").is_dir(),
        "a faxina removeu a pasta vazia de outro que entrou no nome da nossa"
    );
}
