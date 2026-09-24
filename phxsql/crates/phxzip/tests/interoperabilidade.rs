//! O PhxZip contra o mundo: arquivos gravados pelo 7-Zip 23.01 (p7zip, uma
//! vez, com senha inventada -- `tests/dados/`) abertos aqui, e arquivos
//! gravados aqui abertos pelo `7z` quando ele existe na maquina.
//!
//! Ida e volta com o proprio codigo nao prova nada sobre o formato: os dois
//! lados poderiam estar errados juntos. O que prova e o 7-Zip abrir o nosso
//! e nos abrirmos o dele.

use std::path::{Path, PathBuf};
use std::process::Command;

use phxzip::{filetime_de_unix, Arquivo7z, Erro, Escritor, Limites, Opcoes};

const SENHA: &str = "phxzip-teste";

fn dado(nome: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/dados")
        .join(nome);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn a_txt() -> Vec<u8> {
    b"O pai veio antes do filho.\n".repeat(200)
}

/// O conteudo que as fixtures carregam, conferido entrada a entrada.
fn conferir_raiz(a: &Arquivo7z) {
    let mut vistos = Vec::new();
    a.extrair_tudo(|e, d| {
        vistos.push(e.nome.clone());
        match e.nome.as_str() {
            "raiz/a.txt" => assert_eq!(d, a_txt().as_slice()),
            "raiz/sub/b.bin" => {
                assert_eq!(d.len(), 2048);
                assert_eq!(
                    phxhash::hash::para_hex(&phxhash::hash::sha256(d)),
                    "e2184ae4dfd1e541213e627f7e07bb1f253516a4754ab4296573c846edb2d408"
                );
            }
            "raiz/sub/ação.txt" => assert_eq!(d, "texto com acento: ação, 日本\n".as_bytes()),
            "raiz/vazio.txt" => {
                assert!(d.is_empty());
                assert!(!e.e_pasta);
            }
            "raiz" | "raiz/sub" | "raiz/sub/pasta_vazia" => assert!(e.e_pasta, "{}", e.nome),
            outro => panic!("entrada inesperada {outro}"),
        }
        Ok(())
    })
    .unwrap();
    vistos.sort();
    assert_eq!(vistos.len(), 7, "{vistos:?}");
    let a_txt = a
        .entradas()
        .iter()
        .find(|e| e.nome == "raiz/a.txt")
        .unwrap();
    // 2026-09-24 10:00:00 UTC, gravado pelo `touch -d` antes do 7z.
    assert_eq!(a_txt.mtime, Some(filetime_de_unix(1_790_244_000)));
}

#[test]
fn abre_o_que_o_7zip_grava() {
    for nome in [
        "7z-lzma2.7z",
        "7z-lzma.7z",
        "7z-copy.7z",
        "7z-nao-solido-cabecalho-cru.7z",
    ] {
        let b = dado(nome);
        let a = Arquivo7z::abrir(&b, None, Limites::default())
            .unwrap_or_else(|e| panic!("{nome}: {e}"));
        conferir_raiz(&a);
        a.testar().unwrap();
    }
}

#[test]
fn abre_o_7zaes_do_7zip_com_e_sem_nomes_cifrados() {
    let b = dado("7z-lzma2-aes-nomes-cifrados.7z");
    assert_eq!(
        Arquivo7z::abrir(&b, None, Limites::default()).unwrap_err(),
        Erro::SenhaNecessaria
    );
    assert_eq!(
        Arquivo7z::abrir(&b, Some("errada"), Limites::default()).unwrap_err(),
        Erro::SenhaErradaOuCorrompido
    );
    let a = Arquivo7z::abrir(&b, Some(SENHA), Limites::default()).unwrap();
    assert!(a.cabecalho_cifrado());
    conferir_raiz(&a);

    // Nomes visiveis: lista sem senha, mas o conteudo pede.
    let b = dado("7z-lzma2-aes-nomes-visiveis.7z");
    let a = Arquivo7z::abrir(&b, None, Limites::default()).unwrap();
    assert!(!a.cabecalho_cifrado());
    assert!(a
        .entradas()
        .iter()
        .any(|e| e.nome == "raiz/a.txt" && e.cifrada));
    assert_eq!(a.testar().unwrap_err(), Erro::SenhaNecessaria);
    let a = Arquivo7z::abrir(&b, Some("errada"), Limites::default()).unwrap();
    assert_eq!(a.testar().unwrap_err(), Erro::SenhaErradaOuCorrompido);
    let a = Arquivo7z::abrir(&b, Some(SENHA), Limites::default()).unwrap();
    conferir_raiz(&a);
}

#[test]
fn metodos_fora_do_conjunto_atual_sao_recusados_pelo_nome() {
    for (arq, esperado) in [
        ("7z-bzip2.7z", "BZip2"),
        ("7z-ppmd.7z", "PPMd"),
        ("7z-deflate.7z", "Deflate"),
    ] {
        let b = dado(arq);
        let a = Arquivo7z::abrir(&b, None, Limites::default()).unwrap();
        match a.testar() {
            Err(Erro::MetodoRecusado { nome, .. }) => assert_eq!(nome, esperado, "{arq}"),
            outro => panic!("{arq}: {outro:?}"),
        }
    }
}

#[test]
fn caminho_absoluto_gravado_pelo_7zip_nao_vira_caminho_no_disco() {
    let b = dado("7z-caminho-absoluto.7z");
    let a = Arquivo7z::abrir(&b, None, Limites::default()).unwrap();
    let e = &a.entradas()[0];
    assert!(e.nome.starts_with('/'), "{}", e.nome);
    assert_eq!(e.caminho(), Err(Erro::CaminhoInseguro));
    // O conteudo continua legivel: a recusa e do caminho, nao do dado.
    assert_eq!(a.extrair(0).unwrap(), a_txt());
}

#[test]
fn byte_trocado_no_dado_e_crc_que_nao_bate() {
    let mut b = dado("7z-copy.7z");
    // Copy: o dado esta cru logo depois dos 32 bytes da assinatura.
    b[40] ^= 0x01;
    let a = Arquivo7z::abrir(&b, None, Limites::default()).unwrap();
    assert!(
        matches!(a.testar(), Err(Erro::CrcNaoBate(_))),
        "{:?}",
        a.testar()
    );
}

#[test]
fn teto_da_pasta_recusa_antes_de_alocar() {
    let b = dado("7z-lzma2.7z");
    let lim = Limites {
        max_pasta: 1000,
        ..Limites::default()
    };
    let a = Arquivo7z::abrir(&b, None, lim).unwrap();
    assert_eq!(
        a.testar(),
        Err(Erro::Teto("pasta descompacta mais que o teto"))
    );
}

fn opcoes(nivel: u8, senha: Option<&str>, cifrar_nomes: bool) -> Opcoes {
    Opcoes {
        nivel,
        senha: senha.map(String::from),
        cifrar_nomes,
        acaso: [0x5A; 32],
        ..Opcoes::default()
    }
}

fn montar(op: Opcoes) -> Vec<u8> {
    let mut e = Escritor::novo(op);
    let t = Some(filetime_de_unix(1_790_244_000));
    e.pasta("raiz", t).unwrap();
    e.pasta("raiz/sub", t).unwrap();
    e.pasta("raiz/sub/pasta_vazia", t).unwrap();
    e.arquivo("raiz/a.txt", a_txt(), t, None).unwrap();
    e.arquivo("raiz/vazio.txt", Vec::new(), t, None).unwrap();
    e.arquivo(
        "raiz/sub/ação.txt",
        "texto com acento: ação, 日本\n".as_bytes().to_vec(),
        t,
        None,
    )
    .unwrap();
    let b = dado("7z-copy.7z");
    let a = Arquivo7z::abrir(&b, None, Limites::default()).unwrap();
    let i = a
        .entradas()
        .iter()
        .position(|e| e.nome == "raiz/sub/b.bin")
        .unwrap();
    e.arquivo("raiz/sub/b.bin", a.extrair(i).unwrap(), t, None)
        .unwrap();
    e.gravar().unwrap()
}

#[test]
fn ida_e_volta_em_todas_as_combinacoes() {
    for nivel in [0, 1, 5, 9] {
        for (senha, nomes) in [(None, false), (Some(SENHA), false), (Some(SENHA), true)] {
            let b = montar(opcoes(nivel, senha, nomes));
            let a = Arquivo7z::abrir(&b, senha, Limites::default()).unwrap();
            assert_eq!(a.cabecalho_cifrado(), nomes);
            conferir_raiz(&a);
        }
    }
}

#[test]
fn escritor_recusa_nome_inseguro_e_repetido() {
    let mut e = Escritor::novo(Opcoes::default());
    assert_eq!(
        e.arquivo("../fora", vec![1], None, None),
        Err(Erro::CaminhoInseguro)
    );
    e.arquivo("a", vec![1], None, None).unwrap();
    assert_eq!(
        e.arquivo("./a", vec![2], None, None),
        Err(Erro::Uso("nome repetido no arquivo"))
    );
}

#[test]
fn arquivo_sem_entradas_e_o_7z_vazio_de_32_bytes() {
    let b = Escritor::novo(Opcoes::default()).gravar().unwrap();
    assert_eq!(b.len(), 32);
    assert!(Arquivo7z::abrir(&b, None, Limites::default())
        .unwrap()
        .entradas()
        .is_empty());
}

/// Diretorio temporario que se apaga no `Drop` -- inclusive quando uma
/// asserção falha no meio (pedido 150: o `remove_dir_all` so no fim deixava
/// lixo em `/tmp` a cada teste vermelho).
struct DirTemp(PathBuf);

impl DirTemp {
    /// Um nome POR TESTE: com so o pid, dois testes ao mesmo tempo apagavam a
    /// pasta um do outro (visto quando o segundo teste com 7z entrou).
    fn novo(nome: &str) -> DirTemp {
        let d = std::env::temp_dir().join(format!("phxzip-interop-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        DirTemp(d)
    }
}

impl Drop for DirTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sete_z() -> Option<PathBuf> {
    for c in ["7z", "7za", "7zz"] {
        if Command::new(c)
            .arg("i")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Some(PathBuf::from(c));
        }
    }
    None
}

/// O 7-Zip abre o que o PhxZip grava. Sem o `7z` na maquina a prova nao
/// roda -- e o teste DIZ isso, em vez de passar calado como se tivesse medido.
#[test]
fn o_7zip_abre_o_que_o_phxzip_grava() {
    let Some(sete) = sete_z() else {
        eprintln!(
            "NAO MEDIDO: 7z ausente nesta maquina; interoperabilidade de escrita nao conferida"
        );
        return;
    };
    let guarda = DirTemp::novo("escrita");
    let dir = guarda.0.clone();
    for (i, (nivel, senha, nomes)) in [
        (0u8, None, false),
        (5, None, false),
        (9, Some(SENHA), true),
        (1, Some(SENHA), false),
    ]
    .into_iter()
    .enumerate()
    {
        let arq = dir.join(format!("c{i}.7z"));
        std::fs::write(&arq, montar(opcoes(nivel, senha, nomes))).unwrap();
        let destino = dir.join(format!("x{i}"));
        let mut cmd = Command::new(&sete);
        cmd.arg("x")
            .arg(&arq)
            .arg(format!("-o{}", destino.display()))
            .arg("-y");
        cmd.arg(format!("-p{}", senha.unwrap_or("")));
        let o = cmd.output().unwrap();
        assert!(
            o.status.success(),
            "caso {i}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        assert_eq!(
            std::fs::read(destino.join("raiz/a.txt")).unwrap(),
            a_txt(),
            "caso {i}"
        );
        assert_eq!(
            std::fs::read(destino.join("raiz/sub/ação.txt")).unwrap(),
            "texto com acento: ação, 日本\n".as_bytes()
        );
        assert!(destino.join("raiz/sub/pasta_vazia").is_dir());
        assert_eq!(
            std::fs::read(destino.join("raiz/vazio.txt")).unwrap().len(),
            0
        );
        assert_eq!(
            std::fs::read(destino.join("raiz/sub/b.bin")).unwrap().len(),
            2048
        );
    }
}

/// Compactado em BLOCOS (varios fios): o fluxo LZMA2 recomeca o dicionario a
/// cada bloco, e o 7-Zip tem de abrir e conferir o CRC igual. Com senha, para
/// o bloco passar tambem pelo AES.
#[test]
fn o_7zip_abre_o_que_foi_compactado_em_blocos() {
    let Some(sete) = sete_z() else {
        eprintln!("NAO MEDIDO: 7z ausente; compactacao em blocos nao conferida");
        return;
    };
    let mut dados = Vec::new();
    let mut x = 7u32;
    while dados.len() < 700_000 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        dados.extend_from_slice(format!("registro {} do PhxZip\n", x >> 18).as_bytes());
    }
    let mut e = Escritor::novo(Opcoes {
        nivel: 5,
        senha: Some(SENHA.into()),
        acaso: [0x33; 32],
        fios: 3,
        bloco: 1 << 16,
        ..Opcoes::default()
    });
    e.arquivo("blocos.txt", dados.clone(), None, None).unwrap();
    let guarda = DirTemp::novo("blocos");
    let arq = guarda.0.join("blocos.7z");
    std::fs::write(&arq, e.gravar().unwrap()).unwrap();
    let destino = guarda.0.join("x");
    let o = Command::new(&sete)
        .arg("x")
        .arg(&arq)
        .arg(format!("-o{}", destino.display()))
        .arg(format!("-p{SENHA}"))
        .arg("-y")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(std::fs::read(destino.join("blocos.txt")).unwrap(), dados);
}

/// O 7-Zip tambem grava LZMA2 em blocos (`-m0=lzma2:c=`): o nosso leitor
/// acha os cortes e descompacta em paralelo o que ELE gravou, com o mesmo
/// conteudo de um fio so.
#[test]
fn descompacta_em_fios_o_lzma2_em_blocos_do_7zip() {
    let Some(sete) = sete_z() else {
        eprintln!("NAO MEDIDO: 7z ausente; descompactacao em fios nao conferida");
        return;
    };
    let guarda = DirTemp::novo("fios-7z");
    let mut dados = Vec::new();
    let mut x = 11u32;
    while dados.len() < 600_000 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        dados.extend_from_slice(format!("linha {} gravada pelo 7-Zip\n", x >> 19).as_bytes());
    }
    std::fs::write(guarda.0.join("d.txt"), &dados).unwrap();
    let arq = guarda.0.join("d.7z");
    let o = Command::new(&sete)
        .current_dir(&guarda.0)
        .args(["a", "-t7z", "-m0=lzma2:c=64k", "-mmt=4", "-mf=off"])
        .arg(&arq)
        .arg("d.txt")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let b = std::fs::read(&arq).unwrap();
    // Sem cifra e com uma pasta so, o fluxo LZMA2 comeca logo depois dos 32
    // bytes do cabecalho inicial: se o 7-Zip nao tiver cortado em blocos, o
    // teste passaria sem ter exercitado o paralelo -- e tem de dizer.
    let (cortes, _) = phxzip::lzma::cortes_lzma2(&b[32..]).unwrap();
    assert!(
        cortes.len() > 1,
        "o 7-Zip gravou um trecho so: nada a paralelizar"
    );
    for fios in [1, 4] {
        let lim = Limites {
            fios,
            ..Limites::default()
        };
        let a = Arquivo7z::abrir(&b, None, lim).unwrap();
        assert_eq!(a.extrair(0).unwrap(), dados, "{fios} fios");
    }
}
