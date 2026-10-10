//! Pedido 495, fatia F3: o gancho do observador de injecao, pelo `despachar`.
//!
//! Cada prova roda como a conexao roda: uma atividade amarrada, o pedido
//! pelo `despachar`, e a ocorrencia lida na fila do carteiro do servidor.

use super::*;
use crate::aquario::Alarme;
use crate::ocorrencias::Carta;
use phxsql_sql::Sinais;

fn servidor(nome: &str, observar: bool) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("observador-{nome}"));
    let s = abrir(&dir, observar);
    popular(&s);
    (s, dir)
}

fn abrir(dir: &DirTemp, observar: bool) -> Arc<Servidor> {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.politica.observar_injecao_sql = observar;
    Servidor::novo(c).unwrap()
}

fn popular(s: &Servidor) {
    let dono = Sessao::default();
    let p = |t: &str| Json::analisar(t).unwrap();
    s.executar("criar_database", &p(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(20)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                           "primario":true}]}"#),
        &dono,
    )
    .unwrap();
    for (id, nome) in [(1, "Adriano"), (2, "Maria")] {
        s.executar(
            "inserir",
            &p(&format!(
                r#"{{"database":"b","tabela":"clientes","linha":{{"id":{id},"nome":"{nome}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
}

/// O `ARSENAL` de `bancada/seguranca/injecao.py`, lido do proprio arquivo,
/// como a F1 o le: o ataque mora la, e nao digitado aqui. Digitado aqui, ele
/// entraria no corpo LEGITIMO que a catraca `catraca_sinais.py` extrai do
/// repositorio, e a catraca subiria por causa da prova.
fn arsenal() -> Vec<(String, String)> {
    let fonte = include_str!("../../../../bancada/seguranca/injecao.py");
    let ini = fonte.find("ARSENAL = [").expect("ARSENAL no injecao.py");
    let fim = ini + fonte[ini..].find("\n]").expect("fim do ARSENAL");
    fonte[ini..fim]
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_prefix("(\"")?.strip_suffix("\"),")?;
            let (nome, texto) = l.split_once("\", \"")?;
            Some((nome.to_string(), desescapar(texto)))
        })
        .collect()
}

/// Os escapes que o `ARSENAL` usa: `\xNN`, `\\` e `\"`.
fn desescapar(t: &str) -> String {
    let mut out = String::new();
    let mut c = t.chars();
    while let Some(x) = c.next() {
        if x != '\\' {
            out.push(x);
            continue;
        }
        match c.next() {
            Some('x') => {
                let h: String = c.by_ref().take(2).collect();
                out.push(char::from(u8::from_str_radix(&h, 16).expect("\\x")));
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

pub(super) fn do_arsenal(rotulo: &str) -> String {
    arsenal()
        .into_iter()
        .find(|(n, _)| n == rotulo)
        .unwrap_or_else(|| panic!("{rotulo} saiu do ARSENAL"))
        .1
}

/// A tautologia do arsenal -- a mesma que a F1 acusa e que a premissa do
/// SEC mediu executando (2 de 2 linhas).
pub(super) fn tautologia_do_arsenal() -> String {
    do_arsenal("aspa solta / tautologia")
}

/// Um pedido como a conexao o faz: a atividade amarrada (e por ela que a
/// ocorrencia chega a camada DESTE servidor, com quem e de onde) e o
/// `despachar`. O IP e por pedido porque o silencio da camada e por
/// (alarme, usuario, IP).
fn despachar(s: &Arc<Servidor>, ip: &str, corpo: &Json) -> Result<Json> {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar(&format!("dados:{ip}"), "dados", ip, 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido(corpo.texto_ou("op", ""), "root", "b", "", agora);
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(&corpo.escrever(), &mut sessao, ip);
    a.terminou_pedido("root");
    r
}

fn sql(texto: &str) -> Json {
    Json::objeto(vec![
        ("token", Json::texto_de("t")),
        ("op", Json::texto_de("sql")),
        ("database", Json::texto_de("b")),
        ("texto", Json::texto_de(texto)),
    ])
}

/// As ocorrencias que esperam o carteiro (nos testes nao ha thread).
fn ocorrencias(s: &Servidor) -> Vec<crate::ocorrencias::Ocorrencia> {
    s.ocorrencias
        .correio()
        .retirar(std::time::Duration::ZERO, |c| {
            matches!(c, Carta::Ocorrencia(_))
        })
        .into_iter()
        .filter_map(|c| match c {
            Carta::Ocorrencia(o) => Some(o),
            Carta::Saude(_) => None,
        })
        .collect()
}

/// **A prova da F3.** A tautologia do arsenal DA CERTO -- devolve as 2
/// linhas da tabela -- e gera UMA ocorrencia `injecao_suspeita`, com a
/// classe que acusou, e sem o literal.
///
/// # Prova real
///
/// Com o gancho so no `is_err()` (o erro do 215: `if r.is_err() { ... }` em
/// volta do `acusar_injecao`), a tautologia, que da certo, nao vira
/// ocorrencia e o teste cai em `0 != 1` -- o vermelho medido.
#[test]
fn a_tautologia_do_arsenal_gera_uma_ocorrencia_com_duas_linhas_devolvidas() {
    let (s, _dir) = servidor("tautologia", true);
    let texto = tautologia_do_arsenal();
    assert_eq!(texto, "SELECT * FROM clientes WHERE nome = '' OR '1'='1'");
    let r = despachar(&s, "10.9.0.1", &sql(&texto)).expect("a tautologia executa");
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas.len(), 2, "{}", r.escrever());

    let o = ocorrencias(&s);
    assert_eq!(o.len(), 1, "{o:#?}");
    assert_eq!(o[0].alarme, Alarme::InjecaoSuspeita);
    assert_eq!(o[0].sinais, Sinais::CONSTANTE_SOB_OR);
    assert_eq!(o[0].ip, "10.9.0.1");
    let linha = o[0].para_json().escrever();
    assert!(
        linha.contains(r#""sinais":["constante_sob_or"]"#),
        "{linha}"
    );
    assert!(linha.contains(r#""grupo":"ataque""#), "{linha}");
    assert!(!linha.contains("'1'"), "o literal vazou: {linha}");
}

/// No ERRO tambem: o comando empilhado e recusado pela sintaxe, e e
/// justamente o que o observador quer ver.
#[test]
fn o_comando_empilhado_recusado_tambem_vira_ocorrencia() {
    let (s, _dir) = servidor("empilhado", true);
    let r = despachar(&s, "10.9.0.2", &sql(&do_arsenal("segundo comando: DROP")));
    assert!(r.is_err());
    let o = ocorrencias(&s);
    assert_eq!(o.len(), 1, "{o:#?}");
    assert!(o[0].sinais.contem(Sinais::EMPILHADO), "{o:#?}");
}

/// O campo `expressao` de uma op nativa: os simbolos da analise do core,
/// lidos pelas mesmas quatro classes.
///
/// Vermelho: com o `expressao_do_pedido` de volta no
/// `Expressao::analisar` direto, nada entrega e a contagem cai a zero.
#[test]
fn a_tautologia_no_campo_expressao_vira_ocorrencia() {
    let (s, _dir) = servidor("expressao", true);
    let pedido = Json::analisar(
        r#"{"token":"t","op":"varrer","database":"b","tabela":"clientes",
            "expressao":"nome = '' OR '1' = '1'"}"#,
    )
    .unwrap();
    let r = despachar(&s, "10.9.0.3", &pedido).unwrap();
    assert_eq!(r.campo("linhas").and_then(Json::lista).unwrap().len(), 2);
    let o = ocorrencias(&s);
    assert_eq!(o.len(), 1, "{o:#?}");
    assert_eq!(o[0].sinais, Sinais::CONSTANTE_SOB_OR);
}

/// Os outros dois campos: o `tendo` do `agrupar` e o `expressao` do
/// `consultar` -- tambem dentro do `de`, um nivel abaixo, que e texto do
/// cliente do mesmo jeito.
#[test]
fn o_tendo_e_o_consultar_tambem_entregam() {
    let (s, _dir) = servidor("tendo-consultar", true);
    let tendo = Json::analisar(
        r#"{"token":"t","op":"agrupar","database":"b","tabela":"clientes",
            "por":["nome"],"agregados":[{"funcao":"contagem","apelido":"n"}],
            "tendo":"n > 5 OR 1 = 1"}"#,
    )
    .unwrap();
    despachar(&s, "10.9.4.1", &tendo).unwrap();
    let consultar = Json::analisar(
        r#"{"token":"t","op":"consultar","database":"b",
            "de":{"op":"varrer","tabela":"clientes"},
            "expressao":"nome = '' OR '1' = '1'"}"#,
    )
    .unwrap();
    despachar(&s, "10.9.4.2", &consultar).unwrap();
    let aninhado = Json::analisar(
        r#"{"token":"t","op":"consultar","database":"b",
            "de":{"op":"varrer","tabela":"clientes","expressao":"nome = '' OR 1 = 1"}}"#,
    )
    .unwrap();
    despachar(&s, "10.9.4.3", &aninhado).unwrap();
    let o = ocorrencias(&s);
    let ips: Vec<&str> = o.iter().map(|o| o.ip.as_str()).collect();
    assert_eq!(ips, ["10.9.4.1", "10.9.4.2", "10.9.4.3"], "{o:#?}");
    assert!(o.iter().all(|o| o.sinais == Sinais::CONSTANTE_SOB_OR));
}

/// O legitimo nao acusa -- nem o `OR` entre colunas, nem o literal com aspa
/// dobrada. Com `Ok` ou com `Err` (o `WHERE nome = ...` sem indice recusa):
/// o desfecho nao importa, a forma sim.
#[test]
fn o_legitimo_nao_vira_ocorrencia() {
    let (s, _dir) = servidor("legitimo", true);
    for (i, texto) in [
        "SELECT * FROM clientes WHERE nome = 'Maria' OR nome = 'Adriano'",
        "SELECT * FROM clientes WHERE nome = 'x'' OR ''1''=''1'",
        "SELECT * FROM clientes WHERE id = 1",
    ]
    .iter()
    .enumerate()
    {
        let _ = despachar(&s, &format!("10.9.1.{i}"), &sql(texto));
    }
    assert!(ocorrencias(&s).is_empty());
}

/// Desligado (`seguranca.observar_injecao_sql: false`), nada.
#[test]
fn desligado_nao_observa() {
    let (s, _dir) = servidor("desligado", false);
    despachar(&s, "10.9.0.4", &sql(&tautologia_do_arsenal())).unwrap();
    assert!(ocorrencias(&s).is_empty());
}

/// **A resposta ao cliente sai byte a byte igual com o observador e sem
/// ele** -- o ARSENAL inteiro (sucesso, erro de sintaxe, empilhado, o dado
/// escapado), um legitimo e o campo de expressao. O observador so
/// registra.
///
/// Os dois servidores abrem o MESMO diretorio, um depois do outro: o
/// `rowstamp` e o `rowtime` das linhas sao do dado, e dois diretorios
/// populados em instantes diferentes dariam respostas diferentes por um
/// motivo que nao e o observador.
#[test]
fn a_resposta_sai_byte_a_byte_igual_com_e_sem_o_observador() {
    let (com, dir) = servidor("igual", true);
    let mut pedidos: Vec<Json> = arsenal().iter().map(|(_, t)| sql(t)).collect();
    pedidos.push(sql("SELECT * FROM clientes WHERE id = 1"));
    pedidos.push(
        Json::analisar(
            r#"{"token":"t","op":"varrer","database":"b","tabela":"clientes",
                "expressao":"nome = '' OR '1' = '1'"}"#,
        )
        .unwrap(),
    );
    // O `ms` e o relogio da maquina, nao a resposta: dois despachos iguais
    // saem com 1 e 2 conforme a carga (pedido 768). Ele vira `?`, e todo o
    // resto continua comparado byte a byte -- campo a mais, a menos ou
    // diferente continua derrubando a prova.
    let sem_relogio = |t: String| {
        let mut fora = String::with_capacity(t.len());
        let mut resto = t.as_str();
        while let Some(i) = resto.find("\"ms\":") {
            let (antes, depois) = resto.split_at(i + 5);
            fora.push_str(antes);
            fora.push('?');
            resto = depois.trim_start_matches(|c: char| c.is_ascii_digit());
        }
        fora.push_str(resto);
        fora
    };
    let escrito = |r: Result<Json>| {
        sem_relogio(match r {
            Ok(j) => format!("ok {}", j.escrever()),
            Err(e) => format!("erro {} {}", e.codigo(), e),
        })
    };
    let com_observador: Vec<String> = pedidos
        .iter()
        .enumerate()
        .map(|(i, p)| escrito(despachar(&com, &format!("10.9.2.{i}"), p)))
        .collect();
    // E o lado «com» de fato observou: a prova nao passa por o observador
    // estar mudo.
    // Os 5 ataques do arsenal e a expressao.
    assert_eq!(ocorrencias(&com).len(), 6, "o observador ficou mudo");
    drop(com);
    let sem = abrir(&dir, false);
    for (i, p) in pedidos.iter().enumerate() {
        let b = escrito(despachar(&sem, &format!("10.9.2.{i}"), p));
        assert_eq!(com_observador[i], b, "{}", p.escrever());
    }
    assert!(ocorrencias(&sem).is_empty());
}

/// **O medidor de custo.** O observador ligado nao acrescenta NENHUMA
/// passada do lexico ao pedido `sql` comum -- ele classifica os simbolos que
/// a leitura ja fez, e a sintaxe reaproveita a mesma lista.
///
/// So o pedido ACUSADO paga uma passada, e fora do laco comum: a camada da
/// F2 redige o `dados` analisando (o SQL normalizado), e analisar e ler.
/// Por isso a regua roda sobre o que nao acusa -- o caminho de todo pedido.
///
/// # Por que passadas, e nao «ate 2x o parse»
///
/// O desenho escreveu o RED como «relexar no gancho passa de 2x o parse».
/// Medido no exemplo `custo-do-observador` (release, carga 9 em 4 nucleos):
/// reler custa 1,1x a 1,6x o parse, entao a regua de 2x nunca reprovaria quem
/// relexa. Contar e exato e nao floca.
///
/// # Prova real
///
/// Com o gancho relendo o texto (`phxsql_sql::sinais(&analisar_com_comentarios(
/// pedido.texto_ou("texto", "")))` no `acusar_injecao`), o lado ligado faz
/// uma passada a mais e o teste cai -- o vermelho medido.
#[test]
fn o_observador_nao_acrescenta_passada_do_lexico() {
    let (com, _d1) = servidor("custo-com", true);
    let (sem, _d2) = servidor("custo-sem", false);
    for (i, texto) in [
        "SELECT * FROM clientes WHERE id = 1",
        "SELECT * FROM clientes WHERE id = 1 OR id = 2",
        "SELECT nome FROM clientes WHERE id = 2",
        "SELEC * FROM clientes",
    ]
    .into_iter()
    .enumerate()
    {
        let passadas = |s: &Arc<Servidor>| {
            let antes = phxsql_sql::lexico::passadas_nesta_thread();
            let _ = despachar(s, &format!("10.9.3.{i}"), &sql(texto));
            phxsql_sql::lexico::passadas_nesta_thread() - antes
        };
        let (n_com, n_sem) = (passadas(&com), passadas(&sem));
        assert!(
            n_com <= n_sem,
            "o observador acrescentou {} passada(s) do lexico em {texto:?} \
             ({n_com} ligado, {n_sem} desligado)",
            n_com - n_sem
        );
    }
}
