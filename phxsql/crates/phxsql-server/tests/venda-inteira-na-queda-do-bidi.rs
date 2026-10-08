//! Pedido 698: o PROCESSO do bidirecional que morre no meio de um grupo nao
//! deixa a venda pela metade -- provado contra o sistema operacional, com
//! `SIGKILL`. O irmao do `venda-inteira-na-queda-da-replica.rs` (682).
//!
//! # O defeito
//!
//! O 682 deu a marca `.tx` ao grupo da replica fiel. O bidirecional aplica
//! pela CHAVE (`aplicar_grupo_bidi`) e ficou de fora: o processo que morria
//! no meio do grupo deixava no disco parte da venda, e o arranque a servia
//! assim ate a rodada seguinte -- ou para sempre, com o outro lado fora do ar.
//!
//! # Como o processo morre no lugar certo
//!
//! O `phxsqld` de `debug` le `PHXSQL_TESTE_PARAR_NO_GRUPO=N`: depois do N-esimo
//! evento aplicado por chave ele se mata por `SIGKILL`, com a trava na mao.
//!
//! # O que se olha
//!
//! A reabertura acontece com o outro lado INALCANCAVEL: o que o central
//! mostra e so o que o arranque fez com o disco. Depois, com o outro lado de
//! volta, a rodada reaplica o grupo inteiro (a posicao nao andou) e uma
//! segunda venda chega -- o que entrou pela marca nao entra duas vezes.
#![cfg(unix)]

mod comum;
use comum::{pedir, porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Papel, Servidor};

const TOKEN: &str = "venda-inteira-na-queda-do-bidi";
const ITENS: usize = 5;
const ESPERA: Duration = Duration::from_secs(30);

/// O outro lado, aqui dentro: so ele escreve, e o lado que puxa e o
/// processo de fora.
fn subir_origem(base: &Path) -> (Arc<Servidor>, u16) {
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
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = "caixa01".into();
    c.replicacao.imagem_da_linha = true;
    let (ouvinte, _) = comum::ouvinte_reservado();
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    (s, porta)
}

/// Uma conexao que FICA -- a transacao vive na sessao.
struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo.set_nodelay(true).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn exigir(&mut self, corpo: &str) {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
        assert!(j.booleano_ou("ok", false), "{corpo} -> {r}");
    }
}

/// O bidirecional casa pela chave: as tres nascem com a primaria.
fn criar_as_tabelas(porta: u16) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["vendas", "itens", "pagamentos"] {
        b.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}],
               "indices":[{{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}}]"#
        ));
    }
}

/// A venda `n`, com `ITENS` itens e um pagamento, num `COMMIT` so.
fn vender(porta: u16, n: usize) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"begin","database":"loja""#);
    b.exigir(&format!(
        r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{n},"venda":{n}}}"#
    ));
    for i in 1..=ITENS {
        let id = (n - 1) * ITENS + i;
        b.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{id},"venda":{n}}}"#
        ));
    }
    b.exigir(&format!(
        r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{{"id":{n},"venda":{n}}}"#
    ));
    b.exigir(r#""op":"commit""#);
}

/// Onde o gancho de `debug` mata o central: a variavel, o N, e o aviso que
/// ele deixa no erro padrao antes de morrer.
type Parada = (&'static str, u64, &'static str);

/// Depois do N-esimo evento aplicado por chave -- a prova do 698.
fn no_grupo(n: u64) -> Option<Parada> {
    Some((
        "PHXSQL_TESTE_PARAR_NO_GRUPO",
        n,
        "parado no meio do grupo do bidirecional",
    ))
}

/// DENTRO da N-esima inclusao tentada, com o slot no `.reg` e o evento fora
/// do diario -- a prova do 700.
fn no_reg(n: u64) -> Option<Parada> {
    Some((
        "PHXSQL_TESTE_PARAR_NO_REG",
        n,
        "bidirecional parado entre o .reg e o diario",
    ))
}

/// O `phxsqld` bidirecional, puxando de `porta_outro`. `parar` liga o gancho
/// de `debug` que o mata no meio do grupo.
fn subir_central(dir: &Path, vez: u32, porta_outro: u16, parar: Option<Parada>) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "replicacao": {{
                    "papel": "multi", "id_servidor": "central",
                    "imagem_da_linha": true,
                    "origens": [{{ "nome": "caixa01", "host": "127.0.0.1",
                                   "porta": {porta_outro}, "token": "{TOKEN}",
                                   "databases": ["loja"], "reconectar_em": 1 }}]
                }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join(format!("stderr-{vez}.txt"));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_phxsqld"));
    cmd.arg("--config")
        .arg(&config)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()));
    cmd.env_remove("PHXSQL_TESTE_PARAR_NO_GRUPO")
        .env_remove("PHXSQL_TESTE_PARAR_NO_REG");
    if let Some((var, n, _)) = parar {
        cmd.env(var, n.to_string());
    }
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    if let Some((_, _, aviso)) = parar {
        let ate = Instant::now() + ESPERA;
        loop {
            let texto = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
            if texto.contains(aviso) {
                break;
            }
            assert!(
                Instant::now() < ate,
                "o central nunca parou no meio do grupo: {texto}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    (filho, porta)
}

fn contar(porta: u16, tabela: &str) -> usize {
    let r = pedir(
        porta,
        &format!(
            r#"{{"token":"{TOKEN}","op":"varrer","database":"loja","tabela":"{tabela}","max":5000}}"#
        ),
    );
    Json::analisar(&r)
        .ok()
        .and_then(|j| j.campo("resultado").cloned())
        .and_then(|r| r.campo("linhas").and_then(Json::lista).map(<[Json]>::len))
        .unwrap_or(0)
}

/// `(vendas, itens, pagamentos)` no central, em SANDUICHE -- ver
/// `venda-inteira-na-replica.rs`.
fn retrato(porta: u16) -> (usize, usize, usize) {
    loop {
        let antes = contar(porta, "vendas");
        let itens = contar(porta, "itens");
        let pagamentos = contar(porta, "pagamentos");
        if contar(porta, "vendas") == antes {
            return (antes, itens, pagamentos);
        }
    }
}

/// **A prova real do 698.** O central bidirecional morre (`SIGKILL`) com 3
/// dos 7 eventos da venda aplicados por chave, e reabre sem o outro lado: o
/// arranque completa o grupo pela marca, antes de a porta abrir.
///
/// Vermelho medido sem a chamada a `completar_marcas_do_bidi` no
/// `Servidor::novo`: o central reabre mostrando a venda pela metade.
#[test]
fn o_sigkill_no_meio_do_grupo_do_bidi_nao_deixa_a_venda_pela_metade() {
    let base_o = DirTemp::novo("queda-bidi-caixa");
    let base_c = DirTemp::novo("queda-bidi-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_outro, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1);

    let (mut filho, _) = subir_central(&base_c.0, 1, porta_o, no_grupo(3));
    filho.0.kill().unwrap();
    filho.0.wait().unwrap();
    drop(filho);

    let (filho, porta_c) = subir_central(&base_c.0, 2, comum::porta_fechada(), None);
    let r = retrato(porta_c);
    assert_eq!(
        r,
        (1, ITENS, 1),
        "depois do SIGKILL no meio do grupo do bidirecional, o central reabriu \
         mostrando a venda PELA METADE (vendas, itens, pagamentos) = {r:?}"
    );
    let marcas: Vec<_> = std::fs::read_dir(base_c.0.join("dados").join("loja"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("bidi_"))
        .collect();
    assert!(marcas.is_empty(), "a marca completada continua no disco");
    drop(filho);

    // De volta ao outro lado: a rodada reaplica o grupo 1 (a posicao nao
    // andou) sem duplicar, e a segunda venda chega inteira.
    let (filho, porta_c) = subir_central(&base_c.0, 3, porta_o, None);
    vender(porta_o, 2);
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(porta_c);
        if r == (2, 2 * ITENS, 2) {
            break;
        }
        assert!(
            r == (1, ITENS, 1),
            "a segunda venda apareceu pela metade, ou a primeira mudou: {r:?}"
        );
        assert!(
            Instant::now() < ate,
            "a segunda venda nao chegou em {} s: {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(filho);
}

/// O controle: sem queda nenhuma, a venda chega inteira pelo bidirecional, e
/// nenhuma marca de grupo sobra no disco depois do `fsync` da rodada. E o que
/// tem de seguir de pe com qualquer das duas metades do 698 reposta -- a
/// marca existe so para a queda.
#[test]
fn sem_queda_a_venda_chega_inteira_e_a_marca_sai() {
    let base_o = DirTemp::novo("bidi-sem-queda-caixa");
    let base_c = DirTemp::novo("bidi-sem-queda-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_outro, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1);
    let (filho, porta_c) = subir_central(&base_c.0, 1, porta_o, None);
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(porta_c);
        if r == (1, ITENS, 1) {
            break;
        }
        assert!(r == (0, 0, 0), "a venda apareceu pela metade: {r:?}");
        assert!(
            Instant::now() < ate,
            "a venda nao chegou em {} s",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // A marca sai DEPOIS do `fsync` da rodada, que vem depois de a venda
    // ficar visivel: espera-se por ela com o processo vivo, senao matar o
    // processo entre as duas coisas faria a sobra parecer defeito.
    let pasta = base_c.0.join("dados").join("loja");
    loop {
        let sobra: Vec<_> = std::fs::read_dir(&pasta)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("bidi_"))
            .map(|e| e.file_name())
            .collect();
        if sobra.is_empty() {
            break;
        }
        assert!(
            Instant::now() < ate,
            "marca de grupo sobrou no disco: {sobra:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(filho);
}

/// Um evento do diario como a prova o confere: `(operacao, rowid, tx)`.
type EventoLido = (String, u64, u64);

/// O diario de cada tabela do central, lido do disco com o processo morto.
fn diarios(base_c: &Path) -> Vec<(&'static str, Vec<EventoLido>)> {
    let inst = phxsql_store::catalogo::Instancia::nova(base_c.join("dados")).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    ["vendas", "itens", "pagamentos"]
        .into_iter()
        .map(|nome| {
            let mut t = db.abrir_qualificada(nome).unwrap();
            let ev = t
                .diario(0, 0)
                .unwrap()
                .into_iter()
                .map(|e| (e.operacao.nome().to_string(), e.rowid, e.tx))
                .collect();
            (nome, ev)
        })
        .collect()
}

/// **A prova real do 700.** O central bidirecional morre (`SIGKILL`) DENTRO da
/// 3.a inclusao do grupo -- o slot ja no `.reg`, o evento fora do diario -- e
/// reabre sem o outro lado. O arranque completa o grupo pela marca, e a linha
/// que estava sendo incluida entra no diario daqui como a INCLUSAO que ela e,
/// e nao como uma alteracao por cima de um rowid que o diario nunca viu
/// nascer: a replica encadeada nao recebe alteracao de linha que nao tem.
///
/// E o pedido 701 (b) no bidirecional: os eventos do grupo -- os de antes da
/// queda e os que o arranque completou -- levam UM id de transacao so.
///
/// Vermelho medido sem a orfa no `aplicar_por_chave`: o diario de `itens`
/// sai com `alteracao` no rowid 2 e nenhuma inclusao dele. Sem o
/// `adotar_o_id_do_grupo`: o grupo sai com dois ids.
#[test]
fn a_queda_entre_o_reg_e_o_diario_no_bidi_completa_a_inclusao() {
    let base_o = DirTemp::novo("queda-bidi-reg-caixa");
    let base_c = DirTemp::novo("queda-bidi-reg-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_outro, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1);

    let (mut filho, _) = subir_central(&base_c.0, 1, porta_o, no_reg(3));
    filho.0.kill().unwrap();
    filho.0.wait().unwrap();
    drop(filho);

    let (filho, porta_c) = subir_central(&base_c.0, 2, comum::porta_fechada(), None);
    let r = retrato(porta_c);
    assert_eq!(
        r,
        (1, ITENS, 1),
        "depois do SIGKILL dentro da inclusao, o central reabriu com a venda \
         errada (vendas, itens, pagamentos) = {r:?}"
    );
    drop(filho);

    let d = diarios(&base_c.0);
    let mut ids = Vec::new();
    for (nome, eventos) in &d {
        let linhas = if *nome == "itens" { ITENS } else { 1 };
        let esperado: Vec<(String, u64)> = (1..=linhas as u64)
            .map(|r| ("inclusao".to_string(), r))
            .collect();
        let tem: Vec<(String, u64)> = eventos.iter().map(|(o, r, _)| (o.clone(), *r)).collect();
        assert_eq!(
            tem, esperado,
            "o diario de {nome} nao tem uma INCLUSAO por linha: o arranque gravou \
             por cima de um rowid cuja inclusao a queda deixou fora do diario"
        );
        ids.extend(eventos.iter().map(|(_, _, tx)| *tx));
    }
    assert!(ids[0] != 0, "o volume tem de guardar o id");
    assert!(
        ids.iter().all(|t| *t == ids[0]),
        "o grupo saiu do arranque em pedacos -- ids {ids:?}"
    );

    // De volta ao outro lado: nada entra duas vezes, e a 2.a venda chega.
    let (filho, porta_c) = subir_central(&base_c.0, 3, porta_o, None);
    vender(porta_o, 2);
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(porta_c);
        if r == (2, 2 * ITENS, 2) {
            break;
        }
        assert!(
            r == (1, ITENS, 1),
            "a segunda venda apareceu pela metade, ou a primeira mudou: {r:?}"
        );
        assert!(
            Instant::now() < ate,
            "a segunda venda nao chegou em {} s: {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(filho);
}
