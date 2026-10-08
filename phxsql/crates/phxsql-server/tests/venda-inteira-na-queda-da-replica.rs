//! Pedido 682: o PROCESSO da replica que morre no meio de um grupo nao deixa
//! a venda pela metade -- provado contra o sistema operacional, com `SIGKILL`.
//!
//! # O defeito
//!
//! O 676 aplica cada transacao da origem sob UMA tomada da trava: nenhum
//! leitor VIVO ve o meio dela. Mas o processo que morre no meio do grupo
//! deixa no disco parte da venda, e o arranque seguinte a servia assim ate a
//! rodada seguinte completar -- ou para sempre, com a origem fora do ar.
//!
//! # Como o processo morre no lugar certo, sem sorte
//!
//! O `phxsqld` de `debug` le `PHXSQL_TESTE_PARAR_NO_GRUPO=N`: depois do N-esimo
//! evento aplicado num grupo da replica ele avisa no erro padrao e para ali,
//! com a trava na mao. O teste le o aviso e mata o processo por fora
//! (`Child::kill` e `SIGKILL` no Unix) -- nada de `abort`, nada de panico: o
//! processo nao tem chance de arrumar nada.
//!
//! # O que se olha, e quando
//!
//! A reabertura acontece com a origem INALCANCAVEL (porta fechada): o que o
//! central mostra e so o que o arranque fez com o disco, sem rodada nenhuma
//! completando por tras. Com o defeito reposto (sem a marca do grupo), o
//! retrato e a venda pela metade. Depois, de volta a origem verdadeira, uma
//! segunda venda chega: o diario completado pelo arranque continua o da
//! origem, e a conferencia de continuidade nao rompe a tabela.
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

const TOKEN: &str = "venda-inteira-na-queda-da-replica";
const ITENS: usize = 5;
const ESPERA: Duration = Duration::from_secs(30);

/// A origem, aqui dentro: so ela escreve, e a replica e o processo de fora.
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
    c.replicacao.papel = Papel::Source;
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

/// `com_indice: false` cria as tres SEM indice nenhum: a reaplicacao que
/// grava num slot novo nao esbarra em unicidade, e a linha sai duplicada --
/// o caso do pedido 699.
fn criar_as_tabelas(porta: u16, com_indice: bool) {
    let mut b = Ligacao::nova(porta);
    b.exigir(r#""op":"criar_database","database":"loja""#);
    let indices = if com_indice {
        r#"[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#
    } else {
        "[]"
    };
    for tabela in ["vendas", "itens", "pagamentos"] {
        b.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}],
               "indices":{indices}"#
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

/// Os dois ganchos de `debug` que matam o processo no meio do grupo: depois
/// do N-esimo evento (682), ou DENTRO da N-esima inclusao, com o slot no
/// `.reg` e o evento fora do diario (699).
const NO_GRUPO: &str = "PHXSQL_TESTE_PARAR_NO_GRUPO";
const NO_REG: &str = "PHXSQL_TESTE_PARAR_NO_REG";
/// Pedido 713: o evento `<tabela>:<rowid>` falha com erro do dado, sempre.
const FALHAR_NO_EVENTO: &str = "PHXSQL_TESTE_FALHAR_NO_EVENTO";

/// O `phxsqld` replica, puxando de `porta_origem`. `parar_em` liga um dos
/// ganchos de `debug` acima.
fn subir_replica(
    dir: &Path,
    vez: u32,
    porta_origem: u16,
    parar_em: Option<(&str, u64)>,
) -> (Filho, u16) {
    subir_replica_com(dir, vez, porta_origem, parar_em, None)
}

/// [`subir_replica`] com o gancho do 713 ([`FALHAR_NO_EVENTO`]).
fn subir_replica_com(
    dir: &Path,
    vez: u32,
    porta_origem: u16,
    parar_em: Option<(&str, u64)>,
    falhar_em: Option<&str>,
) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "somente_leitura": true,
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "replicacao": {{
                    "papel": "replica", "id_servidor": "central",
                    "imagem_da_linha": true,
                    "origens": [{{ "nome": "caixa01", "host": "127.0.0.1",
                                   "porta": {porta_origem}, "token": "{TOKEN}",
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
    cmd.env_remove(NO_GRUPO)
        .env_remove(NO_REG)
        .env_remove(FALHAR_NO_EVENTO);
    if let Some((gancho, n)) = parar_em {
        cmd.env(gancho, n.to_string());
    }
    if let Some(evento) = falhar_em {
        cmd.env(FALHAR_NO_EVENTO, evento);
    }
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    if parar_em.is_some() {
        let ate = Instant::now() + ESPERA;
        loop {
            let texto = std::fs::read_to_string(&erro_padrao).unwrap_or_default();
            if texto.contains("teste: parado") {
                break;
            }
            assert!(
                Instant::now() < ate,
                "a replica nunca parou no meio do grupo: {texto}"
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

/// `(vendas, itens, pagamentos)` no central, em SANDUICHE: `vendas` de novo
/// no fim, e o retrato so vale se ela nao mudou -- tres pedidos soltos dao
/// um estado que nunca existiu quando um grupo entra entre eles (ver
/// `venda-inteira-na-replica.rs`).
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

/// **A prova real do 682.** O processo da replica morre (`SIGKILL`) com 3 dos
/// 7 eventos da venda no disco, e reabre sem a origem: o arranque completa o
/// grupo pela marca, antes de a porta abrir. Com a marca do grupo removida
/// (o defeito reposto), o central reabre mostrando a venda pela metade.
#[test]
fn o_sigkill_no_meio_do_grupo_nao_deixa_a_venda_pela_metade() {
    let base_o = DirTemp::novo("queda-replica-origem");
    let base_c = DirTemp::novo("queda-replica-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o, true);
    vender(porta_o, 1);

    // 1. A replica alcanca a venda e para no TERCEIRO evento do grupo.
    let (mut filho, _) = subir_replica(&base_c.0, 1, porta_o, Some((NO_GRUPO, 3)));
    // 2. SIGKILL, por fora.
    filho.0.kill().unwrap();
    filho.0.wait().unwrap();
    drop(filho);

    // 3. Reabre com a origem inalcancavel: so o arranque mexe no disco.
    let (filho, porta_c) = subir_replica(&base_c.0, 2, comum::porta_fechada(), None);
    let r = retrato(porta_c);
    assert_eq!(
        r,
        (1, ITENS, 1),
        "depois do SIGKILL no meio do grupo, o central reabriu mostrando a venda \
         PELA METADE (vendas, itens, pagamentos) = {r:?}"
    );
    drop(filho);

    // 4. De volta a origem verdadeira: a segunda venda chega inteira, o que
    //    prova que o diario completado continua o da origem (a conferencia
    //    de continuidade rompe a tabela que nao continua).
    let (filho, porta_c) = subir_replica(&base_c.0, 3, porta_o, None);
    vender(porta_o, 2);
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(porta_c);
        if r == (2, 2 * ITENS, 2) {
            break;
        }
        assert!(
            r == (1, ITENS, 1),
            "a segunda venda apareceu pela metade, ou a primeira encolheu: {r:?}"
        );
        assert!(
            Instant::now() < ate,
            "a segunda venda nao chegou em {} s -- a continuidade rompeu? {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(filho);
}

/// **A prova real do 699.** O processo morre DENTRO da terceira inclusao do
/// grupo, com o slot ja no `.reg` e o evento fora do diario, por `SIGKILL`.
/// As tabelas nao tem indice: nada recusa a linha repetida. A reabertura
/// sem a origem completa o grupo pela marca SEM gravar a linha de novo, e a
/// rodada seguinte, com a origem de volta, continua o diario.
///
/// Vermelho medido sem o `slot_ja_consumido` no `aplicar_evento_da_marca`
/// (o store): a recuperacao grava a terceira linha num slot NOVO, recusa a
/// divergencia de rowid depois de gravar, e o central reabre com a linha
/// duplicada e o resto do grupo de fora.
#[test]
fn o_sigkill_entre_o_reg_e_o_diario_nao_duplica_a_linha() {
    let base_o = DirTemp::novo("queda-no-reg-origem");
    let base_c = DirTemp::novo("queda-no-reg-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o, false);
    vender(porta_o, 1);

    let (mut filho, _) = subir_replica(&base_c.0, 1, porta_o, Some((NO_REG, 3)));
    filho.0.kill().unwrap();
    filho.0.wait().unwrap();
    drop(filho);

    let (filho, porta_c) = subir_replica(&base_c.0, 2, comum::porta_fechada(), None);
    let r = retrato(porta_c);
    assert_eq!(
        r,
        (1, ITENS, 1),
        "depois do SIGKILL entre o .reg e o diario, o central reabriu com \
         (vendas, itens, pagamentos) = {r:?} -- linha duplicada ou grupo pela metade"
    );
    drop(filho);

    let (filho, porta_c) = subir_replica(&base_c.0, 3, porta_o, None);
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
            "a segunda venda nao chegou em {} s -- o diario completado nao continua \
             o da origem? {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(filho);
}

/// O controle: sem queda nenhuma, a venda chega inteira na replica, e
/// nenhuma marca de grupo sobra no disco depois do `fsync` da rodada. E o que
/// tem de seguir de pe com o 682 ou o 699 repostos -- a marca e a conferencia
/// do `.reg` existem so para a queda.
#[test]
fn sem_queda_a_venda_chega_inteira_e_a_marca_sai() {
    let base_o = DirTemp::novo("replica-sem-queda-origem");
    let base_c = DirTemp::novo("replica-sem-queda-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o, false);
    vender(porta_o, 1);
    let (filho, porta_c) = subir_replica(&base_c.0, 1, porta_o, None);
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
            .filter(|e| e.file_name().to_string_lossy().starts_with("transacao_"))
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

fn marcas_na(pasta: &Path) -> Vec<String> {
    std::fs::read_dir(pasta)
        .map(|ls| {
            ls.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("transacao_"))
                .collect()
        })
        .unwrap_or_default()
}

/// **A prova real do 713 (F9).** Um erro do DADO no terceiro evento do grupo
/// (o segundo item), depois de a venda e o primeiro item entrarem. Vermelho
/// medido antes do conserto: o braco de erro APAGAVA a marca, e a replica
/// ficava com `(1, 1, 0)` -- a venda pela metade para sempre, contra a
/// decisao do dono no 685 («inteira ou nao chega, sem excecao»).
///
/// O erro e injetado e permanente: so o caminho da rodada o ve. O que o
/// conserto faz e o que o `COMMIT` faz quando a passada quebra depois da
/// marca -- completar para a FRENTE, na hora, com a mesma trava, pelo motor
/// da recuperacao --, e a venda fica inteira.
#[test]
fn o_erro_de_dado_no_meio_do_grupo_nao_deixa_a_venda_pela_metade() {
    let base_o = DirTemp::novo("dado-replica-origem");
    let base_c = DirTemp::novo("dado-replica-central");
    std::fs::create_dir_all(&base_c.0).unwrap();
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o, true);
    vender(porta_o, 1);

    let (filho, porta_c) = subir_replica_com(&base_c.0, 1, porta_o, None, Some("itens:2"));
    let erro = base_c.0.join("stderr-1.txt");
    let ate = Instant::now() + ESPERA;
    while !std::fs::read_to_string(&erro)
        .unwrap_or_default()
        .contains("teste: erro de dado injetado no evento itens:2")
    {
        assert!(
            Instant::now() < ate,
            "o erro injetado nunca chegou ao evento"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // Uma rodada inteira de folga: o estado que se le e o que fica.
    std::thread::sleep(Duration::from_millis(1500));
    let r = retrato(porta_c);
    let pasta = base_c.0.join("dados").join("loja");
    let sobra = marcas_na(&pasta);
    drop(filho);
    assert!(
        r == (1, ITENS, 1) || r == (0, 0, 0),
        "o erro de dado no meio do grupo deixou a venda PELA METADE: \
         (vendas, itens, pagamentos) = {r:?}; marcas no disco: {sobra:?}"
    );

    // E o arranque, sem a origem, nao muda o que se via.
    let (filho, porta_c) = subir_replica(&base_c.0, 2, comum::porta_fechada(), None);
    let depois = retrato(porta_c);
    drop(filho);
    assert_eq!(depois, r, "o arranque mudou a venda: {r:?} -> {depois:?}");
}
