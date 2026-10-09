//! Pedido 545: o DbLink fala com a rede SEM a trava de dados.
//!
//! `dblink_ligar` e `dblink_sincronizar` iam ao fio com a trava global na mao,
//! e o unico prazo era o de cada leitura: um par que gotejasse um byte abaixo
//! dele prendia todo pedido de todo cliente pelo tempo que quisesse. A prova
//! e pelo soquete, contra um MySQL(R) falso que goteja cada resposta, e o que
//! ela mede e o que o defeito fazia: quanto tempo um `inserir` qualquer, em
//! outra tabela, espera enquanto o par goteja.
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};

/// Quanto cada resposta do par leva para chegar inteira. Cada byte chega
/// bem abaixo do prazo por leitura da ligacao (1 s), que e o caso do
/// defeito: o prazo por leitura nunca vence.
const GOTEJO: Duration = Duration::from_millis(1_200);

/// Quanto o par segura o ULTIMO byte de cada resposta esperando o aval do
/// teste. Nao e o criterio: e a rede de seguranca para o defeito nao travar a
/// suite -- quem decide e se o aval chegou ANTES do par desistir de esperar.
const SEGURA: Duration = Duration::from_secs(4);

/// O aval do teste ao par: cada `()` solta a resposta que esta segurando o
/// ultimo byte. O criterio e a ORDEM (o `inserir` vizinho termina com a
/// resposta ainda presa), nao um prazo de relogio -- sob carga o `inserir`
/// demora o que demorar e o veredito nao muda (pedido 751, metodo do 703).
type Aval = Arc<Mutex<mpsc::Receiver<()>>>;

pub(super) fn quadro(seq: u8, carga: &[u8]) -> Vec<u8> {
    let mut q = (carga.len() as u32).to_le_bytes()[..3].to_vec();
    q.push(seq);
    q.extend_from_slice(carga);
    q
}

pub(super) fn lenenc(saida: &mut Vec<u8>, b: &[u8]) {
    saida.push(b.len() as u8);
    saida.extend_from_slice(b);
}

/// A definicao de uma coluna de `clientes`, no formato do protocolo 41.
pub(super) fn coluna(nome: &str, codigo: u8, bandeiras: u16, tamanho: u32) -> Vec<u8> {
    coluna_em(nome, codigo, bandeiras, tamanho, 45)
}

/// A mesma definicao dizendo o conjunto de caracteres: 63 e o `binary`
/// que o MySQL(R) manda num BLOB, 45 o `utf8mb4` de um texto -- pedido 590.
pub(super) fn coluna_em(
    nome: &str,
    codigo: u8,
    bandeiras: u16,
    tamanho: u32,
    charset: u16,
) -> Vec<u8> {
    let mut p = Vec::new();
    for campo in [&b"def"[..], b"erp", b"clientes", b"clientes"] {
        lenenc(&mut p, campo);
    }
    lenenc(&mut p, nome.as_bytes());
    lenenc(&mut p, nome.as_bytes());
    p.push(0x0c);
    p.extend_from_slice(&charset.to_le_bytes());
    p.extend_from_slice(&tamanho.to_le_bytes());
    p.push(codigo);
    p.extend_from_slice(&bandeiras.to_le_bytes());
    p.extend_from_slice(&[0, 0, 0]);
    p
}

pub(super) const EOF: [u8; 5] = [0xFE, 0, 0, 2, 0];

/// A resposta do par a uma instrucao: o esquema de `clientes` (id INT
/// chave, nome VARCHAR(40)) com a linha remota `1, remoto` quando e o
/// `SELECT` inteiro, e um OK de uma linha para o que nao e `SELECT`.
fn resposta(sql: &str) -> Vec<Vec<u8>> {
    if !sql.starts_with("SELECT") {
        return vec![vec![0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00]];
    }
    let mut r = vec![
        vec![2],
        coluna("id", 0x03, 0x0003, 11),
        coluna("nome", 0xfd, 0, 160),
        EOF.to_vec(),
    ];
    if !sql.contains("LIMIT 0") {
        let mut linha = Vec::new();
        lenenc(&mut linha, b"1");
        lenenc(&mut linha, b"remoto");
        r.push(linha);
    }
    r.push(EOF.to_vec());
    r
}

pub(super) fn ler_pacote(s: &mut TcpStream) -> Option<Vec<u8>> {
    let mut cabeca = [0u8; 4];
    s.read_exact(&mut cabeca).ok()?;
    let n = u32::from_le_bytes([cabeca[0], cabeca[1], cabeca[2], 0]) as usize;
    let mut carga = vec![0u8; n];
    s.read_exact(&mut carga).ok()?;
    Some(carga)
}

/// Saudacao e OK do login, na hora. Diz se a conexao seguiu viva.
pub(super) fn saudar(s: &mut TcpStream) -> bool {
    let mut saudacao = vec![10u8];
    saudacao.extend_from_slice(b"8.0.0-falso\0");
    saudacao.extend_from_slice(&7u32.to_le_bytes());
    saudacao.extend_from_slice(b"abcdefgh\0");
    saudacao.extend_from_slice(&[0xFF, 0xFF, 45, 2, 0, 0xFF, 0x0F, 21]);
    saudacao.extend_from_slice(&[0u8; 10]);
    saudacao.extend_from_slice(b"ijklmnopqrst\0mysql_native_password\0");
    if s.write_all(&quadro(0, &saudacao)).is_err() || ler_pacote(s).is_none() {
        return false;
    }
    let ok = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00];
    s.write_all(&quadro(2, &ok)).is_ok()
}

/// Uma conexao do par: saudacao e OK na hora, e cada resposta a uma
/// instrucao GOTEJADA ao longo do `GOTEJO`. A instrucao vai pelo canal no
/// instante em que chega -- e o sinal de que o DbLink esta no fio.
fn atender(
    mut s: TcpStream,
    avisar: mpsc::Sender<String>,
    aval: Option<Aval>,
    desistiu: Arc<AtomicBool>,
) {
    if !saudar(&mut s) {
        return;
    }
    while let Some(pacote) = ler_pacote(&mut s) {
        // 0x03 e COM_QUERY; o resto (COM_QUIT) encerra.
        if pacote.first() != Some(&0x03) {
            return;
        }
        let sql = String::from_utf8_lossy(&pacote[1..]).into_owned();
        let _ = avisar.send(sql.clone());
        let mut bytes = Vec::new();
        for (i, carga) in resposta(&sql).iter().enumerate() {
            bytes.extend(quadro(i as u8 + 1, carga));
        }
        let passo = GOTEJO / bytes.len() as u32;
        let n = bytes.len();
        for (i, b) in bytes.into_iter().enumerate() {
            std::thread::sleep(passo);
            if i + 1 == n {
                if let Some(aval) = &aval {
                    // Sem o aval em SEGURA, o vizinho nao andou enquanto o
                    // par estava no fio: a trava estava presa.
                    let veio = aval.lock().unwrap().recv_timeout(SEGURA).is_ok();
                    if !veio {
                        desistiu.store(true, Ordering::SeqCst);
                    }
                }
            }
            if s.write_all(&[b]).is_err() {
                return;
            }
        }
    }
}

fn par_que_goteja() -> (u16, mpsc::Receiver<String>) {
    let (porta, avisos, _, _) = par_que_goteja_preso(false);
    (porta, avisos)
}

/// O par, com a opcao de segurar o ultimo byte de cada resposta ate o aval.
fn par_que_goteja_preso(
    com_aval: bool,
) -> (
    u16,
    mpsc::Receiver<String>,
    mpsc::Sender<()>,
    Arc<AtomicBool>,
) {
    let (dar_aval, recebe) = mpsc::channel();
    let aval: Option<Aval> = com_aval.then(|| Arc::new(Mutex::new(recebe)));
    let desistiu = Arc::new(AtomicBool::new(false));
    let desistiu2 = Arc::clone(&desistiu);
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (avisar, avisos) = mpsc::channel();
    std::thread::spawn(move || {
        for s in ouvinte.incoming() {
            let Ok(s) = s else { return };
            let avisar = avisar.clone();
            let (aval, desistiu) = (aval.clone(), Arc::clone(&desistiu2));
            std::thread::spawn(move || atender(s, avisar, aval, desistiu));
        }
    });
    (porta, avisos, dar_aval, desistiu)
}

pub(super) fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

pub(super) fn pede(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        ip: "127.0.0.1".into(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// Espera o par receber uma instrucao que comece por `inicio`, faz um
/// `inserir` em OUTRA tabela enquanto a resposta dele esta presa e devolve
/// se o `inserir` terminou com o par ainda segurando -- o que so acontece se
/// o DbLink esta no fio SEM a trava de dados. Depois solta a resposta.
fn vizinho_andou(
    s: &Arc<Servidor>,
    avisos: &mpsc::Receiver<String>,
    dar_aval: &mpsc::Sender<()>,
    desistiu: &AtomicBool,
    inicio: &str,
    id: u64,
) -> bool {
    let sql = avisos
        .recv_timeout(Duration::from_secs(30))
        .expect("o par nao recebeu instrucao nenhuma");
    assert!(sql.starts_with(inicio), "esperava {inicio:?}, veio {sql:?}");
    pede(
        s,
        &format!(r#""op":"inserir","database":"loja","tabela":"outra","linha":{{"id":{id}}}"#),
    )
    .unwrap();
    // Lido ANTES do aval: o aval solta o par, e so o que ja aconteceu conta.
    let andou = !desistiu.swap(false, Ordering::SeqCst);
    if andou {
        // Aval atrasado deixaria sobra no canal e soltaria a proxima ida cedo.
        let _ = dar_aval.send(());
    }
    andou
}

/// **545: enquanto o par goteja, o resto do banco anda.** As idas ao fio
/// que iam com a trava na mao: o `LIMIT 0` do `dblink_ligar`, e o
/// `SELECT` e o empurrao do `dblink_sincronizar` -- e, desde o 584, o
/// `LIMIT 0` que a sincronia faz antes da leitura.
///
/// # Prova real
///
/// Com a trava tomada antes do fio (o codigo de antes), o `inserir` em
/// `loja.outra` so termina depois que o par solta a resposta -- e o par so
/// solta no `SEGURA`, porque o aval vem depois do `inserir`. Com o conserto,
/// o `inserir` termina com a resposta ainda presa. Pela ordem, nao pelo
/// relogio: o 751 era este teste caindo sob carga com 600 ms absolutos.
#[test]
fn o_par_que_goteja_nao_prende_o_banco() {
    let dir = DirTemp::novo("545-dblink-trava");
    let s = servidor(&dir);
    let (porta, avisos, aval, desistiu) = par_que_goteja_preso(true);
    pede(&s, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        &s,
        r#""op":"criar_tabela","database":"loja","tabela":"outra",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
               "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    pede(
        &s,
        &format!(
            r#""op":"dblink_salvar","nome":"erp","motor":"mysql","host":"127.0.0.1",
                   "porta":{porta},"usuario":"u","database":"erp","timeout_s":10,
                   "somente_leitura":false,"cifra":false"#
        ),
    )
    .unwrap();

    let s2 = Arc::clone(&s);
    let ligar = std::thread::spawn(move || {
        pede(
            &s2,
            r#""op":"dblink_ligar","dblink":"erp","tabelas":[{"remota":"clientes",
                   "local_database":"loja","sentido":"dois","dono":"aqui"}]"#,
        )
    });
    let vizinho = |inicio: &str, id: u64| vizinho_andou(&s, &avisos, &aval, &desistiu, inicio, id);
    let no_ligar = vizinho("SELECT * FROM `clientes` LIMIT 0", 1);
    let r = ligar.join().unwrap().unwrap();
    assert!(
        r.escrever().contains("\"tabela_criada\":true"),
        "{}",
        r.escrever()
    );

    // Uma linha so daqui, para a rodada ter o que empurrar.
    pede(
        &s,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"local"}"#,
    )
    .unwrap();
    let s2 = Arc::clone(&s);
    let sincronizar =
        std::thread::spawn(move || pede(&s2, r#""op":"dblink_sincronizar","dblink":"erp""#));
    // Desde o 584 a rodada vai ao fio duas vezes antes do empurrao: as
    // colunas (`LIMIT 0`) e a leitura com o binario em hexadecimal. As
    // duas tem de andar sem a trava, e as duas se medem.
    let no_select = vizinho("SELECT * FROM `clientes` LIMIT 0", 2);
    let na_leitura = vizinho("SELECT `id`,`nome` FROM `clientes`", 4);
    let no_empurrao = vizinho("INSERT", 3);
    let r = sincronizar.join().unwrap().unwrap().escrever();

    // A rodada continua certa: puxou a remota e empurrou a local.
    assert!(r.contains("\"puxadas_novas\":1"), "{r}");
    assert!(r.contains("\"empurradas\":1"), "{r}");
    // As tres de uma vez na mensagem: o vermelho diz QUAIS idas ao fio
    // prendiam o banco, e nao so a primeira.
    let presas: Vec<String> = [
        ("LIMIT 0 do dblink_ligar", no_ligar),
        ("LIMIT 0 do dblink_sincronizar", no_select),
        ("SELECT do dblink_sincronizar", na_leitura),
        ("empurrao do dblink_sincronizar", no_empurrao),
    ]
    .iter()
    .filter(|(_, andou)| !*andou)
    .map(|(onde, _)| (*onde).to_string())
    .collect();
    assert!(
        presas.is_empty(),
        "um inserir em outra tabela so terminou depois que o par soltou a resposta -- o \
             DbLink estava no fio com a trava de dados na mao: {presas:?}"
    );
}

/// **609: o `dblink_ligar` nao grava por cima do que mudou enquanto ele
/// estava no fio.** A copia da ligacao e lida antes da rede e gravada no
/// fim; sem a conferencia da versao, a ligacao EXCLUIDA no meio voltava ao
/// `dblink.json` com a senha antiga, e a senha TROCADA no meio voltava a
/// ser a antiga.
///
/// # Prova real
///
/// Com a conferencia tirada (o codigo de antes), o `ligar` responde `ok`
/// nos dois casos: a lista volta a ter `erp` e o arquivo volta a ter
/// `SENHA-ANTIGA`. Com ela, o `ligar` recusa com `Conflito` e o cadastro
/// fica como o outro administrador o deixou.
#[test]
fn o_ligar_nao_ressuscita_nem_desfaz_a_ligacao_mexida_no_meio() {
    let dir = DirTemp::novo("609-dblink-ligar");
    let s = servidor(&dir);
    let (porta, avisos) = par_que_goteja();
    pede(&s, r#""op":"criar_database","database":"loja""#).unwrap();
    let salvar = |senha: &str| {
        pede(
            &s,
            &format!(
                r#""op":"dblink_salvar","nome":"erp","motor":"mysql","host":"127.0.0.1",
                       "porta":{porta},"usuario":"u","senha":"{senha}","database":"erp",
                       "timeout_s":5,"cifra":false"#
            ),
        )
        .unwrap();
    };
    let ligar = |s: &Arc<Servidor>| {
        let s2 = Arc::clone(s);
        std::thread::spawn(move || {
            pede(
                &s2,
                r#""op":"dblink_ligar","dblink":"erp","tabelas":[{"remota":"clientes",
                       "local_database":"loja","sentido":"puxar","dono":"la"}]"#,
            )
        })
    };
    let no_fio = || {
        let sql = avisos
            .recv_timeout(Duration::from_secs(10))
            .expect("o par nao recebeu o LIMIT 0 do ligar");
        assert!(sql.contains("LIMIT 0"), "{sql}");
    };
    let arquivo = || std::fs::read_to_string(dir.join("dblink.json")).unwrap_or_default();

    // Caso 1: excluida enquanto o ligar esta no fio.
    salvar("SENHA-ANTIGA");
    let t = ligar(&s);
    no_fio();
    pede(&s, r#""op":"dblink_excluir","nome":"erp""#).unwrap();
    let e = t
        .join()
        .unwrap()
        .expect_err("o ligar gravou a ligacao excluida");
    assert!(
        matches!(e, PhxError::Conflito(_)) && e.to_string().contains("excluida"),
        "{e}"
    );
    let lista = pede(&s, r#""op":"dblink""#).unwrap().escrever();
    assert!(!lista.contains("\"erp\""), "a excluida voltou: {lista}");
    assert!(!arquivo().contains("SENHA-ANTIGA"), "{}", arquivo());

    // Caso 2: senha trocada enquanto o ligar esta no fio.
    salvar("SENHA-ANTIGA");
    let t = ligar(&s);
    no_fio();
    salvar("SENHA-NOVA");
    let e = t
        .join()
        .unwrap()
        .expect_err("o ligar desfez a troca de senha");
    assert!(
        matches!(e, PhxError::Conflito(_)) && e.to_string().contains("alterada"),
        "{e}"
    );
    let no_disco = arquivo();
    assert!(
        no_disco.contains("SENHA-NOVA") && !no_disco.contains("SENHA-ANTIGA"),
        "{no_disco}"
    );

    // E o comportamento velho: ninguem mexeu, o ligar grava a sincronia.
    let t = ligar(&s);
    no_fio();
    let r = t.join().unwrap().unwrap().escrever();
    assert!(r.contains("\"remota\":\"clientes\""), "{r}");
    assert!(arquivo().contains("SENHA-NOVA"), "{}", arquivo());
}
