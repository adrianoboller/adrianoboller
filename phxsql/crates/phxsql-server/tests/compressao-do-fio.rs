//! A compressao no caminho CLARO, provada PELO SOQUETE.
//!
//! # O gap (pedido 226) e o que esta frente fecha
//!
//! A F3 mediu a premissa com o `zlib` do Python: uma resposta de `varrer` com
//! 5.000 linhas comprime bem, e faltava so a negociacao e o enquadramento.
//! Este arquivo mede com o DEFLATE DESTA CASA (`phxsql_core::zip`), que e
//! Huffman FIXO -- o proprio modulo diz que perde uns por cento para o
//! dinamico do zlib -- entao o numero certo e o que sai daqui, e nao o do
//! script em Python. Numero citado e numero que nao se mede.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! `phxsql_core::zip::{deflate,inflate}` ja tem teste de unidade proprio
//! (ida e volta, e contra vetor do zlib). O que so o soquete prova e o LACO DE
//! CONEXAO: se o servidor so comprime quem pediu, se o cliente de sempre
//! continua recebendo a linha de sempre byte a byte, e se o tunel cifrado
//! IGNORA o pedido de compressao -- a mesma licao do `BULKINSERT` e da cifra
//! do fio.
//!
//! # Por que nunca dentro do tunel
//!
//! Comprimir e depois cifrar a MESMA resposta vaza tamanho (estilo
//! CRIME/BREACH) quando o atacante influencia parte do conteudo. Decidir se
//! vale mitigar isso e escolha de seguranca que fica fora desta frente -- ver
//! `docs/CIFRA-DO-FIO.md` §10. Aqui so se prova que o servidor NUNCA comprime
//! dentro do tunel, mesmo quando o pedido pede.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::base64;
use phxsql_core::fio::{Canal, Iniciador, Recebido};
use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token de servico deste teste";
/// Quantas linhas o `varrer` traz -- grande o bastante para passar, com folga,
/// do limiar de 256 bytes que faz o servidor sequer tentar comprimir.
const LINHAS: usize = 5000;

static PROXIMA: AtomicU16 = AtomicU16::new(7300);

/// Uma porta livre na faixa reservada a esta bancada -- faixa fixa, e nao
/// efemera, para nunca esbarrar num servidor de outra prova na mesma maquina.
fn porta_livre() -> u16 {
    loop {
        let porta = PROXIMA.fetch_add(1, Ordering::SeqCst);
        assert!(porta < 7350, "acabaram as portas entre 7300 e 7349");
        if let Ok(l) = TcpListener::bind(("127.0.0.1", porta)) {
            drop(l);
            return porta;
        }
    }
}

fn pasta(nome: &str) -> DirTemp {
    let d = DirTemp::novo(&format!("comp-{nome}"));
    std::fs::create_dir_all(d.join("base")).unwrap();
    d
}

/// Sobe um servidor com `max_linhas` alto o bastante para o `varrer` de
/// [`LINHAS`] linhas nao vir cortado -- o padrao (1.000) bastaria aqui, mas
/// o numero fica explicito para nao depender de um padrao que pode mudar.
fn subir(base: &std::path::Path, porta: u16) -> Arc<Servidor> {
    let mut c = Config {
        bind: format!("127.0.0.1:{porta}"),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        caminho: Some(base.join("config.json")),
        max_linhas: (LINHAS as u64) * 2,
        ..Default::default()
    };
    c.web.ligado = false;
    no_ar(Servidor::novo(c).unwrap(), porta)
}

fn no_ar(s: Arc<Servidor>, porta: u16) -> Arc<Servidor> {
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return s;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

/// Uma conexao aberta, com PRAZO -- teste que pendura nao reprova, trava.
struct Conexao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
    canal: Canal,
}

impl Conexao {
    fn abrir(porta: u16) -> Conexao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        fluxo
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Conexao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
            canal: Canal::Claro,
        }
    }

    /// O aperto de mao da CIFRA -- so para o teste que prova que compressao
    /// nao entra dentro do tunel. Copiado do padrao de `cifra-do-fio.rs`.
    fn cifrar(&mut self) {
        let (iniciador, m1) = Iniciador::comecar(None);
        writeln!(
            self.escrita,
            r#"{{"op":"cifrar","e":"{}"}}"#,
            base64::codificar(&m1)
        )
        .unwrap();
        self.escrita.flush().unwrap();
        let mut resposta = String::new();
        self.leitor.read_line(&mut resposta).unwrap();
        let j = Json::analisar(&resposta).unwrap();
        assert!(
            j.booleano_ou("ok", false),
            "o aperto tinha de fechar: {resposta}"
        );
        let m2 = base64::decodificar(
            j.campo("resultado")
                .map(|r| r.texto_ou("m2", ""))
                .unwrap_or(""),
        )
        .unwrap();
        let (t, _apresentada) = iniciador.terminar(&m2).unwrap();
        self.canal = Canal::Cifrado(Box::new(t));
    }

    fn pedir(&mut self, linha: &str) -> String {
        self.canal.escrever(&mut self.escrita, linha).unwrap();
        match self.canal.ler(&mut self.leitor) {
            Ok(Recebido::Linha(l)) => l,
            Ok(Recebido::Fim) => panic!("o servidor encerrou no meio: {linha}"),
            Err(e) => panic!("erro lendo a resposta de {linha}: {e}"),
        }
    }
}

/// Cria a base, a tabela e insere [`LINHAS`] linhas -- o mesmo formato do
/// `varrer` que a F3 mediu (id, nome, cidade), so que pelo Rust em vez do
/// Python da bancada.
fn semear(c: &mut Conexao, base: &str) {
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"criar_database","database":"{base}"}}"#
    ));
    assert!(r.contains("\"ok\":true"), "criar_database: {r}");
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"criar_tabela","database":"{base}","tabela":"medida","colunas":[{{"nome":"id","tipo":"Int8"}},{{"nome":"nome","tipo":"Str(60)"}},{{"nome":"cidade","tipo":"Str(40)"}}]}}"#
    ));
    assert!(r.contains("\"ok\":true"), "criar_tabela: {r}");

    let linhas: Vec<String> = (1..=LINHAS)
        .map(|i| format!(r#"{{"id":{i},"nome":"Cliente numero {i}","cidade":"Blumenau"}}"#))
        .collect();
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"inserir_lote","database":"{base}","tabela":"medida","linhas":[{}]}}"#,
        linhas.join(",")
    ));
    assert!(r.contains("\"ok\":true"), "inserir_lote: {r}");
}

fn varrer(c: &mut Conexao, base: &str, comprimir: bool) -> String {
    let campo = if comprimir {
        r#","aceita_compressao":true"#
    } else {
        ""
    };
    c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"varrer","database":"{base}","tabela":"medida","max":{LINHAS}{campo}}}"#
    ))
}

/// Decodifica o envelope `{"cz":"<base64>"}` e devolve o JSON de dentro, ja
/// como texto -- o mesmo que uma resposta sem compressao teria mandado.
fn descomprimir(envelope: &str) -> String {
    let j = Json::analisar(envelope).expect("envelope comprimido nao e JSON valido");
    let em_base64 = j
        .campo("cz")
        .and_then(Json::texto)
        .expect("o envelope comprimido nao tem o campo \"cz\"");
    let comprimido = base64::decodificar(em_base64).expect("o \"cz\" nao e Base64 valido");
    let bytes = phxsql_core::zip::inflate(&comprimido).expect("o DEFLATE de dentro nao abriu");
    String::from_utf8(bytes).expect("o conteudo descomprimido nao e UTF-8")
}

// ---------------------------------------------------------------------------
// A regra petrea: quem nao pede, nao recebe
// ---------------------------------------------------------------------------

/// **A regra que mais importa.** Um cliente que nunca ouviu falar da
/// compressao grava e le exatamente como hoje -- linha `{"ok":...}` de sempre,
/// sem `"cz"` nenhum -- mesmo numa resposta grande o bastante para a
/// compressao valer a pena.
///
/// Se este teste cair, a frente inteira esta errada: guarda nova entra
/// PEDIDA, nunca imposta.
#[test]
fn cliente_que_nao_pede_continua_sem_compressao() {
    let base = pasta("velho");
    let porta = porta_livre();
    let _s = subir(&base, porta);

    let mut c = Conexao::abrir(porta);
    semear(&mut c, "fio_velho");
    let r = varrer(&mut c, "fio_velho", false);

    assert!(
        r.trim_start().starts_with(r#"{"ok":true"#),
        "a resposta de quem nao pediu compressao mudou de forma: {}",
        &r[..r.len().min(120)]
    );
    assert!(
        !r.contains("\"cz\""),
        "uma resposta nao pedida veio comprimida (campo \"cz\" presente)"
    );
    assert!(r.contains("Blumenau"), "o conteudo de sempre sumiu: {r}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// Quem pede, recebe -- e reconstroi o MESMO conteudo
// ---------------------------------------------------------------------------

/// Quem manda `"aceita_compressao":true` recebe o envelope `{"cz":...}`, e
/// descomprimindo ele chega ao MESMO conteudo que a resposta sem compressao
/// traria -- byte a byte, depois de tirar so o `ms` (que mede tempo, e os
/// dois pedidos nao rodam no mesmo instante).
#[test]
fn cliente_que_pede_recebe_comprimido_e_reconstroi_o_mesmo_conteudo() {
    let base = pasta("pede");
    let porta = porta_livre();
    let _s = subir(&base, porta);

    let mut c = Conexao::abrir(porta);
    semear(&mut c, "fio_pede");

    let sem = varrer(&mut c, "fio_pede", false);
    let com = varrer(&mut c, "fio_pede", true);

    assert!(
        !sem.contains("\"cz\""),
        "a resposta sem pedir ja veio comprimida: {}",
        &sem[..sem.len().min(120)]
    );
    assert!(
        com.trim_start().starts_with(r#"{"cz":"#),
        "a resposta de quem pediu nao veio no envelope esperado: {}",
        &com[..com.len().min(120)]
    );

    let reconstruido = descomprimir(&com);

    // Compara pelo JSON analisado, e nao pela string crua: a ORDEM dos
    // campos e igual (mesmo codigo monta os dois), mas comparar a arvore e
    // o que realmente prova "mesmo conteudo" em vez de "mesmos bytes por
    // coincidencia de formatacao".
    let j_sem = Json::analisar(&sem).unwrap();
    let j_com = Json::analisar(&reconstruido).unwrap();
    assert_eq!(j_sem.campo("ok"), j_com.campo("ok"));
    assert_eq!(j_sem.campo("op"), j_com.campo("op"));
    assert_eq!(
        j_sem.campo("resultado").and_then(|r| r.campo("linhas")),
        j_com.campo("resultado").and_then(|r| r.campo("linhas")),
        "as linhas descomprimidas nao batem com as da resposta em claro"
    );
    assert_eq!(
        j_sem
            .campo("resultado")
            .and_then(|r| r.campo("devolvidas"))
            .and_then(Json::inteiro),
        Some(LINHAS as i64)
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// O ganho, MEDIDO com o DEFLATE desta casa -- nao citado do Python
// ---------------------------------------------------------------------------

/// Mede o ganho de verdade: bytes crus vs. bytes do envelope comprimido, os
/// dois lidos do SOQUETE -- e nao calculados por fora chamando `deflate`
/// direto, porque o que importa e o que TRAFEGA.
///
/// Roda com `--nocapture` para ver o numero: `cargo test -p phxsql-server
/// --test compressao-do-fio medir_o_ganho -- --nocapture`.
#[test]
fn medir_o_ganho_do_deflate_desta_casa() {
    let base = pasta("medida");
    let porta = porta_livre();
    let _s = subir(&base, porta);

    let mut c = Conexao::abrir(porta);
    semear(&mut c, "fio_medida");

    let sem = varrer(&mut c, "fio_medida", false);
    let t0 = Instant::now();
    let com = varrer(&mut c, "fio_medida", true);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;

    let bytes_crus = sem.len();
    let bytes_envelope = com.len();
    let razao = bytes_crus as f64 / bytes_envelope as f64;
    println!(
        "compressao-do-fio: {LINHAS} linhas, {bytes_crus} B -> {bytes_envelope} B \
         ({razao:.2}x), pedido+resposta em {ms:.2} ms"
    );

    assert!(
        com.trim_start().starts_with(r#"{"cz":"#),
        "a medida precisa do envelope comprimido: {}",
        &com[..com.len().min(120)]
    );
    // O limiar da bancada da F3 (zlib, Python) era 3x; o deflate desta casa e
    // Huffman FIXO, mais simples, entao a fasquia aqui e deliberadamente mais
    // baixa -- o que importa e que o numero REAL, medido agora, fique
    // registrado, e nao que bata um valor de outro compressor.
    assert!(
        razao > 2.0,
        "a compressao mal valeu a pena nesta rodada: {razao:.2}x ({bytes_crus} -> {bytes_envelope})"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// Por pedido, nao por conexao
// ---------------------------------------------------------------------------

/// A mesma conexao, dois pedidos: sem o campo, sem compressao; com o campo,
/// comprimido -- na hora, sem precisar de um segundo aperto ou de um `op` de
/// negociacao a parte. E o que "pedida por PEDIDO" quer dizer na pratica.
#[test]
fn a_mesma_conexao_alterna_por_pedido() {
    let base = pasta("alterna");
    let porta = porta_livre();
    let _s = subir(&base, porta);

    let mut c = Conexao::abrir(porta);
    semear(&mut c, "fio_alterna");

    let r1 = varrer(&mut c, "fio_alterna", false);
    assert!(
        !r1.contains("\"cz\""),
        "primeiro pedido veio comprimido sem pedir"
    );

    let r2 = varrer(&mut c, "fio_alterna", true);
    assert!(
        r2.trim_start().starts_with(r#"{"cz":"#),
        "segundo pedido, com o campo, nao veio comprimido: {}",
        &r2[..r2.len().min(120)]
    );

    let r3 = varrer(&mut c, "fio_alterna", false);
    assert!(
        !r3.contains("\"cz\""),
        "terceiro pedido, sem o campo, continuou comprimido -- a negociacao \
         ficou GRUDADA na conexao em vez de valer por pedido"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// Abaixo do limiar, o pedido nao muda nada
// ---------------------------------------------------------------------------

/// Uma resposta pequena (um `ping`) pedindo compressao continua saindo em
/// claro: o envelope so pesaria mais que a linha original.
#[test]
fn resposta_pequena_nao_comprime_mesmo_pedindo() {
    let base = pasta("pequena");
    let porta = porta_livre();
    let _s = subir(&base, porta);

    let mut c = Conexao::abrir(porta);
    let r = c.pedir(&format!(
        r#"{{"token":"{TOKEN}","op":"ping","aceita_compressao":true}}"#
    ));
    assert!(r.contains("\"ok\":true"), "ping: {r}");
    assert!(
        !r.contains("\"cz\""),
        "uma resposta minuscula veio comprimida, que so pesaria mais: {r}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// NUNCA dentro do tunel -- o ponto central desta frente
// ---------------------------------------------------------------------------

/// Dentro do tunel cifrado, pedir compressao NAO tem efeito algum: a
/// resposta, depois de decifrada, continua sendo o `{"ok":...}` de sempre --
/// nunca o envelope `{"cz":...}`.
///
/// Esta e a prova de que esta frente NAO fez compress-then-encrypt: comprimir
/// dentro do tunel fica registrado como decisao de SEGURANCA em
/// `docs/CIFRA-DO-FIO.md` §10, e nao como esquecimento.
#[test]
fn dentro_do_tunel_o_pedido_de_compressao_e_ignorado() {
    let base = pasta("tunel");
    let porta = porta_livre();
    let _s = subir(&base, porta);

    let mut c = Conexao::abrir(porta);
    c.cifrar();
    semear(&mut c, "fio_tunel");

    // A mensagem chega DECIFRADA pelo `canal.ler` -- o que se ve aqui e o
    // JSON de dentro do tunel, exatamente como o cliente cifrado o recebe.
    let dentro = varrer(&mut c, "fio_tunel", true);
    assert!(
        dentro.trim_start().starts_with(r#"{"ok":true"#),
        "dentro do tunel, pedir compressao produziu outra coisa que nao a \
         resposta de sempre: {}",
        &dentro[..dentro.len().min(160)]
    );
    assert!(
        !dentro.contains("\"cz\""),
        "o servidor comprimiu DENTRO do tunel cifrado -- isto e \
         compress-then-encrypt, e esta frente decidiu nao fazer isso \
         (docs/CIFRA-DO-FIO.md secao 10)"
    );
    assert!(dentro.contains("Blumenau"), "o conteudo sumiu: {dentro}");
    let _ = std::fs::remove_dir_all(&base);
}
