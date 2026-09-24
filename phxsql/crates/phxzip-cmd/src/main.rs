//! PhxZipCmd -- o PhxZip no terminal.
//!
//! Casca fina sobre a API publica da `phxzip` (pedido 454): nenhuma decisao de
//! formato, de nome seguro ou de senha mora aqui. O que mora aqui e so o que
//! a biblioteca nao pode ter por ser `no_std`: disco, relogio e acaso.
//!
//! ```text
//! phxzipcmd a <arquivo.7z> <caminho>... [-p<senha>|-p-] [-mx=N] [-mmt=N|-mmt=off] [--nomes-visiveis] [-y]
//! phxzipcmd x <arquivo.7z> [-o<pasta>] [-p<senha>|-p-] [-mmt=N] [-y]
//! phxzipcmd l <arquivo.7z> [-p<senha>|-p-]
//! phxzipcmd t <arquivo.7z> [-p<senha>|-p-]
//! ```
//!
//! `-p-` le a senha da primeira linha da entrada padrao, para ela nao ficar na
//! lista de processos nem no historico do shell.
//!
//! Codigo de saida: 0 bem; 1 uso; 2 arquivo corrompido ou CRC; 3 senha
//! (faltou, ou errada); 4 metodo recusado ou nao suportado; 5 disco; 6 teto
//! ou caminho inseguro.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use phxzip::{filetime_de_unix, unix_de_filetime, Arquivo7z, Erro, Escritor, Limites, Opcoes};

struct Args {
    comando: String,
    arquivo: PathBuf,
    caminhos: Vec<PathBuf>,
    senha: Option<String>,
    nivel: u8,
    /// Fios da compactacao (`-mmt=`, o nome do 7-Zip). Padrao: os nucleos.
    fios: usize,
    destino: PathBuf,
    nomes_visiveis: bool,
    sobrescrever: bool,
}

fn uso() -> ExitCode {
    eprintln!(
        "PhxZipCmd {} -- 7z (LZMA2 + AES-256)\n\n\
         uso:\n  phxzipcmd a <arquivo.7z> <caminho>... [-p<senha>|-p-] [-mx=0..9] [-mmt=N|-mmt=off] [--nomes-visiveis] [-y]\n  \
         phxzipcmd x <arquivo.7z> [-o<pasta>] [-p<senha>|-p-] [-mmt=N] [-y]\n  \
         phxzipcmd l <arquivo.7z> [-p<senha>|-p-]\n  \
         phxzipcmd t <arquivo.7z> [-p<senha>|-p-]",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::from(1)
}

fn ler_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let comando = it.next().ok_or("falta o comando")?;
    let mut a = Args {
        comando,
        arquivo: PathBuf::new(),
        caminhos: Vec::new(),
        senha: None,
        nivel: 5,
        fios: phxzip::fios_padrao(),
        destino: PathBuf::from("."),
        nomes_visiveis: false,
        sobrescrever: false,
    };
    let mut posicionais = Vec::new();
    for x in it {
        if x == "-p-" {
            let mut linha = String::new();
            io::stdin()
                .lock()
                .read_line(&mut linha)
                .map_err(|e| format!("lendo a senha: {e}"))?;
            a.senha = Some(linha.trim_end_matches(['\r', '\n']).to_string());
        } else if let Some(s) = x.strip_prefix("-p") {
            a.senha = Some(s.to_string());
        } else if let Some(n) = x.strip_prefix("-mx=") {
            a.nivel = n
                .parse()
                .ok()
                .filter(|n| *n <= 9)
                .ok_or("-mx= vai de 0 a 9")?;
        } else if let Some(n) = x.strip_prefix("-mmt=") {
            // `off` e 1 dao o arquivo de um bloco so, o menor possivel; mais
            // de 1 corta em blocos que compactam em paralelo.
            a.fios = if n == "off" {
                1
            } else {
                n.parse()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or("-mmt= precisa de um numero >= 1 ou off")?
            };
        } else if let Some(d) = x.strip_prefix("-o") {
            a.destino = PathBuf::from(d);
        } else if x == "--nomes-visiveis" {
            a.nomes_visiveis = true;
        } else if x == "-y" {
            a.sobrescrever = true;
        } else if x.starts_with('-') && x.len() > 1 {
            return Err(format!("opcao desconhecida: {x}"));
        } else {
            posicionais.push(PathBuf::from(x));
        }
    }
    let mut p = posicionais.into_iter();
    a.arquivo = p.next().ok_or("falta o arquivo")?;
    a.caminhos = p.collect();
    Ok(a)
}

fn codigo(e: &Erro) -> u8 {
    match e {
        Erro::Uso(_) => 1,
        Erro::NaoE7z | Erro::VersaoDesconhecida(..) | Erro::Corrompido(_) | Erro::CrcNaoBate(_) => {
            2
        }
        Erro::SenhaNecessaria | Erro::SenhaErradaOuCorrompido => 3,
        Erro::MetodoRecusado { .. } | Erro::NaoSuportado(_) => 4,
        Erro::Teto(_) | Erro::CaminhoInseguro | Erro::GrandeDemaisParaEsteAlvo => 6,
    }
}

enum Falha {
    Zip(Erro),
    Disco(String),
    Uso(String),
}

impl From<Erro> for Falha {
    fn from(e: Erro) -> Falha {
        Falha::Zip(e)
    }
}

fn disco<T>(r: io::Result<T>, o_que: &Path) -> Result<T, Falha> {
    r.map_err(|e| Falha::Disco(format!("{}: {e}", o_que.display())))
}

fn main() -> ExitCode {
    let a = match ler_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("phxzipcmd: {e}");
            return uso();
        }
    };
    let r = match a.comando.as_str() {
        "a" => criar(&a),
        "x" => extrair(&a),
        "l" => listar(&a),
        "t" => testar(&a),
        _ => return uso(),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(Falha::Zip(e)) => {
            eprintln!("phxzipcmd: {e}");
            ExitCode::from(codigo(&e))
        }
        Err(Falha::Disco(e)) => {
            eprintln!("phxzipcmd: {e}");
            ExitCode::from(5)
        }
        Err(Falha::Uso(e)) => {
            eprintln!("phxzipcmd: {e}");
            ExitCode::from(1)
        }
    }
}

fn mtime(md: &fs::Metadata) -> Option<u64> {
    let t = md.modified().ok()?;
    let s = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };
    Some(filetime_de_unix(s))
}

#[cfg(unix)]
fn atributos(md: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    let base = if md.is_dir() { 0x10 } else { 0x20 };
    Some(base | 0x8000 | (md.permissions().mode() << 16))
}

#[cfg(not(unix))]
fn atributos(md: &fs::Metadata) -> Option<u32> {
    Some(if md.is_dir() { 0x10 } else { 0x20 })
}

fn juntar(e: &mut Escritor, disco_: &Path, nome: &str) -> Result<(), Falha> {
    let md = disco(fs::symlink_metadata(disco_), disco_)?;
    if md.file_type().is_symlink() {
        // Ligacao nao entra: o motor recusaria extrai-la, e gravar o que nao
        // volta seria mentir sobre o arquivo.
        eprintln!(
            "phxzipcmd: ignorada (ligacao simbolica): {}",
            disco_.display()
        );
        return Ok(());
    }
    if md.is_dir() {
        e.pasta(nome, mtime(&md))?;
        let mut filhos: Vec<_> = disco(fs::read_dir(disco_), disco_)?
            .filter_map(|x| x.ok())
            .collect();
        // Ordem estavel: o mesmo diretorio gera o mesmo arquivo.
        filhos.sort_by_key(|x| x.file_name());
        for f in filhos {
            let n = f.file_name().to_string_lossy().into_owned();
            juntar(e, &f.path(), &format!("{nome}/{n}"))?;
        }
    } else {
        let d = disco(fs::read(disco_), disco_)?;
        e.arquivo(nome, d, mtime(&md), atributos(&md))?;
    }
    Ok(())
}

fn criar(a: &Args) -> Result<(), Falha> {
    if a.caminhos.is_empty() {
        return Err(Falha::Uso("nada para compactar".into()));
    }
    if a.arquivo.exists() && !a.sobrescrever {
        return Err(Falha::Uso(format!(
            "{} ja existe (use -y para sobrescrever)",
            a.arquivo.display()
        )));
    }
    let mut acaso = [0u8; 32];
    phxsql_core::cifra::sortear(&mut acaso);
    let op = Opcoes {
        nivel: a.nivel,
        senha: a.senha.clone(),
        cifrar_nomes: !a.nomes_visiveis,
        acaso,
        fios: a.fios,
        bloco: 0,
    };
    let mut e = Escritor::novo(op);
    for c in &a.caminhos {
        let nome = c
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| Falha::Uso(format!("caminho sem nome: {}", c.display())))?;
        juntar(&mut e, c, &nome)?;
    }
    let bytes = e.gravar()?;
    disco(fs::write(&a.arquivo, &bytes), &a.arquivo)?;
    println!("{}: {} bytes", a.arquivo.display(), bytes.len());
    Ok(())
}

/// Os tetos de sempre, com os fios do `-mmt=`: um lugar so para `l`, `t` e
/// `x` lerem com o mesmo numero de fios.
fn limites(a: &Args) -> Limites {
    Limites {
        fios: a.fios,
        ..Limites::default()
    }
}

fn abrir(a: &Args) -> Result<Vec<u8>, Falha> {
    disco(fs::read(&a.arquivo), &a.arquivo)
}

fn listar(a: &Args) -> Result<(), Falha> {
    let bytes = abrir(a)?;
    let z = Arquivo7z::abrir(&bytes, a.senha.as_deref(), limites(a))?;
    let mut total = 0u64;
    let mut out = io::stdout().lock();
    for (i, e) in z.entradas().iter().enumerate() {
        let quando = e
            .mtime
            .map(|t| unix_de_filetime(t).to_string())
            .unwrap_or_else(|| "-".into());
        let tipo = if e.e_pasta { "D" } else { "A" };
        let metodos = z.metodos(i).join("+");
        let _ = writeln!(
            out,
            "{tipo} {:>12} {quando:>12} {:<12} {}",
            e.tamanho, metodos, e.nome
        );
        total += e.tamanho;
    }
    let _ = writeln!(out, "{} entradas, {total} bytes", z.entradas().len());
    Ok(())
}

fn testar(a: &Args) -> Result<(), Falha> {
    let bytes = abrir(a)?;
    let z = Arquivo7z::abrir(&bytes, a.senha.as_deref(), limites(a))?;
    z.testar()?;
    println!("tudo certo: {} entradas", z.entradas().len());
    Ok(())
}

/// Recusa escrever atraves de uma ligacao que ja exista no destino: o nome
/// e seguro, mas `destino/a` pode ser ligacao para `/etc`.
fn sem_ligacao_no_caminho(base: &Path, rel: &str) -> Result<PathBuf, Falha> {
    let mut p = base.to_path_buf();
    for parte in rel.split('/') {
        p.push(parte);
        if let Ok(md) = fs::symlink_metadata(&p) {
            if md.file_type().is_symlink() {
                return Err(Falha::Zip(Erro::CaminhoInseguro));
            }
        }
    }
    Ok(p)
}

fn extrair(a: &Args) -> Result<(), Falha> {
    let bytes = abrir(a)?;
    let z = Arquivo7z::abrir(&bytes, a.senha.as_deref(), limites(a))?;
    // Todos os nomes se conferem ANTES de escrever o primeiro byte: arquivo
    // com uma entrada hostil nao deixa meia extracao no disco.
    for e in z.entradas() {
        e.caminho()?;
        if e.e_ligacao() {
            return Err(Falha::Zip(Erro::NaoSuportado(
                "ligacao simbolica dentro do arquivo",
            )));
        }
    }
    disco(fs::create_dir_all(&a.destino), &a.destino)?;
    let mut n = 0usize;
    let r = z.extrair_tudo(|e, dados| {
        let rel = e.caminho()?;
        let p = match sem_ligacao_no_caminho(&a.destino, &rel) {
            Ok(p) => p,
            Err(_) => return Err(Erro::CaminhoInseguro),
        };
        let gravar = || -> io::Result<()> {
            if e.e_pasta {
                fs::create_dir_all(&p)?;
                return Ok(());
            }
            if let Some(pai) = p.parent() {
                fs::create_dir_all(pai)?;
            }
            if p.exists() && !a.sobrescrever {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "ja existe (use -y)",
                ));
            }
            let f = fs::File::create(&p)?;
            (&f).write_all(dados)?;
            if let Some(t) = e.mtime {
                let s = unix_de_filetime(t);
                if s >= 0 {
                    let _ = f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(s as u64));
                }
            }
            #[cfg(unix)]
            if let Some(m) = e.modo_unix() {
                use std::os::unix::fs::PermissionsExt;
                // So os bits de permissao; setuid/setgid de arquivo alheio nao.
                let _ = fs::set_permissions(&p, fs::Permissions::from_mode(m & 0o777));
            }
            Ok(())
        };
        gravar().map_err(|err| {
            eprintln!("phxzipcmd: {}: {err}", p.display());
            Erro::Uso("falha de disco ao extrair")
        })?;
        n += 1;
        Ok(())
    });
    match r {
        Ok(()) => {
            println!("{n} entradas extraidas em {}", a.destino.display());
            Ok(())
        }
        Err(Erro::Uso("falha de disco ao extrair")) => {
            Err(Falha::Disco("extracao interrompida".into()))
        }
        Err(e) => Err(Falha::Zip(e)),
    }
}
