//! A **porta** do `PSCH` v10, pelo protocolo -- pedido 407.
//!
//! O defeito que esta bateria repoe e' a ausencia: `Table::migrar_para_psch_v10`
//! existia, retomavel e idempotente, e **nenhuma operacao, comando ou passo de
//! arranque a alcancava**. Medido em 23/09/2026: os unicos chamadores do
//! repositorio eram quatro linhas de teste. Uma tabela v9 de producao nao tinha
//! como chegar ao v10 sem alguem escrever Rust contra a biblioteca.
//!
//! Por isso a prova e' pelo SOQUETE e nao por chamada de funcao: o que faltava
//! era exatamente a porta, e porta so se prova batendo nela.
//!
//! O que cada teste trava:
//!
//! 1. a v9 alcanca o v10 pelo protocolo (tire o braco `"migrar_esquema"` do
//!    `executar` e esta prova reprova com «operacao desconhecida»);
//! 2. **nunca automatica e silenciosa**: sem `confirmar` a operacao responde o
//!    custo e nao escreve byte;
//! 3. **retomavel de verdade**: com a primeira passada feita e a segunda nao, a
//!    chamada seguinte acrescenta so a que falta;
//! 4. o **portao** e' `administrar`, e a varredura -- que nao tem campo
//!    `tabela` -- paga conferencia propria;
//! 5. **guarda pedida**: a v9 que ninguem migrou continua v9 e continua
//!    gravando.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema, COLUNA_ROWSTAMP, COLUNA_ROWTIME};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::{Config, Servidor};
use phxsql_store::{RegFile, Table};

const TOKEN: &str = "porta-do-v10";
const SENHA: &str = "segredo-de-teste";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Sobe o servidor. `com_cadastro` falso e' o servidor sem usuario nenhum --
/// o caso VELHO, que uma regra nova nao pode mudar.
fn subir(base: &Path, com_cadastro: bool) -> (Arc<Servidor>, u16) {
    let porta = porta_livre();
    // Uma iteracao so: a senha real nao interessa aqui, e 210.000 por login
    // fariam a bateria levar segundos por nada.
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let usuarios = if com_cadastro {
        format!(
            r#""root": {{ "login": "root", "senha_hash": "{h}" }},
               "usuarios": [
                 {{ "id": 2, "login": "so_le", "nome": "So Le", "senha_hash": "{h}",
                    "ativo": true, "bases": {{ "*": {{ "ler": true }} }} }},
                 {{ "id": 3, "login": "dono_de_uma", "nome": "Dono de Uma",
                    "senha_hash": "{h}", "ativo": true,
                    "bases": {{ "loja": {{ "ler": true,
                       "tabelas": {{ "velha": {{ "ler": true, "administrar": true }} }} }} }} }},
                 {{ "id": 4, "login": "quase_tudo", "nome": "Quase Tudo",
                    "senha_hash": "{h}", "ativo": true,
                    "bases": {{ "loja": {{ "ler": true, "administrar": true,
                       "tabelas": {{ "outra": {{ "ler": true }} }} }} }} }} ],"#
        )
    } else {
        String::new()
    };
    // A cifra do fio nasce exigida desde o pedido 370; esta bateria conecta em
    // claro porque o que ela mede e' a PORTA da migracao. Sem esta linha a
    // recusa lida seria a da cifra, e a prova mediria outra coisa.
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:{porta}", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              {usuarios}
              "cifra_fio": {{ "exigir": false }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        }
    }

    fn entrar(porta: u16, login: &str) -> Ligacao {
        let mut c = Ligacao::nova(porta);
        let r = c.pedir(&format!(
            r#""op":"login","usuario":"{login}","senha":"{SENHA}""#
        ));
        assert!(
            r.booleano_ou("ok", false),
            "login de {login}: {}",
            r.escrever()
        );
        c
    }

    /// O protocolo e' JSON por LINHA: o corpo escrito em varias linhas aqui
    /// vira uma so antes de sair.
    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

fn erro(j: &Json) -> String {
    j.texto_ou("erro", "").to_string()
}

/// O corpo da resposta, exigindo que o pedido tenha dado certo -- o protocolo
/// embrulha tudo em `resultado`.
///
/// Por VALOR e nao por referencia: `ok(c.pedir(...))` e' a forma que todo teste
/// aqui usa, e devolver `&Json` de um temporario nao compila.
fn ok(j: Json) -> Json {
    assert!(
        j.booleano_ou("ok", false),
        "o pedido falhou: {}",
        j.escrever()
    );
    j.campo("resultado").cloned().unwrap_or(j)
}

/// O esquema como ele sai do disco de uma tabela ANTERIOR ao v10: `do_disco`
/// monta exatamente as colunas dadas e nao acrescenta nenhuma, que e' o que um
/// motor v10 obtem ao abrir um bloco v9.
fn esquema_v9(nome: &str) -> Schema {
    Schema::do_disco(
        nome.to_string(),
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)),
            Column::new(phxsql_core::schema::COLUNA_SOFTDELETED, ColumnType::Bool).obrigatoria(),
            Column::new(phxsql_core::schema::COLUNA_ROWNUM, ColumnType::UInt8).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// Cria, no diretorio do database, uma tabela v9 com `linhas` linhas -- e
/// FECHA, porque quem opera nela daqui em diante e' o servidor.
fn criar_v9(base: &Path, database: &str, tabela: &str, linhas: i64) {
    let mut t = Table::criar(base.join(database), esquema_v9(tabela)).unwrap();
    assert!(
        t.esquema().coluna_rowstamp().is_none(),
        "a tabela de apoio nasceu no v10: a prova mediria outra coisa"
    );
    for i in 1..=linhas {
        t.inserir(&[Value::Int(i), Value::Str(format!("linha {i}"))])
            .unwrap();
    }
    t.sincronizar().unwrap();
}

/// A PRIMEIRA PASSADA da migracao, e nada mais -- o estado que uma queda entre
/// as duas passadas deixa em disco.
///
/// Nao e' uma imitacao: e' a mesma chamada que `migrar_para_psch_v10` faz na
/// passada 1, com os mesmos argumentos (a coluna tirada do esquema MODELO, no
/// fim da lista, com zeros). O que falta depois dela e' exatamente o que
/// faltaria depois de um `kill -9` no meio.
fn so_a_primeira_passada(base: &Path, database: &str, tabela: &str) {
    let mut reg = RegFile::abrir(base.join(database), tabela).unwrap();
    let modelo = Schema::new("modelo", vec![Column::new("x", ColumnType::Int4)], vec![]).unwrap();
    let i = modelo.coluna_por_nome(COLUNA_ROWSTAMP).unwrap();
    let coluna = modelo.colunas()[i].clone();
    let largura = coluna.ty.largura();
    let posicao = reg.esquema().colunas().len();
    let novo = reg.esquema().com_coluna(coluna, posicao).unwrap();
    reg.acrescentar_coluna(novo, posicao, &vec![0u8; largura], false)
        .unwrap();
}

fn colunas_do_esquema(c: &mut Ligacao, database: &str, tabela: &str) -> Vec<String> {
    let r = c.pedir(&format!(
        r#""op":"esquema","database":"{database}","tabela":"{tabela}""#
    ));
    ok(r)
        .campo("colunas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|x| x.texto_ou("nome", "").to_string())
        .collect()
}

/// **A prova do pedido 407**: uma tabela v9 alcanca o v10 pelo protocolo.
///
/// # O defeito que ela repoe
///
/// Tire o braco `"migrar_esquema" => self.op_migrar_esquema(p, sessao)` do
/// `executar`: o servidor responde «operacao desconhecida» e a tabela fica na
/// v9 para sempre -- que e' o estado em que o motor estava antes deste pedido.
#[test]
fn a_v9_alcanca_o_v10_pelo_protocolo() {
    let base = DirTemp::novo("v10-porta");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    criar_v9(&base, "loja", "velha", 7);

    // A tabela esta mesmo na v9 aos olhos do servidor.
    let colunas = colunas_do_esquema(&mut c, "loja", "velha");
    assert!(
        !colunas.iter().any(|n| n == COLUNA_ROWSTAMP),
        "a tabela ja nasceu carimbada: {colunas:?}"
    );

    // Passo 1 -- a VISTA PREVIA. Ela diz o custo e NAO migra.
    let r = c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"velha""#);
    let v = ok(r);
    assert!(v.booleano_ou("precisa", false), "{}", v.escrever());
    assert!(!v.booleano_ou("migrado", true), "{}", v.escrever());
    assert_eq!(v.inteiro_ou("passadas", -1), 2, "{}", v.escrever());
    assert_eq!(v.inteiro_ou("registros", -1), 7, "{}", v.escrever());
    // 7 slots reescritos uma vez por coluna que falta.
    assert_eq!(
        v.inteiro_ou("slots_a_reescrever", -1),
        14,
        "{}",
        v.escrever()
    );
    assert!(
        v.texto_ou("aviso", "").contains("INTEIRO"),
        "a vista previa nao diz o que vai acontecer: {}",
        v.escrever()
    );

    // **E ela nao escreveu byte**: a segunda pergunta responde a mesma coisa.
    let v = ok(c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"velha""#));
    assert!(v.booleano_ou("precisa", false), "a vista previa migrou");
    let colunas = colunas_do_esquema(&mut c, "loja", "velha");
    assert!(!colunas.iter().any(|n| n == COLUNA_ROWSTAMP));

    // Passo 2 -- confirmado pelo nome repetido, como o `excluir_tabela`.
    let e = erro(
        &c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"velha","confirmar":"outra""#),
    );
    assert!(e.contains("confirmar"), "veio {e}");

    let v = ok(
        c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"velha","confirmar":"velha""#)
    );
    assert!(v.booleano_ou("migrado", false), "{}", v.escrever());
    assert_eq!(v.inteiro_ou("slots_reescritos", -1), 7, "{}", v.escrever());

    // O v10 chegou, e chegou pela rede.
    let colunas = colunas_do_esquema(&mut c, "loja", "velha");
    assert!(
        colunas.iter().any(|n| n == COLUNA_ROWSTAMP) && colunas.iter().any(|n| n == COLUNA_ROWTIME),
        "a tabela nao chegou ao v10: {colunas:?}"
    );

    // Idempotente PELA PORTA: chamar de novo nao e' erro e nao toca em disco.
    let v = ok(
        c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"velha","confirmar":"velha""#)
    );
    assert!(!v.booleano_ou("precisa", true), "{}", v.escrever());
    assert!(!v.booleano_ou("migrado", true), "{}", v.escrever());

    // E a tabela continua gravando, agora carimbando.
    let r = c.pedir(
        r#""op":"inserir","database":"loja","tabela":"velha","linha":{"id":8,"nome":"depois"}"#,
    );
    ok(r);
}

/// **Retomavel de verdade**: com a primeira passada feita e a segunda nao, a
/// chamada seguinte acrescenta SO a que falta -- e o custo anunciado cai para
/// uma passada, porque o custo sai da mesma lista que a migracao executa.
///
/// # O defeito que ela repoe
///
/// Faca o plano ignorar o que ja existe (tirar o `filter` do
/// `colunas_do_v10_que_faltam`): a operacao anunciaria duas passadas onde ha
/// uma, e a migracao tentaria acrescentar uma coluna que ja esta la.
#[test]
fn a_queda_no_meio_retoma_na_proxima_chamada() {
    let base = DirTemp::novo("v10-retoma");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    criar_v9(&base, "loja", "meio", 5);
    so_a_primeira_passada(&base, "loja", "meio");

    // O estado da queda: a primeira coluna entrou, a segunda nao.
    let colunas = colunas_do_esquema(&mut c, "loja", "meio");
    assert!(colunas.iter().any(|n| n == COLUNA_ROWSTAMP), "{colunas:?}");
    assert!(!colunas.iter().any(|n| n == COLUNA_ROWTIME), "{colunas:?}");

    // A porta ve o que falta, e so o que falta.
    let v = ok(c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"meio""#));
    assert!(v.booleano_ou("precisa", false), "{}", v.escrever());
    assert_eq!(v.inteiro_ou("passadas", -1), 1, "{}", v.escrever());
    let faltando: Vec<String> = v
        .campo("colunas_faltando")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|x| x.texto().unwrap_or_default().to_string())
        .collect();
    assert_eq!(faltando, vec![COLUNA_ROWTIME.to_string()], "{faltando:?}");
    assert_eq!(
        v.inteiro_ou("slots_a_reescrever", -1),
        5,
        "{}",
        v.escrever()
    );

    // E a retomada termina o servico.
    let v = ok(
        c.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"meio","confirmar":"meio""#)
    );
    assert!(v.booleano_ou("migrado", false), "{}", v.escrever());
    let colunas = colunas_do_esquema(&mut c, "loja", "meio");
    assert!(colunas.iter().any(|n| n == COLUNA_ROWTIME), "{colunas:?}");
}

/// **O portao**: migrar e' administrar, e a VARREDURA -- que nao tem campo
/// `tabela` -- paga conferencia propria.
///
/// # O defeito que ela repoe
///
/// Tire o `pode_administrar_tabela` da varredura: ela passa a entregar o nome,
/// a contagem e o tamanho de TODA tabela da base a quem administra a base --
/// inclusive a que uma regra de tabela nega. Medido com o defeito reposto:
/// `outra` aparece na resposta de `quase_tudo`.
///
/// **O que este teste NAO prova, e o motivo esta escrito para ninguem se
/// enganar:** trocar a linha `"migrar_esquema" => Atividade::Administrar` por
/// nada nao afrouxa nada -- o braco padrao do `Atividade::da_operacao` ja e'
/// `Administrar`. Ela existe para a operacao nao cair no padrao em SILENCIO, e
/// quem cobra isso e' o conferidor do catalogo em `usuarios.rs`, nao esta
/// prova.
#[test]
fn migrar_e_administrar_e_a_varredura_confere_por_dentro() {
    let base = DirTemp::novo("v10-portao");
    let (_s, porta) = subir(&base, true);
    let mut dono = Ligacao::entrar(porta, "root");
    ok(dono.pedir(r#""op":"criar_database","database":"loja""#));
    criar_v9(&base, "loja", "velha", 3);
    criar_v9(&base, "loja", "outra", 2);

    // Quem so le nao migra -- nem confirma, nem espia o custo.
    let mut leitor = Ligacao::entrar(porta, "so_le");
    for corpo in [
        r#""op":"migrar_esquema","database":"loja","tabela":"velha""#,
        r#""op":"migrar_esquema","database":"loja","tabela":"velha","confirmar":"velha""#,
    ] {
        let r = leitor.pedir(corpo);
        assert!(!r.booleano_ou("ok", false), "passou: {}", r.escrever());
        assert!(erro(&r).contains("administrar"), "veio {}", erro(&r));
    }

    // Quem administra UMA tabela migra aquela...
    let mut um = Ligacao::entrar(porta, "dono_de_uma");
    let v = ok(
        um.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"velha","confirmar":"velha""#)
    );
    assert!(v.booleano_ou("migrado", false), "{}", v.escrever());

    // ...e NAO a outra.
    let r =
        um.pedir(r#""op":"migrar_esquema","database":"loja","tabela":"outra","confirmar":"outra""#);
    assert!(!r.booleano_ou("ok", false), "passou: {}", r.escrever());

    // E a VARREDURA -- o pedido sem campo `tabela` -- cai na regra da BASE no
    // portao geral, como todo pedido que nao nomeia tabela. Quem administra so
    // uma tabela nao levanta a base inteira, e isto e' a regra de sempre e nao
    // uma excecao desta operacao.
    let r = um.pedir(r#""op":"migrar_esquema","database":"loja""#);
    assert!(!r.booleano_ou("ok", false), "passou: {}", r.escrever());

    // A conferencia PROPRIA da varredura aparece em quem PASSA pelo portao
    // geral: `quase_tudo` administra a base, menos a tabela `outra`.
    let mut quase = Ligacao::entrar(porta, "quase_tudo");
    let v = ok(quase.pedir(r#""op":"migrar_esquema","database":"loja""#));
    let cru = v.escrever();
    assert!(
        !cru.contains("outra"),
        "a varredura vazou a tabela que ele nao administra: {cru}"
    );
    // `velha` ja foi migrada acima, entao nao sobra pendente visivel para ele
    // -- e a `outra`, que esta pendente, nao e' dele para ver.
    assert_eq!(v.inteiro_ou("pendentes", -1), 0, "{cru}");

    // O dono ve as duas, e a que falta aparece com o custo.
    let v = ok(dono.pedir(r#""op":"migrar_esquema","database":"loja""#));
    assert_eq!(v.inteiro_ou("pendentes", -1), 1, "{}", v.escrever());
    assert_eq!(v.inteiro_ou("no_formato_atual", -1), 1, "{}", v.escrever());
    assert_eq!(
        v.inteiro_ou("slots_a_reescrever", -1),
        4,
        "{}",
        v.escrever()
    );
}

/// **Guarda pedida, nao imposta**: a tabela v9 que ninguem migrou continua v9
/// e continua trabalhando. A porta nova nao muda nada para quem nao bater
/// nela.
///
/// # O defeito que ela repoe
///
/// Chame `migrar_para_psch_v10` na abertura da tabela (o passo de arranque que
/// esta frente RECUSOU): a primeira leitura de um banco grande viraria uma
/// reescrita de todos os `.reg` dele, sem ninguem ter pedido -- e esta prova
/// reprovaria na primeira linha, porque a tabela chegaria carimbada.
#[test]
fn a_v9_que_ninguem_migrou_continua_v9_e_continua_gravando() {
    let base = DirTemp::novo("v10-pedida");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    criar_v9(&base, "loja", "velha", 2);

    // Grava, le e altera -- tudo pelo protocolo, sem nunca migrar.
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"velha","linha":{"id":3,"nome":"nova"}"#,
    ));
    // Sem `com_versao`, o `ler` devolve a linha crua.
    let v = ok(c.pedir(r#""op":"ler","database":"loja","tabela":"velha","rowid":3"#));
    assert_eq!(v.texto_ou("nome", ""), "nova", "{}", v.escrever());
    ok(c.pedir(
        r#""op":"atualizar","database":"loja","tabela":"velha","rowid":3,
           "linha":{"id":3,"nome":"mexida"}"#,
    ));

    // E continua na v9: nenhuma abertura a migrou por conta propria.
    let colunas = colunas_do_esquema(&mut c, "loja", "velha");
    assert!(
        !colunas.iter().any(|n| n == COLUNA_ROWSTAMP),
        "alguem migrou a tabela sem pedido: {colunas:?}"
    );
}
