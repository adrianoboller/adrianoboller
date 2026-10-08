//! # Direito por COLUNA -- o nivel abaixo do direito por tabela
//!
//! A folha de pagamento e a tabela de clientes moram no mesmo banco, e o
//! direito por tabela ja resolveu isso. Falta o caso de dentro: o RH le a
//! folha inteira, o gestor le a folha SEM o salario -- e nao ha como dar
//! «tudo menos uma coluna» com uma regra que para na tabela.
//!
//! O que estes testes travam, em ordem de importancia:
//!
//! 1. **um `config.json` sem `"colunas"` continua se comportando igual** --
//!    e o teste que mais importa, e e o do comportamento VELHO;
//! 2. a coluna negada sai da resposta de `ler`, `varrer` e `buscar`;
//! 3. escrever nela recusa nomeando-a;
//! 4. **nao escrever nela PRESERVA o valor gravado** -- porque quem nao le a
//!    coluna manda a linha sem ela, e o motor trataria ausencia como NULL;
//! 5. o SQL herda os dois lados, e herda pelo `executar_derivado`;
//! 6. as operacoes que devolvem linha por outro caminho RECUSAM a tabela;
//! 7. a pergunta tambem responde: filtro e indice sobre a coluna negada param
//!    antes de a peneira ter chance de agir.
use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("dc-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// O cadastro sai do JSON, e nao de uma struct montada a mao: e a LEITURA
/// do `config.json` que precisa ser exercitada, porque e la que o direito
/// por coluna e escrito de verdade.
fn cadastro(bases: &str) -> Cadastro {
    Cadastro::de_json(&pedido(&format!(
        r#"{{"usuarios":[{{"login":"ana","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00","bases":{bases}}}]}}"#
    )))
    .unwrap()
}

/// Ana pode tudo na base, e a folha tem uma regra de coluna: `salario`
/// nao se le nem se altera.
fn so_a_folha_tem_regra() -> Cadastro {
    cadastro(
        r#"{"*":{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                 "criar":true,"reindexar":true,"diario":true,"verificar":true,
                 "replicar":true,"administrar":true,
                 "tabelas":{"folha":{"ler":true,"inserir":true,"alterar":true,
                   "excluir":true,"criar":true,"diario":true,"administrar":true,
                   "replicar":true,"verificar":true,"reindexar":true,
                   "colunas":{"salario":{"ler":false,"alterar":false}}}}}}"#,
    )
}

/// O MESMO cadastro sem o campo `"colunas"`. E o controle de toda esta
/// bateria: o que muda entre os dois e uma linha de JSON.
fn sem_regra_de_coluna() -> Cadastro {
    cadastro(
        r#"{"*":{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                 "criar":true,"reindexar":true,"diario":true,"verificar":true,
                 "replicar":true,"administrar":true,
                 "tabelas":{"folha":{"ler":true,"inserir":true,"alterar":true,
                   "excluir":true,"criar":true,"diario":true,"administrar":true,
                   "replicar":true,"verificar":true,"reindexar":true}}}}"#,
    )
}

/// O cenario do pedido 343: a coluna marcada e a CHAVE PRIMARIA da tabela,
/// que e o caso tipico (`cpf`, `cnpj`) e nao o exotico.
fn a_chave_primaria_marcada() -> Cadastro {
    cadastro(
        r#"{"*":{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                 "criar":true,"reindexar":true,"diario":true,"verificar":true,
                 "replicar":true,"administrar":true,
                 "tabelas":{"pessoas":{"ler":true,"inserir":true,"alterar":true,
                   "excluir":true,"criar":true,"diario":true,"administrar":true,
                   "replicar":true,"verificar":true,"reindexar":true,
                   "colunas":{"cpf":{"ler":false,"alterar":false}}}}}}"#,
    )
}

/// O `Config` de um servidor que mora em `dir`, com este cadastro.
fn config_em(dir: &std::path::Path, cadastro: Cadastro) -> Config {
    Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro,
        ..Config::default()
    }
}

/// Uma base `b` com `clientes` (id, nome) e `folha` (id, nome, salario).
fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> (Arc<Servidor>, Sessao) {
    let s = Servidor::novo(config_em(dir, cadastro.clone())).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"folha",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"salario","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true},
                               {"nome":"porSalario","colunas":["salario"]}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"clientes","linha":{"id":1,"nome":"x"}}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"b","tabela":"folha",
                    "linha":{"id":1,"nome":"ana","salario":5000}}"#,
        ),
        &dono,
    )
    .unwrap();
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

/// Pelo `despachar`, que e por onde o pedido entra de verdade.
fn pede(s: &Arc<Servidor>, sessao: &Sessao, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        usuario: sessao.usuario.clone(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// O que esta GRAVADO, visto por quem nao tem restricao nenhuma. E o
/// arbitro de toda prova de preservacao: perguntar pela mesma sessao
/// restrita nunca mostraria a coluna, e o teste passaria por engano.
fn salario_gravado(s: &Arc<Servidor>) -> i64 {
    s.executar(
        "ler",
        &pedido(r#"{"database":"b","tabela":"folha","rowid":1}"#),
        &Sessao::default(),
    )
    .unwrap()
    .inteiro_ou("salario", -1)
}

// ------------------------------------------------------- comportamento velho

/// **O teste que mais importa.** Regra nova que muda o significado da
/// configuracao que ja existe tira o direito de alguem sem ninguem ter
/// pedido -- e aqui tiraria dado, que e pior: o `atualizar` de sempre
/// passaria a repor colunas por conta.
#[test]
fn sem_colunas_no_cadastro_nada_muda() {
    let dir = dir_temp("igual");
    let (s, ses) = servidor(&dir, sem_regra_de_coluna());

    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.inteiro_ou("salario", -1), 5000, "a coluna sumiu: {l:?}");

    let v = pede(&s, &ses, r#""op":"varrer","database":"b","tabela":"folha""#).unwrap();
    let linha = &v.campo("linhas").and_then(Json::lista).unwrap()[0];
    assert_eq!(linha.inteiro_ou("salario", -1), 5000);

    // As que passam a RECUSAR com regra de coluna continuam passando.
    assert!(pede(
        &s,
        &ses,
        r#""op":"exportar","database":"b","tabela":"folha","formato":"json""#
    )
    .is_ok());
    assert!(pede(
        &s,
        &ses,
        r#""op":"juntar","database":"b","a":{"tabela":"folha","chave":"id"},
               "b":{"tabela":"clientes","chave":"id"}"#
    )
    .is_ok());

    // E o `atualizar` sem a coluna continua ZERANDO, que e o
    // comportamento de sempre do motor: a linha inteira e gravada e o
    // ausente vira nulo. Preservar aqui seria a guarda nova entrando
    // imposta -- e mudaria o que todo cliente de hoje ja faz.
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":{"id":1,"nome":"z"}"#,
    )
    .unwrap();
    assert_eq!(
        salario_gravado(&s),
        -1,
        "o comportamento velho do atualizar mudou para quem nao pediu nada"
    );
}

// ------------------------------------------------------------------ leitura

/// A coluna negada sai da resposta -- nas duas formas do `ler`, no
/// `varrer` e no `buscar`.
#[test]
fn a_leitura_esconde_a_coluna_negada() {
    let dir = dir_temp("esconde");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());

    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert!(l.campo("salario").is_none(), "vazou: {}", l.escrever());
    assert_eq!(l.texto_ou("nome", ""), "ana", "levou o resto junto");

    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1,"com_versao":true"#,
    )
    .unwrap();
    assert!(l.campo("linha").unwrap().campo("salario").is_none());
    assert!(l.campo("versao").is_some(), "o envelope se perdeu");

    for corpo in [
        r#""op":"varrer","database":"b","tabela":"folha""#,
        r#""op":"buscar","database":"b","tabela":"folha","indice":"porId","chave":[1]"#,
        r#""op":"varrer","database":"b","tabela":"folha","indice":"porId""#,
    ] {
        let r = pede(&s, &ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
        let linha = &r.campo("linhas").and_then(Json::lista).unwrap()[0];
        assert!(
            linha.campo("salario").is_none(),
            "{corpo} vazou: {}",
            r.escrever()
        );
        assert_eq!(linha.texto_ou("nome", ""), "ana", "{corpo}");
    }

    // E a tabela SEM regra da mesma base continua inteira.
    let c = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#,
    )
    .unwrap();
    assert_eq!(c.texto_ou("nome", ""), "x");
}

/// **Pedido 350:** o `jobs` devolvia o pedido salvo inteiro, e o valor
/// de uma coluna negada digitado na definicao do job saia para quem nao
/// le essa coluna. Decisao do dono (30/09/2026): redigir analisando. O
/// `salario` vira o tamanho, o resto do pedido fica, e o job `sql` -- que
/// nao diz a tabela que alcanca -- sai inteiro como tamanho.
///
/// # Prova real
///
/// Com `jobs` de volta a classe `Nenhum`, o `5000` sai na resposta -- o
/// vermelho medido.
#[test]
fn o_jobs_redige_a_coluna_negada_na_definicao_do_job() {
    let dir = dir_temp("350-jobs");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    {
        let mut r = s.jobs.tomar("jobs").unwrap();
        for item in [
            r#"{"nome":"reajuste","pedido":{"op":"atualizar","database":"b",
                    "tabela":"folha","rowid":1,"valores":{"id":1,"nome":"ana","salario":5000}}}"#,
            r#"{"nome":"consulta","pedido":{"op":"sql","database":"b",
                    "texto":"SELECT salario FROM folha WHERE salario = 5000"}}"#,
        ] {
            r.salvar(crate::jobs::Job::de_json(&Json::analisar(item).unwrap()).unwrap())
                .unwrap();
        }
    }
    // O apelido chega ao portao como o nome chega: os dois, ou o que
    // ficar de fora vira a porta dos fundos.
    for op in ["jobs", "job_listar"] {
        let r = pede(&s, &ses, &format!(r#""op":"{op}""#)).unwrap();
        let texto = r.escrever();
        assert!(
            !texto.contains("5000"),
            "{op} vazou a coluna negada: {texto}"
        );
        assert!(texto.contains("<redigido,"), "{op}: {texto}");
        assert!(
            texto.contains("\"nome\":\"ana\""),
            "{op} levou o resto junto: {texto}"
        );
        assert!(
            texto.contains("<pedido,"),
            "{op}: o sql nao virou tamanho: {texto}"
        );
    }
}

/// **A pergunta tambem responde.** A peneira tira o valor DEPOIS de o
/// filtro ja ter contado as linhas que casam -- vinte perguntas dessas
/// dizem o salario sem ele nunca ter aparecido.
#[test]
fn perguntar_pela_coluna_negada_recusa() {
    let dir = dir_temp("pergunta");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    for corpo in [
        r#""op":"varrer","database":"b","tabela":"folha",
               "onde":[{"coluna":"salario","op":"=","valor":5000}]"#,
        r#""op":"varrer","database":"b","tabela":"folha","indice":"porSalario""#,
        r#""op":"buscar","database":"b","tabela":"folha","indice":"porSalario","chave":[5000]"#,
    ] {
        let e = pede(&s, &ses, corpo).expect_err("a pergunta passou");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
        assert!(
            format!("{e}").contains("salario"),
            "a recusa tem de dizer QUAL coluna: {e}"
        );
    }
    // Filtrar por uma coluna que se pode ler continua funcionando.
    let r = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"b","tabela":"folha",
               "onde":[{"coluna":"nome","op":"=","valor":"ana"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1);
}

/// Estrutura nao e dado: a coluna continua no esquema, e a resposta diz o
/// que este usuario nao alcanca -- senao a tela pinta um campo que nunca
/// chega preenchido, e quem olha conclui que a coluna esta vazia.
#[test]
fn o_esquema_continua_inteiro_e_diz_o_que_falta() {
    let dir = dir_temp("esquema");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let e = pede(
        &s,
        &ses,
        r#""op":"esquema","database":"b","tabela":"folha""#,
    )
    .unwrap();
    let nomes: Vec<String> = e
        .campo("colunas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .map(|c| c.texto_ou("nome", "").to_string())
        .collect();
    assert!(
        nomes.contains(&"salario".to_string()),
        "a estrutura foi podada: {nomes:?}"
    );
    for campo in ["colunas_sem_leitura", "colunas_sem_alteracao"] {
        let l = e.campo(campo).and_then(Json::lista).unwrap_or(&[]);
        assert_eq!(
            l.iter()
                .map(|x| x.texto().unwrap_or(""))
                .collect::<Vec<_>>(),
            vec!["salario"],
            "{campo}: {}",
            e.escrever()
        );
    }
    // A tabela sem regra nao ganha campo nenhum: quem nao pediu nada
    // continua recebendo a resposta de sempre.
    let c = pede(
        &s,
        &ses,
        r#""op":"esquema","database":"b","tabela":"clientes""#,
    )
    .unwrap();
    assert!(c.campo("colunas_sem_leitura").is_none(), "{}", c.escrever());
}

/// Um servidor a parte, so para o teste dos baldes: a base do fixture
/// comum nao tem tabela particionada por letra, e criar uma aqui evita
/// mexer no que as outras baterias deste modulo ja dependem.
fn servidor_com_particao_por_letra(
    dir: &std::path::Path,
    cadastro: Cadastro,
) -> (Arc<Servidor>, Sessao) {
    let s = Servidor::novo(config_em(dir, cadastro.clone())).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"vip",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"cidade","tipo":"Str(20)","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}],
                    "registros_por_arquivo":1000,
                    "particao":"letra","particao_coluna":"cidade"}"#,
        ),
        &dono,
    )
    .unwrap();
    for (id, cidade) in [(1, "amparo"), (2, "blumenau"), (3, "boituva")] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"vip","linha":{{"id":{id},"cidade":"{cidade}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let sessao = Sessao {
        usuario: cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    (s, sessao)
}

/// Pedido 369: `baldes[].registros` e' agregado do DADO -- quantas linhas
/// caem em cada classe do primeiro caractere --, e nao da estrutura. Some
/// so' quando a coluna que particiona a tabela esta negada a este
/// usuario; quem nao tem regra na coluna da particao continua vendo o
/// histograma de sempre, que e' o comportamento velho que "guarda nova
/// entra pedida, nao imposta" exige preservar.
#[test]
fn baldes_perdem_a_contagem_so_quando_a_coluna_da_particao_esta_negada() {
    let dir = dir_temp("baldes");
    let cadastro_negando_cidade = cadastro(
        r#"{"*":{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                 "criar":true,"reindexar":true,"diario":true,"verificar":true,
                 "replicar":true,"administrar":true,
                 "tabelas":{"vip":{"ler":true,"inserir":true,"alterar":true,
                   "excluir":true,"criar":true,"diario":true,"administrar":true,
                   "replicar":true,"verificar":true,"reindexar":true,
                   "colunas":{"cidade":{"ler":false,"alterar":false}}}}}}"#,
    );
    let (s, ana) = servidor_com_particao_por_letra(&dir, cadastro_negando_cidade);

    let e = pede(&s, &ana, r#""op":"esquema","database":"b","tabela":"vip""#).unwrap();
    let baldes = e
        .campo("paginacao")
        .and_then(|p| p.campo("baldes"))
        .and_then(Json::lista)
        .unwrap();
    assert!(!baldes.is_empty(), "os 37 baldes sumiram: {}", e.escrever());
    assert!(
        baldes.iter().all(|b| b.campo("registros").is_none()),
        "a contagem por balde vazou com a coluna da particao negada: {}",
        e.escrever()
    );
    // A ESTRUTURA continua inteira -- so o agregado do dado saiu.
    assert!(
        baldes.iter().all(|b| b.campo("letra").is_some()
            && b.campo("arquivo").is_some()
            && b.campo("primeiro_rowid").is_some()),
        "a peneira levou junto o que nao e dado: {}",
        e.escrever()
    );

    // CONTROLE (comportamento velho): sem regra de coluna nenhuma, o
    // histograma de sempre continua vindo, contagem inclusive.
    let dono = Sessao::default();
    let e2 = s
        .executar(
            "esquema",
            &pedido(r#"{"database":"b","tabela":"vip"}"#),
            &dono,
        )
        .unwrap();
    let baldes2 = e2
        .campo("paginacao")
        .and_then(|p| p.campo("baldes"))
        .and_then(Json::lista)
        .unwrap();
    assert!(
        baldes2.iter().any(|b| b.campo("registros").is_some()),
        "o comportamento velho mudou para quem nao tem regra nenhuma: {}",
        e2.escrever()
    );
}

/// **Pedido 543: com a coluna que particiona negada, a primeira letra (ou
/// o periodo) de cada linha nao sai por porta nenhuma.**
///
/// As quatro portas do parecer, e a quinta, pelo `despachar`: (1)
/// `paginacao.baldes[].existe`; (2) `esquema.slots` (e o `arquivos`, que e
/// o `existe` com outro nome); (3) `sistabelas.slots`; (4) o rowid do
/// `varrer` -- e do `ler` e da escrita, que andam pelo mesmo rowid; (5)
/// `volumes[].periodo` numa particao mensal sobre `Date` negada.
///
/// O controle e o comportamento velho: com a regra numa coluna que NAO
/// particiona (`id`), ana varre e o esquema vem inteiro.
///
/// **Defeito reposto** (a decisao `dc::coluna_do_rowid_negada`
/// respondendo sempre `None`, que e o que o codigo de antes fazia por nao
/// perguntar): caem as cinco portas, uma linha por porta no vermelho --
/// `existe`, `slots`, `arquivos`, `sistabelas.slots`, o rowid pelo
/// `varrer`, `ler`, `excluir` e `atualizar`, e o `periodo`.
#[test]
fn a_coluna_que_particiona_negada_nao_sai_pelo_rowid_nem_pelo_balde() {
    let dir = dir_temp("oraculo-543");
    let negando = |coluna: &str| {
        cadastro(&format!(
            r#"{{"*":{{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                     "criar":true,"reindexar":true,"diario":true,"verificar":true,
                     "replicar":true,"administrar":true,
                     "tabelas":{{"vip":{{"ler":true,"inserir":true,"alterar":true,
                       "excluir":true,"criar":true,"diario":true,"administrar":true,
                       "replicar":true,"verificar":true,"reindexar":true,
                       "colunas":{{"{coluna}":{{"ler":false,"alterar":false}}}}}},
                     "lanc":{{"ler":true,"inserir":true,"alterar":true,
                       "excluir":true,"criar":true,"diario":true,"administrar":true,
                       "replicar":true,"verificar":true,"reindexar":true,
                       "colunas":{{"emissao":{{"ler":false,"alterar":false}}}}}}}}}}}}"#
        ))
    };
    let (s, ana) = servidor_com_particao_por_letra(&dir, negando("cidade"));
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"lanc",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                               {"nome":"emissao","tipo":"Date","obrigatoria":true}],
                    "registros_por_arquivo":1000,
                    "particao":"mensal","particao_coluna":"emissao"}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"lanc","linha":{"id":1,"emissao":"2026-03-05"}}"#),
        &dono,
    )
    .unwrap();

    let mut vazou = Vec::new();
    let e = pede(&s, &ana, r#""op":"esquema","database":"b","tabela":"vip""#).unwrap();
    let baldes = e
        .campo("paginacao")
        .and_then(|p| p.campo("baldes"))
        .and_then(Json::lista)
        .unwrap_or(&[]);
    if baldes.iter().any(|b| b.campo("existe").is_some()) {
        vazou.push("(1) paginacao.baldes[].existe".to_string());
    }
    for campo in ["slots", "arquivos"] {
        if e.campo(campo).is_some() {
            vazou.push(format!("(2) esquema.{campo}"));
        }
    }
    let cat = pede(&s, &ana, r#""op":"sistabelas","database":"b""#).unwrap();
    for t in cat.campo("tabelas").and_then(Json::lista).unwrap_or(&[]) {
        if t.texto_ou("tabela", "") == "vip" && t.campo("slots").is_some() {
            vazou.push("(3) sistabelas.slots".to_string());
        }
    }
    for (porta, corpo) in [
        ("varrer", r#""op":"varrer","database":"b","tabela":"vip""#),
        (
            "ler",
            r#""op":"ler","database":"b","tabela":"vip","rowid":1"#,
        ),
        (
            "excluir",
            r#""op":"excluir","database":"b","tabela":"vip","rowid":999999"#,
        ),
        (
            "atualizar",
            r#""op":"atualizar","database":"b","tabela":"vip","rowid":999999,
                   "valores":{"id":9}"#,
        ),
    ] {
        match pede(&s, &ana, corpo) {
            Err(PhxError::Autorizacao(m)) if m.contains("cidade") => {}
            outro => vazou.push(format!("(4) rowid pelo {porta}: {outro:?}")),
        }
    }
    let el = pede(&s, &ana, r#""op":"esquema","database":"b","tabela":"lanc""#).unwrap();
    if el.escrever().contains("2026-03") {
        vazou.push(format!("(5) volumes[].periodo: {}", el.escrever()));
    }
    assert!(
        vazou.is_empty(),
        "a coluna que particiona, negada, ainda sai por:\n  {}",
        vazou.join("\n  ")
    );
    // A estrutura do balde fica: e geometria, igual para toda tabela.
    assert!(
        !baldes.is_empty()
            && baldes
                .iter()
                .all(|b| b.campo("letra").is_some() && b.campo("primeiro_rowid").is_some()),
        "a peneira levou junto o que nao e dado: {}",
        e.escrever()
    );

    // CONTROLE: a regra numa coluna que NAO particiona nao muda nada.
    let dir2 = dir_temp("oraculo-543-controle");
    let (s2, ana2) = servidor_com_particao_por_letra(&dir2, negando("id"));
    let v = pede(&s2, &ana2, r#""op":"varrer","database":"b","tabela":"vip""#).unwrap();
    assert_eq!(
        v.campo("linhas").and_then(Json::lista).map(|l| l.len()),
        Some(3),
        "{}",
        v.escrever()
    );
    let e2 = pede(
        &s2,
        &ana2,
        r#""op":"esquema","database":"b","tabela":"vip""#,
    )
    .unwrap();
    assert!(e2.campo("slots").is_some(), "{}", e2.escrever());
}

/// **Pedido 600: a marca d'agua da tabela particionada pela coluna negada
/// nao sai pelas operacoes de administracao.**
///
/// O 543 tirou `slots`/`volumes`/`arquivos` do `esquema` e do
/// `sistabelas`; estas quatro devolviam o mesmo numero com outro nome:
/// `verificar` (`slots` e quantos volumes do `.reg` existem),
/// `migrar_esquema` (`slots`, `slots_a_reescrever` e o `aviso` que os
/// escreve por extenso), `acrescentar_coluna` (`slots_reescritos`) e
/// `memoria_carregar` (a ficha da residente). Pelo `despachar`.
///
/// `reindexar` e `estatisticas`, nomeados no pedido, foram MEDIDOS aqui e
/// nao carregam slot nenhum -- a conferencia fica, para o dia em que
/// passarem a carregar.
///
/// O controle e o comportamento velho: a regra numa coluna que NAO
/// particiona deixa o `verificar` e o `acrescentar_coluna` como sempre.
///
/// **Defeito reposto** (as quatro de volta em `PorColuna::Nenhum`): cai
/// uma linha por porta.
#[test]
fn a_marca_dagua_da_particao_negada_nao_sai_pela_administracao() {
    let dir = dir_temp("marca-dagua-600");
    let negando = |coluna: &str| {
        cadastro(&format!(
            r#"{{"*":{{"ler":true,"inserir":true,"alterar":true,"excluir":true,
                     "criar":true,"reindexar":true,"diario":true,"verificar":true,
                     "replicar":true,"administrar":true,
                     "tabelas":{{"vip":{{"ler":true,"inserir":true,"alterar":true,
                       "excluir":true,"criar":true,"diario":true,"administrar":true,
                       "replicar":true,"verificar":true,"reindexar":true,
                       "colunas":{{"{coluna}":{{"ler":false,"alterar":false}}}}}}}}}}}}"#
        ))
    };
    let (s, ana) = servidor_com_particao_por_letra(&dir, negando("cidade"));

    let mut vazou = Vec::new();
    let mut olhar = |porta: &str, corpo: &str| {
        let r = pede(&s, &ana, corpo).unwrap_or_else(|e| panic!("{porta}: {e}"));
        for k in crate::direito_coluna::CHAVES_DA_MARCA_DAGUA {
            if r.campo(k).is_some() {
                vazou.push(format!("{porta}.{k}: {}", r.escrever()));
            }
        }
    };
    olhar(
        "verificar",
        r#""op":"verificar","database":"b","tabela":"vip""#,
    );
    olhar(
        "migrar_esquema",
        r#""op":"migrar_esquema","database":"b","tabela":"vip""#,
    );
    olhar(
        "acrescentar_coluna",
        r#""op":"acrescentar_coluna","database":"b","tabela":"vip",
               "coluna":{"nome":"obs","tipo":"Str(10)"}"#,
    );
    olhar(
        "memoria_carregar",
        r#""op":"memoria_carregar","database":"b","tabela":"vip""#,
    );
    // As duas que o pedido nomeou e que a medida absolveu.
    olhar(
        "reindexar",
        r#""op":"reindexar","database":"b","tabela":"vip""#,
    );
    olhar("estatisticas", r#""op":"estatisticas""#);
    assert!(
        vazou.is_empty(),
        "a marca d'agua da particao negada ainda sai por:\n  {}",
        vazou.join("\n  ")
    );

    // CONTROLE: a regra numa coluna que NAO particiona nao muda nada.
    let dir2 = dir_temp("marca-dagua-600-controle");
    let (s2, ana2) = servidor_com_particao_por_letra(&dir2, negando("id"));
    let v = pede(
        &s2,
        &ana2,
        r#""op":"verificar","database":"b","tabela":"vip""#,
    )
    .unwrap();
    assert!(v.campo("slots").is_some(), "{}", v.escrever());
    let a = pede(
        &s2,
        &ana2,
        r#""op":"acrescentar_coluna","database":"b","tabela":"vip",
               "coluna":{"nome":"obs","tipo":"Str(10)"}"#,
    )
    .unwrap();
    assert!(a.campo("slots_reescritos").is_some(), "{}", a.escrever());
}

/// O `esquema` diz o material EM DISCO da tabela. Aqui o cofre esta
/// desligado -- este binario nao pode liga-lo, a chave e do processo --,
/// entao a resposta certa e `em_claro`, e o campo tem de EXISTIR: um
/// campo que so aparece quando cifrado deixaria a tela sem saber se a
/// tabela e antiga ou se o servidor nao sabe responder. O lado `cifrado`
/// esta em `tests/cifra-pelo-config.rs`, num processo proprio.
#[test]
fn o_esquema_diz_o_material_em_disco() {
    let dir = dir_temp("material");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let e = pede(
        &s,
        &ses,
        r#""op":"esquema","database":"b","tabela":"clientes""#,
    )
    .unwrap();
    assert_eq!(
        e.texto_ou("material", "<ausente>"),
        "em_claro",
        "{}",
        e.escrever()
    );
}

// ------------------------------------------------------------------ escrita

/// Mandar valor na coluna negada recusa, e a recusa NOMEIA a coluna --
/// senao quem recebeu o erro nao sabe qual campo tirar do formulario.
#[test]
fn a_escrita_com_valor_na_coluna_negada_mantem_o_gravado_e_diz_isso() {
    let dir = dir_temp("mantem");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    for corpo in [
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":{"id":1,"nome":"ana","salario":9000}"#,
        // Nulo EXPLICITO e o que a ficha manda na coluna que nao leu.
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":{"id":1,"nome":"ana2","salario":null}"#,
        // A forma de LISTA, por posicao.
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":[1,"ana3",7]"#,
    ] {
        let r = pede(&s, &ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
        assert_eq!(
            salario_gravado(&s),
            5000,
            "{corpo}: gravou na coluna negada"
        );
        let m = r.campo("colunas_mantidas").and_then(Json::lista);
        assert_eq!(
            m.map(|l| l.len()),
            Some(1),
            "{corpo}: a resposta nao diz o que manteve: {r:?}"
        );
        assert_eq!(m.unwrap()[0].texto(), Some("salario"));
    }
    // E o que ela PODIA alterar mudou -- a operacao passou de verdade.
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.texto_ou("nome", ""), "ana3");

    // No inserir a coluna SAI do pedido e nasce nula, como se ninguem a
    // tivesse mandado -- e a resposta diz que ela foi mantida de fora.
    for (corpo, rowid) in [
        (
            r#""op":"inserir","database":"b","tabela":"folha",
                   "linha":{"id":2,"nome":"beto","salario":9000}"#,
            2,
        ),
        (
            r#""op":"inserir_lote","database":"b","tabela":"folha",
                   "linhas":[{"id":3,"nome":"caio","salario":1},[4,"duda",2]]"#,
            3,
        ),
    ] {
        let r = pede(&s, &ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
        assert!(r.campo("colunas_mantidas").is_some(), "{corpo}: {r:?}");
        let l = s
            .executar(
                "ler",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"folha","rowid":{rowid}}}"#
                )),
                &Sessao::default(),
            )
            .unwrap();
        assert!(
            l.campo("salario").is_some_and(Json::e_nulo),
            "{corpo}: {l:?}"
        );
    }

    // Inserir SEM a coluna passa igual, e ai nao ha nada a dizer.
    let r = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"folha","linha":{"id":5,"nome":"eva"}"#,
    )
    .expect("inserir sem a coluna negada tinha de passar");
    assert!(r.campo("colunas_mantidas").is_none(), "{r:?}");
}

/// **A FICHA de quem tem regra de coluna inclui e salva.** O vendedor nao
/// altera `limite_credito` de `clientes`, e a ficha manda a linha
/// inteira -- com a coluna que nao leu como `null`. Antes, a presenca da
/// coluna recusava a operacao toda, e um usuario com qualquer regra de
/// coluna nao conseguia incluir nem salvar nada, nem mexendo so no que
/// podia: protecao que quebra todo cliente nao e protecao. Agora inclui
/// (o limite nasce nulo), salva (o limite gravado fica) e a resposta diz
/// o que manteve. O teste velho -- quem nao tem regra grava igual -- e
/// `sem_colunas_no_cadastro_nada_muda`.
#[test]
fn a_ficha_de_quem_tem_regra_de_coluna_inclui_e_salva() {
    let dir = dir_temp("ficha");
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"vendedor","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"inserir":true,"alterar":true,
                   "tabelas":{"clientes":{"ler":true,"inserir":true,"alterar":true,
                     "colunas":{"limite_credito":{"ler":false,"alterar":false}}}}}}}]}"#,
    ))
    .unwrap();
    let s = Servidor::novo(config_em(&dir, cadastro.clone())).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(30)"},
                               {"nome":"limite_credito","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    let vendedor = Sessao {
        usuario: cadastro.por_login("vendedor").cloned(),
        ..Sessao::default()
    };
    let limite = |s: &Arc<Servidor>| -> Json {
        s.executar(
            "ler",
            &pedido(r#"{"database":"loja","tabela":"clientes","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap()
        .campo("limite_credito")
        .cloned()
        .unwrap_or(Json::Nulo)
    };

    // INCLUIR pela ficha: a linha inteira, com a coluna que nao leu nula.
    let r = pede(
        &s,
        &vendedor,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{"id":1,"nome":"Ana","limite_credito":null}"#,
    )
    .expect("a ficha nao conseguiu incluir");
    assert_eq!(r.inteiro_ou("rowid", -1), 1);
    assert!(limite(&s).e_nulo());

    // O dono poe o limite; o vendedor SALVA mexendo so no nome, e a ficha
    // manda o `null` de novo -- e depois manda um valor, que tambem fica
    // de fora. Nas duas o limite gravado fica, e a resposta diz.
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"loja","tabela":"clientes","rowid":1,
                    "valores":{"id":1,"nome":"Ana","limite_credito":500}}"#,
        ),
        &dono,
    )
    .unwrap();
    for (corpo, nome) in [
        (
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "valores":{"id":1,"nome":"Ana Maria","limite_credito":null}"#,
            "Ana Maria",
        ),
        (
            r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
                   "valores":{"id":1,"nome":"Ana M.","limite_credito":999}"#,
            "Ana M.",
        ),
    ] {
        let r = pede(&s, &vendedor, corpo).expect("a ficha nao conseguiu salvar");
        assert_eq!(limite(&s).inteiro(), Some(500), "{corpo}: o limite mudou");
        let l = pede(
            &s,
            &vendedor,
            r#""op":"ler","database":"loja","tabela":"clientes","rowid":1"#,
        )
        .unwrap();
        assert_eq!(l.texto_ou("nome", ""), nome, "{corpo}: nao salvou o nome");
        assert!(l.campo("limite_credito").is_none(), "vazou: {l:?}");
        let m = r.campo("colunas_mantidas").and_then(Json::lista).unwrap();
        assert_eq!(m[0].texto(), Some("limite_credito"), "{r:?}");
    }

    // E quem LE e nao altera devolve a linha como leu: passa, e nao ha o
    // que dizer -- nada foi mantido contra o pedido.
    let cadastro = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"vendedor","id":9,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"ler":true,"inserir":true,"alterar":true,
                   "tabelas":{"clientes":{"ler":true,"inserir":true,"alterar":true,
                     "colunas":{"limite_credito":{"ler":true,"alterar":false}}}}}}}]}"#,
    ))
    .unwrap();
    let dir2 = dir_temp("ficha-le");
    let s2 = Servidor::novo(config_em(&dir2, cadastro.clone())).unwrap();
    s2.executar("criar_database", &pedido(r#"{"database":"loja"}"#), &dono)
        .unwrap();
    s2.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"loja","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(30)"},
                               {"nome":"limite_credito","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s2.executar(
        "inserir",
        &pedido(
            r#"{"database":"loja","tabela":"clientes",
                    "linha":{"id":1,"nome":"Ana","limite_credito":500}}"#,
        ),
        &dono,
    )
    .unwrap();
    let vendedor = Sessao {
        usuario: cadastro.por_login("vendedor").cloned(),
        ..Sessao::default()
    };
    let l = pede(
        &s2,
        &vendedor,
        r#""op":"ler","database":"loja","tabela":"clientes","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.inteiro_ou("limite_credito", -1), 500);
    let r = pede(
        &s2,
        &vendedor,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
               "valores":{"id":1,"nome":"Ana Maria","limite_credito":500}"#,
    )
    .expect("devolver a linha como leu tinha de passar");
    assert!(r.campo("colunas_mantidas").is_none(), "{r:?}");
    assert_eq!(limite(&s2).inteiro(), Some(500));
}

/// **O caso que motivou tudo.** Quem nao le a coluna manda a linha sem
/// ela, e o `atualizar` grava a linha INTEIRA: sem esta reposicao, alterar
/// o nome zerava o salario do outro em silencio.
#[test]
fn o_atualizar_sem_a_coluna_preserva_o_valor_gravado() {
    let dir = dir_temp("preserva");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":{"id":1,"nome":"ana maria"}"#,
    )
    .expect("o atualizar sem a coluna negada tinha de passar");
    assert_eq!(
        salario_gravado(&s),
        5000,
        "o salario foi zerado por quem nem podia ve-lo"
    );
    // E o que ele PODIA alterar mudou mesmo -- senao a prova passaria com
    // um servidor que simplesmente nao gravou nada.
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.texto_ou("nome", ""), "ana maria");
}

/// Com `{"ler": true, "alterar": false}` o cliente le a linha inteira e a
/// devolve inteira: o valor IGUAL passa sem nada a dizer, porque nao
/// altera nada; o valor DIFERENTE passa com o gravado mantido, e a
/// resposta diz. A comparacao so existe para quem PODE ler: para quem
/// nao le ela seria um oraculo.
#[test]
fn quem_le_a_coluna_e_nao_a_altera_continua_gravando() {
    let dir = dir_temp("so-leitura");
    let (s, ses) = servidor(
        &dir,
        cadastro(
            r#"{"*":{"ler":true,"inserir":true,"alterar":true,
                     "tabelas":{"folha":{"ler":true,"inserir":true,"alterar":true,
                       "colunas":{"salario":{"ler":true,"alterar":false}}}}}}"#,
        ),
    );
    // A coluna continua chegando: `ler` nao foi negado.
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.inteiro_ou("salario", -1), 5000);

    // Devolver a linha inteira, com o MESMO salario, passa.
    pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":{"id":1,"nome":"outra","salario":5000}"#,
    )
    .expect("devolver o valor igual e nao alterar nada");
    assert_eq!(salario_gravado(&s), 5000);

    // Mudar NAO recusa: o gravado fica, e a resposta diz que ficou.
    let r = pede(
        &s,
        &ses,
        r#""op":"atualizar","database":"b","tabela":"folha","rowid":1,
               "valores":{"id":1,"nome":"outra2","salario":6000}"#,
    )
    .expect("a presenca da coluna recusou a operacao inteira");
    assert_eq!(salario_gravado(&s), 5000, "alterou a coluna que so le");
    let m = r.campo("colunas_mantidas").and_then(Json::lista).unwrap();
    assert_eq!(m[0].texto(), Some("salario"), "{r:?}");
}

// -------------------------------------------------- os outros caminhos

/// As operacoes que devolvem linha por um caminho que a peneira nao
/// percorre recusam a TABELA -- e so ela: as outras da mesma base
/// continuam abrindo.
#[test]
fn quem_nao_peneira_recusa_a_tabela_restrita() {
    let dir = dir_temp("recusa-op");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    for (rotulo, corpo) in [
        (
            "exportar",
            r#""op":"exportar","database":"b","tabela":"folha","formato":"json""#,
        ),
        (
            "juntar",
            r#""op":"juntar","database":"b","a":{"tabela":"clientes","chave":"id"},
                   "b":{"tabela":"folha","chave":"id"}"#,
        ),
        (
            "unir",
            r#""op":"unir","database":"b","tabelas":["clientes","folha"]"#,
        ),
        ("diario", r#""op":"diario","database":"b","tabela":"folha""#),
        (
            "lixeira",
            r#""op":"lixeira","database":"b","tabela":"folha""#,
        ),
        (
            "checksum",
            r#""op":"checksum","database":"b","tabela":"folha""#,
        ),
        (
            "duplicar_tabela",
            r#""op":"duplicar_tabela","database":"b","tabela":"folha","destino":"copia""#,
        ),
    ] {
        let e = pede(&s, &ses, corpo)
            .err()
            .unwrap_or_else(|| panic!("{rotulo} devolveu a tabela restrita inteira"));
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{rotulo}: {e}");
        assert!(
            format!("{e}").contains(rotulo),
            "{rotulo}: a recusa tem de dizer qual operacao nao aplica o direito: {e}"
        );
    }
    // E a tabela SEM regra de coluna continua saindo por todas elas.
    assert!(pede(
        &s,
        &ses,
        r#""op":"exportar","database":"b","tabela":"clientes","formato":"json""#
    )
    .is_ok());
}

/// O `juntar` esconde a tabela do portao GERAL -- e tem de esconde-la
/// tambem deste. Sem isto, pedir a folha como o lado B era o caminho de
/// fora da peneira.
#[test]
fn a_tabela_escondida_do_portao_tambem_recusa_aqui() {
    let dir = dir_temp("escondida");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let e = pede(
        &s,
        &ses,
        r#""op":"juntar","database":"b","a":{"tabela":"clientes","chave":"id"},
               "b":{"tabela":"folha","chave":"id"}"#,
    )
    .expect_err("a folha saiu inteira pelo lado B da juncao");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(format!("{e}").contains("juntar"), "{e}");

    // E a juncao de duas tabelas SEM regra continua acontecendo.
    assert!(pede(
        &s,
        &ses,
        r#""op":"unir","database":"b","tabelas":["clientes","clientes"]"#
    )
    .is_ok());
}

// ---------------------------------------------------------------------- SQL

/// **O irmao que so o `executar_derivado` alcanca.** Um `SELECT` esconde a
/// coluna, e um `UPDATE` de OUTRA coluna deixa o salario como estava --
/// e nenhum dos passos do UPDATE chega pelo `despachar`.
#[test]
fn o_sql_herda_o_direito_por_coluna() {
    let dir = dir_temp("sql");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());

    let r = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"SELECT * FROM folha""#,
    )
    .unwrap();
    let linhas = r
        .campo("linhas")
        .and_then(Json::lista)
        .or_else(|| {
            r.campo("resultado")
                .and_then(|x| x.campo("linhas"))
                .and_then(Json::lista)
        })
        .expect("o SELECT nao devolveu linhas");
    assert!(
        linhas[0].campo("salario").is_none(),
        "o SELECT vazou a coluna: {}",
        r.escrever()
    );

    // E o UPDATE de outra coluna PRESERVA o salario. E o buscar -> ler ->
    // atualizar do `executar_dml`: o `ler` volta sem a coluna, e a
    // mesclagem gravaria NULL nela.
    pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"UPDATE folha SET nome = 'zeta' WHERE id = 1""#,
    )
    .expect("o UPDATE de outra coluna tinha de passar");
    assert_eq!(
        salario_gravado(&s),
        5000,
        "o UPDATE pelo SQL zerou a coluna que quem mandou nem le"
    );

    // E o UPDATE da propria coluna negada MANTEM o gravado, e a resposta
    // do passo `atualizar` diz o que manteve.
    let r = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"UPDATE folha SET salario = 1 WHERE id = 1""#,
    )
    .expect("a presenca da coluna recusou o UPDATE inteiro");
    assert_eq!(
        salario_gravado(&s),
        5000,
        "o UPDATE alterou a coluna negada"
    );
    assert!(r.escrever().contains("colunas_mantidas"), "{r:?}");
}

/// **O caso que motivou tudo, de volta pela porta do `se_existir`.** O
/// upsert que ATUALIZA grava a linha inteira pelo mesmo `atualizar`, e o
/// ausente virava NULL por cima do salario -- para quem nao le E para
/// quem le e nao altera, pelo protocolo, pelo SQL (`ON CONFLICT DO UPDATE`)
/// e dentro de transacao. Passou por todos os testes do `atualizar`
/// porque o `op` dizia `inserir`.
///
/// A prova mede o dado gravado visto por quem nao tem restricao, e mede
/// tambem que o nome MUDOU: um servidor que nao gravasse nada passaria na
/// primeira metade.
#[test]
fn o_upsert_sem_a_coluna_preserva_o_valor_gravado() {
    let dir = dir_temp("upsert");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());

    // Pelo protocolo.
    let r = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"ana-upsert"},"se_existir":"atualizar""#,
    )
    .expect("o upsert sem a coluna negada tinha de passar");
    assert!(r.booleano_ou("atualizada", false), "{r:?}");
    assert_eq!(salario_gravado(&s), 5000, "o upsert zerou o salario");
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.texto_ou("nome", ""), "ana-upsert", "nao gravou nada");

    // Pelo SQL: o SET vai no `atualizar`, e a linha lida e o que entra
    // por cima -- com o salario que ana nao ve.
    pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"INSERT INTO folha (id, nome) VALUES (1, 'sql') ON CONFLICT (id) DO UPDATE SET nome = 'sql'""#,
    )
    .expect("o ON CONFLICT DO UPDATE de outra coluna tinha de passar");
    assert_eq!(salario_gravado(&s), 5000, "o ON CONFLICT zerou o salario");

    // O valor EXPLICITO -- na linha e no `atualizar` -- e mantido de
    // fora, e a resposta diz.
    for corpo in [
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"x","salario":1},"se_existir":"atualizar""#,
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"y"},"se_existir":"atualizar",
               "atualizar":{"salario":1}"#,
    ] {
        let r = pede(&s, &ses, corpo).unwrap_or_else(|e| panic!("{corpo}: {e}"));
        assert_eq!(
            salario_gravado(&s),
            5000,
            "{corpo}: gravou na coluna negada"
        );
        let m = r.campo("colunas_mantidas").and_then(Json::lista).unwrap();
        assert_eq!(m[0].texto(), Some("salario"), "{corpo}: {r:?}");
    }

    // Dentro de transacao: o `empilhar` recebe o pedido ja reposto.
    let mut tx = Sessao {
        usuario: ses.usuario.clone(),
        ligacao: 7,
        ..Sessao::default()
    };
    let mut na_tx = |corpo: &str| -> Result<Json> {
        let (_, _, r) = s.despachar(&format!(r#"{{"token":"t",{corpo}}}"#), &mut tx, "127.0.0.1");
        r
    };
    na_tx(r#""op":"begin","database":"b""#).unwrap();
    na_tx(
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"tx"},"se_existir":"atualizar""#,
    )
    .expect("o upsert na transacao tinha de empilhar");
    na_tx(r#""op":"commit","database":"b""#).unwrap();
    assert_eq!(salario_gravado(&s), 5000, "o commit zerou o salario");

    // `ignorar` nao grava e nao muda nada.
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"zzz"},"se_existir":"ignorar""#,
    )
    .unwrap();
    assert_eq!(salario_gravado(&s), 5000);

    // E quem LE a coluna e nao a altera: o ausente e reposto, o igual
    // passa calado, o diferente e mantido e dito -- a regra do `atualizar`.
    let dir2 = dir_temp("upsert-so-leitura");
    let (s, ses) = servidor(
        &dir2,
        cadastro(
            r#"{"*":{"ler":true,"inserir":true,"alterar":true,
                     "tabelas":{"folha":{"ler":true,"inserir":true,"alterar":true,
                       "colunas":{"salario":{"ler":true,"alterar":false}}}}}}"#,
        ),
    );
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"gil"},"se_existir":"atualizar""#,
    )
    .expect("o upsert sem a coluna tinha de passar");
    assert_eq!(
        salario_gravado(&s),
        5000,
        "zerou o salario de quem le e nao altera"
    );
    pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"gil2","salario":5000},"se_existir":"atualizar""#,
    )
    .expect("o valor igual ao gravado tinha de passar");
    let r = pede(
        &s,
        &ses,
        r#""op":"inserir","database":"b","tabela":"folha",
               "linha":{"id":1,"nome":"gil3","salario":1},"se_existir":"atualizar""#,
    )
    .expect("a presenca da coluna recusou o upsert inteiro");
    assert_eq!(salario_gravado(&s), 5000, "alterou a coluna que so le");
    assert!(r.campo("colunas_mantidas").is_some(), "{r:?}");
}

/// **`SELECT salario` por quem nao le `salario` RECUSA -- e nao devolve
/// `{"salario": null}` em toda linha.** O `varrer` do plano Simples saia
/// peneirado e a projecao punha `null` na coluna que a linha nao tinha:
/// certo cada um sozinho, mentira sobre o dado os dois juntos. O
/// `consultar` (visao, junção) ja recusava; este e o irmao.
#[test]
fn o_select_da_coluna_negada_recusa_em_vez_de_devolver_nulo() {
    let dir = dir_temp("select-nulo");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    for texto in [
        "SELECT salario FROM folha",
        "SELECT nome, salario FROM folha",
        "SELECT salario AS s FROM folha",
        // O DISTINCT entra NESTA lista, e nao num teste proprio: ele vai
        // parar noutra operacao (`agrupar` em vez de `varrer`), mas usa o
        // MESMO campo do plano -- a `Saida::Colunas` que
        // `recusar_projecao_sobre_coluna_negada` le. Irmao e quem chama
        // as mesmas funcoes na mesma ordem.
        "SELECT DISTINCT salario FROM folha",
        "SELECT DISTINCT nome, salario FROM folha",
    ] {
        let e = pede(
            &s,
            &ses,
            &format!(r#""op":"sql","database":"b","texto":"{texto}""#),
        )
        .expect_err("devolveu a coluna negada como nulo");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{texto}: {e}");
        assert!(e.to_string().contains("salario"), "{texto}: {e}");
    }
    // As colunas que ana LE continuam saindo pelo mesmo plano.
    let r = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"SELECT nome FROM folha""#,
    )
    .expect("a projecao de coluna permitida tinha de passar");
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas[0].texto_ou("nome", ""), "ana", "{r:?}");
    assert!(linhas[0].campo("salario").is_none());
    // E sem regra nenhuma nada muda: a mesma projecao devolve o dado.
    let dir2 = dir_temp("select-sem-regra");
    let (s, ses) = servidor(&dir2, sem_regra_de_coluna());
    let r = pede(
        &s,
        &ses,
        r#""op":"sql","database":"b","texto":"SELECT salario FROM folha""#,
    )
    .unwrap();
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas[0].inteiro_ou("salario", -1), 5000, "{r:?}");
}

/// **O indice PARCIAL cujo filtro cita a coluna negada e um oraculo.**
/// `porId` com `onde: "salario > 5000"` so tem quem ganha mais que isso:
/// varrer por ele devolve exatamente essa lista, e `buscar` por id
/// responde «este ganha mais que 5000?» -- sem o salario aparecer. A
/// conferencia do indice olhava so as colunas da CHAVE.
#[test]
fn o_indice_parcial_cujo_filtro_cita_a_coluna_negada_recusa() {
    let dir = dir_temp("indice-parcial");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let dono = Sessao::default();
    // Outra base, com uma `folha` que tem o indice parcial: a regra de
    // ana vale para `folha` em qualquer base.
    s.executar("criar_database", &pedido(r#"{"database":"b2"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b2","tabela":"folha",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"salario","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true},
                               {"nome":"so_ricos","colunas":["id"],
                                "onde":"salario > 5000"}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for (id, sal) in [(1, 3000), (2, 9000)] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b2","tabela":"folha",
                         "linha":{{"id":{id},"nome":"n","salario":{sal}}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    for corpo in [
        r#""op":"varrer","database":"b2","tabela":"folha","indice":"so_ricos""#,
        r#""op":"buscar","database":"b2","tabela":"folha","indice":"so_ricos","chave":[2]"#,
    ] {
        let e = pede(&s, &ses, corpo).expect_err("o indice parcial respondeu");
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{corpo}: {e}");
        let t = e.to_string();
        assert!(
            t.contains("salario") && t.contains("filtro do indice"),
            "{t}"
        );
    }
    // O indice de sempre continua servindo, sem a coluna.
    let r = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"b2","tabela":"folha","indice":"porId""#,
    )
    .expect("o indice sem filtro tinha de passar");
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas.len(), 2);
    assert!(linhas.iter().all(|l| l.campo("salario").is_none()), "{r:?}");
}

/// **`em` com `campo` na coluna negada RECUSA -- e nao responde «nenhum
/// casa» com `ok: true`.** O sub-pedido chega peneirado, sem a coluna, e
/// o `filter_map` engolia a ausencia: conjunto vazio, zero linhas, e a
/// pergunta «que clientes tem id igual a algum salario?» respondida com
/// um silencio que parecia resposta.
#[test]
fn o_em_com_campo_negado_recusa_em_vez_de_zero_linhas() {
    let dir = dir_temp("em-negado");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let e = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"clientes"},
               "em":[{"coluna":"id","de":{"op":"varrer","tabela":"folha"},
                      "campo":"salario"}]"#,
    )
    .expect_err("respondeu zero linhas calado");
    let t = e.to_string();
    assert!(t.contains("salario") && t.contains("\"em\""), "{t}");
    // O `campo` que ana LE continua filtrando: folha tem id 1, clientes
    // tem id 1.
    let r = pede(
        &s,
        &ses,
        r#""op":"consultar","database":"b","de":{"op":"varrer","tabela":"clientes"},
               "em":[{"coluna":"id","de":{"op":"varrer","tabela":"folha"},
                      "campo":"id"}]"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("devolvidas", -1), 1, "{r:?}");
}

// ------------------------------------------------------------- as excecoes

/// Supervisor passa por cima, como ja passa por cima da regra de tabela.
/// **E supervisor, e nao `nivel: admin`** -- o portao por tabela so abre
/// para o supervisor, e inventar um segundo caminho de excecao aqui faria
/// os dois portoes poderem discordar sobre a mesma pessoa.
#[test]
fn o_supervisor_ignora_o_direito_por_coluna() {
    let dir = dir_temp("super");
    let c = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","id":9,"supervisor":true,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"*":{"tabelas":{"folha":{
                   "colunas":{"salario":{"ler":false,"alterar":false}}}}}}}]}"#,
    ))
    .unwrap();
    let (s, ses) = servidor(&dir, c);
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(l.inteiro_ou("salario", -1), 5000);
    assert!(pede(
        &s,
        &ses,
        r#""op":"exportar","database":"b","tabela":"folha","formato":"json""#
    )
    .is_ok());
}

/// Direito que nao existe por coluna RECUSA a carga do cadastro, e a
/// recusa nomeia onde ele esta. Campo que ninguem le mente -- e mente
/// pior quando o assunto e quem alcanca o dado.
#[test]
fn o_cadastro_recusa_direito_que_nao_existe_por_coluna() {
    let e = Cadastro::de_json(&pedido(
        r#"{"usuarios":[{"login":"ana","senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{"b":{"tabelas":{"folha":{
                   "colunas":{"salario":{"excluir":false}}}}}}}]}"#,
    ))
    .expect_err("aceitou um direito que nao existe por coluna");
    let t = e.to_string();
    for pedaco in ["excluir", "salario", "folha", "ana"] {
        assert!(t.contains(pedaco), "a recusa nao diz {pedaco}: {t}");
    }
}

/// **A EXPRESSAO pergunta sem nomear campo `coluna` nenhum.**
///
/// `{"onde":[{"coluna":"salario",...}]}` ja recusava; `"expressao":
/// "salario >= 5000"` fazia a MESMA pergunta por outra porta, e a resposta
/// vinha na contagem: vinte perguntas dessas dizem o salario sem ele nunca
/// ter aparecido numa linha.
///
/// PROVA REAL: tirando o laco de `["expressao", "tendo"]` de
/// `recusar_pergunta_sobre_coluna_negada`, a primeira assercao volta a
/// receber `Ok` -- com `devolvidas: 1`, que E a resposta: «sim, alguem
/// aqui ganha 5.000 ou mais». A pergunta e escrita de proposito para
/// responder SIM sobre o dado gravado; uma que respondesse zero vazaria
/// igual, mas provaria menos.
#[test]
fn a_expressao_nao_pergunta_pela_coluna_negada() {
    let dir = dir_temp("expr");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());

    let e = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"b","tabela":"folha","expressao":"salario >= 5000""#,
    )
    .expect_err("a expressao respondeu sobre a coluna negada");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(e.to_string().contains("salario"), "{e}");

    // E o nome QUALIFICADO nao contorna: `p.salario` e a mesma coluna.
    let e = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"b","tabela":"folha","expressao":"p.salario >= 5000""#,
    )
    .expect_err("o nome qualificado contornou a regra");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");

    // Expressao sobre coluna PERMITIDA continua passando -- a guarda que
    // recusasse tudo seria pior que a guarda que falta.
    let ok = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"b","tabela":"folha","expressao":"id > 0""#,
    )
    .expect("a coluna permitida foi barrada");
    assert_eq!(ok.inteiro_ou("devolvidas", -1), 1);
    // E a peneira continua tirando a coluna da resposta.
    let l = ok.campo("linhas").and_then(Json::lista).unwrap();
    assert!(l[0].campo("salario").is_none(), "a coluna vazou: {l:?}");

    // Erro de SINTAXE na expressao nao vira «acesso negado»: quem digitou
    // errado tem de ler onde esta o erro, e nao procurar permissao.
    let e = pede(
        &s,
        &ses,
        r#""op":"varrer","database":"b","tabela":"folha","expressao":"id >""#,
    )
    .expect_err("expressao truncada tinha de recusar");
    assert_ne!(e.nome(), "ACESSO_NEGADO", "{e}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// **O `agrupar` RECUSA a tabela com regra de coluna, e nao peneira.**
///
/// Peneirar nao fecharia: `{"funcao":"maximo","coluna":"salario"}` devolve
/// o maior salario num campo chamado `maximo_salario`, e a peneira procura
/// pelo NOME da coluna. Recusar e mais seguro que vazar, e a recusa diz o
/// nome da operacao.
///
/// O controle na mesma corrida: sem a regra de coluna, o mesmo pedido
/// passa. Sem ele, um `agrupar` quebrado por outro motivo faria este teste
/// passar por engano.
#[test]
fn o_agrupar_recusa_a_tabela_com_regra_de_coluna() {
    let dir = dir_temp("agrupar-coluna");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let corpo = r#""op":"agrupar","database":"b","tabela":"folha",
                       "agregados":[{"funcao":"maximo","coluna":"salario"}]"#;
    let e = pede(&s, &ses, corpo).expect_err("o agrupar devolveu o salario resumido");
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(
        e.to_string().contains("agrupar"),
        "a recusa nao se nomeia: {e}"
    );
    // A tabela SEM regra de coluna continua agrupando.
    let ok = pede(
        &s,
        &ses,
        r#""op":"agrupar","database":"b","tabela":"clientes","por":["nome"]"#,
    )
    .expect("a tabela sem regra de coluna foi barrada");
    assert_eq!(ok.inteiro_ou("grupos", -1), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// **O defeito do pedido 343.** A resposta do `motivos` traz `identidade`,
/// que e a chave primaria em TEXTO CLARO (`Table::identidade_de_valores`).
/// Com o `cpf` como chave primaria e marcado, quem nao le a coluna
/// recebia `"identidade":"cpf=01234567890"` para cada linha excluida, com
/// data, hora e autor ao lado.
///
/// A classe era `Nenhum` com a justificativa do `excluir` ao lado --
/// «mexe na LINHA inteira, nao ha coluna no pedido» --, e ela e verdadeira
/// para o `excluir` e herdada por vizinhanca aqui: o `PorColuna` pergunta
/// por onde a RESPOSTA devolve dado de linha, nao o que o pedido recebe.
/// Os tres irmaos que devolvem passado -- `lixeira`, `trilha`, `diario` --
/// ja eram `Recusa`, cada um com o motivo escrito; o quarto caiu no bloco
/// do `Nenhum`.
///
/// Reponha o defeito trocando a classe de `motivos` para
/// `PorColuna::Nenhum`: o `match` do meio passa a cair no `Ok` e o
/// `panic!` imprime o CPF.
#[test]
fn o_motivos_nao_entrega_a_chave_primaria_marcada() {
    let dir = dir_temp("motivos-identidade");
    // A tabela nasce e a linha morre por um servidor SEM regra nenhuma: o
    // `.reason` e o dado que ja esta em disco quando a guarda entra, e a
    // regra so vale do arranque seguinte -- com a tabela ja gravada, que
    // e onde o cadastro confere se a coluna existe.
    {
        let (s, ses) = servidor(&dir, sem_regra_de_coluna());
        let dono = Sessao::default();
        s.executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"pessoas",
                        "colunas":[{"nome":"cpf","tipo":"Str(11)","obrigatoria":true},
                                   {"nome":"nome","tipo":"Str(20)"}],
                        "indices":[{"nome":"porCpf","colunas":["cpf"],"unico":true,
                                    "primario":true}]}"#,
            ),
            &dono,
        )
        .unwrap();
        s.executar(
            "inserir",
            &pedido(
                r#"{"database":"b","tabela":"pessoas",
                        "linha":{"cpf":"01234567890","nome":"ana"}}"#,
            ),
            &dono,
        )
        .unwrap();
        s.executar(
            "excluir",
            &pedido(
                r#"{"database":"b","tabela":"pessoas","rowid":1,
                        "motivo":"pedido do titular"}"#,
            ),
            &dono,
        )
        .unwrap();

        // **O COMPORTAMENTO VELHO, na mesma corrida.** A MESMA Ana, com o
        // MESMO cadastro sem o campo `"colunas"`, continua lendo os
        // motivos com o CPF dentro. A guarda entra pedida: quem nao tem
        // regra de coluna nenhuma nao muda de comportamento.
        let velho = pede(
            &s,
            &ses,
            r#""op":"motivos","database":"b","tabela":"pessoas""#,
        )
        .expect("quem nao tem regra de coluna parou de ler os motivos");
        assert!(
            velho.escrever().contains("cpf=01234567890"),
            "o .reason nao guardou a identidade, e sem ela nao ha vazamento \
                 a provar: {}",
            velho.escrever()
        );
    }

    // O segundo arranque, do MESMO diretorio, agora com o `cpf` marcado.
    let com_regra = a_chave_primaria_marcada();
    let s = Servidor::novo(config_em(&dir, com_regra.clone()))
        .expect("recusou regra sobre coluna que existe");
    let ses = Sessao {
        usuario: com_regra.por_login("ana").cloned(),
        ..Sessao::default()
    };

    match pede(
        &s,
        &ses,
        r#""op":"motivos","database":"b","tabela":"pessoas""#,
    ) {
        Ok(j) => panic!(
            "o motivos entregou a chave primaria marcada em texto claro: {}",
            j.escrever()
        ),
        Err(e) => {
            assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
            // **Contra o lastro.** Tabela negada e falta de `administrar`
            // recusariam com OUTRO texto, e o teste passaria por engano.
            // Estas duas frases so saem do ramo `PorColuna::Recusa`.
            let t = e.to_string();
            for pedaco in [
                "o direito por coluna nao e aplicado em motivos",
                "ha coluna negada em b.pessoas",
            ] {
                assert!(
                    t.contains(pedaco),
                    "a recusa nao veio da classe Recusa do direito por coluna: {t}"
                );
            }
        }
    }

    // **O CONTROLE POSITIVO.** A MESMA sessao continua lendo os motivos da
    // tabela que nao tem regra de coluna. Guarda que recusa tudo e pior
    // que a guarda que faltava.
    let ok = pede(
        &s,
        &ses,
        r#""op":"motivos","database":"b","tabela":"clientes""#,
    )
    .expect("a tabela sem regra de coluna foi barrada");
    assert_eq!(ok.inteiro_ou("total", -1), 0, "{}", ok.escrever());

    // E o apelido recusa junto: `reasons` chega ao portao como `motivos`
    // chega, e classificar so o canonico deixaria a porta dos fundos
    // aberta com outro nome.
    let e = pede(
        &s,
        &ses,
        r#""op":"reasons","database":"b","tabela":"pessoas""#,
    )
    .expect_err("o apelido entregou o que o nome canonico recusa");
    assert!(e.to_string().contains("nao e aplicado em reasons"), "{e}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// A ficha mostra as regras de coluna: administrador que nao consegue ver
/// o que concedeu acaba concedendo duas vezes.
#[test]
fn a_ficha_do_usuario_mostra_as_regras_de_coluna() {
    let c = so_a_folha_tem_regra();
    let f = c.por_login("ana").unwrap().ficha();
    let d = f
        .campo("colunas")
        .and_then(|x| x.campo("*"))
        .and_then(|x| x.campo("folha"))
        .and_then(|x| x.campo("salario"))
        .expect("a ficha nao mostra a regra de coluna");
    assert!(!d.booleano_ou("ler", true));
    assert!(!d.booleano_ou("alterar", true));
}

// -------------------------- a carga confere a coluna contra o esquema

/// Um servidor nascido de um `config.json` DE VERDADE, com `b.folha`
/// (id, nome, salario) ja em disco. As tres operacoes de cadastro gravam
/// no arquivo, e a conferencia de coluna precisa de tabela em disco: sem
/// os dois nao ha o que exercitar.
fn servidor_de_arquivo(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = dir_temp(nome);
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{"token":"t","bind":"127.0.0.1:5399","base":"{}",
                    "usuarios":[{{"id":1,"login":"ana","supervisor":true,
                                  "senha_hash":"pbkdf2-sha256$1000$00$00"}}]}}"#,
            dir.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"folha",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"},
                               {"nome":"salario","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                                "primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    (s, dir)
}

/// A sessao da ana, supervisora, que e quem pode mexer no cadastro.
fn como_ana(s: &Servidor) -> Sessao {
    Sessao {
        usuario: s.cadastro().por_login("ana").cloned(),
        ..Sessao::default()
    }
}

/// `usuario_criar` da bia, com UMA regra de coluna em `b.<tabela>`.
fn criar_bia_com_regra_em(s: &Arc<Servidor>, tabela: &str, coluna: &str) -> Result<Json> {
    s.executar(
        "usuario_criar",
        &pedido(&format!(
            r#"{{"login":"bia","senha":"segredo-da-bia","nivel":"leitor",
                    "bases":{{"b":{{"ler":true,"tabelas":{{"{tabela}":{{"ler":true,
                    "colunas":{{"{coluna}":{{"ler":false,"alterar":false}}}}}}}}}}}}}}"#
        )),
        &como_ana(s),
    )
}

/// **O defeito do pedido 235.** `"colunas": {"salrio": …}` carregava
/// calado, e `salario` continuava sem regra: quem escreveu o cadastro
/// achava que restringiu e nao restringiu nada. Com a tabela em disco, o
/// arranque RECUSA -- nomeando usuario, base, tabela e coluna, e listando
/// as colunas que existem, para o erro de digitacao se achar.
#[test]
fn arranque_recusa_regra_que_cita_coluna_que_a_tabela_nao_tem() {
    let dir = dir_temp("typo-arranque");
    // As tabelas nascem por um servidor sem regra nenhuma; ele morre, e
    // o segundo sobe do MESMO diretorio com a regra errada.
    drop(servidor(&dir, sem_regra_de_coluna()));
    let com_typo = cadastro(
        r#"{"b":{"ler":true,"tabelas":{"folha":{"ler":true,
                 "colunas":{"salrio":{"ler":false,"alterar":false}}}}}}"#,
    );
    let e = Servidor::novo(config_em(&dir, com_typo))
        .err()
        .expect("subiu com uma regra que cita coluna que a tabela nao tem");
    let texto = e.to_string();
    for pedaco in ["ana", "b.folha", "salrio", "salario"] {
        assert!(
            texto.contains(pedaco),
            "a recusa nao diz {pedaco:?}: {texto}"
        );
    }
}

/// O controle, na mesma corrida: a MESMA regra com a coluna escrita certa
/// sobe sem aviso -- e continua valendo com a tabela ja em disco.
#[test]
fn arranque_aceita_regra_cuja_coluna_existe_e_ela_continua_valendo() {
    let dir = dir_temp("certa-arranque");
    drop(servidor(&dir, sem_regra_de_coluna()));
    let certa = cadastro(
        r#"{"b":{"ler":true,"tabelas":{"folha":{"ler":true,
                 "colunas":{"salario":{"ler":false,"alterar":false}}}}}}"#,
    );
    let s = Servidor::novo(config_em(&dir, certa.clone())).expect("recusou coluna que existe");
    assert!(
        s.config().cadastro.avisos.is_empty(),
        "{:?}",
        s.config().cadastro.avisos
    );
    let ses = Sessao {
        usuario: certa.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let l = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert!(l.campo("salario").is_none(), "a coluna negada saiu: {l:?}");
}

/// A tabela que ainda NAO existe aceita, com aviso -- a mesma decisao da
/// chave estrangeira declarada antes da tabela: e ordem legitima de
/// modelagem. O aviso nomeia usuario e tabela e entra na lista do
/// cadastro, que e a que o arranque imprime.
#[test]
fn arranque_aceita_regra_sobre_tabela_que_ainda_nao_existe_e_avisa() {
    let dir = dir_temp("futura-arranque");
    drop(servidor(&dir, sem_regra_de_coluna()));
    let futura = cadastro(
        r#"{"b":{"ler":true,"tabelas":{"bonus":{"ler":true,
                 "colunas":{"valor":{"ler":false}}}}}}"#,
    );
    let s = Servidor::novo(config_em(&dir, futura)).expect("recusou tabela que ainda nao existe");
    let avisos = &s.config().cadastro.avisos;
    assert_eq!(avisos.len(), 1, "{avisos:?}");
    assert!(
        avisos[0].contains("b.bonus") && avisos[0].contains("ana"),
        "{avisos:?}"
    );
}

/// As tres operacoes de cadastro passam pela MESMA conferencia do
/// arranque: `usuario_criar` com a regra errada recusa nomeando -- e nao
/// grava nada, nem no arquivo nem no cadastro vivo.
#[test]
fn usuario_criar_recusa_regra_que_cita_coluna_que_a_tabela_nao_tem() {
    let (s, dir) = servidor_de_arquivo("typo-op");
    let e = criar_bia_com_regra_em(&s, "folha", "salrio").unwrap_err();
    let texto = e.to_string();
    for pedaco in ["bia", "b.folha", "salrio", "salario"] {
        assert!(
            texto.contains(pedaco),
            "a recusa nao diz {pedaco:?}: {texto}"
        );
    }
    let arquivo = std::fs::read_to_string(dir.join("config.json")).unwrap();
    assert!(
        !arquivo.contains("bia"),
        "a recusa gravou assim mesmo: {arquivo}"
    );
    assert!(s.cadastro().por_login("bia").is_none());
}

/// O controle: a mesma regra com a coluna certa grava sem aviso; e a regra
/// sobre tabela que ainda nao existe grava COM aviso na resposta -- e a
/// unica porta por onde quem chamou a op fica sabendo.
#[test]
fn usuario_criar_aceita_coluna_que_existe_e_avisa_da_tabela_futura() {
    let (s, _dir) = servidor_de_arquivo("certa-op");
    let r = criar_bia_com_regra_em(&s, "folha", "salario").expect("recusou coluna que existe");
    assert!(r.campo("avisos").is_none(), "{}", r.escrever());

    let r = s
        .executar(
            "usuario_alterar",
            &pedido(
                r#"{"login":"bia","bases":{"b":{"ler":true,"tabelas":{"bonus":{"ler":true,
                        "colunas":{"valor":{"ler":false}}}}}}}"#,
            ),
            &como_ana(&s),
        )
        .expect("recusou tabela que ainda nao existe");
    let avisos = r
        .campo("avisos")
        .and_then(Json::lista)
        .expect("a tabela futura passou sem aviso na resposta");
    assert_eq!(avisos.len(), 1, "{}", r.escrever());
    assert!(
        avisos[0].texto().unwrap().contains("b.bonus"),
        "{}",
        r.escrever()
    );
}

/// A tabela que nasce DEPOIS, com uma regra ja cadastrada citando coluna
/// que ela nao tem: a regra e inerte, e o `criar_tabela` diz isso na
/// resposta -- em vez de deixar quem modelou achar que a coluna esta
/// protegida.
#[test]
fn criar_tabela_avisa_da_regra_de_coluna_que_ficou_inerte() {
    let dir = dir_temp("nasce-depois");
    let (s, _) = servidor(
        &dir,
        cadastro(
            r#"{"b":{"ler":true,"criar":true,"tabelas":{"bonus":{"ler":true,
                     "colunas":{"valr":{"ler":false}}}}}}"#,
        ),
    );
    let r = s
        .executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"bonus",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                                   {"nome":"valor","tipo":"Int4"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    let avisos = r
        .campo("avisos")
        .and_then(Json::lista)
        .expect("a regra inerte passou calada");
    let texto = avisos[0].texto().unwrap();
    assert!(
        texto.contains("valr") && texto.contains("ana") && texto.contains("valor"),
        "{texto}"
    );
}

/// E o controle: regra cuja coluna a tabela nova TEM nao gera aviso, e a
/// resposta do `criar_tabela` fica exatamente como era.
#[test]
fn criar_tabela_nao_avisa_quando_a_regra_casa_com_a_coluna() {
    let dir = dir_temp("nasce-certa");
    let (s, _) = servidor(
        &dir,
        cadastro(
            r#"{"b":{"ler":true,"criar":true,"tabelas":{"bonus":{"ler":true,
                     "colunas":{"valor":{"ler":false}}}}}}"#,
        ),
    );
    let r = s
        .executar(
            "criar_tabela",
            &pedido(
                r#"{"database":"b","tabela":"bonus",
                        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                                   {"nome":"valor","tipo":"Int4"}]}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    assert!(r.campo("avisos").is_none(), "{}", r.escrever());
}

// ------------------------------------- revisao SEC do 245 O2b, achado A1

/// **A calculada que cita coluna negada e recusada na declaracao.** Ana
/// tem `administrar` na folha e `salario` negado; `acrescentar_coluna`
/// com `calculada: "salario"` copiaria todo salario para `x`, que ela le.
/// O CHECK que cita `salario` tambem recusa: ele contaria as linhas por
/// ele («2 das 4»). A folha fica com as colunas que tinha.
///
/// O controle: a MESMA calculada sobre `nome`, que Ana le, entra.
///
/// Reponha o defeito tirando a conferencia `definicao_cita_negada` do
/// ramo `MarcaDagua`: a coluna entra e o `varrer` devolve 5000 em `x`.
#[test]
fn calculada_que_cita_coluna_negada_e_recusada_na_declaracao() {
    let dir = dir_temp("sec-a1-declara");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    let colunas = |s: &Arc<Servidor>| {
        s.executar(
            "esquema",
            &pedido(r#"{"database":"b","tabela":"folha"}"#),
            &Sessao::default(),
        )
        .unwrap()
        .campo("colunas")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap()
    };
    let antes = colunas(&s);
    for def in [
        r#"{"nome":"x","tipo":"Int4","calculada":"salario"}"#,
        r#"{"nome":"x","tipo":"Int4","calculada":"SALARIO + 0"}"#,
        r#"{"nome":"x","tipo":"Int4","check":"salario > 4000"}"#,
    ] {
        let e = pede(
            &s,
            &ses,
            &format!(r#""op":"acrescentar_coluna","database":"b","tabela":"folha","coluna":{def}"#),
        )
        .expect_err(def)
        .to_string();
        assert!(
            e.to_lowercase().contains("salario") && e.contains("x"),
            "{def}: {e}"
        );
    }
    assert_eq!(antes, colunas(&s), "uma recusa mexeu na tabela");
    let r = pede(&s, &ses, r#""op":"varrer","database":"b","tabela":"folha""#).unwrap();
    assert!(!r.escrever().contains("5000"), "{}", r.escrever());

    // CONTROLE: a calculada sobre o que Ana le entra, e se le.
    pede(
        &s,
        &ses,
        r#""op":"acrescentar_coluna","database":"b","tabela":"folha",
               "coluna":{"nome":"y","tipo":"Str(20)","calculada":"UPPER(nome)"}"#,
    )
    .unwrap();
    let r = pede(
        &s,
        &ses,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(r.texto_ou("y", ""), "ANA", "{}", r.escrever());
}

/// **A calculada DERIVADA de coluna negada sai da leitura junto.** O dono
/// (sem restricao) declara `x = salario`; Ana, que nao le `salario`, nao
/// le `x` -- e o salario por outro nome. Pela leitura e pela varredura,
/// e o filtro por `x` recusa como o filtro por `salario`.
///
/// Reponha o defeito fazendo `derivadas_de_negadas` devolver vazio: o
/// `varrer` de Ana devolve 5000.
#[test]
fn calculada_derivada_de_coluna_negada_nao_se_le() {
    let dir = dir_temp("sec-a1-le");
    let (s, ses) = servidor(&dir, so_a_folha_tem_regra());
    s.executar(
        "acrescentar_coluna",
        &pedido(
            r#"{"database":"b","tabela":"folha",
                    "coluna":{"nome":"x","tipo":"Int4","calculada":"salario"}}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    // O dono le o valor preenchido: e isso que Ana NAO pode ver.
    let dono = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"folha","rowid":1}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(dono.inteiro_ou("x", -1), 5000);

    for corpo in [
        r#""op":"varrer","database":"b","tabela":"folha""#,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    ] {
        let r = pede(&s, &ses, corpo).unwrap();
        assert!(!r.escrever().contains("5000"), "{corpo}: {}", r.escrever());
        assert!(r.escrever().contains("ana"), "{corpo}: {}", r.escrever());
    }
    // E o `esquema` diz a Ana que `x` nao chega, como diz de `salario`.
    let e = pede(
        &s,
        &ses,
        r#""op":"esquema","database":"b","tabela":"folha""#,
    )
    .unwrap();
    let sem_ler = e.campo("colunas_sem_leitura").unwrap().escrever();
    assert!(sem_ler.contains("\"x\""), "{sem_ler}");

    // CONTROLE: sem regra de coluna, Ana le `x` como sempre.
    let dir2 = dir_temp("sec-a1-le-controle");
    let (s2, ses2) = servidor(&dir2, sem_regra_de_coluna());
    s2.executar(
        "acrescentar_coluna",
        &pedido(
            r#"{"database":"b","tabela":"folha",
                    "coluna":{"nome":"x","tipo":"Int4","calculada":"salario"}}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();
    let r = pede(
        &s2,
        &ses2,
        r#""op":"ler","database":"b","tabela":"folha","rowid":1"#,
    )
    .unwrap();
    assert_eq!(r.inteiro_ou("x", -1), 5000);
}
