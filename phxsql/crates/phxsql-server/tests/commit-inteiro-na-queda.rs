//! Pedido 702: o `COMMIT` que o processo nao terminou e que o arranque
//! completa chega a replica encadeada como UMA transacao -- provado contra o
//! sistema operacional, com `SIGKILL`.
//!
//! # O defeito
//!
//! A passada do `COMMIT` grava cada evento com o id da unidade de transacao
//! (pedido 676), e a replica junta as tabelas por esse id. O processo que
//! morre no meio da passada deixa parte da venda no diario com o id X; o
//! arranque completava o resto pela marca com um id NOVO, e a replica da
//! replica recebia a venda em dois pedacos -- o leitor dela via o meio.
//!
//! # Como o processo morre no lugar certo
//!
//! O `phxsqld` de `debug` le `PHXSQL_TESTE_PARAR_NO_COMMIT=N` e se mata por
//! `SIGKILL` depois da N-esima escrita da passada, com a trava na mao e a
//! marca no disco.
//!
//! # O que se olha
//!
//! O diario das tres tabelas, aberto direto do disco com o servidor parado:
//! cada venda tem um id so, e as duas vendas tem ids diferentes. A primeira
//! venda e o controle do outro lado -- o processo morre depois dela com a
//! marca ainda pendente da janela, e a recuperacao que a acha INTEIRA nao
//! pode pendurar nada nela nem junta-la a segunda.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "commit-inteiro-na-queda";
const ITENS: u64 = 5;
const GANCHO: &str = "PHXSQL_TESTE_PARAR_NO_COMMIT";

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    /// `None` quando a conexao caiu sem resposta -- o que o `SIGKILL` faz.
    fn pedir(&mut self, corpo: &str) -> Option<Json> {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
        let mut r = String::new();
        match self.leitor.read_line(&mut r) {
            Ok(n) if n > 0 => Some(Json::analisar(&r).unwrap()),
            _ => None,
        }
    }

    fn exigir(&mut self, corpo: &str) {
        let r = self.pedir(corpo).expect("a conexao caiu sem resposta");
        assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    }
}

fn subir(dir: &Path, vez: u32, parar_em: Option<u64>) -> (Filho, u16) {
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
                "replicacao": {{ "papel": "source", "id_servidor": "caixa01",
                                 "imagem_da_linha": true }}
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
        .env_remove(GANCHO);
    if let Some(n) = parar_em {
        cmd.env(GANCHO, n.to_string());
    }
    let mut filho = Filho(cmd.spawn().expect("nao consegui iniciar o phxsqld"));
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

/// A venda `n`: uma linha em `vendas`, `ITENS` em `itens` e uma em
/// `pagamentos`, num `COMMIT` so. Devolve se o `COMMIT` respondeu.
fn vender(porta: u16, n: u64) -> bool {
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
    b.pedir(r#""op":"commit""#).is_some()
}

/// Os ids de transacao dos eventos de cada venda, lidos do disco: a venda de
/// um evento e a coluna `venda` da imagem dele.
fn ids_por_venda(dados: &Path) -> Vec<BTreeSet<u64>> {
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let mut por_venda = vec![BTreeSet::new(), BTreeSet::new()];
    for tabela in ["vendas", "itens", "pagamentos"] {
        let mut t = db.abrir_qualificada(tabela).unwrap();
        let total = t.eventos().unwrap();
        for (ev, imagem) in t.diario_com_imagem(0, total).unwrap() {
            let v = t.valores_da_imagem(&imagem).unwrap();
            let venda = match v.get(1) {
                Some(phxsql_core::value::Value::Int(n)) => *n as usize,
                outro => panic!("imagem sem a coluna venda: {outro:?}"),
            };
            por_venda[venda - 1].insert(ev.tx);
        }
    }
    por_venda
}

/// **A prova real do 702.** Vermelho medido sem o `id_da_metade_que_entrou`
/// (o store): a segunda venda sai com DOIS ids -- o da metade que entrou e o
/// novo que o arranque deu ao resto.
#[test]
fn o_commit_completado_no_arranque_leva_o_id_da_metade_que_entrou() {
    let base = DirTemp::novo("commit-702");
    std::fs::create_dir_all(&base.0).unwrap();

    // 1. A primeira venda inteira; o processo morre com a marca dela ainda
    //    pendente da janela (o controle: marca INTEIRA no arranque).
    let (filho, porta) = subir(&base.0, 1, None);
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
    drop(b);
    assert!(vender(porta, 1), "a primeira venda nao confirmou");
    drop(filho);

    // 2. A segunda venda morre depois da TERCEIRA escrita da passada: a venda
    //    e dois itens no diario, o resto so na marca.
    let (filho, porta) = subir(&base.0, 2, Some(3));
    assert!(
        !vender(porta, 2),
        "o COMMIT respondeu: o gancho do 702 nao matou o processo"
    );
    let ate = Instant::now() + Duration::from_secs(20);
    let erro = base.0.join("stderr-2.txt");
    while !std::fs::read_to_string(&erro)
        .unwrap_or_default()
        .contains("teste: parado no meio da passada do COMMIT")
    {
        assert!(Instant::now() < ate, "o processo nao parou na passada");
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(filho);

    // 3. O arranque completa a marca antes de abrir a porta.
    let (filho, _) = subir(&base.0, 3, None);
    drop(filho);

    let ids = ids_por_venda(&base.0.join("dados"));
    assert_eq!(
        ids[1].len(),
        1,
        "a venda completada no arranque saiu com {} ids de transacao {:?}: a replica \
         encadeada a recebe em pedacos",
        ids[1].len(),
        ids[1]
    );
    assert_eq!(
        ids[0].len(),
        1,
        "a primeira venda saiu partida: {:?}",
        ids[0]
    );
    assert_ne!(
        ids[0], ids[1],
        "a recuperacao juntou a segunda venda a primeira"
    );
}
