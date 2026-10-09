//! # PhxZipCmd -- o PhxZip no terminal
//!
//! Pedido 454, fatia Z9 do `docs/propostas/plano-0.21.md`. O motor e o crate
//! `phxzip`, e este crate e uma CASCA sobre a API publica dele: abrir, listar,
//! percorrer e escrever moram la, uma vez so. O que mora aqui e o que o motor
//! diz, no topo dele, que o extrator tem de fazer por conta propria -- e e por
//! isso que as tres garantias abaixo sao deste arquivo, e nao de la.
//!
//! ## A senha nunca vem por argumento
//!
//! Argumento de linha de comando aparece no `ps` de qualquer usuario da
//! maquina, em `/proc/<pid>/cmdline`, e fica no historico do shell. O
//! `phxsqlcmd` aceita `--senha` com um aviso; aqui ele e RECUSADO, antes de
//! qualquer outra coisa -- inclusive antes do `--help` --, e a recusa nao
//! repete o valor. A senha vem de [`ENV_SENHA`] ou da entrada padrao: num
//! terminal, com o eco desligado pelo `stty`; num cano, a primeira linha.
//!
//! Variavel de ambiente tambem nao e cofre: o dono do processo (e o root) le
//! `/proc/<pid>/environ`. A diferenca para o argumento e quem ve -- o `ps` e de
//! todos, o `environ` e do dono -- e e por isso que ela e aceita e o argumento
//! nao.
//!
//! ## A extracao nunca sai da raiz e nunca segue link -- pelo motor
//!
//! Quem decide e o `phxzip::disco::Destino`, o MESMO que o tar usa: o nome
//! conferido por texto e pelo `Path` da plataforma, a colisao arquivo/pasta
//! antes do primeiro byte, cada pasta olhada sem seguir link, o arquivo com
//! `create_new`, e o `--sobrescrever` apagando o nome (o link), nunca o alvo.
//! Esta casca nao toca o disco na extracao: a mesma decisao escrita aqui de
//! novo seria a copia que envelhece calada (petrea «funcao e comando vem do
//! mesmo motor»). O que fica aqui e o que e de TELA: o aviso da entrada que
//! veio marcada como link -- e virou arquivo comum, porque o `Destino` nao
//! tem caminho que crie link -- e o escape do caractere de controle no nome.
//! O limite (TOCTOU) esta dito no topo do `disco.rs`.

use std::fmt;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use phxzip::disco::{marcado_como_link, Destino};
use phxzip::{Arquivo, Erro, Escritor, Limites, Metodo, Opcoes};

/// A variavel de ambiente da senha. Documentada no `--help`.
pub const ENV_SENHA: &str = "PHXZIP_SENHA";

/// A maior senha aceita pela entrada padrao, em bytes. Teto para o que vem de
/// um cano sem quebra de linha: sem ele, `cat /dev/zero | phxzipcmd ...`
/// juntaria memoria ate o processo cair.
pub const TETO_DA_SENHA: usize = 1024;

/// O maior arquivo lido sob os limites padrao. O motor trabalha sobre uma
/// fatia, entao o arquivo inteiro vai para a memoria; o teto faz um arquivo de
/// 40 GB recusar dizendo o tamanho, em vez de derrubar o processo. Fonte
/// confiavel e grande e `--confiavel`, que tira o teto.
pub const TETO_DO_ARQUIVO: u64 = 1 << 30;

/// O codigo de saida de uso errado. Falha de operacao sai com 1.
pub const SAIDA_USO: u8 = 2;

const USO: &str = "\
phxzipcmd -- o PhxZip no terminal (formato 7z: Copy, LZMA, LZMA2 e 7zAES)

USO:
  phxzipcmd listar    ARQUIVO.7z [--confiavel]
  phxzipcmd testar    ARQUIVO.7z [--confiavel]
  phxzipcmd extrair   ARQUIVO.7z [--destino PASTA] [--sobrescrever] [--confiavel]
  phxzipcmd compactar ARQUIVO.7z CAMINHO... [--copia] [--sobrescrever]
                      [--cifrar [--nomes-claros] [--ciclos N]]
  phxzipcmd --help | --version

A SENHA NUNCA VEM POR ARGUMENTO. --senha, --password, --pass, --pwd, -p (em
qualquer caixa, com ou sem = ou :) sao recusados: o argumento aparece no `ps`
de qualquer usuario e fica no historico do shell. Ela vem, nesta ordem:
  1. da variavel PHXZIP_SENHA;
  2. da entrada padrao: num terminal e perguntada SEM ECO (o /bin/stty desliga,
     e o eco volta no fim, no ctrl+D e no ctrl+C); num cano, e a primeira
     linha (ate 1024 bytes).

CAMINHO QUE COMECA COM HIFEN vai com ./ na frente: ./-planilhas, e nao
-planilhas (que seria lido como opcao, e -p... como senha por argumento).

  PHXZIP_SENHA='a senha' phxzipcmd extrair copia.7z --destino saida
  phxzipcmd compactar copia.7z docs --cifrar < arquivo-da-senha

A EXTRACAO NUNCA SAI DA PASTA DE DESTINO E NUNCA SEGUE LINK. Entrada marcada
como link simbolico vira arquivo comum, com aviso; pasta do destino que e
link recusa a entrada. Arquivo que ja existe recusa, a menos que venha
--sobrescrever (e ai o que se apaga e o arquivo ou o link, nunca o alvo).

Nenhuma pasta do caminho pode aceitar escrita de outro usuario (nem ser
dele): ele poderia trocar uma pasta por um link no meio da extracao. Extraia
numa pasta sua. Conteudo que veio cifrado nasce com permissao 0600.

A COMPACTACAO NAO SEGUE LINK: link e arquivo especial sao pulados, com aviso,
e arquivo trocado por link durante a leitura aborta. O .7z nasce 0600.

--confiavel abre com a folga do 7-Zip (ciclos ate 24, sem o teto de 1 GiB do
arquivo nem o de 4 GiB da soma descompactada). Use so para arquivo que voce
mesmo gravou.

SAIDA: 0 deu certo; 1 a operacao falhou; 2 uso errado.
";

/// A senha. O `Debug` e escrito a mao: derivado, um `dbg!` a imprimiria.
pub struct Senha(String);

impl Senha {
    pub fn texto(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Senha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Senha(oculta)")
    }
}

/// Por que o comando parou, e com qual codigo de saida.
#[derive(Debug, PartialEq, Eq)]
pub enum Falha {
    /// Uso errado: sai com [`SAIDA_USO`].
    Uso(String),
    /// A operacao falhou: sai com 1.
    Operacao(String),
}

impl Falha {
    pub fn codigo(&self) -> u8 {
        match self {
            Falha::Uso(_) => SAIDA_USO,
            Falha::Operacao(_) => 1,
        }
    }

    pub fn texto(&self) -> &str {
        match self {
            Falha::Uso(t) | Falha::Operacao(t) => t,
        }
    }
}

fn op(t: impl Into<String>) -> Falha {
    Falha::Operacao(t.into())
}

/// O erro do motor, com a frase dele e o nome estavel entre colchetes -- o
/// nome e o que um script decide; a frase pode melhorar de redacao.
fn do_motor(e: Erro) -> Falha {
    op(format!("{e} [{}]", e.nome()))
}

/// O primeiro argumento que traria a senha, pelo NOME da opcao -- nunca o
/// valor, que e justamente o que nao se repete na tela.
///
/// `-p` e a forma do 7-Zip (`-pSEGREDO`, colado): quem vem de la digita assim
/// por reflexo, e aceitar seria o mesmo furo com outra grafia.
///
/// Sem diferenca de caixa e com `=` ou `:` colados (`--SENHA=x`, `--pwd:x`):
/// a revisao SEC da Z9 achou `--pass=` e `--SENHA=` passando, e caindo na
/// «opcao desconhecida» -- que repetia o valor na tela.
pub fn argumento_de_senha(args: &[String]) -> Option<&'static str> {
    const NOMES: [(&str, &str); 6] = [
        ("senha", "--senha"),
        ("password", "--password"),
        ("passwd", "--passwd"),
        ("pass", "--pass"),
        ("pwd", "--pwd"),
        ("passphrase", "--passphrase"),
    ];
    for a in args {
        if !a.starts_with('-') {
            continue;
        }
        let nome = nome_da_opcao(a).to_lowercase();
        let sem_hifen = nome.trim_start_matches('-');
        if let Some((_, mostrado)) = NOMES.iter().find(|(n, _)| *n == sem_hifen) {
            return Some(mostrado);
        }
        if !a.starts_with("--") && (a.starts_with("-p") || a.starts_with("-P")) {
            return Some("-p");
        }
    }
    None
}

/// O nome de uma opcao, sem o valor colado por `=` ou `:` -- e so ele que
/// pode ir para a tela.
fn nome_da_opcao(a: &str) -> &str {
    a.split(['=', ':']).next().unwrap_or(a)
}

/// Le uma linha de senha de `r`, com teto. Sem `read_line` de proposito: ele
/// nao tem teto, e o teto e o que impede um cano sem quebra de linha de
/// encher a memoria.
pub fn ler_linha_de_senha<R: BufRead>(r: R) -> Result<Senha, Falha> {
    let mut bytes = Vec::new();
    let mut viu_algo = false;
    for b in r.take(TETO_DA_SENHA as u64 + 1).bytes() {
        let b = b.map_err(|e| op(format!("nao consegui ler a senha da entrada padrao: {e}")))?;
        viu_algo = true;
        if b == b'\n' {
            break;
        }
        if bytes.len() == TETO_DA_SENHA {
            return Err(op(format!(
                "a senha passa de {TETO_DA_SENHA} bytes sem quebra de linha"
            )));
        }
        bytes.push(b);
    }
    if !viu_algo {
        return Err(op(format!(
            "a entrada padrao terminou sem senha; use {ENV_SENHA} ou mande a senha pela entrada"
        )));
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    let s = String::from_utf8(bytes).map_err(|_| op("a senha nao e UTF-8"))?;
    if s.is_empty() {
        return Err(op("senha vazia"));
    }
    Ok(Senha(s))
}

/// A pergunta sem eco, feita por um `/bin/sh` filho.
///
/// Sem `unsafe` e sem crate nao ha `tcsetattr` nem tratador de sinal. Com o
/// `stty -echo` rodado DESTE processo, um ctrl+C matava o processo com o eco
/// desligado e o terminal ficava cego (revisao SEC da Z9). Aqui quem desliga o
/// eco e o shell filho, e e ELE que segura o sinal: o `trap ... EXIT` religa o
/// eco no fim normal, no ctrl+D (a leitura acaba vazia e o shell sai) e no
/// ctrl+C (o `trap ... INT` sai, e a saida dispara o EXIT) -- mesmo que este
/// processo ja tenha morrido pelo mesmo ctrl+C.
///
/// Quem le a linha e um `/usr/bin/head -n 1` filho do shell, e nao o `read` embutido:
/// medido no dash, o `read` embutido NAO e interrompido pelo ctrl+C com
/// `trap` -- o trap so roda quando chega a linha, e o terminal ficava cego
/// esperando um Enter. O `head` morre pelo sinal, o shell ve e roda o trap.
/// A senha passa pela entrada e pela saida do `head`, nunca por argumento, e
/// volta pelo cano do `stdout` do shell. `/bin/stty` pelo caminho inteiro: um
/// `stty` no `PATH` de quem chama nao decide se o eco desliga.
///
/// O que fica de fora, dito: um `kill -9` neste processo E no shell deixa o
/// eco desligado (nenhum processo sobra para religar); `stty echo` resolve.
const SCRIPT_SEM_ECO: &str = "trap '/bin/stty echo 2>/dev/null' EXIT; \
     trap 'exit 130' INT HUP TERM; \
     /bin/stty -echo 2>/dev/null || exit 3; \
     /usr/bin/head -n 1";

fn ler_sem_eco(pergunta: &str) -> Result<Senha, Falha> {
    let mut err = io::stderr();
    let _ = write!(err, "{pergunta}");
    let _ = err.flush();
    let filho = Command::new("/bin/sh")
        .arg("-c")
        .arg(SCRIPT_SEM_ECO)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut filho = match filho {
        Ok(f) => f,
        Err(_) => return Err(sem_eco()),
    };
    let r = match filho.stdout.take() {
        Some(cano) => ler_linha_de_senha(io::BufReader::new(cano)),
        None => Err(sem_eco()),
    };
    let codigo = filho.wait().ok().and_then(|s| s.code());
    let _ = writeln!(err);
    // Sem conseguir desligar o eco, NAO se pergunta: a senha apareceria na
    // tela, que e o furo que esta casca existe para fechar.
    if codigo == Some(3) {
        return Err(sem_eco());
    }
    r
}

fn sem_eco() -> Falha {
    op(format!(
        "nao consegui desligar o eco do terminal (/bin/stty), e a senha nao se digita a \
         vista; use {ENV_SENHA} ou mande a senha pela entrada padrao (phxzipcmd ... < arquivo)"
    ))
}

/// A senha: da variavel de ambiente, ou da entrada padrao.
fn obter_senha(confirmar: bool) -> Result<Senha, Falha> {
    if let Some(v) = std::env::var_os(ENV_SENHA) {
        let s = v
            .into_string()
            .map_err(|_| op(format!("{ENV_SENHA} nao e UTF-8")))?;
        if s.is_empty() {
            return Err(op(format!("{ENV_SENHA} esta definida e vazia")));
        }
        return Ok(Senha(s));
    }
    if io::stdin().is_terminal() {
        let s = ler_sem_eco("senha: ")?;
        if confirmar {
            let de_novo = ler_sem_eco("a mesma senha, de novo: ")?;
            if de_novo.0 != s.0 {
                return Err(op("as duas senhas digitadas nao conferem"));
            }
        }
        return Ok(s);
    }
    ler_linha_de_senha(io::stdin().lock())
}

/// Nome que veio de dentro do arquivo vai para a tela com o caractere de
/// controle escapado: um nome com `ESC [` reescreveria o terminal de quem lista.
///
/// E os de FORMATO tambem (categoria Cf): `fatura<U+202E>txt.exe` aparece na
/// tela como `fatura` + `exe.txt` -- o nome mente sobre a extensao sem um
/// unico caractere de controle (revisao SEC da Z9).
pub fn para_tela(nome: &str) -> String {
    let mut s = String::with_capacity(nome.len());
    for c in nome.chars() {
        if c.is_control() || de_formato(c) {
            s.push_str(&format!("\\u{{{:x}}}", c as u32));
        } else {
            s.push(c);
        }
    }
    s
}

/// Os caracteres de formato (Cf) do Unicode que mudam o que a tela mostra:
/// direcao (U+200E/F, U+202A-E, U+2066-9, U+061C), largura zero (U+200B-D,
/// U+2060-4, U+FEFF), hifen invisivel (U+00AD) e os de anotacao.
fn de_formato(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
    )
}

/// A saida da tela: TODO texto escrito passa pelo [`para_tela`], menos a
/// quebra de linha. Escapar so o nome que se lembrou de escapar deixava
/// passar o caminho cru dentro de uma mensagem de erro do disco (revisao SEC
/// da Z9); aqui a decisao e uma so, na porta.
struct Tela<'a>(&'a mut dyn Write);

impl Write for Tela<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let texto = String::from_utf8_lossy(buf);
        let mut limpo = String::with_capacity(texto.len());
        for (i, linha) in texto.split('\n').enumerate() {
            if i > 0 {
                limpo.push('\n');
            }
            limpo.push_str(&para_tela(linha));
        }
        self.0.write_all(limpo.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

// ------------------------------------------------------------- o pedido

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comando {
    Listar,
    Testar,
    Extrair,
    Compactar,
}

#[derive(Debug)]
struct Pedido {
    comando: Comando,
    arquivo: PathBuf,
    caminhos: Vec<PathBuf>,
    destino: Option<PathBuf>,
    sobrescrever: bool,
    confiavel: bool,
    cifrar: bool,
    nomes_claros: bool,
    copia: bool,
    ciclos: Option<u8>,
}

fn analisar(args: &[String]) -> Result<Pedido, Falha> {
    let uso = |t: String| Falha::Uso(format!("{t} (phxzipcmd --help)"));
    let (cmd, resto) = args
        .split_first()
        .ok_or_else(|| uso("falta o comando".into()))?;
    let comando = match cmd.as_str() {
        "listar" => Comando::Listar,
        "testar" => Comando::Testar,
        "extrair" => Comando::Extrair,
        "compactar" => Comando::Compactar,
        outro => return Err(uso(format!("comando desconhecido: {:?}", para_tela(outro)))),
    };
    let mut p = Pedido {
        comando,
        arquivo: PathBuf::new(),
        caminhos: Vec::new(),
        destino: None,
        sobrescrever: false,
        confiavel: false,
        cifrar: false,
        nomes_claros: false,
        copia: false,
        ciclos: None,
    };
    let mut posicionais = Vec::new();
    let mut i = 0;
    while i < resto.len() {
        let a = resto[i].as_str();
        i += 1;
        // Opcao que nao vale para o comando recusa, em vez de ser ignorada:
        // `extrair --cifrar` calado seria alguem achando que pediu algo.
        let so = |vale: &[Comando]| {
            if vale.contains(&comando) {
                Ok(())
            } else {
                Err(uso(format!("{a} nao vale para este comando")))
            }
        };
        match a {
            "--destino" => {
                so(&[Comando::Extrair])?;
                let v = resto
                    .get(i)
                    .ok_or_else(|| uso("--destino pede uma pasta".into()))?;
                i += 1;
                p.destino = Some(PathBuf::from(v));
            }
            "--sobrescrever" => {
                so(&[Comando::Extrair, Comando::Compactar])?;
                p.sobrescrever = true;
            }
            "--confiavel" => {
                so(&[Comando::Listar, Comando::Testar, Comando::Extrair])?;
                p.confiavel = true;
            }
            "--cifrar" => {
                so(&[Comando::Compactar])?;
                p.cifrar = true;
            }
            "--nomes-claros" => {
                so(&[Comando::Compactar])?;
                p.nomes_claros = true;
            }
            "--copia" => {
                so(&[Comando::Compactar])?;
                p.copia = true;
            }
            "--ciclos" => {
                so(&[Comando::Compactar])?;
                let v = resto
                    .get(i)
                    .ok_or_else(|| uso("--ciclos pede um numero".into()))?;
                i += 1;
                let n: u8 = v
                    .parse()
                    .ok()
                    .filter(|n| *n <= phxzip::CICLOS_MAXIMO)
                    .ok_or_else(|| {
                        uso(format!(
                            "--ciclos vai de 0 a {}, veio {:?}",
                            phxzip::CICLOS_MAXIMO,
                            para_tela(v)
                        ))
                    })?;
                p.ciclos = Some(n);
            }
            _ if a.starts_with('-') && a.len() > 1 => {
                // So o NOME: o que vem depois do `=` ou do `:` pode ser um segredo
                // digitado com a grafia errada.
                return Err(uso(format!(
                    "opcao desconhecida: {:?}",
                    para_tela(nome_da_opcao(a))
                )));
            }
            _ => posicionais.push(PathBuf::from(a)),
        }
    }
    if (p.nomes_claros || p.ciclos.is_some()) && !p.cifrar {
        return Err(uso("--nomes-claros e --ciclos so valem com --cifrar".into()));
    }
    let mut pos = posicionais.into_iter();
    p.arquivo = pos.next().ok_or_else(|| uso("falta o ARQUIVO.7z".into()))?;
    p.caminhos = pos.collect();
    match (comando, p.caminhos.is_empty()) {
        (Comando::Compactar, true) => return Err(uso("compactar pede ao menos um CAMINHO".into())),
        (Comando::Compactar, false) => {}
        (_, false) => return Err(uso("este comando recebe um ARQUIVO.7z so".into())),
        (_, true) => {}
    }
    Ok(p)
}

// ------------------------------------------------------------- abrir

fn limites(p: &Pedido) -> Limites {
    if p.confiavel {
        Limites::confiavel()
    } else {
        Limites::default()
    }
}

fn ler_arquivo(p: &Pedido) -> Result<Vec<u8>, Falha> {
    ler_com_teto(
        &p.arquivo,
        if p.confiavel {
            None
        } else {
            Some(TETO_DO_ARQUIVO)
        },
    )
}

/// Le o arquivo inteiro, parando em `teto + 1` bytes. O teto e medido no QUE
/// SE LE, e nao no tamanho que o sistema declara: FIFO, `/dev/zero` e os
/// arquivos do `/proc` declaram 0 e entregam quanto quiserem -- conferir o
/// `metadata().len()` e depois ler tudo deixava o `/dev/zero` encher a
/// memoria (revisao SEC da Z9).
pub fn ler_com_teto(caminho: &Path, teto: Option<u64>) -> Result<Vec<u8>, Falha> {
    let nome = caminho.display();
    let f = fs::File::open(caminho).map_err(|e| op(format!("{nome}: {e}")))?;
    let mut dados = Vec::new();
    match teto {
        None => {
            let mut f = f;
            f.read_to_end(&mut dados)
                .map_err(|e| op(format!("{nome}: {e}")))?;
        }
        Some(t) => {
            f.take(t.saturating_add(1))
                .read_to_end(&mut dados)
                .map_err(|e| op(format!("{nome}: {e}")))?;
            if dados.len() as u64 > t {
                return Err(op(format!(
                    "{nome}: passa do teto de {t} bytes; para arquivo que voce mesmo \
                     gravou, --confiavel"
                )));
            }
        }
    }
    Ok(dados)
}

/// Abre, pedindo a senha so quando o arquivo precisa: cabecalho cifrado
/// pede para listar; conteudo cifrado pede para testar e extrair.
fn abrir<'a>(dados: &'a [u8], p: &Pedido, conteudo: bool) -> Result<Arquivo<'a>, Falha> {
    let lim = limites(p);
    if std::env::var_os(ENV_SENHA).is_some() {
        let s = obter_senha(false)?;
        return Arquivo::abrir(dados, Some(s.texto()), lim).map_err(do_motor);
    }
    match Arquivo::abrir(dados, None, lim) {
        Err(Erro::SenhaAusente) => {}
        Ok(a) if conteudo && a.entradas().iter().any(|e| e.cifrada) => {}
        r => return r.map_err(do_motor),
    }
    let s = obter_senha(false)?;
    Arquivo::abrir(dados, Some(s.texto()), lim).map_err(do_motor)
}

fn data_para_tela(ft: Option<u64>) -> String {
    match ft {
        Some(ft) => {
            let ms = phxzip::filetime_para_unix(ft).saturating_mul(1000);
            let mut s = phxsql_core::datahora::instante_iso(ms);
            s.truncate(19);
            s
        }
        None => "-".repeat(19),
    }
}

fn listar(p: &Pedido, out: &mut dyn Write) -> Result<(), Falha> {
    let dados = ler_arquivo(p)?;
    let a = abrir(&dados, p, false)?;
    let mut total = 0u64;
    let _ = writeln!(out, "     tamanho  modificado (UTC)     tipo  nome");
    for e in a.entradas() {
        total = total.saturating_add(e.tamanho);
        let tipo = format!(
            "{}{}{}",
            if e.pasta { 'D' } else { '-' },
            if e.cifrada { 'C' } else { '-' },
            if marcado_como_link(e.atributos) {
                'L'
            } else {
                '-'
            }
        );
        let _ = writeln!(
            out,
            "{:>12}  {}  {tipo}   {}",
            e.tamanho,
            data_para_tela(e.modificado),
            para_tela(&e.nome)
        );
    }
    let _ = writeln!(
        out,
        "{} entrada(s), {total} bytes{}",
        a.entradas().len(),
        if a.cabecalho_cifrado() {
            ", cabecalho cifrado"
        } else {
            ""
        }
    );
    Ok(())
}

fn testar(p: &Pedido, out: &mut dyn Write) -> Result<(), Falha> {
    let dados = ler_arquivo(p)?;
    let mut a = abrir(&dados, p, true)?;
    let mut n = 0usize;
    // O CRC de cada entrada e conferido pelo motor antes de entrega-la.
    a.percorrer(|_, _| n += 1).map_err(do_motor)?;
    let _ = writeln!(out, "ok: {n} entrada(s) conferida(s)");
    Ok(())
}

fn extrair(p: &Pedido, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), Falha> {
    let dados = ler_arquivo(p)?;
    let mut a = abrir(&dados, p, true)?;
    // Toda a decisao de disco -- nome, colisao, link, sobrescrever -- e do
    // `Destino` do motor, o mesmo do tar. Aqui fica so o que e de tela.
    Destino::conferir_lote(a.entradas().iter().map(|e| (e.nome.as_str(), e.pasta)))
        .map_err(do_motor)?;
    let raiz = p.destino.clone().unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&raiz).map_err(|e| {
        op(format!(
            "nao consegui criar o destino {}: {e}",
            raiz.display()
        ))
    })?;
    let mut destino = Destino::novo(&raiz).map_err(do_motor)?;
    if p.sobrescrever {
        destino = destino.sobrescrevendo();
    }
    let mut falha = None;
    let mut gravadas = 0usize;
    let r = a.percorrer(|e, d| {
        if falha.is_some() {
            return;
        }
        if marcado_como_link(e.atributos) {
            let _ = writeln!(
                err,
                "aviso: {:?} veio marcada como link simbolico; gravada como arquivo comum \
                 (o PhxZipCmd nao cria link)",
                para_tela(&e.nome)
            );
        }
        let r = if e.pasta {
            destino.pasta(&e.nome)
        } else {
            let quando = e.modificado.map(phxzip::filetime_para_unix);
            if e.cifrada {
                destino.arquivo_privado(&e.nome, d, quando)
            } else {
                destino.arquivo(&e.nome, d, quando)
            }
        };
        match r {
            Ok(()) => {
                gravadas += 1;
                let _ = writeln!(out, "{}", para_tela(&e.nome));
            }
            Err(erro) => falha = Some(erro),
        }
    });
    let interrompida = |e: Erro| {
        op(format!(
            "{e} [{}]; extracao interrompida com {gravadas} entrada(s) ja gravada(s) em {}",
            e.nome(),
            raiz.display()
        ))
    };
    if let Some(e) = falha {
        return Err(interrompida(e));
    }
    if let Err(e) = r {
        return Err(interrompida(e));
    }
    let _ = writeln!(
        out,
        "{gravadas} entrada(s) extraida(s) em {}",
        raiz.display()
    );
    Ok(())
}

// ------------------------------------------------------------- compactar

fn data_de(m: &fs::Metadata) -> Option<u64> {
    let t = m.modified().ok()?;
    let s = match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).ok()?,
        Err(e) => -i64::try_from(e.duration().as_secs()).ok()?,
    };
    Some(phxzip::unix_para_filetime(s))
}

struct Coleta<'a> {
    esc: Escritor,
    saida: Option<PathBuf>,
    err: &'a mut dyn Write,
    entradas: usize,
}

impl Coleta<'_> {
    fn acrescentar(&mut self, caminho: &Path, nome: &str) -> Result<(), Falha> {
        let m =
            fs::symlink_metadata(caminho).map_err(|e| op(format!("{}: {e}", caminho.display())))?;
        let tipo = m.file_type();
        if tipo.is_symlink() {
            let _ = writeln!(
                self.err,
                "aviso: {} e link, pulado (o PhxZipCmd nao segue link)",
                caminho.display()
            );
            return Ok(());
        }
        if tipo.is_dir() {
            self.esc.pasta(nome, data_de(&m)).map_err(do_motor)?;
            self.entradas += 1;
            let mut filhos: Vec<_> = fs::read_dir(caminho)
                .map_err(|e| op(format!("{}: {e}", caminho.display())))?
                .map(|r| r.map(|d| d.file_name()))
                .collect::<Result<_, _>>()
                .map_err(|e| op(format!("{}: {e}", caminho.display())))?;
            // Ordem fixa: o mesmo diretorio da o mesmo arquivo, byte a byte.
            filhos.sort();
            for f in filhos {
                let texto = f.to_str().ok_or_else(|| {
                    op(format!(
                        "{}: nome que nao e UTF-8 dentro da pasta",
                        caminho.display()
                    ))
                })?;
                self.acrescentar(&caminho.join(&f), &format!("{nome}/{texto}"))?;
            }
            return Ok(());
        }
        if !tipo.is_file() {
            let _ = writeln!(
                self.err,
                "aviso: {} nao e arquivo comum nem pasta, pulado",
                caminho.display()
            );
            return Ok(());
        }
        // O proprio arquivo de saida, quando ele mora dentro do que se
        // compacta (com --sobrescrever): entraria a versao velha dentro da nova.
        if self.saida.is_some() && fs::canonicalize(caminho).ok() == self.saida {
            return Ok(());
        }
        let dados = ler_o_mesmo(caminho, &m)?;
        self.esc
            .arquivo(nome, &dados, data_de(&m))
            .map_err(do_motor)?;
        self.entradas += 1;
        Ok(())
    }
}

/// Le o arquivo que o `symlink_metadata` OLHOU, e nao o que estiver no nome
/// na hora de abrir. Entre olhar («arquivo comum») e o `fs::read` (que segue
/// link), quem escreve na pasta trocava o arquivo por um link para um segredo
/// de quem compacta -- medido 2 de 10 pela revisao SEC da Z9. Aqui o arquivo
/// e aberto, e o (dispositivo, inode) do DESCRITOR aberto tem de ser o do que
/// foi olhado; o conteudo sai do descritor, que nao troca mais de alvo.
///
/// No Windows a `std` estavel nao expoe o indice do arquivo: la confere-se so
/// que o aberto e arquivo comum, e a troca por link fica dita aqui.
fn ler_o_mesmo(caminho: &Path, olhado: &fs::Metadata) -> Result<Vec<u8>, Falha> {
    let nome = caminho.display();
    let mut f = fs::File::open(caminho).map_err(|e| op(format!("{nome}: {e}")))?;
    let aberto = f.metadata().map_err(|e| op(format!("{nome}: {e}")))?;
    #[cfg(unix)]
    let mesmo = {
        use std::os::unix::fs::MetadataExt;
        aberto.dev() == olhado.dev() && aberto.ino() == olhado.ino()
    };
    #[cfg(not(unix))]
    let mesmo = {
        let _ = olhado;
        true
    };
    if !mesmo || !aberto.is_file() {
        return Err(op(format!(
            "{nome}: trocado durante a compactacao (o que se abriu nao e o arquivo que se \
             olhou -- um link posto no lugar?); nada foi gravado"
        )));
    }
    let mut dados = Vec::new();
    f.read_to_end(&mut dados)
        .map_err(|e| op(format!("{nome}: {e}")))?;
    Ok(dados)
}

fn nome_de_raiz(c: &Path) -> Result<String, Falha> {
    let nome = match c.file_name() {
        Some(n) => n.to_os_string(),
        None => fs::canonicalize(c)
            .ok()
            .and_then(|a| a.file_name().map(|n| n.to_os_string()))
            .ok_or_else(|| op(format!("{}: caminho sem nome para guardar", c.display())))?,
    };
    nome.into_string()
        .map_err(|_| op(format!("{}: nome que nao e UTF-8", c.display())))
}

fn gravar_arquivo_novo(destino: &Path, bytes: &[u8]) -> Result<(), Falha> {
    let mut parcial = destino.as_os_str().to_os_string();
    parcial.push(format!(".parcial-{}", std::process::id()));
    let parcial = PathBuf::from(parcial);
    let r = (|| -> io::Result<()> {
        let mut abrir = fs::OpenOptions::new();
        abrir.write(true).create_new(true);
        // 0600 desde o nascimento, e nao um `chmod` depois: o 7z junta
        // arquivos de permissoes diferentes, e o mais estreito que nao alarga
        // nenhuma delas e o do dono so. Quem quer abrir para o grupo da o
        // `chmod` de proposito (revisao SEC da Z9).
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            abrir.mode(0o600);
        }
        let mut f = abrir.open(&parcial)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        // `rename` sobre um link troca o LINK, nunca escreve no alvo dele.
        fs::rename(&parcial, destino)
    })();
    r.map_err(|e| {
        let _ = fs::remove_file(&parcial);
        op(format!("{}: {e}", destino.display()))
    })
}

fn compactar(p: &Pedido, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), Falha> {
    if fs::symlink_metadata(&p.arquivo).is_ok() && !p.sobrescrever {
        return Err(op(format!(
            "{} ja existe; use --sobrescrever",
            p.arquivo.display()
        )));
    }
    let senha = if p.cifrar {
        Some(obter_senha(true)?)
    } else {
        None
    };
    let esc = Escritor::novo(Opcoes {
        metodo: if p.copia {
            Metodo::Copia
        } else {
            Metodo::Lzma2
        },
        senha: senha.as_ref().map(|s| s.texto().to_string()),
        ciclos: p.ciclos.unwrap_or(phxzip::CICLOS_PADRAO),
        cifrar_cabecalho: !p.nomes_claros,
    })
    .map_err(do_motor)?;
    let mut coleta = Coleta {
        esc,
        saida: fs::canonicalize(&p.arquivo).ok(),
        err,
        entradas: 0,
    };
    for c in &p.caminhos {
        let nome = nome_de_raiz(c)?;
        coleta.acrescentar(c, &nome)?;
    }
    if coleta.entradas == 0 {
        return Err(op("nada para compactar: so havia link ou arquivo especial"));
    }
    let n = coleta.entradas;
    let bytes = coleta.esc.terminar();
    gravar_arquivo_novo(&p.arquivo, &bytes)?;
    let _ = writeln!(
        out,
        "{n} entrada(s) em {}, {} bytes{}",
        p.arquivo.display(),
        bytes.len(),
        if p.cifrar { ", cifrado" } else { "" }
    );
    Ok(())
}

/// Roda o comando. `args` sem o nome do programa. Devolve o codigo de saida.
pub fn rodar(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut out = Tela(out);
    let mut err = Tela(err);
    let (out, err): (&mut dyn Write, &mut dyn Write) = (&mut out, &mut err);
    // PRIMEIRO, antes do --help e de qualquer leitura: a recusa nao pode
    // depender de o resto da linha estar certo.
    if let Some(opcao) = argumento_de_senha(args) {
        let _ = writeln!(
            err,
            "phxzipcmd: {opcao} recusado -- a senha nunca vem por argumento, porque o \
             argumento aparece no `ps` de qualquer usuario e fica no historico do shell.\n\
             Use a variavel {ENV_SENHA}, ou deixe o phxzipcmd perguntar (sem eco), ou mande \
             a senha pela entrada padrao: phxzipcmd ... < arquivo-da-senha"
        );
        return SAIDA_USO;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        let _ = writeln!(out, "{}", phxsql_core::versao_completa("phxzipcmd"));
        return 0;
    }
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        let _ = write!(out, "{USO}");
        return if args.is_empty() { SAIDA_USO } else { 0 };
    }
    let r = analisar(args).and_then(|p| match p.comando {
        Comando::Listar => listar(&p, out),
        Comando::Testar => testar(&p, out),
        Comando::Extrair => extrair(&p, out, err),
        Comando::Compactar => compactar(&p, out, err),
    });
    match r {
        Ok(()) => 0,
        Err(f) => {
            let _ = writeln!(err, "phxzipcmd: {}", f.texto());
            f.codigo()
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn toda_grafia_da_senha_por_argumento_e_achada() {
        for (linha, opcao) in [
            (&["extrair", "x.7z", "--senha", "s"][..], "--senha"),
            (&["extrair", "--senha=s", "x.7z"][..], "--senha"),
            (&["listar", "x.7z", "--password", "s"][..], "--password"),
            (&["listar", "x.7z", "--password=s"][..], "--password"),
            (&["testar", "-psegredo", "x.7z"][..], "-p"),
            (&["testar", "-p", "x.7z"][..], "-p"),
            // Revisao SEC da Z9: caixa, `:` e os nomes de outros programas.
            (&["listar", "x.7z", "--SENHA=s"][..], "--senha"),
            (&["listar", "x.7z", "--Password:s"][..], "--password"),
            (&["listar", "x.7z", "--pass=s"][..], "--pass"),
            (&["listar", "x.7z", "--pwd:s"][..], "--pwd"),
            (&["listar", "x.7z", "--passwd", "s"][..], "--passwd"),
            (&["listar", "x.7z", "-Psegredo"][..], "-p"),
        ] {
            assert_eq!(argumento_de_senha(&a(linha)), Some(opcao), "{linha:?}");
        }
        // O irmao: linha sem senha nao e recusada, senao o portao recusaria tudo.
        assert_eq!(
            argumento_de_senha(&a(&["extrair", "x.7z", "--destino", "p", "--sobrescrever"])),
            None
        );
    }

    #[test]
    fn linha_de_senha_tem_teto_e_tira_a_quebra() {
        assert_eq!(
            ler_linha_de_senha(&b"abc\r\nresto"[..]).unwrap().texto(),
            "abc"
        );
        assert_eq!(ler_linha_de_senha(&b"abc"[..]).unwrap().texto(), "abc");
        assert!(ler_linha_de_senha(&b""[..]).is_err());
        assert!(ler_linha_de_senha(&b"\n"[..]).is_err());
        let longa = vec![b'x'; TETO_DA_SENHA + 10];
        assert!(ler_linha_de_senha(&longa[..]).is_err());
        let no_teto = vec![b'x'; TETO_DA_SENHA];
        assert_eq!(
            ler_linha_de_senha(&no_teto[..]).unwrap().texto().len(),
            TETO_DA_SENHA
        );
    }

    /// O teto vale no que se LE: `/dev/zero` declara 0 bytes e nunca acaba, e
    /// o `/proc` declara 0 e entrega o que tem.
    ///
    /// Prova real: com a conferencia antiga (`metadata().len()` e depois
    /// `fs::read`), o `/proc/self/maps` de 0 bytes declarados passa do teto
    /// de 64 e volta inteiro, e o teste cai.
    #[cfg(target_os = "linux")]
    #[test]
    fn o_teto_do_arquivo_vale_no_que_se_le() {
        assert!(ler_com_teto(Path::new("/proc/self/maps"), Some(64)).is_err());
        let ok = ler_com_teto(Path::new("/proc/self/maps"), Some(1 << 30)).unwrap();
        assert!(ok.len() > 64, "o irmao: sob o teto le inteiro");
        // Por ultimo: com a conferencia antiga, este nao acaba nunca.
        assert!(ler_com_teto(Path::new("/dev/zero"), Some(1024)).is_err());
    }

    /// Caractere de formato muda o que a tela mostra sem ser controle.
    ///
    /// Prova real: sem o `de_formato`, o U+202E passa cru e o teste cai.
    #[test]
    fn caractere_de_formato_sai_escapado() {
        assert_eq!(para_tela("fatura\u{202E}txt.exe"), "fatura\\u{202e}txt.exe");
        for c in ['\u{200E}', '\u{200F}', '\u{2066}', '\u{2069}', '\u{FEFF}'] {
            assert!(!para_tela(&format!("a{c}b")).contains(c), "{:x}", c as u32);
        }
        assert_eq!(para_tela("ação.txt"), "ação.txt");
    }

    #[test]
    fn o_debug_da_senha_nao_a_mostra() {
        let s = Senha("s3nh4-secreta".into());
        assert!(!format!("{s:?}").contains("s3nh4"));
    }

    #[test]
    fn nome_com_controle_sai_escapado() {
        assert_eq!(para_tela("a\x1b[2Jb"), "a\\u{1b}[2Jb");
    }
}
