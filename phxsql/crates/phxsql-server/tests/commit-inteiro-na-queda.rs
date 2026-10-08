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
/// Pedido 710: o processo morre DENTRO da N-esima escrita da passada, quando
/// ela e uma inclusao -- com o slot e o contador no `.reg` e sem o evento.
const GANCHO_DO_REG: &str = "PHXSQL_TESTE_PARAR_NO_REG_DO_COMMIT";

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
    subir_com(dir, vez, parar_em.map(|n| (GANCHO, n)))
}

/// O gancho que se nomeia: o do 702 ([`GANCHO`]) ou o do 710
/// ([`GANCHO_DO_REG`]). Os dois saem do ambiente antes, para um nao herdar o
/// outro.
fn subir_com(dir: &Path, vez: u32, gancho: Option<(&str, u64)>) -> (Filho, u16) {
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
        .env_remove(GANCHO)
        .env_remove(GANCHO_DO_REG);
    if let Some((nome, n)) = gancho {
        cmd.env(nome, n.to_string());
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

/// As inclusoes de cada rowid de `tabela`, lidas do diario no disco, e os
/// rowids com slot vivo.
fn inclusoes_e_slots(dados: &Path, tabela: &str) -> (Vec<u64>, Vec<u64>) {
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let mut t = db.abrir_qualificada(tabela).unwrap();
    let total = t.eventos().unwrap();
    let inclusoes = t
        .diario(0, total)
        .unwrap()
        .iter()
        .filter(|e| e.operacao == phxsql_store::log::Operacao::Inclusao)
        .map(|e| e.rowid)
        .collect();
    let vivos = (1..=t.slots())
        .filter(|&r| t.ler(r).unwrap().is_some())
        .collect();
    (inclusoes, vivos)
}

/// **A prova real do 710 (F2).** O processo morre DENTRO da terceira escrita
/// da passada -- a inclusao do segundo item --, com o slot no `.reg` e sem o
/// evento. Vermelho medido antes do conserto: o arranque via o slot consumido,
/// respondia «ja estava» e a linha ficava sem inclusao no diario -- nunca
/// chegaria a replica, e o `verificar` nao acusa.
#[test]
fn a_inclusao_do_commit_com_o_slot_e_sem_o_evento_completa_o_diario() {
    let base = DirTemp::novo("commit-710");
    std::fs::create_dir_all(&base.0).unwrap();
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
    drop(filho);

    let (filho, porta) = subir_com(&base.0, 2, Some((GANCHO_DO_REG, 3)));
    assert!(
        !vender(porta, 1),
        "o COMMIT respondeu: o gancho do 710 nao matou o processo"
    );
    let ate = Instant::now() + Duration::from_secs(20);
    let erro = base.0.join("stderr-2.txt");
    while !std::fs::read_to_string(&erro)
        .unwrap_or_default()
        .contains("teste: COMMIT parado entre o .reg e o diario")
    {
        assert!(
            Instant::now() < ate,
            "o processo nao parou dentro da inclusao"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(filho);
    // Premissa: o slot do item 2 esta no `.reg` e o diario nao o tem.
    let (inclusoes, vivos) = inclusoes_e_slots(&base.0.join("dados"), "itens");
    assert_eq!(
        (inclusoes.clone(), vivos.clone()),
        (vec![1], vec![1, 2]),
        "premissa: a queda nao deixou o slot sem o evento"
    );

    let (filho, _) = subir(&base.0, 3, None);
    drop(filho);
    let dados = base.0.join("dados");
    let (inclusoes, vivos) = inclusoes_e_slots(&dados, "itens");
    assert_eq!(vivos, (1..=ITENS).collect::<Vec<_>>());
    assert_eq!(
        inclusoes, vivos,
        "o diario de itens nao tem a inclusao de cada slot vivo: a linha sem evento \
         nunca chega a replica"
    );
    for tabela in ["vendas", "pagamentos"] {
        let (inclusoes, vivos) = inclusoes_e_slots(&dados, tabela);
        assert_eq!((inclusoes, vivos), (vec![1], vec![1]), "{tabela}");
    }
    // E a venda inteira com UM id de transacao: o evento completado entra no
    // id da metade que entrou.
    let ids = ids_por_venda(&dados);
    assert_eq!(ids[0].len(), 1, "a venda saiu partida: {:?}", ids[0]);
}

/// Os eventos de `tabela` no diario, do disco: `(operacao, rowid, coluna venda
/// da imagem)`, e a versao do slot 1.
/// `(operacao, rowid, coluna venda da imagem)` de cada evento do diario.
type Eventos = Vec<(String, u64, Option<i64>)>;

fn eventos_de(dados: &Path, tabela: &str) -> (Eventos, Option<u64>) {
    let inst = Instancia::nova(dados).unwrap();
    let db = inst.abrir_database("loja").unwrap();
    let mut t = db.abrir_qualificada(tabela).unwrap();
    let total = t.eventos().unwrap();
    let mut saida = Vec::new();
    for (ev, imagem) in t.diario_com_imagem(0, total).unwrap() {
        let venda = if imagem.is_empty() {
            None
        } else {
            match t.valores_da_imagem(&imagem).unwrap().get(1) {
                Some(phxsql_core::value::Value::Int(n)) => Some(*n),
                _ => None,
            }
        };
        saida.push((ev.operacao.nome().to_string(), ev.rowid, venda));
    }
    let versao = if t.slots() >= 1 {
        t.versao(1).unwrap()
    } else {
        None
    };
    (saida, versao)
}

/// O irmao do 710 que a mesma regra cobre: a ALTERACAO e a EXCLUSAO DE VEZ do
/// `COMMIT` mortas entre o `.reg` e o diario. Vermelho medido com o braco da
/// alteracao e o da exclusao tirados do `so_o_diario` (store): o diario fica
/// sem o evento, e a replica com a linha velha.
#[test]
fn a_alteracao_e_a_exclusao_do_commit_sem_o_evento_completam_o_diario() {
    for (rotulo, corpo, esperado) in [
        (
            "alteracao",
            r#""op":"atualizar","database":"loja","tabela":"vendas","rowid":1,
               "valores":{"id":1,"venda":2}"#,
            vec![
                ("inclusao".to_string(), 1, Some(1)),
                ("alteracao".to_string(), 1, Some(2)),
            ],
        ),
        (
            "exclusao",
            r#""op":"excluir","database":"loja","tabela":"vendas","rowid":1,
               "fisico":true,"motivo":"teste""#,
            vec![
                ("inclusao".to_string(), 1, Some(1)),
                ("exclusao".to_string(), 1, Some(1)),
            ],
        ),
    ] {
        let base = DirTemp::novo(&format!("commit-710-{rotulo}"));
        std::fs::create_dir_all(&base.0).unwrap();
        let (filho, porta) = subir(&base.0, 1, None);
        let mut b = Ligacao::nova(porta);
        b.exigir(r#""op":"criar_database","database":"loja""#);
        b.exigir(
            r#""op":"criar_tabela","database":"loja","tabela":"vendas",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"venda","tipo":"Int8"}],
               "indices":[{"nome":"pk_id","colunas":["id"],"unico":true,"primario":true}]"#,
        );
        b.exigir(
            r#""op":"inserir","database":"loja","tabela":"vendas","linha":{"id":1,"venda":1}"#,
        );
        drop(b);
        drop(filho);

        let (filho, porta) = subir_com(&base.0, 2, Some((GANCHO_DO_REG, 1)));
        let mut b = Ligacao::nova(porta);
        b.exigir(r#""op":"begin","database":"loja""#);
        b.exigir(corpo);
        assert!(
            b.pedir(r#""op":"commit""#).is_none(),
            "{rotulo}: o COMMIT respondeu, e o gancho nao matou o processo"
        );
        let ate = Instant::now() + Duration::from_secs(20);
        let erro = base.0.join("stderr-2.txt");
        while !std::fs::read_to_string(&erro)
            .unwrap_or_default()
            .contains("teste: COMMIT parado entre o .reg e o diario")
        {
            assert!(Instant::now() < ate, "{rotulo}: o processo nao parou");
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(b);
        drop(filho);
        let dados = base.0.join("dados");
        let (antes, _) = eventos_de(&dados, "vendas");
        assert_eq!(
            antes.len(),
            1,
            "{rotulo}: premissa -- o evento nao podia ter entrado"
        );

        let (filho, _) = subir(&base.0, 3, None);
        drop(filho);
        let (eventos, versao) = eventos_de(&dados, "vendas");
        assert_eq!(
            eventos, esperado,
            "{rotulo}: o diario nao tem o evento que o .reg ja tem"
        );
        if rotulo == "alteracao" {
            assert_eq!(
                versao,
                Some(2),
                "a alteracao foi reaplicada em vez de completada"
            );
        }
    }
}
