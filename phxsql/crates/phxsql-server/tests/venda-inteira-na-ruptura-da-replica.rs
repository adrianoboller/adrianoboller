//! Pedido 299 (F0-F2): onde a replica fiel PARA, ela para por TRANSACAO, e
//! nao por tabela -- provado contra processo `phxsqld` de verdade.
//!
//! # Os dois restos que este arquivo prova
//!
//! **R1** -- a continuidade que rompe numa tabela tirava SO aquela tabela do
//! grupo: a venda que tocava `vendas`, `itens` e `pagamentos` entrava nas duas
//! primeiras e nunca na terceira, legivel e indistinguivel de uma venda sem
//! pagamento. O caminho real e a escrita local numa replica sem
//! `somente_leitura`, que toma o lugar do evento da origem no diario daqui.
//!
//! **R2** -- a divergencia DETERMINISTICA no meio do grupo da replica fiel (o
//! rowid que nao bate) batia de novo na completacao do 713, a marca ficava no
//! disco, e o arranque a tratava como impossivel e a APAGAVA: a metade ficava
//! para sempre. O caminho real e o slot sem evento (498 H3): a inclusao que
//! morre entre o `.reg` e o diario, numa escrita local, deixa no `.reg` uma
//! linha que o diario nao conta, e a inclusao seguinte da origem sai com o
//! rowid um a frente.
//!
//! # A conta que se olha
//!
//! Por VENDA, e nao por tabela: `(vendas, itens, pagamentos)` da venda `n`
//! tem de ser `(0,0,0)` ou `(1,ITENS,1)` -- nunca o meio.
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
use phxsql_core::value::Value;
use phxsql_server::{Config, Papel, Servidor};

const TOKEN: &str = "venda-inteira-na-ruptura-da-replica";
const ITENS: usize = 4;
const ESPERA: Duration = Duration::from_secs(30);

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

/// A venda `n` num `COMMIT` so: a venda, `ITENS` itens e -- com `pagar` -- um
/// pagamento. A venda sem pagamento e a transacao que NAO toca `pagamentos`.
fn vender(porta: u16, n: usize, pagar: bool) {
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
    if pagar {
        b.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{{"id":{n},"venda":{n}}}"#
        ));
    }
    b.exigir(r#""op":"commit""#);
}

/// Pedido 713: o evento `<tabela>:<rowid>` falha com erro do dado.
const FALHAR_NO_EVENTO: &str = "PHXSQL_TESTE_FALHAR_NO_EVENTO";

/// O `phxsqld` replica. `somente_leitura: false` e o que deixa a escrita
/// local entrar -- o caminho real do R1.
fn subir_replica(dir: &Path, vez: u32, porta_origem: u16, somente_leitura: bool) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "somente_leitura": {somente_leitura},
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
        .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
        .env_remove(FALHAR_NO_EVENTO)
        .env_remove("PHXSQL_TESTE_PARAR_NO_GRUPO")
        .env_remove("PHXSQL_TESTE_PARAR_NO_REG");
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

/// As linhas de `tabela` no central, pelo campo `venda`.
fn vendas_em(porta: u16, tabela: &str) -> Vec<i64> {
    let r = pedir(
        porta,
        &format!(
            r#"{{"token":"{TOKEN}","op":"varrer","database":"loja","tabela":"{tabela}","max":5000}}"#
        ),
    );
    Json::analisar(&r)
        .ok()
        .and_then(|j| j.campo("resultado").cloned())
        .and_then(|r| {
            r.campo("linhas")
                .and_then(Json::lista)
                .map(<[Json]>::to_vec)
        })
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.campo("venda").and_then(Json::inteiro))
        .collect()
}

/// `(vendas, itens, pagamentos)` da venda `n`, em SANDUICHE: as tres tabelas
/// lidas de novo ate duas leituras seguidas darem o mesmo -- tres pedidos
/// soltos dao um estado que nunca existiu quando um grupo entra entre eles.
fn retrato(porta: u16, n: i64) -> (usize, usize, usize) {
    let ler = || {
        let conta = |t: &str| vendas_em(porta, t).iter().filter(|v| **v == n).count();
        (conta("vendas"), conta("itens"), conta("pagamentos"))
    };
    let mut antes = ler();
    loop {
        let agora = ler();
        if agora == antes {
            return agora;
        }
        antes = agora;
    }
}

fn esperar(porta: u16, n: i64, alvo: (usize, usize, usize)) {
    let ate = Instant::now() + ESPERA;
    loop {
        let r = retrato(porta, n);
        if r == alvo {
            return;
        }
        assert!(
            Instant::now() < ate,
            "a venda {n} nao chegou a {alvo:?} em {} s: {r:?}",
            ESPERA.as_secs()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn estado(porta: u16) -> Json {
    let r = pedir(
        porta,
        &format!(r#"{{"token":"{TOKEN}","op":"replicacao_estado"}}"#),
    );
    let j = Json::analisar(&r).unwrap();
    j.campo("resultado").cloned().unwrap_or(j)
}

/// O que a origem `caixa01` diz neste central.
fn da_origem(porta: u16) -> Json {
    estado(porta)
        .campo("origens")
        .and_then(|o| o.campo("caixa01"))
        .cloned()
        .unwrap_or(Json::Nulo)
}

/// **A prova real do R1.** A escrita local em `pagamentos` toma o lugar do
/// evento da origem no diario daqui; a venda 3 da origem, que toca as tres
/// tabelas, chega. Vermelho medido no HEAD (`edbd6180`): `(1, ITENS, 0)` -- a
/// venda entra sem o pagamento, para sempre.
///
/// E o comportamento velho junto, nos dois lados da venda parada: a venda 2,
/// que NAO toca `pagamentos` e vem ANTES, entra; a venda 4, que tambem nao
/// toca e vem DEPOIS, espera -- ela pode depender da parada (os tres maduros
/// param o fluxo, e nao a tabela) --, e a 5 tambem.
#[test]
fn a_tabela_rompida_segura_a_transacao_inteira_e_as_seguintes() {
    let base_o = DirTemp::novo("ruptura-origem");
    let base_c = DirTemp::novo("ruptura-central");
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1, true);

    let (filho, porta_c) = subir_replica(&base_c.0, 1, porta_o, false);
    // A transacao sa entra, como sempre.
    esperar(porta_c, 1, (1, ITENS, 1));

    // DUAS escritas locais em `pagamentos`, aceitas porque o central nao e
    // `somente_leitura`. Duas, e nao uma: a conferencia de continuidade
    // compara so o evento `posicao - 1`, e com duas ele e o SEGUNDO evento da
    // origem que falta aqui -- a barreira tirada dele deixaria a venda 3
    // passar e seguraria so a 5.
    let mut l = Ligacao::nova(porta_c);
    for id in [999, 998] {
        l.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{{"id":{id},"venda":999}}"#
        ));
    }

    vender(porta_o, 2, false);
    vender(porta_o, 3, true);
    vender(porta_o, 4, false);
    vender(porta_o, 5, true);

    // A venda 2 nao toca a rompida e vem antes: entra.
    esperar(porta_c, 2, (1, ITENS, 0));
    // Rodadas de folga: o estado que se le e o que fica.
    std::thread::sleep(Duration::from_millis(2500));
    let tres = retrato(porta_c, 3);
    let quatro = retrato(porta_c, 4);
    let cinco = retrato(porta_c, 5);
    let origem = da_origem(porta_c);
    drop(filho);
    assert_eq!(
        tres,
        (0, 0, 0),
        "a venda 3 entrou PELA METADE (vendas, itens, pagamentos) = {tres:?}: a \
         tabela rompida saiu do grupo e as irmas da mesma transacao entraram. \
         Estado: {}",
        origem.escrever()
    );
    assert_eq!(
        quatro,
        (0, 0, 0),
        "a venda 4 vem DEPOIS da transacao parada e entrou: {quatro:?}"
    );
    assert_eq!(cinco, (0, 0, 0), "a venda 5 entrou: {cinco:?}");
    // E a replica DIZ qual transacao parou, e em que tabela.
    let parada = origem
        .campo("transacoes_paradas")
        .and_then(|p| p.campo("loja"))
        .cloned()
        .unwrap_or(Json::Nulo);
    let texto = parada.escrever();
    assert!(
        parada.campo("tx").and_then(Json::inteiro).unwrap_or(0) > 0,
        "a parada nao nomeia o id da transacao: {}",
        origem.escrever()
    );
    assert!(
        texto.contains("pagamentos"),
        "a parada nao nomeia a tabela rompida: {texto}"
    );
}

/// Faz, no disco do central PARADO, a escrita que cai entre o `.reg` e o
/// diario (o 498 H3), pelo panico de teste de `debug` no `ponto`: o `.reg`
/// mudou e o diario nao. E o que nenhuma conferencia de continuidade ve -- o
/// diario daqui continua o da origem, evento a evento.
///
/// - `InserirDepoisDoContador`: um slot a mais, sem evento -- a proxima
///   inclusao da origem sai aqui com o rowid um a frente;
/// - `ExcluirDepoisDoSlot`: o slot 1 livre, sem evento -- a proxima
///   alteracao da origem nele nao acha a linha.
fn escrita_sem_evento(
    base_c: &Path,
    tabela: &str,
    ponto: phxsql_store::ndx::panico_de_teste::Ponto,
) {
    use phxsql_store::ndx::panico_de_teste::{armar, Ponto};
    let inst = phxsql_store::catalogo::Instancia::nova(base_c.join("dados")).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let mut t = db.abrir_qualificada(tabela).unwrap();
    let eventos = t.eventos().unwrap();
    let caiu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        armar(ponto);
        match ponto {
            Ponto::InserirDepoisDoContador => {
                let _ = t.inserir(&[Value::Int(999), Value::Int(999)]);
            }
            _ => {
                let _ = t.excluir_de_vez(1, "teste");
            }
        }
    }));
    assert!(caiu.is_err(), "o panico de teste nao disparou");
    drop(t);
    let mut t = db.abrir_qualificada(tabela).unwrap();
    assert_eq!(
        t.eventos().unwrap(),
        eventos,
        "o cenario mudou: a escrita foi ao diario"
    );
}

/// Espera o primeiro erro da rodada, e o devolve: o `ultimo_erro` some na
/// rodada seguinte que der certo, entao ele se le enquanto esta la.
fn esperar_erro(porta: u16) -> String {
    let ate = Instant::now() + ESPERA;
    loop {
        let erro = da_origem(porta).texto_ou("ultimo_erro", "").to_string();
        if !erro.is_empty() {
            return erro;
        }
        assert!(
            Instant::now() < ate,
            "a rodada nunca deu erro: {}",
            da_origem(porta).escrever()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A venda `n` que, no lugar de um pagamento novo, ALTERA o pagamento de
/// rowid 1 para apontar para ela.
fn vender_alterando_o_pagamento(porta: u16, n: usize) {
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
        r#""op":"atualizar","database":"loja","tabela":"pagamentos","rowid":1,"linha":{{"id":1,"venda":{n}}}"#
    ));
    b.exigir(r#""op":"commit""#);
}

/// O central vivo e, depois, o arranque sem a origem: o retrato da venda 2
/// nos dois, e o erro que a rodada disse.
fn vivo_e_arranque(
    base_c: &Path,
    porta_o: u16,
) -> ((usize, usize, usize), (usize, usize, usize), String) {
    let (filho, porta_c) = subir_replica(base_c, 2, porta_o, true);
    let erro = esperar_erro(porta_c);
    // Rodadas de folga: o estado que se le e o que fica.
    std::thread::sleep(Duration::from_millis(2000));
    let vivo = retrato(porta_c, 2);
    drop(filho);
    let (filho, porta_c) = subir_replica(base_c, 3, comum::porta_fechada(), true);
    let depois = retrato(porta_c, 2);
    drop(filho);
    (vivo, depois, erro)
}

/// **A prova real do R2.** A venda 2 altera o pagamento de rowid 1, que no
/// central ja saiu do `.reg` sem evento: a alteracao nao acha a linha -- erro
/// DETERMINISTICO, que bate de novo na completacao do 713 e no arranque, que
/// a conta como impossivel e apaga a marca. Vermelho medido no HEAD
/// (`edbd6180`): `(1, ITENS, 0)` com o central vivo e depois do arranque -- a
/// metade fica para sempre.
///
/// O ensaio do grupo (F1) ve a falta da linha ANTES do primeiro evento: nada
/// da venda 2 entra, e o arranque nao tem o que completar. E diz.
#[test]
fn a_divergencia_no_meio_do_grupo_nao_deixa_metade_no_arranque() {
    use phxsql_store::ndx::panico_de_teste::Ponto;
    let base_o = DirTemp::novo("divergencia-origem");
    let base_c = DirTemp::novo("divergencia-central");
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1, true);
    let (filho, porta_c) = subir_replica(&base_c.0, 1, porta_o, true);
    esperar(porta_c, 1, (1, ITENS, 1));
    drop(filho);

    escrita_sem_evento(&base_c.0, "pagamentos", Ponto::ExcluirDepoisDoSlot);
    vender_alterando_o_pagamento(porta_o, 2);

    let (vivo, depois, erro) = vivo_e_arranque(&base_c.0, porta_o);
    assert_eq!(
        (vivo, depois),
        ((0, 0, 0), (0, 0, 0)),
        "a venda 2 ficou PELA METADE (com o central vivo, depois do arranque). \
         Erro: {erro}"
    );
    assert!(
        erro.contains("ANTES do primeiro evento: nada dela entrou"),
        "a parada nao diz que o ensaio a pegou: {erro}"
    );
}

/// O irmao pela INCLUSAO: o slot a mais faz a inclusao do pagamento sair no
/// rowid 3 onde a origem diz 2. O `aplicar_evento` confere o rowid DEPOIS de
/// gravar, entao, sem o ensaio, a linha entra no rowid errado antes da recusa
/// -- vermelho medido no HEAD (`edbd6180`): `(1, ITENS, 1)` com o pagamento
/// no rowid 3 e o diario daqui fora do da origem dali em diante. Com o
/// ensaio, a conta `slot_count + 1` recusa antes: nada da venda entra.
#[test]
fn a_inclusao_que_diverge_nao_grava_no_rowid_errado() {
    use phxsql_store::ndx::panico_de_teste::Ponto;
    let base_o = DirTemp::novo("divergencia-inc-origem");
    let base_c = DirTemp::novo("divergencia-inc-central");
    let (_origem, porta_o) = subir_origem(&base_o.0);
    criar_as_tabelas(porta_o);
    vender(porta_o, 1, true);
    let (filho, porta_c) = subir_replica(&base_c.0, 1, porta_o, true);
    esperar(porta_c, 1, (1, ITENS, 1));
    drop(filho);

    escrita_sem_evento(&base_c.0, "pagamentos", Ponto::InserirDepoisDoContador);
    vender(porta_o, 2, true);

    let (vivo, depois, erro) = vivo_e_arranque(&base_c.0, porta_o);
    assert_eq!(
        (vivo, depois),
        ((0, 0, 0), (0, 0, 0)),
        "a venda 2 entrou com o rowid divergente (com o central vivo, depois do \
         arranque). Erro: {erro}"
    );
    assert!(
        erro.contains("ANTES do primeiro evento: nada dela entrou"),
        "a parada nao diz que o ensaio a pegou: {erro}"
    );
}
