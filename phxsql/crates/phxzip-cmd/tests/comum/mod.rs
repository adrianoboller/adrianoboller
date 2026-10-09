//! Apoio aos testes de integracao do PhxZipCmd: a pasta com guarda, o
//! binario rodado de verdade e o 7z hostil montado a mao.
//!
//! A pasta mora no `CARGO_TARGET_TMPDIR` -- dentro de `target/`, e nao no
//! `/tmp` --, com `Drop` que apaga inclusive quando o teste cai no meio
//! (pedido 150).

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use phxzip::{Escritor, Metodo, Opcoes};

static SEQ: AtomicU64 = AtomicU64::new(0);

pub struct DirTemp(pub PathBuf);

impl DirTemp {
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("phxzipcmd-{}-{rotulo}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        DirTemp(p)
    }
}

impl Drop for DirTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for DirTemp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

pub struct Saida {
    pub codigo: i32,
    pub out: String,
    pub err: String,
}

impl Saida {
    pub fn tudo(&self) -> String {
        format!("{}{}", self.out, self.err)
    }
}

/// Roda o binario de verdade, com a variavel da senha APAGADA do ambiente
/// herdado (senao o teste dependeria de quem o roda), o idioma FIXO em
/// portugues (as provas procuram frases, e o `LANG` de quem roda a suite nao
/// pode mudar o veredito) e a entrada padrao como cano -- nunca um terminal.
pub fn phxzipcmd(cwd: &Path, args: &[&str], senha_env: Option<&str>, entrada: &[u8]) -> Saida {
    phxzipcmd_no_ambiente(
        cwd,
        args,
        senha_env,
        entrada,
        &[(phxzip_cmd::ENV_IDIOMA, Some("Portugues"))],
    )
}

/// O mesmo, com variaveis de ambiente escolhidas: `Some` define, `None` apaga.
pub fn phxzipcmd_no_ambiente(
    cwd: &Path,
    args: &[&str],
    senha_env: Option<&str>,
    entrada: &[u8],
    ambiente: &[(&str, Option<&str>)],
) -> Saida {
    let mut c = Command::new(env!("CARGO_BIN_EXE_phxzipcmd"));
    for (k, v) in ambiente {
        match v {
            Some(v) => c.env(k, v),
            None => c.env_remove(k),
        };
    }
    c.args(args)
        .current_dir(cwd)
        .env_remove(phxzip_cmd::ENV_SENHA)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(s) = senha_env {
        c.env(phxzip_cmd::ENV_SENHA, s);
    }
    let mut filho = c.spawn().expect("o binario phxzipcmd roda");
    // O filho pode sair sem ler (a recusa da senha sai antes): cano quebrado
    // aqui nao e defeito do teste.
    let _ = filho.stdin.take().unwrap().write_all(entrada);
    let o = filho.wait_with_output().unwrap();
    Saida {
        codigo: o.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
        err: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// Um 7z em claro, sem compressao e com o cabecalho em claro: a base para
/// trocar nome e atributo como um atacante faria.
pub fn claro(entradas: &[(&str, &[u8])]) -> Vec<u8> {
    let mut e = Escritor::novo(Opcoes {
        metodo: Metodo::Copia,
        senha: None,
        ciclos: 0,
        cifrar_cabecalho: false,
    })
    .unwrap();
    for (nome, dado) in entradas {
        e.arquivo(nome, dado, None).unwrap();
    }
    e.terminar()
}

/// Um 7z cifrado, conteudo e cabecalho, com poucos ciclos para o teste nao
/// pagar 2^19 SHA-256 em debug.
pub fn cifrado(entradas: &[(&str, &[u8])], senha: &str) -> Vec<u8> {
    let mut e = Escritor::novo(Opcoes {
        metodo: Metodo::Lzma2,
        senha: Some(senha.to_string()),
        ciclos: 4,
        cifrar_cabecalho: true,
    })
    .unwrap();
    for (nome, dado) in entradas {
        e.arquivo(nome, dado, None).unwrap();
    }
    e.terminar()
}

fn faixa_do_cabecalho(d: &[u8]) -> (usize, usize) {
    let off = u64::from_le_bytes(d[12..20].try_into().unwrap()) as usize;
    let tam = u64::from_le_bytes(d[20..28].try_into().unwrap()) as usize;
    (32 + off, 32 + off + tam)
}

/// Refaz o CRC do cabecalho e o do cabecalho de inicio.
pub fn refazer_crcs(d: &mut [u8]) {
    let (ini, fim) = faixa_do_cabecalho(d);
    let crc = phxsql_core::crc32(&d[ini..fim]);
    d[28..32].copy_from_slice(&crc.to_le_bytes());
    let crc_inicio = phxsql_core::crc32(&d[12..32]);
    d[8..12].copy_from_slice(&crc_inicio.to_le_bytes());
}

/// Troca um nome gravado em UTF-16 no cabecalho em claro por outro de mesmo
/// tamanho -- o nome que o escritor se recusaria a gravar.
pub fn trocar_nome(d: &mut [u8], de: &str, para: &str) {
    let de: Vec<u8> = de.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let para: Vec<u8> = para.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    assert_eq!(
        de.len(),
        para.len(),
        "o nome novo tem de ter o mesmo tamanho"
    );
    let onde = d
        .windows(de.len())
        .position(|w| w == de.as_slice())
        .unwrap();
    d[onde..onde + de.len()].copy_from_slice(&para);
    refazer_crcs(d);
}

/// Marca a entrada `i` de `n` com o modo Unix de LINK SIMBOLICO, como o 7-Zip
/// do Linux grava (`0x8000` e `st_mode` nos 16 bits altos). Os atributos sao a
/// ultima propriedade do cabecalho que o PhxZip escreve: `n` palavras de 4
/// bytes antes dos dois `FIM`.
pub fn marcar_como_link(d: &mut [u8], n: usize, i: usize) {
    let (_, fim) = faixa_do_cabecalho(d);
    let pos = fim - 2 - 4 * (n - i);
    let antes = u32::from_le_bytes(d[pos..pos + 4].try_into().unwrap());
    assert_eq!(antes, 0x20, "o atributo nao estava onde se esperava");
    let link: u32 = 0x8000 | 0x20 | (0o120_777 << 16);
    d[pos..pos + 4].copy_from_slice(&link.to_le_bytes());
    refazer_crcs(d);
}
