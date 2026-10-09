//! O `.tar` do PhxZip (fatia Z4): ida e volta, prova cruzada com o `tar` do
//! sistema nos dois sentidos, e a extracao que nao segue link nem sai da raiz.
//!
//! A prova cruzada roda quando ha `tar` no `PATH`. Sem ele o teste diz que
//! pulou -- e o relatorio da rodada tem de dizer o mesmo, porque um verde que
//! nao rodou a prova nao prova nada.

mod comum;

use std::path::Path;
use std::process::Command;

use comum::DirTemp;

use phxzip::tar::{self, EntradaTar, EscritorTar, TipoTar};
use phxzip::Erro;

/// 49 + `/` + 100 = 150 bytes: cabe no ustar partido em `prefix` + `name`.
fn nome_150_partivel() -> String {
    format!("{}/{}", "d".repeat(49), "n".repeat(100))
}

/// 150 bytes sem barra: so o pax carrega.
fn nome_150_inteiro() -> String {
    "x".repeat(146) + ".txt"
}

const DATA: i64 = 1_790_000_000;

fn ha_tar() -> bool {
    Command::new("tar")
        .arg("--version")
        .output()
        .is_ok_and(|s| s.status.success())
}

fn so<'a>(ents: &'a [EntradaTar<'a>], nome: &str) -> &'a EntradaTar<'a> {
    ents.iter()
        .find(|e| e.nome == nome)
        .unwrap_or_else(|| panic!("{nome} sumiu"))
}

/// Um arquivo com cada caso que o ustar sozinho nao alcanca.
fn arquivo_de_prova() -> Vec<u8> {
    let mut w = EscritorTar::novo();
    w.pasta("raiz", DATA).unwrap();
    w.arquivo("raiz/curto.txt", b"conteudo curto\n", DATA)
        .unwrap();
    w.arquivo(&nome_150_partivel(), b"partido", DATA).unwrap();
    w.arquivo(&nome_150_inteiro(), b"so no pax", DATA).unwrap();
    w.arquivo("raiz/ação.txt", "acentuado\n".as_bytes(), DATA)
        .unwrap();
    w.arquivo("raiz/bloco.bin", &[7u8; 512], DATA).unwrap();
    w.arquivo("raiz/vazio", b"", DATA).unwrap();
    w.terminar()
}

#[test]
fn nome_de_150_bytes_atravessa_intacto() {
    let bytes = arquivo_de_prova();
    assert_eq!(bytes.len() % 512, 0);
    let ents = tar::ler(&bytes).unwrap();
    assert_eq!(ents.len(), 7);
    let p = so(&ents, &nome_150_partivel());
    assert_eq!(p.nome.len(), 150);
    assert_eq!(p.conteudo, b"partido");
    let i = so(&ents, &nome_150_inteiro());
    assert_eq!(i.nome.len(), 150);
    assert_eq!(i.conteudo, b"so no pax");
    assert_eq!(so(&ents, "raiz").tipo, TipoTar::Pasta);
    assert_eq!(
        so(&ents, "raiz/ação.txt").conteudo,
        "acentuado\n".as_bytes()
    );
    assert_eq!(so(&ents, "raiz/bloco.bin").conteudo, &[7u8; 512][..]);
    assert_eq!(so(&ents, "raiz/vazio").conteudo, b"");
    assert!(ents.iter().all(|e| e.modificado == DATA));
}

/// O que escrevemos o `tar` do sistema le: nomes, conteudo e data.
#[test]
fn o_tar_do_sistema_le_o_que_gravamos() {
    if !ha_tar() {
        eprintln!("PULADO: sem `tar` no PATH");
        return;
    }
    let d = DirTemp::novo("tar-ida");
    let arq = d.join("nosso.tar");
    std::fs::write(&arq, arquivo_de_prova()).unwrap();
    let lista = Command::new("tar").arg("-tf").arg(&arq).output().unwrap();
    assert!(lista.status.success(), "{lista:?}");
    let lista = String::from_utf8(lista.stdout).unwrap();
    let nomes: Vec<&str> = lista.lines().collect();
    assert!(nomes.contains(&nome_150_partivel().as_str()), "{nomes:?}");
    assert!(nomes.contains(&nome_150_inteiro().as_str()), "{nomes:?}");
    // O `ação.txt` se confere extraido, logo abaixo: a LISTA do GNU tar
    // escapa o nao-ASCII conforme o locale, e isso e da tela dele.
    assert_eq!(nomes.len(), 7, "{nomes:?}");
    assert!(
        !nomes.iter().any(|n| n.contains("PaxHeaders")),
        "o tar extraiu o cabecalho pax como arquivo: {nomes:?}"
    );

    let fora = d.join("fora");
    std::fs::create_dir(&fora).unwrap();
    let x = Command::new("tar")
        .arg("-xf")
        .arg(&arq)
        .arg("-C")
        .arg(&fora)
        .output()
        .unwrap();
    assert!(x.status.success(), "{x:?}");
    assert_eq!(
        std::fs::read(fora.join(nome_150_partivel())).unwrap(),
        b"partido"
    );
    assert_eq!(
        std::fs::read(fora.join(nome_150_inteiro())).unwrap(),
        b"so no pax"
    );
    assert_eq!(
        std::fs::read(fora.join("raiz/ação.txt")).unwrap(),
        "acentuado\n".as_bytes()
    );
    let m = std::fs::metadata(fora.join("raiz/curto.txt")).unwrap();
    let s = m
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert_eq!(s as i64, DATA);
}

/// O que o `tar` do sistema grava nos lemos -- nos tres formatos dele: o da
/// norma (`pax`), o `ustar` puro e o `gnu` (o padrao do GNU tar, com o
/// `././@LongLink`).
#[test]
fn lemos_o_que_o_tar_do_sistema_grava() {
    if !ha_tar() {
        eprintln!("PULADO: sem `tar` no PATH");
        return;
    }
    let d = DirTemp::novo("tar-volta");
    let src = d.join("src");
    std::fs::create_dir_all(src.join("raiz")).unwrap();
    std::fs::create_dir_all(src.join("d".repeat(49))).unwrap();
    std::fs::write(src.join(nome_150_partivel()), b"partido").unwrap();
    std::fs::write(src.join(nome_150_inteiro()), b"so no pax").unwrap();
    std::fs::write(src.join("raiz").join("ação.txt"), b"acentuado\n").unwrap();
    std::fs::write(src.join("raiz").join("grande.bin"), vec![9u8; 5000]).unwrap();

    for (formato, ve_150_inteiro) in [("pax", true), ("gnu", true), ("ustar", false)] {
        let arq = d.join(format!("{formato}.tar"));
        let mut cmd = Command::new("tar");
        cmd.arg(format!("--format={formato}"))
            .arg("--mtime=@1790000000")
            .arg("-cf")
            .arg(&arq)
            .arg("-C")
            .arg(&src)
            .arg("raiz")
            .arg("d".repeat(49));
        if ve_150_inteiro {
            cmd.arg(nome_150_inteiro());
        }
        let s = cmd.output().unwrap();
        assert!(s.status.success(), "{formato}: {s:?}");
        let bytes = std::fs::read(&arq).unwrap();
        // Que o caminho que se quer provar foi mesmo o exercitado: sem isto,
        // um `tar` que partisse o nome no ustar faria o teste passar sem ler
        // pax nem LongLink nenhum.
        let tem = |agulha: &[u8]| bytes.windows(agulha.len()).any(|w| w == agulha);
        match formato {
            "gnu" => assert!(tem(b"././@LongLink"), "o gnu nao usou LongLink"),
            "pax" => assert!(tem(b"PaxHeaders"), "o pax nao usou cabecalho x"),
            _ => assert!(!tem(b"PaxHeaders") && !tem(b"@LongLink")),
        }
        let ents = tar::ler(&bytes).unwrap_or_else(|e| panic!("{formato}: {e}"));
        assert_eq!(
            so(&ents, &nome_150_partivel()).conteudo,
            b"partido",
            "{formato}"
        );
        if ve_150_inteiro {
            assert_eq!(
                so(&ents, &nome_150_inteiro()).conteudo,
                b"so no pax",
                "{formato}"
            );
        }
        assert_eq!(
            so(&ents, "raiz/ação.txt").conteudo,
            b"acentuado\n",
            "{formato}"
        );
        assert_eq!(
            so(&ents, "raiz/grande.bin").conteudo,
            &[9u8; 5000][..],
            "{formato}"
        );
        assert_eq!(so(&ents, "raiz").tipo, TipoTar::Pasta, "{formato}");
        assert!(ents.iter().all(|e| e.modificado == DATA), "{formato}");
    }
}

// ------------------------------------------------- extracao (zip-slip)

/// Monta um tar com um cabecalho ustar cru -- o nosso escritor recusa os
/// nomes que estes testes precisam, e e isso que se quer dele.
fn cabecalho_cru(nome: &str, tipo: u8, alvo: &str, dados: &[u8]) -> Vec<u8> {
    let mut h = [0u8; 512];
    h[..nome.len()].copy_from_slice(nome.as_bytes());
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{:011o}\0", dados.len()).as_bytes());
    h[136..148].copy_from_slice(b"00000000000\0");
    h[156] = tipo;
    h[157..157 + alvo.len()].copy_from_slice(alvo.as_bytes());
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let soma: u32 = h
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if (148..156).contains(&i) {
                32
            } else {
                b as u32
            }
        })
        .sum();
    h[148..156].copy_from_slice(format!("{soma:06o}\0 ").as_bytes());
    let mut v = h.to_vec();
    v.extend_from_slice(dados);
    v.resize(v.len().div_ceil(512) * 512, 0);
    v
}

fn fechar(partes: &[Vec<u8>]) -> Vec<u8> {
    let mut v: Vec<u8> = partes.concat();
    v.resize(v.len() + 1024, 0);
    v
}

/// Nada foi criado dentro de `p` (nem pasta).
fn vazia(p: &Path) -> bool {
    std::fs::read_dir(p).unwrap().next().is_none()
}

#[test]
fn extrai_dentro_da_raiz() {
    let d = DirTemp::novo("tar-extrai");
    let n = tar::extrair_em(&arquivo_de_prova(), &d).unwrap();
    assert_eq!(n, 7);
    assert_eq!(
        std::fs::read(d.join(nome_150_inteiro())).unwrap(),
        b"so no pax"
    );
    assert_eq!(
        std::fs::read(d.join("raiz/bloco.bin")).unwrap(),
        vec![7u8; 512]
    );
    assert!(d.join("raiz").is_dir());
}

/// `../` e caminho absoluto nao abrem o arquivo: nada e gravado, nem dentro
/// nem fora.
#[test]
fn nome_que_escapa_da_raiz_e_recusado() {
    let d = DirTemp::novo("tar-slip");
    let raiz = d.join("raiz");
    std::fs::create_dir(&raiz).unwrap();
    for ruim in [
        "../fora.txt",
        "a/../../fora.txt",
        "/tmp/fora.txt",
        "..\\fora.txt",
    ] {
        let t = fechar(&[
            cabecalho_cru("bom.txt", b'0', "", b"bom"),
            cabecalho_cru(ruim, b'0', "", b"mal"),
        ]);
        assert!(
            matches!(tar::ler(&t), Err(Erro::NomePerigoso(_))),
            "{ruim} abriu"
        );
        assert!(
            matches!(tar::extrair_em(&t, &raiz), Err(Erro::NomePerigoso(_))),
            "{ruim} extraiu"
        );
        assert!(vazia(&raiz), "{ruim}: algo foi gravado dentro");
        assert!(!d.join("fora.txt").exists(), "{ruim}: escapou");
    }
}

/// Um link no ARQUIVO e recusado antes do primeiro byte: o par classico
/// `link -> fora` seguido de `link/x` nao grava nada, nem o que vinha antes.
#[test]
fn link_no_arquivo_nunca_e_criado_nem_seguido() {
    let d = DirTemp::novo("tar-link");
    let raiz = d.join("raiz");
    let fora = d.join("fora");
    std::fs::create_dir(&raiz).unwrap();
    std::fs::create_dir(&fora).unwrap();
    let alvo = fora.to_str().unwrap();
    for tipo in [b'2', b'1'] {
        let t = fechar(&[
            cabecalho_cru("antes.txt", b'0', "", b"antes"),
            cabecalho_cru("link", tipo, alvo, b""),
            cabecalho_cru("link/x.txt", b'0', "", b"pelo link"),
        ]);
        // Listar mostra o link como informacao, com o alvo.
        let ents = tar::ler(&t).unwrap();
        assert_eq!(ents[1].alvo.as_deref(), Some(alvo));
        let r = tar::extrair_em(&t, &raiz);
        assert!(
            matches!(&r, Err(Erro::EntradaEspecial { nome, .. }) if nome == "link"),
            "{r:?}"
        );
        assert!(vazia(&raiz), "tipo {}: gravou meia extracao", tipo as char);
        assert!(vazia(&fora), "tipo {}: escreveu pelo link", tipo as char);
    }
}

/// Um link que JA esta no destino (plantado antes) nao e atravessado, nem
/// como pasta do caminho nem como o proprio arquivo.
#[cfg(unix)]
#[test]
fn link_ja_plantado_no_destino_nao_e_seguido() {
    let d = DirTemp::novo("tar-plantado");
    let raiz = d.join("raiz");
    let fora = d.join("fora");
    std::fs::create_dir(&raiz).unwrap();
    std::fs::create_dir(&fora).unwrap();
    std::os::unix::fs::symlink(&fora, raiz.join("sub")).unwrap();
    std::os::unix::fs::symlink(fora.join("alvo.txt"), raiz.join("arq.txt")).unwrap();

    let mut w = EscritorTar::novo();
    w.arquivo("sub/x.txt", b"pela pasta", DATA).unwrap();
    let t = w.terminar();
    let r = tar::extrair_em(&t, &raiz);
    assert!(matches!(r, Err(Erro::DestinoInseguro(_))), "{r:?}");

    let mut w = EscritorTar::novo();
    w.arquivo("arq.txt", b"pelo arquivo", DATA).unwrap();
    let t = w.terminar();
    let r = tar::extrair_em(&t, &raiz);
    assert!(matches!(r, Err(Erro::DestinoInseguro(_))), "{r:?}");

    assert!(vazia(&fora), "escreveu atraves do link plantado");
}

#[test]
fn soma_errada_e_dado_cortado_sao_recusados() {
    let mut t = arquivo_de_prova();
    assert!(tar::ler(&t).is_ok());
    // Um byte trocado no nome da primeira entrada.
    t[0] ^= 1;
    assert!(matches!(tar::ler(&t), Err(Erro::Corrompido(_))));
    t[0] ^= 1;
    // Todo corte antes do fim recusa: o tar nao «abre pela metade».
    let fim = t.len() - 1024;
    for corte in (0..fim).step_by(97) {
        assert!(tar::ler(&t[..corte]).is_err(), "o prefixo de {corte} abriu");
    }
}

// ------------------------------------------- revisao SEC da Z9 (09/10/2026)

/// `cfg.json` e `./cfg.json` (e `a/b` com `a//b` ou `a/./b`) sao o MESMO
/// arquivo no disco. Antes passavam como dois nomes, e o segundo
/// sobrescrevia o primeiro calado (`admin=false` virava `admin=true`).
///
/// Prova real: com o `conferir_nome` devolvendo o nome sem canonizar, o
/// `tar::ler` abre os pares e o teste cai.
#[test]
fn nome_repetido_por_ponto_ou_barra_dupla_e_recusado() {
    for (a, b) in [
        ("cfg.json", "./cfg.json"),
        ("cfg.json", ".//cfg.json"),
        ("a/b", "a//b"),
        ("a/b", "a/./b"),
    ] {
        let t = fechar(&[
            cabecalho_cru(a, b'0', "", b"admin=false\n"),
            cabecalho_cru(b, b'0', "", b"admin=true\n"),
        ]);
        let r = tar::ler(&t);
        assert!(
            matches!(&r, Err(Erro::NomeRepetido(_))),
            "{a:?} e {b:?} abriram: {r:?}"
        );
    }
}

/// O `./` do `tar -C pasta -cf x.tar .` e o proprio destino: pulado, e o
/// `./a.txt` sai como `a.txt`. Canonizar nao pode recusar o tar mais comum.
#[test]
fn a_pasta_raiz_do_tar_e_pulada_e_o_resto_sai_canonico() {
    let t = fechar(&[
        cabecalho_cru("./", b'5', "", b""),
        cabecalho_cru("./a.txt", b'0', "", b"a"),
    ]);
    let ents = tar::ler(&t).unwrap();
    assert_eq!(ents.len(), 1);
    assert_eq!(ents[0].nome, "a.txt");
}

/// Link (e pasta) com `size` diferente de zero: o GNU tar pula o dado, um
/// leitor que nao pula ve o cabecalho escondido nele como entrada. Recusa.
///
/// Prova real: sem a recusa, o `tar::ler` devolve o `escondido.sh` que o GNU
/// tar nunca mostraria, e o teste cai.
#[test]
fn link_ou_pasta_com_size_e_recusado() {
    let escondido = cabecalho_cru("escondido.sh", b'0', "", b"rm -rf ~\n");
    for (nome, tipo) in [("l", b'2'), ("h", b'1'), ("d/", b'5')] {
        let t = fechar(&[
            cabecalho_cru(nome, tipo, "x", &escondido),
            cabecalho_cru("visivel.txt", b'0', "", b"ok\n"),
        ]);
        let r = tar::ler(&t);
        assert!(
            matches!(&r, Err(Erro::Tar(_))),
            "tipo {}: abriu {:?}",
            tipo as char,
            r.map(|v| v.into_iter().map(|e| e.nome).collect::<Vec<_>>())
        );
    }
}
