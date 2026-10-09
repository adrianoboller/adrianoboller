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

use phxsql_core::idiomas::{idioma_do_ambiente, Fala};
use phxzip::disco::{marcado_como_link, Destino};
use phxzip::{Arquivo, Erro, Escritor, Limites, Metodo, Opcoes};

pub mod textos;

/// A variavel de ambiente da senha. Documentada no `--help`.
pub const ENV_SENHA: &str = "PHXZIP_SENHA";

/// A variavel de ambiente do idioma; sem ela, o `LANG` do sistema. Documentada
/// no `--help`.
pub const ENV_IDIOMA: &str = "PHXZIP_IDIOMA";

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

/// O `--help`, montado pela fabrica: cada paragrafo e uma chave, para o
/// tradutor trabalhar paragrafo a paragrafo e uma frase nova nao invalidar a
/// traducao do resto.
fn uso(f: Fala) -> String {
    let teto = TETO_DA_SENHA.to_string();
    let var = [("var", ENV_SENHA), ("teto", teto.as_str())];
    let idioma = [("var", ENV_IDIOMA)];
    let paragrafos = [
        f.texto("zipcmd.uso_titulo", &[]),
        f.texto("zipcmd.uso_comandos", &[]),
        f.texto("zipcmd.uso_senha", &var),
        f.texto("zipcmd.uso_hifen", &[]),
        f.texto("zipcmd.uso_exemplos", &var),
        f.texto("zipcmd.uso_extracao", &[]),
        f.texto("zipcmd.uso_pasta_alheia", &[]),
        f.texto("zipcmd.uso_compactacao", &[]),
        f.texto("zipcmd.uso_confiavel", &[]),
        f.texto("zipcmd.uso_idioma", &idioma),
        f.texto("zipcmd.uso_saida", &[]),
    ];
    let mut s = paragrafos.join("\n\n");
    s.push('\n');
    s
}

/// O idioma de quem roda: [`ENV_IDIOMA`], senao o `LANG`. Desconhecido fala
/// portugues -- a decisao e do motor do core, a mesma do servidor.
pub fn fala_do_ambiente() -> Fala {
    let pedido = std::env::var(ENV_IDIOMA).ok();
    let lang = std::env::var("LANG").ok();
    let idioma = idioma_do_ambiente(&[pedido.as_deref(), lang.as_deref()]);
    Fala::nova(textos::FABRICA, idioma)
}

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
pub fn ler_linha_de_senha<R: BufRead>(f: Fala, r: R) -> Result<Senha, Falha> {
    let mut bytes = Vec::new();
    let mut viu_algo = false;
    for b in r.take(TETO_DA_SENHA as u64 + 1).bytes() {
        let b =
            b.map_err(|e| op(f.texto("zipcmd.senha_ler_falhou", &[("erro", &e.to_string())])))?;
        viu_algo = true;
        if b == b'\n' {
            break;
        }
        if bytes.len() == TETO_DA_SENHA {
            return Err(op(f.texto(
                "zipcmd.senha_longa",
                &[("teto", &TETO_DA_SENHA.to_string())],
            )));
        }
        bytes.push(b);
    }
    if !viu_algo {
        return Err(op(f.texto("zipcmd.senha_ausente", &[("var", ENV_SENHA)])));
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    let s = String::from_utf8(bytes).map_err(|_| op(f.texto("zipcmd.senha_nao_utf8", &[])))?;
    if s.is_empty() {
        return Err(op(f.texto("zipcmd.senha_vazia", &[])));
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
/// O `trap '' HUP` e o pedido 754. Este processo e, no terminal, quem morre
/// primeiro pelo ctrl+C; se ele lidera a sessao (o caso do pseudo-terminal, e
/// do programa rodado direto pelo `ssh`), a morte dele manda SIGHUP ao grupo
/// da frente -- e o SIGHUP chegava no meio do `trap ... EXIT`, matando o
/// `stty echo` que religava o eco. Medido com o `trap ... HUP` antigo: 5 de
/// 50 corridas com o eco AINDA desligado 2 s depois; com o HUP ignorado (o
/// shell e os filhos herdam o SIG_IGN), 0 de 50, e 0 de 50 sob carga. Num
/// desligamento de verdade o terminal some, o `head` le o fim e sai, e o
/// shell sai com ele: ignorar o HUP nao deixa processo pendurado.
///
/// O que fica de fora, dito: um `kill -9` neste processo E no shell deixa o
/// eco desligado (nenhum processo sobra para religar); `stty echo` resolve.
const SCRIPT_SEM_ECO: &str = "trap '' HUP; \
     trap '/bin/stty echo 2>/dev/null' EXIT; \
     trap 'exit 130' INT TERM; \
     /bin/stty -echo 2>/dev/null || exit 3; \
     /usr/bin/head -n 1";

fn ler_sem_eco(f: Fala, pergunta: &str) -> Result<Senha, Falha> {
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
        Err(_) => return Err(sem_eco(f)),
    };
    let r = match filho.stdout.take() {
        Some(cano) => ler_linha_de_senha(f, io::BufReader::new(cano)),
        None => Err(sem_eco(f)),
    };
    let codigo = filho.wait().ok().and_then(|s| s.code());
    let _ = writeln!(err);
    // Sem conseguir desligar o eco, NAO se pergunta: a senha apareceria na
    // tela, que e o furo que esta casca existe para fechar.
    if codigo == Some(3) {
        return Err(sem_eco(f));
    }
    r
}

fn sem_eco(f: Fala) -> Falha {
    op(f.texto("zipcmd.sem_eco", &[("var", ENV_SENHA)]))
}

/// A senha: da variavel de ambiente, ou da entrada padrao.
fn obter_senha(f: Fala, confirmar: bool) -> Result<Senha, Falha> {
    if let Some(v) = std::env::var_os(ENV_SENHA) {
        let s = v
            .into_string()
            .map_err(|_| op(f.texto("zipcmd.var_nao_utf8", &[("var", ENV_SENHA)])))?;
        if s.is_empty() {
            return Err(op(f.texto("zipcmd.var_vazia", &[("var", ENV_SENHA)])));
        }
        return Ok(Senha(s));
    }
    if io::stdin().is_terminal() {
        let s = ler_sem_eco(f, &f.texto("zipcmd.pergunta_senha", &[]))?;
        if confirmar {
            let de_novo = ler_sem_eco(f, &f.texto("zipcmd.pergunta_de_novo", &[]))?;
            if de_novo.0 != s.0 {
                return Err(op(f.texto("zipcmd.senhas_diferentes", &[])));
            }
        }
        return Ok(s);
    }
    ler_linha_de_senha(f, io::stdin().lock())
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

struct Pedido {
    /// O idioma da tela -- viaja no pedido para toda mensagem sair dele.
    fala: Fala,
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

fn analisar(f: Fala, args: &[String]) -> Result<Pedido, Falha> {
    let uso = |t: String| Falha::Uso(format!("{t} (phxzipcmd --help)"));
    let (cmd, resto) = args
        .split_first()
        .ok_or_else(|| uso(f.texto("zipcmd.falta_comando", &[])))?;
    let comando = match cmd.as_str() {
        "listar" => Comando::Listar,
        "testar" => Comando::Testar,
        "extrair" => Comando::Extrair,
        "compactar" => Comando::Compactar,
        outro => {
            return Err(uso(f.texto(
                "zipcmd.comando_desconhecido",
                &[("comando", &format!("{:?}", para_tela(outro)))],
            )))
        }
    };
    let mut p = Pedido {
        fala: f,
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
                Err(uso(f.texto("zipcmd.opcao_nao_vale", &[("opcao", a)])))
            }
        };
        match a {
            "--destino" => {
                so(&[Comando::Extrair])?;
                let v = resto
                    .get(i)
                    .ok_or_else(|| uso(f.texto("zipcmd.destino_pede_pasta", &[])))?;
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
                    .ok_or_else(|| uso(f.texto("zipcmd.ciclos_pede_numero", &[])))?;
                i += 1;
                let n: u8 = v
                    .parse()
                    .ok()
                    .filter(|n| *n <= phxzip::CICLOS_MAXIMO)
                    .ok_or_else(|| {
                        uso(f.texto(
                            "zipcmd.ciclos_fora",
                            &[
                                ("maximo", &phxzip::CICLOS_MAXIMO.to_string()),
                                ("veio", &format!("{:?}", para_tela(v))),
                            ],
                        ))
                    })?;
                p.ciclos = Some(n);
            }
            _ if a.starts_with('-') && a.len() > 1 => {
                // So o NOME: o que vem depois do `=` ou do `:` pode ser um segredo
                // digitado com a grafia errada.
                return Err(uso(f.texto(
                    "zipcmd.opcao_desconhecida",
                    &[("opcao", &format!("{:?}", para_tela(nome_da_opcao(a))))],
                )));
            }
            _ => posicionais.push(PathBuf::from(a)),
        }
    }
    if (p.nomes_claros || p.ciclos.is_some()) && !p.cifrar {
        return Err(uso(f.texto("zipcmd.so_com_cifrar", &[])));
    }
    let mut pos = posicionais.into_iter();
    p.arquivo = pos
        .next()
        .ok_or_else(|| uso(f.texto("zipcmd.falta_arquivo", &[])))?;
    p.caminhos = pos.collect();
    match (comando, p.caminhos.is_empty()) {
        (Comando::Compactar, true) => return Err(uso(f.texto("zipcmd.falta_caminho", &[]))),
        (Comando::Compactar, false) => {}
        (_, false) => return Err(uso(f.texto("zipcmd.arquivo_so_um", &[]))),
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
        p.fala,
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
pub fn ler_com_teto(f: Fala, caminho: &Path, teto: Option<u64>) -> Result<Vec<u8>, Falha> {
    let nome = caminho.display();
    let arq = fs::File::open(caminho).map_err(|e| op(format!("{nome}: {e}")))?;
    let mut dados = Vec::new();
    match teto {
        None => {
            let mut arq = arq;
            arq.read_to_end(&mut dados)
                .map_err(|e| op(format!("{nome}: {e}")))?;
        }
        Some(t) => {
            arq.take(t.saturating_add(1))
                .read_to_end(&mut dados)
                .map_err(|e| op(format!("{nome}: {e}")))?;
            if dados.len() as u64 > t {
                return Err(op(f.texto(
                    "zipcmd.passa_do_teto",
                    &[("nome", &nome.to_string()), ("teto", &t.to_string())],
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
        let s = obter_senha(p.fala, false)?;
        return Arquivo::abrir(dados, Some(s.texto()), lim).map_err(do_motor);
    }
    match Arquivo::abrir(dados, None, lim) {
        Err(Erro::SenhaAusente) => {}
        Ok(a) if conteudo && a.entradas().iter().any(|e| e.cifrada) => {}
        r => return r.map_err(do_motor),
    }
    let s = obter_senha(p.fala, false)?;
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
    let f = p.fala;
    // As larguras sao as das linhas abaixo; o rotulo e da fabrica.
    let _ = writeln!(
        out,
        "{:>12}  {:<19}  {:<4}  {}",
        f.texto("zipcmd.col_tamanho", &[]),
        f.texto("zipcmd.col_modificado", &[]),
        f.texto("zipcmd.col_tipo", &[]),
        f.texto("zipcmd.col_nome", &[])
    );
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
        "{}{}",
        f.texto(
            "zipcmd.total_listado",
            &[
                ("entradas", &a.entradas().len().to_string()),
                ("bytes", &total.to_string()),
            ],
        ),
        if a.cabecalho_cifrado() {
            f.texto("zipcmd.sufixo_cabecalho_cifrado", &[])
        } else {
            String::new()
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
    let _ = writeln!(
        out,
        "{}",
        p.fala.texto("zipcmd.testar_ok", &[("n", &n.to_string())])
    );
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
    let f = p.fala;
    fs::create_dir_all(&raiz).map_err(|e| {
        op(f.texto(
            "zipcmd.destino_falhou",
            &[
                ("destino", &raiz.display().to_string()),
                ("erro", &e.to_string()),
            ],
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
                "{}",
                f.texto(
                    "zipcmd.aviso_link_gravado_comum",
                    &[("nome", &format!("{:?}", para_tela(&e.nome)))],
                )
            );
        }
        match destino.entrada(e, d) {
            Ok(()) => {
                gravadas += 1;
                let _ = writeln!(out, "{}", para_tela(&e.nome));
            }
            Err(erro) => falha = Some(erro),
        }
    });
    let interrompida = |e: Erro| {
        op(f.texto(
            "zipcmd.extracao_interrompida",
            &[
                ("erro", &format!("{e} [{}]", e.nome())),
                ("gravadas", &gravadas.to_string()),
                ("destino", &raiz.display().to_string()),
            ],
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
        "{}",
        f.texto(
            "zipcmd.extraidas",
            &[
                ("n", &gravadas.to_string()),
                ("destino", &raiz.display().to_string()),
            ],
        )
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
    fala: Fala,
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
                "{}",
                self.fala.texto(
                    "zipcmd.aviso_link_pulado",
                    &[("caminho", &caminho.display().to_string())],
                )
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
                    op(self.fala.texto(
                        "zipcmd.nome_nao_utf8_na_pasta",
                        &[("caminho", &caminho.display().to_string())],
                    ))
                })?;
                self.acrescentar(&caminho.join(&f), &format!("{nome}/{texto}"))?;
            }
            return Ok(());
        }
        if !tipo.is_file() {
            let _ = writeln!(
                self.err,
                "{}",
                self.fala.texto(
                    "zipcmd.aviso_especial_pulado",
                    &[("caminho", &caminho.display().to_string())],
                )
            );
            return Ok(());
        }
        // O proprio arquivo de saida, quando ele mora dentro do que se
        // compacta (com --sobrescrever): entraria a versao velha dentro da nova.
        if self.saida.is_some() && fs::canonicalize(caminho).ok() == self.saida {
            return Ok(());
        }
        let dados = ler_o_mesmo(self.fala, caminho, &m)?;
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
fn ler_o_mesmo(fala: Fala, caminho: &Path, olhado: &fs::Metadata) -> Result<Vec<u8>, Falha> {
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
        return Err(op(
            fala.texto("zipcmd.trocado_durante", &[("nome", &nome.to_string())])
        ));
    }
    let mut dados = Vec::new();
    f.read_to_end(&mut dados)
        .map_err(|e| op(format!("{nome}: {e}")))?;
    Ok(dados)
}

fn nome_de_raiz(f: Fala, c: &Path) -> Result<String, Falha> {
    let nome = match c.file_name() {
        Some(n) => n.to_os_string(),
        None => fs::canonicalize(c)
            .ok()
            .and_then(|a| a.file_name().map(|n| n.to_os_string()))
            .ok_or_else(|| {
                op(f.texto(
                    "zipcmd.caminho_sem_nome",
                    &[("caminho", &c.display().to_string())],
                ))
            })?,
    };
    nome.into_string().map_err(|_| {
        op(f.texto(
            "zipcmd.nome_nao_utf8",
            &[("caminho", &c.display().to_string())],
        ))
    })
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
    let f = p.fala;
    if fs::symlink_metadata(&p.arquivo).is_ok() && !p.sobrescrever {
        return Err(op(f.texto(
            "zipcmd.ja_existe",
            &[("arquivo", &p.arquivo.display().to_string())],
        )));
    }
    let senha = if p.cifrar {
        Some(obter_senha(f, true)?)
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
        fala: f,
        esc,
        saida: fs::canonicalize(&p.arquivo).ok(),
        err,
        entradas: 0,
    };
    for c in &p.caminhos {
        let nome = nome_de_raiz(f, c)?;
        coleta.acrescentar(c, &nome)?;
    }
    if coleta.entradas == 0 {
        return Err(op(f.texto("zipcmd.nada_para_compactar", &[])));
    }
    let n = coleta.entradas;
    let bytes = coleta.esc.terminar();
    gravar_arquivo_novo(&p.arquivo, &bytes)?;
    let _ = writeln!(
        out,
        "{}{}",
        f.texto(
            "zipcmd.compactado",
            &[
                ("n", &n.to_string()),
                ("arquivo", &p.arquivo.display().to_string()),
                ("bytes", &bytes.len().to_string()),
            ],
        ),
        if p.cifrar {
            f.texto("zipcmd.sufixo_cifrado", &[])
        } else {
            String::new()
        }
    );
    Ok(())
}

/// Roda o comando. `args` sem o nome do programa. Devolve o codigo de saida.
/// O idioma vem do ambiente ([`fala_do_ambiente`]).
pub fn rodar(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    rodar_em(fala_do_ambiente(), args, out, err)
}

/// O mesmo [`rodar`], com o idioma dado -- para o teste nao depender do `LANG`
/// de quem roda a suite.
pub fn rodar_em(f: Fala, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut out = Tela(out);
    let mut err = Tela(err);
    let (out, err): (&mut dyn Write, &mut dyn Write) = (&mut out, &mut err);
    // PRIMEIRO, antes do --help e de qualquer leitura: a recusa nao pode
    // depender de o resto da linha estar certo.
    if let Some(opcao) = argumento_de_senha(args) {
        let _ = writeln!(
            err,
            "phxzipcmd: {}",
            f.texto(
                "zipcmd.senha_por_argumento",
                &[("opcao", opcao), ("var", ENV_SENHA)],
            )
        );
        return SAIDA_USO;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        let _ = writeln!(out, "{}", phxsql_core::versao_completa("phxzipcmd"));
        return 0;
    }
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        let _ = write!(out, "{}", uso(f));
        return if args.is_empty() { SAIDA_USO } else { 0 };
    }
    let r = analisar(f, args).and_then(|p| match p.comando {
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

    fn pt() -> Fala {
        Fala::nova(textos::FABRICA, 0)
    }

    fn fala(idioma: &str) -> Fala {
        Fala::nova(
            textos::FABRICA,
            phxsql_core::idiomas::indice_do_idioma(idioma),
        )
    }

    /// As chaves que este crate pede, varridas do FONTE -- e nao de uma lista
    /// escrita a mao, que envelheceria calada. A tabela mora no `textos.rs`,
    /// fora da varredura, para nao se contar como pedida por ela mesma.
    fn chaves_pedidas() -> std::collections::HashSet<String> {
        let mut usadas = std::collections::HashSet::new();
        for fonte in [include_str!("lib.rs"), include_str!("main.rs")] {
            usadas.extend(phxsql_core::idiomas::chaves_no_fonte(
                fonte,
                textos::PREFIXO,
            ));
        }
        usadas
    }

    /// **RED da Z2, o laco.** Chave pedida que nao existe sai na tela como a
    /// chave crua; chave morta e traducao que ninguem ve. Os dois acusam.
    ///
    /// Prova real: trocar no fonte a chave `senha_vazia` por `senha_vazia_x`
    /// reprova pelos DOIS lados (a nova falta, a velha morre); apagar uma
    /// linha da tabela reprova pelo primeiro. (Chave entre aspas neste
    /// comentario contaria como pedida -- a varredura e do fonte inteiro.)
    #[test]
    fn o_laco_de_chaves_fecha() {
        let pedidas = chaves_pedidas();
        assert!(
            pedidas.len() > 40,
            "so {} chaves no fonte: o laco se soltou",
            pedidas.len()
        );
        let laco = phxsql_core::idiomas::conferir_laco(textos::FABRICA, &pedidas);
        assert!(
            laco.faltando.is_empty(),
            "o fonte pede chaves que a tabela nao tem: {:?}",
            laco.faltando
        );
        assert!(
            laco.mortas.is_empty(),
            "a tabela tem chaves que ninguem pede: {:?}",
            laco.mortas
        );
    }

    #[test]
    fn a_tabela_e_bem_formada() {
        let defeitos = phxsql_core::idiomas::defeitos_da_tabela(textos::FABRICA, textos::PREFIXO);
        assert!(defeitos.is_empty(), "{defeitos:#?}");
    }

    /// Os seis idiomas tem toda chave traduzida -- nenhuma celula cai no
    /// portugues por falta. Se um dia uma ficar vazia de proposito, o numero
    /// aqui sobe com o motivo; calada, nao.
    #[test]
    fn nenhuma_celula_vazia() {
        let vazias = phxsql_core::idiomas::vazias_por_idioma(textos::FABRICA);
        assert_eq!(vazias, [0; phxsql_core::idiomas::QUANTOS]);
    }

    /// O idioma pedido manda, e o desconhecido fala portugues -- pelo `rodar`
    /// inteiro, nao so pelo motor.
    #[test]
    fn o_comando_fala_o_idioma_pedido() {
        let roda = |f: Fala, args: &[&str]| {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let c = rodar_em(f, &a(args), &mut out, &mut err);
            let t = String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap();
            (c, t)
        };
        let (c, t) = roda(fala("Ingles"), &["listar"]);
        assert_eq!(c, SAIDA_USO);
        assert_eq!(t, "phxzipcmd: missing the ARCHIVE.7z (phxzipcmd --help)\n");
        let (_, t) = roda(fala("Alemao"), &["--help"]);
        assert!(t.starts_with("phxzipcmd -- PhxZip im Terminal"), "{t}");
        assert!(
            t.contains("PHXZIP_SENHA") && t.contains("PHXZIP_IDIOMA"),
            "{t}"
        );
        let (_, t) = roda(fala("Klingon"), &["foo"]);
        assert_eq!(
            t,
            "phxzipcmd: comando desconhecido: \"foo\" (phxzipcmd --help)\n"
        );
        // A recusa da senha tambem traduz, e continua sem repetir o valor.
        let (c, t) = roda(fala("Espanhol"), &["listar", "x.7z", "--senha=s3gr3d0"]);
        assert_eq!(c, SAIDA_USO);
        assert!(t.contains("nunca viene por argumento"), "{t}");
        assert!(!t.contains("s3gr3d0"), "{t}");
    }

    /// O `--help` em portugues e o de antes da fabrica, linha a linha nas
    /// partes que um script ou um teste procuram.
    #[test]
    fn o_help_em_portugues_e_o_de_sempre() {
        let u = uso(pt());
        assert!(u.starts_with("phxzipcmd -- o PhxZip no terminal (formato 7z"));
        assert!(u.contains("  1. da variavel PHXZIP_SENHA;\n"));
        assert!(u.contains("     linha (ate 1024 bytes).\n\nCAMINHO QUE COMECA"));
        assert!(u.ends_with("SAIDA: 0 deu certo; 1 a operacao falhou; 2 uso errado.\n"));
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
            ler_linha_de_senha(pt(), &b"abc\r\nresto"[..])
                .unwrap()
                .texto(),
            "abc"
        );
        assert_eq!(
            ler_linha_de_senha(pt(), &b"abc"[..]).unwrap().texto(),
            "abc"
        );
        assert!(ler_linha_de_senha(pt(), &b""[..]).is_err());
        assert!(ler_linha_de_senha(pt(), &b"\n"[..]).is_err());
        let longa = vec![b'x'; TETO_DA_SENHA + 10];
        assert!(ler_linha_de_senha(pt(), &longa[..]).is_err());
        let no_teto = vec![b'x'; TETO_DA_SENHA];
        assert_eq!(
            ler_linha_de_senha(pt(), &no_teto[..])
                .unwrap()
                .texto()
                .len(),
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
        assert!(ler_com_teto(pt(), Path::new("/proc/self/maps"), Some(64)).is_err());
        let ok = ler_com_teto(pt(), Path::new("/proc/self/maps"), Some(1 << 30)).unwrap();
        assert!(ok.len() > 64, "o irmao: sob o teto le inteiro");
        // Por ultimo: com a conferencia antiga, este nao acaba nunca.
        assert!(ler_com_teto(pt(), Path::new("/dev/zero"), Some(1024)).is_err());
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
