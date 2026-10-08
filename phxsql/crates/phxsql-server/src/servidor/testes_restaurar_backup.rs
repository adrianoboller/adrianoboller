use super::*;
use crate::usuarios::Cadastro;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("rst-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn servidor(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let c = Config {
        base: dir.join("dados"),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        cadastro,
        ..Config::default()
    };
    Servidor::novo(c).unwrap()
}

/// Dois bancos: `Comercial` com tres linhas e `Folha`, que existe para o
/// teste de permissao ter o que negar.
fn com_dados(dir: &std::path::Path, cadastro: Cadastro) -> Arc<Servidor> {
    let s = servidor(dir, cadastro);
    let ses = Sessao::default();
    for db in ["Comercial", "Folha"] {
        s.executar(
            "criar_database",
            &pedido(&format!(r#"{{"database":"{db}"}}"#)),
            &ses,
        )
        .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"{db}","tabela":"clientes",
                        "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}},
                                   {{"nome":"nome","tipo":"Str(20)"}},
                                   {{"nome":"cidade","tipo":"Str(20)"}}],
                        "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}}]}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    for (id, nome, cidade) in [
        (1, "Adriano", "Blumenau"),
        (2, "Maria", "Joinville"),
        (3, "Joao", "Itajai"),
    ] {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"Comercial","tabela":"clientes",
                         "linha":{{"id":{id},"nome":"{nome}","cidade":"{cidade}"}}}}"#
            )),
            &ses,
        )
        .unwrap();
    }
    s
}

/// O backup em ZIP do banco `Comercial`, no diretorio de copias.
fn backup_zip(s: &Arc<Servidor>, dir: &std::path::Path) -> String {
    let r = s
        .executar(
            "backup",
            &pedido(&format!(
                r#"{{"destino":"{}","database":"Comercial","zip":true}}"#,
                dir.join("copias").display()
            )),
            &Sessao::default(),
        )
        .unwrap();
    r.texto_ou("arquivo", "").to_string()
}

fn checksum(s: &Arc<Servidor>, db: &str) -> String {
    s.executar(
        "checksum",
        &pedido(&format!(r#"{{"database":"{db}","tabela":"clientes"}}"#)),
        &Sessao::default(),
    )
    .unwrap()
    .texto_ou("checksum", "?")
    .to_string()
}

/// **O caminho principal.** Restaurar com outro nome cria um database
/// integro, sem parar o servico e sem tocar no original.
#[test]
fn restaurar_com_outro_nome_cria_o_banco_integro() {
    let dir = dir_temp("novonome");
    let s = com_dados(&dir, Cadastro::default());
    let zip = backup_zip(&s, &dir);
    // O original muda DEPOIS do backup: se a restauracao copiasse do banco
    // vivo em vez do arquivo, este registro apareceria no restaurado.
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"Comercial","tabela":"clientes",
                    "linha":{"id":4,"nome":"Depois do backup","cidade":"Gaspar"}}"#,
        ),
        &Sessao::default(),
    )
    .unwrap();

    let r = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(
                r#"{{"origem":"{zip}","database":"Comercial_de_ontem"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.texto_ou("de", ""), "Comercial");
    assert_eq!(r.texto_ou("modo", ""), "novo");
    assert!(matches!(r.campo("substituiu"), Some(Json::Bool(false))));
    assert_eq!(
        r.campo("tabelas").and_then(Json::lista).unwrap().len(),
        1,
        "{}",
        r.escrever()
    );

    // O dado restaurado e o do BACKUP: tres linhas, e nao quatro.
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"Comercial_de_ontem","tabela":"clientes"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(v.inteiro_ou("devolvidas", -1), 3);
    let linhas = v.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas[0].texto_ou("nome", ""), "Adriano");
    assert_eq!(linhas[2].texto_ou("cidade", ""), "Itajai");

    // O original continua com as quatro, intocado.
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"Comercial","tabela":"clientes"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(v.inteiro_ou("devolvidas", -1), 4);

    // E a tabela restaurada esta VIVA, e nao so no disco: da para gravar
    // nela, o que exige o `.ndx` inteiro e o esquema legivel.
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"Comercial_de_ontem","tabela":"clientes",
                    "linha":{"id":9,"nome":"Depois de restaurar","cidade":"Brusque"}}"#,
        ),
        &Sessao::default(),
    )
    .expect("a tabela restaurada tem de aceitar escrita");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A prova mais forte de que o restaurado e o mesmo dado: a soma de
/// verificacao, que depende da ORDEM DE DIGITACAO, bate com a do original.
#[test]
fn o_restaurado_tem_a_mesma_soma_de_verificacao() {
    let dir = dir_temp("checksum");
    let s = com_dados(&dir, Cadastro::default());
    let antes = checksum(&s, "Comercial");
    let zip = backup_zip(&s, &dir);
    s.executar(
        "restaurar_backup",
        &pedido(&format!(r#"{{"origem":"{zip}","database":"Copia"}}"#)),
        &Sessao::default(),
    )
    .unwrap();
    assert_eq!(antes, checksum(&s, "Copia"), "o dado restaurado difere");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Manifesto adulterado e RECUSADO, e nada e escrito.** Mesmo tamanho e
/// conteudo diferente: so o SHA-256 pega.
#[test]
fn backup_adulterado_nao_vira_database() {
    let dir = dir_temp("adulterado");
    let s = com_dados(&dir, Cadastro::default());
    let copia = dir.join("copia");
    s.executar(
        "backup",
        &pedido(&format!(r#"{{"destino":"{}"}}"#, copia.display())),
        &Sessao::default(),
    )
    .unwrap();

    let alvo = copia.join("Comercial/clientes.reg");
    let mut bytes = std::fs::read(&alvo).unwrap();
    let n = bytes.len() / 2;
    bytes[n] ^= 0xff;
    std::fs::write(&alvo, &bytes).unwrap();

    let e = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(
                r#"{{"origem":"{}","de":"Comercial","database":"Suspeito"}}"#,
                copia.display()
            )),
            &Sessao::default(),
        )
        .unwrap_err();
    assert_eq!(e.nome(), "CORROMPIDO", "veio {e}");
    assert!(e.to_string().contains("SHA-256"), "{e}");
    assert!(
        !s.config.base.join("Suspeito").exists(),
        "o backup podre comecou a virar database"
    );
    // E o `conferir_backup`, que ja existia, aponta o mesmo arquivo.
    let c = s
        .executar(
            "conferir_backup",
            &pedido(&format!(r#"{{"destino":"{}"}}"#, copia.display())),
            &Sessao::default(),
        )
        .unwrap();
    assert!(matches!(c.campo("integro"), Some(Json::Bool(false))));
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Por cima com o servico no ar e recusado**, e a recusa diz o que fazer.
#[test]
fn por_cima_com_a_porta_de_dados_no_ar_e_recusado() {
    let dir = dir_temp("noar");
    let s = com_dados(&dir, Cadastro::default());
    let zip = backup_zip(&s, &dir);
    // O que `escutar` faria: a porta de dados esta atendendo.
    s.porta_no_ar.store(true, Ordering::SeqCst);

    let e = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(
                r#"{{"origem":"{zip}","database":"Comercial",
                         "modo":"por_cima","confirmar":true}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("servico_parar"), "veio {e}");
    assert_eq!(
        checksum(&s, "Comercial"),
        checksum(&s, "Comercial"),
        "o banco continua legivel"
    );

    // Com a porta parada, a mesma restauracao passa.
    s.porta_no_ar.store(false, Ordering::SeqCst);
    let r = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(
                r#"{{"origem":"{zip}","database":"Comercial",
                         "modo":"por_cima","confirmar":true}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert!(matches!(r.campo("substituiu"), Some(Json::Bool(true))));
    let guardado = r.texto_ou("anterior_em", "");
    assert!(!guardado.is_empty(), "nao disse onde ficou o anterior");
    assert!(
        std::path::Path::new(guardado).is_dir(),
        "o database substituido foi apagado: {guardado}"
    );
    let _ = std::fs::remove_dir_all(guardado);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Substituir um database inteiro nao acontece por engano de campo.
#[test]
fn por_cima_sem_confirmar_e_recusado() {
    let dir = dir_temp("semconfirmar");
    let s = com_dados(&dir, Cadastro::default());
    let zip = backup_zip(&s, &dir);
    let e = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(
                r#"{{"origem":"{zip}","database":"Comercial","modo":"por_cima"}}"#
            )),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("confirmar"), "{e}");
    // E o modo `novo` num nome que ja existe tambem recusa, dizendo o
    // caminho: nome ocupado nao vira substituicao silenciosa.
    let e = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(r#"{{"origem":"{zip}","database":"Comercial"}}"#)),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("ja existe"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A simulacao le, confere o manifesto e nao escreve nada -- e o que a
/// tela usa para mostrar o conteudo antes de alguem decidir.
#[test]
fn simular_mostra_o_conteudo_sem_escrever() {
    let dir = dir_temp("simular");
    let s = com_dados(&dir, Cadastro::default());
    let zip = backup_zip(&s, &dir);
    let r = s
        .executar(
            "restaurar_backup",
            &pedido(&format!(r#"{{"origem":"{zip}","simular":true}}"#)),
            &Sessao::default(),
        )
        .unwrap();
    assert!(matches!(r.campo("simulado"), Some(Json::Bool(true))));
    assert_eq!(r.texto_ou("de", ""), "Comercial");
    assert!(r.inteiro_ou("arquivos", 0) >= 7, "{}", r.escrever());
    let tabelas: Vec<String> = r
        .campo("tabelas")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(|t| t.texto().map(str::to_string))
        .collect();
    assert_eq!(tabelas, vec!["clientes".to_string()]);
    assert!(matches!(
        r.campo("escopo_declarado"),
        Some(Json::Bool(true))
    ));

    // A lista da pasta enxerga o mesmo arquivo.
    let l = s
        .executar(
            "backups",
            &pedido(&format!(
                r#"{{"pasta":"{}"}}"#,
                dir.join("copias").display()
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(l.inteiro_ou("total", -1), 1);
    let primeiro = &l.campo("backups").and_then(Json::lista).unwrap()[0];
    assert!(matches!(primeiro.campo("legivel"), Some(Json::Bool(true))));
    assert!(primeiro.inteiro_ou("no_disco", 0) > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------ permissao

/// Ana administra o servidor inteiro MENOS um banco: e a regra por base
/// substituindo a `"*"`, que ja era o comportamento do cadastro.
fn cadastro_admin_menos(negado: &str) -> Cadastro {
    Cadastro::de_json(&pedido(&format!(
        r#"{{"usuarios":[{{"login":"ana","id":7,
                 "senha_hash":"pbkdf2-sha256$1000$00$00",
                 "bases":{{"*":{{"administrar":true,"ler":true,"criar":true,
                                 "inserir":true}},
                           "{negado}":{{}}}}}}]}}"#
    )))
    .unwrap()
}

fn como_ana(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut ses = Sessao {
        usuario: s.config.cadastro.por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// **A porta dos fundos que o portao geral nao ve.** O portao confere o
/// campo `"database"`, que na restauracao e o DESTINO. Quem administra so
/// um banco de rascunho nao pode despejar dentro dele o backup da folha e
/// ler tudo por outra porta.
#[test]
fn o_backup_do_banco_alheio_nao_entra_por_um_destino_permitido() {
    let dir = dir_temp("portadosfundos");
    let s = com_dados(&dir, cadastro_admin_menos("Folha"));
    // Um backup da raiz inteira: dentro dele estao Comercial e Folha.
    let copia = dir.join("copia");
    s.executar(
        "backup",
        &pedido(&format!(r#"{{"destino":"{}"}}"#, copia.display())),
        &Sessao::default(),
    )
    .unwrap();

    // O banco que ela administra passa pelos dois portoes.
    como_ana(
        &s,
        &format!(
            r#""op":"restaurar_backup","origem":"{}","de":"Comercial","database":"Rascunho""#,
            copia.display()
        ),
    )
    .expect("o portao proprio nao pode barrar quem tem o direito");

    // O outro nao passa -- nem entrando por um destino que ela administra.
    let Err(e) = como_ana(
        &s,
        &format!(
            r#""op":"restaurar_backup","origem":"{}","de":"Folha","database":"Rascunho2""#,
            copia.display()
        ),
    ) else {
        panic!("o backup do banco negado entrou por um destino permitido");
    };
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(e.to_string().contains("Folha"), "a recusa diz qual: {e}");
    assert!(
        !s.config.base.join("Rascunho2").exists(),
        "recusou depois de ja ter escrito"
    );

    // E a simulacao tambem nao serve de espelho: ela mostraria as tabelas
    // da folha para quem nao pode ve-las.
    assert!(como_ana(
        &s,
        &format!(
            r#""op":"restaurar_backup","origem":"{}","de":"Folha","simular":true"#,
            copia.display()
        ),
    )
    .is_err());

    // A lista da pasta esconde o que ela nao poderia restaurar, e diz
    // quantos escondeu -- em vez de sumir com o arquivo inteiro.
    let l = como_ana(
        &s,
        &format!(r#""op":"backups","pasta":"{}""#, dir.display()),
    )
    .unwrap();
    let texto = l.escrever();
    assert!(
        !texto.contains("\"Folha\""),
        "vazou o banco negado: {texto}"
    );
    assert!(l.inteiro_ou("escondidos", 0) > 0, "{texto}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **O comportamento VELHO.** Quem nao chama a operacao nova nao ve
/// diferenca nenhuma: o `restaurar` de LINHA continua sendo o que era, o
/// backup continua conferindo, e o manifesto com os campos novos e lido
/// pelo `conferir_backup` de sempre.
#[test]
fn quem_nao_usa_a_operacao_nova_nao_ve_diferenca() {
    let dir = dir_temp("velho");
    let s = com_dados(&dir, Cadastro::default());
    let ses = Sessao::default();

    // 1. `restaurar` continua sendo desfazer uma exclusao de LINHA -- o
    //    nome nao mudou de dono.
    s.executar(
        "excluir",
        &pedido(r#"{"database":"Comercial","tabela":"clientes","rowid":2,"motivo":"engano"}"#),
        &ses,
    )
    .unwrap();
    let r = s
        .executar(
            "restaurar",
            &pedido(r#"{"database":"Comercial","tabela":"clientes","rowid":2,"motivo":"voltou"}"#),
            &ses,
        )
        .unwrap();
    assert!(matches!(r.campo("restaurado"), Some(Json::Bool(true))));
    assert_eq!(
        Atividade::da_operacao("restaurar"),
        Some(Atividade::Excluir),
        "o poder exigido pelo restaurar de linha mudou"
    );

    // 2. Backup e conferencia, exatamente como antes -- com o manifesto
    //    ja trazendo os campos novos dentro.
    let copia = dir.join("copia");
    let b = s
        .executar(
            "backup",
            &pedido(&format!(r#"{{"destino":"{}"}}"#, copia.display())),
            &ses,
        )
        .unwrap();
    assert!(b.inteiro_ou("arquivos", 0) > 0);
    let c = s
        .executar(
            "conferir_backup",
            &pedido(&format!(r#"{{"destino":"{}"}}"#, copia.display())),
            &ses,
        )
        .unwrap();
    assert!(
        matches!(c.campo("integro"), Some(Json::Bool(true))),
        "{}",
        c.escrever()
    );

    // 3. E a raiz de dados continua tendo os dois bancos de sempre: a
    //    operacao nova nao deixa nada para tras no disco.
    let bancos = s.executar("bancos", &pedido("{}"), &ses).unwrap();
    let nomes: Vec<String> = Json::lista(&bancos)
        .unwrap()
        .iter()
        .filter_map(|b| b.texto().map(str::to_string))
        .collect();
    assert_eq!(nomes, vec!["Comercial".to_string(), "Folha".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}
