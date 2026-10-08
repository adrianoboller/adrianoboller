//! Pedido 706: o diario do caixa tem expurgo, e o central que volta nao perde
//! venda -- provado PELO SOQUETE, com o fio derrubado e religado.
//!
//! # Por que um binario proprio
//!
//! O corte do `.log` sem paginacao e um global do processo
//! (`phxsql_store::diario::definir_corte_do_expurgo`). Ligado num binario com
//! outros testes, faria o diario deles virar de volume no meio da corrida.
//!
//! # O que se prova
//!
//! 1. Com o central puxando e confirmando, o diario do caixa sai do disco
//!    volume a volume e fica limitado.
//! 2. Com o fio caido, o caixa continua vendendo (zero recusa) e o diario
//!    SEGURA tudo o que o central nao confirmou.
//! 3. O fio volta, o central alcanca, e caixa e central tem as mesmas linhas,
//!    byte a byte -- nenhuma venda perdida depois do expurgo.
//! 4. Quem pede abaixo do que ainda existe recebe a recusa DITA, pela fabrica
//!    de idiomas, e nao o evento seguinte no lugar.
//! 5. Sem consumidor declarado, nada confirmado sai (o prazo de 30 dias e o
//!    unico caminho): quem nao esta na lista nao segura, e tambem nao solta.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "expurgo-do-diario";
const ESPERA: Duration = Duration::from_secs(60);

struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = pedir(self.porta, r#""op":"servico_parar""#);
    }
}

fn config(base: &Path, id: &str, papel: Papel) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = papel;
    c.replicacao.id_servidor = id.into();
    c.replicacao.imagem_da_linha = true;
    c
}

/// O caixa: origem, com o expurgo ligado num volume de 64 KiB e uma passada
/// por segundo -- o perfil caixa, encolhido para caber num teste.
fn caixa(base: &Path, consumidores: &[&str]) -> NoAr {
    caixa_com_prazo(base, consumidores, 0)
}

/// O caixa com o prazo de ENSAIO (`diario.prazo_s`): o que ninguem
/// confirmou sai depois de `prazo_s` segundos, e nao de trinta dias.
fn caixa_com_prazo(base: &Path, consumidores: &[&str], prazo_s: u64) -> NoAr {
    let mut c = config(base, "caixa01", Papel::Source);
    c.diario.prazo_s = prazo_s;
    c.diario.expurgo = true;
    c.diario.consumidores = consumidores.iter().map(|s| s.to_string()).collect();
    c.diario.volume_kib = 64;
    c.diario.passada_s = 1;
    c.diario.prazo_dias = 30;
    // `Config::ler` aplica; quem monta a configuracao no codigo aplica aqui.
    c.diario.aplicar();
    subir(c)
}

fn central(base: &Path, porta_da_origem: u16) -> NoAr {
    let mut c = config(base, "central", Papel::Replica);
    c.somente_leitura = true;
    c.replicacao.origens = vec![Origem {
        nome: "caixa01".into(),
        host: "127.0.0.1".into(),
        porta: porta_da_origem,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
        espelho: false,
    }];
    subir(c)
}

fn subir(c: Config) -> NoAr {
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn pedir(porta: u16, corpo: &str) -> Option<Json> {
    let fluxo = TcpStream::connect(("127.0.0.1", porta)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(20))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .ok()?;
    let mut r = String::new();
    leitor.read_line(&mut r).ok()?;
    Json::analisar(&r).ok()
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo).unwrap_or_else(|| panic!("sem resposta para {corpo}"));
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r
}

fn criar_vendas(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"vendas",
           "motivo_obrigatorio":false,
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"total","tipo":"Int8"},
                      {"nome":"momento_ms","tipo":"Int8"}],
           "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

/// Vende `de..ate` em lotes de 500. Devolve quantas vendas o caixa RECUSOU.
fn vender(porta: u16, de: u64, ate: u64) -> u64 {
    let mut recusadas = 0;
    let mut i = de;
    while i < ate {
        let fim = (i + 500).min(ate);
        let linhas: Vec<String> = (i..fim)
            .map(|k| {
                format!(
                    r#"{{"id":{k},"total":{},"momento_ms":{}}}"#,
                    k * 7,
                    k * 1000
                )
            })
            .collect();
        let r = pedir(
            porta,
            &format!(
                r#""op":"inserir_lote","database":"loja","tabela":"vendas","linhas":[{}]"#,
                linhas.join(",")
            ),
        );
        if !r.is_some_and(|r| r.booleano_ou("ok", false)) {
            recusadas += fim - i;
        }
        i = fim;
    }
    recusadas
}

/// Todas as linhas de `vendas`, na ordem do `.reg`, como texto -- comparar o
/// texto e comparar linha a linha, com o rowid.
fn linhas(porta: u16) -> Vec<String> {
    let mut saida = Vec::new();
    let mut depois = 0i64;
    loop {
        let Some(r) = pedir(
            porta,
            &format!(
                r#""op":"varrer","database":"loja","tabela":"vendas","max":2000,
                   "depois":{depois},"visao":"todas""#
            ),
        ) else {
            return saida;
        };
        let Some(res) = r.campo("resultado") else {
            return saida;
        };
        let lista = res.campo("linhas").and_then(Json::lista).unwrap_or(&[]);
        saida.extend(lista.iter().map(Json::escrever));
        if !res.booleano_ou("ha_mais", false) || lista.is_empty() {
            return saida;
        }
        depois = res.inteiro_ou("cursor_fim", 0);
    }
}

fn esperar(o_que: &str, mut ok: impl FnMut() -> bool) {
    let ate = Instant::now() + ESPERA;
    while !ok() {
        assert!(Instant::now() < ate, "nao aconteceu em {ESPERA:?}: {o_que}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Os volumes do diario de `vendas` no disco do caixa, e os bytes deles.
fn diario_no_disco(base: &Path) -> (Vec<String>, u64) {
    let pasta = base.join("loja");
    let mut nomes = Vec::new();
    let mut bytes = 0;
    for e in std::fs::read_dir(&pasta).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n == "vendas.log" || (n.starts_with("vendas#") && n.ends_with(".log")) {
            bytes += e.metadata().unwrap().len();
            nomes.push(n);
        }
    }
    nomes.sort();
    (nomes, bytes)
}

/// O repasse entre o central e o caixa: o fio que o teste derruba e religa.
struct Fio {
    porta: u16,
    caido: Arc<AtomicBool>,
    vivas: Arc<Mutex<Vec<TcpStream>>>,
}

impl Fio {
    fn ligar(origem: u16) -> Fio {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let caido = Arc::new(AtomicBool::new(false));
        let vivas: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        let (c, v) = (Arc::clone(&caido), Arc::clone(&vivas));
        std::thread::spawn(move || {
            for cliente in ouvinte.incoming() {
                let Ok(cliente) = cliente else { return };
                if c.load(Ordering::SeqCst) {
                    let _ = cliente.shutdown(Shutdown::Both);
                    continue;
                }
                let Ok(servidor) = TcpStream::connect(("127.0.0.1", origem)) else {
                    continue;
                };
                {
                    let mut v = v.lock().unwrap();
                    v.push(cliente.try_clone().unwrap());
                    v.push(servidor.try_clone().unwrap());
                }
                for (mut de, mut para) in [
                    (cliente.try_clone().unwrap(), servidor.try_clone().unwrap()),
                    (servidor, cliente),
                ] {
                    std::thread::spawn(move || {
                        let mut buf = [0u8; 16 * 1024];
                        loop {
                            match de.read(&mut buf) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    if para.write_all(&buf[..n]).is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        let _ = para.shutdown(Shutdown::Both);
                    });
                }
            }
        });
        Fio {
            porta,
            caido,
            vivas,
        }
    }

    fn porta(&self) -> u16 {
        self.porta
    }

    fn derrubar(&self) {
        self.caido.store(true, Ordering::SeqCst);
        for s in self.vivas.lock().unwrap().drain(..) {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    fn religar(&self) {
        self.caido.store(false, Ordering::SeqCst);
    }
}

#[test]
fn o_diario_do_caixa_encolhe_e_o_central_que_volta_nao_perde_venda() {
    let base_caixa = DirTemp::novo("expurgo-diario-caixa");
    let base_central = DirTemp::novo("expurgo-diario-central");
    let cx = caixa(&base_caixa.0, &["central"]);
    criar_vendas(cx.porta);
    let fio = Fio::ligar(cx.porta);
    let ct = central(&base_central.0, fio.porta());

    // 1. Com o central puxando, o diario do caixa sai do disco.
    assert_eq!(vender(cx.porta, 0, 6_000), 0, "o caixa recusou venda");
    esperar("o central alcancar 6.000 vendas", || {
        linhas(ct.porta).len() == 6_000
    });
    esperar("o primeiro volume do diario do caixa sair", || {
        !base_caixa.0.join("loja").join("vendas#001.log").exists()
    });
    // Fica o ultimo fechado, o penultimo como margem e o ativo: o diario do
    // caixa com o central em dia cabe em quatro volumes, venda a mais ou a
    // menos.
    esperar("o diario do caixa caber em quatro volumes", || {
        diario_no_disco(&base_caixa.0).1 <= 4 * 64 * 1024
    });

    // 2. O fio cai: o caixa vende, e o diario SEGURA o que nao foi puxado.
    // O retrato de antes da queda e tirado DEPOIS de duas passadas com o fio
    // caido: a confirmacao que o central mandou antes de cair (a do fim da
    // rodada, ja no disco dele) ainda pode tirar um volume, e e legitima.
    fio.derrubar();
    std::thread::sleep(Duration::from_millis(2_500));
    let (antes_da_queda, bytes_antes) = diario_no_disco(&base_caixa.0);
    assert_eq!(
        vender(cx.porta, 6_000, 12_000),
        0,
        "o caixa recusou venda com o central fora"
    );
    std::thread::sleep(Duration::from_secs(3)); // tres passadas do expurgo
    let (na_queda, bytes_na_queda) = diario_no_disco(&base_caixa.0);
    assert!(
        bytes_na_queda > bytes_antes,
        "o diario nao cresceu com o central fora: {na_queda:?}"
    );
    // O menor volume que existe e o mesmo de antes da queda: nada que o
    // central nao confirmou saiu.
    assert_eq!(
        na_queda.first(),
        antes_da_queda.first(),
        "saiu volume nao confirmado"
    );

    // 3. O fio volta: o central alcanca, e as linhas sao as mesmas.
    fio.religar();
    esperar("o central alcancar 12.000 vendas", || {
        linhas(ct.porta).len() == 12_000
    });
    assert_eq!(
        linhas(cx.porta),
        linhas(ct.porta),
        "caixa e central divergem"
    );
    esperar("o diario do caixa encolher de novo", || {
        diario_no_disco(&base_caixa.0).1 <= 4 * 64 * 1024
    });

    // 4. Quem pede abaixo do que existe recebe a recusa dita.
    let r = pedir(
        cx.porta,
        r#""op":"replicar","database":"loja","tabela":"vendas","desde":0,"max":5"#,
    )
    .unwrap();
    assert!(!r.booleano_ou("ok", true), "{}", r.escrever());
    let erro = r.escrever();
    assert!(
        erro.contains("expurgo") && erro.contains("copia"),
        "a recusa nao diz o que houve nem a saida: {erro}"
    );
    // E o caixa continua servindo de onde o diario ainda tem.
    let r = exigir(cx.porta, r#""op":"posicao","database":"loja""#);
    assert!(r.escrever().contains("vendas"));
}

#[test]
fn sem_consumidor_declarado_nada_sai_antes_do_prazo() {
    let base_caixa = DirTemp::novo("expurgo-diario-sem-consumidor");
    let base_central = DirTemp::novo("expurgo-diario-sem-consumidor-central");
    let cx = caixa(&base_caixa.0, &[]);
    criar_vendas(cx.porta);
    let ct = central(&base_central.0, cx.porta);
    assert_eq!(vender(cx.porta, 0, 6_000), 0);
    esperar("o central alcancar 6.000 vendas", || {
        linhas(ct.porta).len() == 6_000
    });
    std::thread::sleep(Duration::from_secs(3)); // tres passadas do expurgo
    let (volumes, _) = diario_no_disco(&base_caixa.0);
    assert!(
        volumes.iter().any(|v| v == "vendas#001.log"),
        "saiu volume sem consumidor declarado e dentro do prazo: {volumes:?}"
    );
    assert!(
        volumes.len() > 2,
        "o diario nem virou de volume: {volumes:?}"
    );
}

/// A base que a origem anuncia para `vendas` no `posicao` (pedido 706).
fn base_anunciada(porta: u16) -> u64 {
    pedir(porta, r#""op":"posicao","database":"loja""#)
        .and_then(|r| {
            r.campo("resultado")
                .and_then(|r| r.campo("tabelas"))
                .and_then(|t| t.campo("vendas"))
                .map(|v| v.inteiro_ou("base", 0).max(0) as u64)
        })
        .unwrap_or(0)
}

/// Quantos eventos o diario de `vendas` tem num servidor.
fn eventos(porta: u16) -> u64 {
    pedir(porta, r#""op":"posicao","database":"loja""#)
        .and_then(|r| {
            r.campo("resultado")
                .and_then(|r| r.campo("tabelas"))
                .and_then(|t| t.campo("vendas"))
                .map(|v| v.inteiro_ou("eventos", 0).max(0) as u64)
        })
        .unwrap_or(0)
}

/// **O item (d) do parecer, a rota automatica:** o central fica fora alem do
/// prazo (de ENSAIO, 2 s), o caixa solta o diario que ele nao puxou e
/// continua vendendo; o central volta, ve pela base anunciada que ficou atras
/// do que o caixa guarda, se refaz pelo retrato do caixa sem mao humana e
/// alcanca -- linhas iguais, nenhuma venda perdida.
///
/// Defeito reposto (o central que nunca pede retrato): ele fica parado na
/// recusa `erro.diario_expurgado` e nao alcanca as 12.000.
#[test]
fn o_central_fora_alem_do_prazo_se_refaz_pelo_retrato_e_alcanca() {
    let base_caixa = DirTemp::novo("expurgo-diario-retrato-caixa");
    let base_central = DirTemp::novo("expurgo-diario-retrato-central");
    let cx = caixa_com_prazo(&base_caixa.0, &["central"], 2);
    criar_vendas(cx.porta);
    let fio = Fio::ligar(cx.porta);
    let ct = central(&base_central.0, fio.porta());
    assert_eq!(vender(cx.porta, 0, 6_000), 0);
    esperar("o central alcancar 6.000 vendas", || {
        linhas(ct.porta).len() == 6_000
    });
    let do_central = eventos(ct.porta);

    fio.derrubar();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(vender(cx.porta, 6_000, 12_000), 0, "o caixa recusou venda");
    // O prazo passa e o caixa solta o que o central nao puxou.
    esperar("o caixa soltar o diario alem do que o central tem", || {
        base_anunciada(cx.porta) > do_central
    });
    // Quem pede de onde o central parou recebe a recusa dita.
    let r = pedir(
        cx.porta,
        &format!(r#""op":"replicar","database":"loja","tabela":"vendas","desde":{do_central}"#),
    )
    .unwrap();
    assert!(!r.booleano_ou("ok", true), "{}", r.escrever());

    fio.religar();
    esperar("o central se refazer e alcancar 12.000 vendas", || {
        linhas(ct.porta).len() == 12_000
    });
    assert_eq!(
        linhas(cx.porta),
        linhas(ct.porta),
        "caixa e central divergem"
    );
    // E continua seguindo: o que o caixa vende depois chega.
    assert_eq!(vender(cx.porta, 12_000, 12_500), 0);
    esperar("o central seguir depois do retrato", || {
        linhas(ct.porta).len() == 12_500
    });
    assert_eq!(linhas(cx.porta), linhas(ct.porta));
    assert_eq!(
        eventos(cx.porta),
        eventos(ct.porta),
        "a posicao nao e a da origem"
    );
}

/// **Prova 6 do parecer, pelo `duravel`:** a confirmacao e o que a replica
/// JA levou ao disco, e nao o `desde`, que conta a cauda da rodada ainda sem
/// `fsync`. Um consumidor que pede tudo dizendo `duravel: 0` nao solta nada;
/// o mesmo pedido com `duravel` igual ao `desde` solta.
///
/// Defeito reposto (o `duravel` ignorado): o primeiro pedido ja solta o
/// volume 1, e o teste cai.
#[test]
fn a_confirmacao_e_o_duravel_e_nao_o_desde() {
    let base_caixa = DirTemp::novo("expurgo-diario-duravel");
    let cx = caixa(&base_caixa.0, &["central"]);
    criar_vendas(cx.porta);
    assert_eq!(vender(cx.porta, 0, 6_000), 0);
    let total = eventos(cx.porta);
    let pede = |duravel: u64| {
        exigir(
            cx.porta,
            &format!(
                r#""op":"replicar","database":"loja","tabela":"vendas","desde":{total},
                   "max":1,"consumidor":"central","duravel":{duravel}"#
            ),
        )
    };
    pede(0);
    std::thread::sleep(Duration::from_secs(3)); // tres passadas
    assert!(
        base_caixa.0.join("loja").join("vendas#001.log").exists(),
        "saiu volume que a replica nao tinha no disco"
    );
    pede(total);
    esperar("o volume 1 sair com o duravel confirmado", || {
        !base_caixa.0.join("loja").join("vendas#001.log").exists()
    });
}

/// **Prova 8 do parecer:** a copia mais velha que o diario vivo ainda guarda
/// nao reaplica o PITR, e a recusa diz «expurgo» pela fabrica -- e nao «a
/// copia diverge» nem o texto cru do motor.
///
/// Defeito reposto (sem a pergunta da base no restaurar): sai a frase do
/// motor, sem «so guarda desde», e o teste cai.
#[test]
fn o_pitr_mais_velho_que_a_base_recusa_dizendo_expurgo() {
    let base_caixa = DirTemp::novo("expurgo-diario-pitr");
    let copias = DirTemp::novo("expurgo-diario-pitr-copias");
    let cx = caixa_com_prazo(&base_caixa.0, &[], 1);
    criar_vendas(cx.porta);
    assert_eq!(vender(cx.porta, 0, 100), 0);
    let destino = copias.0.display().to_string();
    let zip = exigir(
        cx.porta,
        &format!(r#""op":"backup","destino":"{destino}","database":"loja","zip":true"#),
    )
    .campo("resultado")
    .map(|r| r.texto_ou("arquivo", "").to_string())
    .unwrap_or_default();
    assert!(!zip.is_empty());
    assert_eq!(vender(cx.porta, 100, 6_000), 0);
    esperar("o caixa expurgar alem da copia", || {
        base_anunciada(cx.porta) > 100
    });
    let r = exigir(
        cx.porta,
        &format!(
            r#""op":"restaurar_backup","origem":"{zip}","database":"loja_volta",
               "ate":"2099-01-01T00:00:00Z""#
        ),
    );
    let texto = r.escrever();
    assert!(
        texto.contains("so guarda desde"),
        "a recusa do PITR nao saiu pela fabrica: {texto}"
    );
}
