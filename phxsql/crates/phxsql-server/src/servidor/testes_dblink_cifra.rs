//! O QUARTO lugar da cifra do DbLink: o salvar pela tela (pedido 378).
//!
//! Os outros tres -- o campo, a recusa por motor e o `cifrar` antes do
//! `autenticar` -- moram em `dblink::`. Este mora aqui porque e o encontro do
//! cadastro com a tela, e e o unico em que a falta nao aparece como erro:
//! aparece como uma ligacao que continua anunciando «cifrada» com o tunel sem
//! ancora.
use super::*;
use crate::dblink::Definicao;

/// Um pino valido: 32 bytes em hexadecimal.
const PINO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn servidor(dir: &std::path::Path) -> std::sync::Arc<Servidor> {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    // O escape escrito da ENTRADA deste servidor: o que se mede aqui e o
    // cadastro, nao o aperto de mao do soquete.
    c.cifra_fio.exigir = false;
    Servidor::novo(c).unwrap()
}

fn salvar(s: &Servidor, txt: &str) -> Result<Json> {
    s.op_dblink_salvar(&Json::analisar(txt).unwrap())
}

fn ligacao(s: &Servidor, nome: &str) -> Definicao {
    s.dblink.lock().unwrap().achar(nome).unwrap().clone()
}

/// Salvar pela tela NAO apaga o pino, e nao apaga o `"cifra": false`.
///
/// A tela nunca recebe o pino de volta (`para_json` so da `tem_pino`),
/// entao ela nao tem como devolve-lo -- e sem a heranca todo salvar comum
/// o apagaria, deixando a ligacao com tunel sem ancora e o painel
/// identico. E o mesmo defeito que `com_o_token_de` e `com_as_sincronias_de`
/// ja consertaram, no campo que faltava.
#[test]
fn salvar_pela_tela_nao_apaga_o_pino_nem_a_decisao_da_cifra() {
    let dir = DirTemp::novo("dblink-cifra-heranca");
    let s = servidor(&dir);
    salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"10.0.0.7",
                     "token_remoto":"TOK","chave_do_fio":"{PINO}"}}"#
        ),
    )
    .unwrap();
    assert_eq!(ligacao(&s, "erp").chave_do_fio, PINO);

    // O salvar que a TELA manda: os campos do formulario, sem pino, sem
    // token e sem cifra -- porque nenhum dos tres volta no `para_json`.
    // O destino fica o MESMO: trocar o host sem o token recusa desde o
    // pedido 470 (ver `trocar_o_host_sem_a_senha_recusa_em_vez_de_herdar`).
    let r = salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"10.0.0.7",
                "database":"erp"}"#,
    )
    .unwrap();
    let d = ligacao(&s, "erp");
    assert_eq!(d.database, "erp", "a edicao nao pegou");
    assert_eq!(d.chave_do_fio, PINO, "o salvar pela tela APAGOU o pino");
    assert_eq!(d.token(), "TOK", "o salvar pela tela apagou o token");
    assert!(d.cifra());
    // E a ficha devolvida diz a verdade, em vez de anunciar uma ancora que
    // nao existe mais.
    let ficha = r.campo("ligacao").unwrap().escrever();
    assert!(ficha.contains("\"tem_pino\":true"), "{ficha}");
    assert!(!ficha.contains(PINO), "o pino vazou na resposta: {ficha}");

    // O disco tambem: e de la que o proximo arranque le.
    let lido = crate::dblink::Registro::abrir(&dir.join("dblink.json")).unwrap();
    assert_eq!(lido.achar("erp").unwrap().chave_do_fio, PINO);

    // O outro campo, com a CONDICAO DELE: quem escreveu o escape nao pode
    // ser religado por uma troca de porta.
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"claro","motor":"phxsql","host":"h","cifra":false}"#,
    )
    .unwrap();
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"claro","motor":"phxsql","host":"h2"}"#,
    )
    .unwrap();
    let d = ligacao(&s, "claro");
    assert_eq!(d.host, "h2");
    assert!(
        !d.cifra(),
        "o salvar pela tela religou uma ligacao que alguem mandou ficar em claro"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Campo PRESENTE e vazio APAGA -- isso e decisao escrita, e nao omissao.
///
/// Sem este lado, nao haveria como tirar um pino pela tela: a heranca
/// devolveria o antigo para sempre. As duas metades da regra precisam
/// existir juntas.
#[test]
fn pino_vazio_no_pedido_apaga_o_pino() {
    let dir = DirTemp::novo("dblink-cifra-apaga");
    let s = servidor(&dir);
    salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h",
                     "chave_do_fio":"{PINO}"}}"#
        ),
    )
    .unwrap();
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h","chave_do_fio":""}"#,
    )
    .unwrap();
    let d = ligacao(&s, "erp");
    assert_eq!(d.chave_do_fio, "", "o pino escrito vazio nao foi apagado");
    // Sem pino a cifra volta ao padrao -- que continua LIGADO. Apagar a
    // ancora nao e desligar o tunel.
    assert!(d.cifra());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Pedido 470: trocar o DESTINO sem mandar a credencial recusa, em vez de
/// herdar a guardada.
///
/// O defeito: `dblink_salvar` com host novo e sem `senha` herdava a senha
/// gravada, e uma sessao de administrador roubada a recebia apontando a
/// ligacao para um ouvinte dela. Cada campo do destino se prova sozinho,
/// porque cada um e um jeito de apontar para outro lugar.
#[test]
fn trocar_o_host_sem_a_senha_recusa_em_vez_de_herdar() {
    let dir = DirTemp::novo("dblink-470-senha");
    let s = servidor(&dir);
    let base = r#""op":"dblink_salvar","nome":"erp","motor":"mysql""#;
    salvar(
        &s,
        &format!(
            r#"{{{base},"host":"10.0.0.7","porta":3306,"usuario":"leitor",
                     "senha":"MARCA-470"}}"#
        ),
    )
    .unwrap();
    for (campo, trocado) in [
        (
            "host",
            r#""host":"10.6.6.6","porta":3306,"usuario":"leitor""#,
        ),
        (
            "porta",
            r#""host":"10.0.0.7","porta":3307,"usuario":"leitor""#,
        ),
        (
            "usuario",
            r#""host":"10.0.0.7","porta":3306,"usuario":"outro""#,
        ),
    ] {
        let e = salvar(&s, &format!("{{{base},{trocado}}}")).unwrap_err();
        let t = e.to_string();
        assert!(t.contains("mande a senha"), "{campo}: {t}");
        assert!(t.contains(&format!("({campo})")), "{campo}: {t}");
        assert!(!t.contains("MARCA-470"), "a recusa vazou a senha: {t}");
    }
    // O motor tambem e destino: a mesma senha num postgres no mesmo host
    // e outro servico ouvindo.
    let e = salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"postgres","host":"10.0.0.7",
                "porta":3306,"usuario":"leitor"}"#,
    )
    .unwrap_err();
    assert!(e.to_string().contains("(motor)"), "{e}");
    // E nada foi gravado: a ligacao continua a que era, com a senha dela.
    let d = ligacao(&s, "erp");
    assert_eq!(d.host, "10.0.0.7");
    assert_eq!(d.senha().unwrap(), "MARCA-470");
    let _ = std::fs::remove_dir_all(&dir);
}

/// O token do outro PhxSql e credencial pelo mesmo motivo, e o pino e
/// destino: trocar o pino e aceitar outra chave do outro lado.
#[test]
fn trocar_o_host_ou_o_pino_sem_o_token_recusa_em_vez_de_herdar() {
    let dir = DirTemp::novo("dblink-470-token");
    let s = servidor(&dir);
    salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h",
                     "token_remoto":"TOK-470","chave_do_fio":"{PINO}"}}"#
        ),
    )
    .unwrap();
    let e = salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h2"}"#,
    )
    .unwrap_err();
    let t = e.to_string();
    assert!(t.contains("mande a token_remoto"), "{t}");
    assert!(!t.contains("TOK-470"), "a recusa vazou o token: {t}");
    let outro = "c".repeat(64);
    let e = salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h",
                     "chave_do_fio":"{outro}"}}"#
        ),
    )
    .unwrap_err();
    assert!(e.to_string().contains("(chave_do_fio)"), "{e}");
    let d = ligacao(&s, "erp");
    assert_eq!(d.host, "h");
    assert_eq!(d.chave_do_fio, PINO);
    // Quem MANDA a credencial junto troca o destino normalmente.
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h2",
                "token_remoto":"TOK-NOVO"}"#,
    )
    .unwrap();
    let d = ligacao(&s, "erp");
    assert_eq!(d.host, "h2");
    assert_eq!(d.token(), "TOK-NOVO");
    let _ = std::fs::remove_dir_all(&dir);
}

/// O comportamento VELHO continua: salvar sem trocar o destino herda a
/// senha (a tela nunca a recebe de volta), e quem manda a senha junto
/// troca o host. Sem este lado, a guarda recusaria todo salvar da tela.
#[test]
fn salvar_sem_trocar_o_destino_continua_herdando_a_senha() {
    let dir = DirTemp::novo("dblink-470-velho");
    let s = servidor(&dir);
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"mysql","host":"ERP.local",
                "usuario":"leitor","senha":"MARCA-470"}"#,
    )
    .unwrap();
    // Mesma maquina com outra caixa, base e teto novos: nada disso aponta
    // para outro lugar.
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"mysql","host":"erp.local",
                "usuario":"leitor","database":"vendas","timeout_s":9}"#,
    )
    .unwrap();
    let d = ligacao(&s, "erp");
    assert_eq!(d.database, "vendas");
    assert_eq!(
        d.senha().unwrap(),
        "MARCA-470",
        "o salvar da tela apagou a senha"
    );
    // A senha nova junto do host novo passa, e a guardada NAO vai junto.
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"mysql","host":"10.0.0.9",
                "usuario":"leitor","senha":"NOVA-470"}"#,
    )
    .unwrap();
    let d = ligacao(&s, "erp");
    assert_eq!(d.host, "10.0.0.9");
    assert_eq!(d.senha().unwrap(), "NOVA-470");
    // E a variavel de ambiente conta como mandar: e escolha escrita.
    salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"mysql","host":"10.0.0.10",
                "usuario":"leitor","senha_env":"PHX_470_NAO_EXISTE"}"#,
    )
    .unwrap();
    assert_eq!(ligacao(&s, "erp").host, "10.0.0.10");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A recusa por motor alcanca a HERANCA, e nao so a declaracao.
///
/// Trocar o motor de `phxsql` para `mysql` sem mandar `cifra` passa pelo
/// `de_json` (o pedido nao traz o campo) e herdaria um pino que aquele fio
/// nao sabe usar. E o caminho irmao da recusa, e ele chama as mesmas
/// funcoes na mesma ordem.
#[test]
fn trocar_o_motor_por_um_alheio_recusa_em_vez_de_herdar_o_pino() {
    let dir = DirTemp::novo("dblink-cifra-motor");
    let s = servidor(&dir);
    salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"erp","motor":"phxsql","host":"h",
                     "chave_do_fio":"{PINO}"}}"#
        ),
    )
    .unwrap();
    let e = salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"mysql","host":"h"}"#,
    )
    .unwrap_err();
    let t = e.to_string();
    assert!(t.contains("mysql"), "a recusa nao nomeia o motor: {t}");
    assert!(t.contains("chave_do_fio"), "{t}");
    // E nada foi gravado: a ligacao continua a que era.
    let d = ligacao(&s, "erp");
    assert_eq!(d.motor.nome(), "phxsql");
    assert_eq!(d.chave_do_fio, PINO);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A recusa tambem alcanca a DECLARACAO pela tela -- a porta unica.
///
/// `op_dblink_salvar` chama o MESMO `Definicao::de_json` do arquivo, entao
/// a recusa nao precisou de portao proprio para valer nos dois.
#[test]
fn declarar_cifra_em_motor_alheio_pela_tela_recusa() {
    let dir = DirTemp::novo("dblink-cifra-declaracao");
    let s = servidor(&dir);
    let e = salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"erp","motor":"postgres","host":"h","cifra":true}"#,
    )
    .unwrap_err();
    let t = e.to_string();
    assert!(t.contains("postgres"), "{t}");
    assert!(s.dblink.lock().unwrap().achar("erp").is_err(), "gravou");
    let _ = std::fs::remove_dir_all(&dir);
}

/// (d) Pedido 372: a chave mestra AUSENTE nao derruba o servidor.
///
/// O cadastro foi cifrado com uma chave num arquivo de fora; o servidor
/// sobe declarando a chave numa variavel que ninguem exportou. Ele SOBE,
/// a ligacao cifrada fica trancada com o motivo (que nomeia a variavel), e
/// a vizinha sem credencial chega a REDE -- a recusa dela, se houver, e do
/// outro lado, e nao da chave.
///
/// Com o defeito reposto (a tranca virando erro da abertura), o
/// `Servidor::novo` recusa: o motor de dados inteiro fora do ar por causa
/// de uma ligacao do DbLink -- e o vermelho diz isso com o erro.
#[test]
fn a_chave_mestra_ausente_nao_derruba_o_servidor_e_tranca_so_a_cifrada() {
    const CHAVE: &str = "a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0";
    const VAR: &str = "PHXSQL_TESTE_372_SERVIDOR_CHAVE_QUE_NINGUEM_EXPORTA";
    const SENHA: &str = "SENHA-372-DO-SERVIDOR-QUE-NAO-PODE-SAIR";
    let dir = DirTemp::novo("dblink-372-servidor");
    let fora = DirTemp::novo("dblink-372-servidor-chave");
    std::fs::write(fora.join("k.hex"), CHAVE).unwrap();
    let com_chave = Config::de_json(
        &Json::analisar(&format!(
            r#"{{"cifra_do_dblink":{{"chave_mestra_arquivo":{:?}}}}}"#,
            fora.join("k.hex").display().to_string()
        ))
        .unwrap(),
    )
    .unwrap()
    .cifra_do_dblink;
    let mut r = crate::dblink::Registro::abrir_com(&dir.join("dblink.json"), &com_chave).unwrap();
    for json in [
        format!(r#"{{"nome":"loja","host":"127.0.0.1","porta":1,"senha":"{SENHA}"}}"#),
        r#"{"nome":"publica","host":"127.0.0.1","porta":1,"usuario":"leitor"}"#.to_string(),
    ] {
        r.salvar(Definicao::de_json(&Json::analisar(&json).unwrap()).unwrap())
            .unwrap();
    }
    drop(r);

    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    c.cifra_do_dblink = Config::de_json(
        &Json::analisar(&format!(
            r#"{{"cifra_do_dblink":{{"chave_mestra_env":"{VAR}"}}}}"#
        ))
        .unwrap(),
    )
    .unwrap()
    .cifra_do_dblink;
    let s = match Servidor::novo(c) {
        Ok(s) => s,
        Err(e) => panic!(
            "sem a chave mestra o SERVIDOR NAO SUBIU -- o motor de dados \
                 inteiro fora do ar por causa de uma ligacao do DbLink: {e}"
        ),
    };

    // A lista diz o estado, e nunca o valor.
    let lista = s.op_dblink().unwrap();
    let texto = lista.escrever();
    assert!(!texto.contains(SENHA), "a senha vazou na lista");
    assert!(!texto.contains(CHAVE), "a chave vazou na lista");
    let cifra = lista.campo("cifra_do_cadastro").unwrap();
    assert_eq!(cifra.campo("cifrado").and_then(Json::booleano), Some(true));
    assert_eq!(cifra.texto_ou("chave", ""), "indisponivel");
    assert!(cifra.texto_ou("trancado", "").contains(VAR), "{texto}");

    // A cifrada recusa ANTES da rede, dizendo qual variavel falta.
    let erro = match s.ligar(&Json::analisar(r#"{"dblink":"loja"}"#).unwrap()) {
        Ok(_) => panic!("a ligacao cifrada conectou sem chave"),
        Err(e) => e.to_string(),
    };
    assert!(erro.contains(VAR), "{erro}");
    // A vizinha vai a rede: a porta 1 recusa, e o erro e DELA, nao da chave.
    let erro = match s.ligar(&Json::analisar(r#"{"dblink":"publica"}"#).unwrap()) {
        Ok(_) => panic!("havia alguem na porta 1"),
        Err(e) => e.to_string(),
    };
    assert!(
        !erro.contains(VAR),
        "a chave trancou a ligacao que nao e cifrada: {erro}"
    );
    assert!(!erro.contains("CIFRADO"), "{erro}");

    // Salvar pela tela a ligacao trancada, sem mandar a senha (a tela nunca
    // a recebe), preserva o envelope: a heranca carrega a tranca inteira.
    let antes = std::fs::read_to_string(dir.join("dblink.json")).unwrap();
    let envelope = Json::analisar(&antes)
        .unwrap()
        .campo("ligacoes")
        .and_then(Json::lista)
        .unwrap()[0]
        .texto_ou("senha_cifrada", "")
        .to_string();
    assert!(!envelope.is_empty());
    // O destino fica o MESMO: trocar a porta sem a senha recusa desde o
    // pedido 470, e aqui o que se prova e a heranca do envelope.
    let r = s
        .op_dblink_salvar(
            &Json::analisar(
                r#"{"op":"dblink_salvar","nome":"loja","host":"127.0.0.1","porta":1,
                        "descricao":"editada pela tela"}"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        r.campo("cadastro")
            .and_then(|c| c.campo("cifrado"))
            .and_then(Json::booleano),
        Some(true)
    );
    let depois = std::fs::read_to_string(dir.join("dblink.json")).unwrap();
    assert!(
        depois.contains(&envelope),
        "salvar pela tela perdeu o envelope da trancada"
    );
    assert!(!depois.contains(SENHA));
    // E credencial nova em claro num cadastro cifrado sem chave: recusada
    // -- conferida no DISCO primeiro, que e onde o dano mora.
    let salvou = s.op_dblink_salvar(
        &Json::analisar(r#"{"op":"dblink_salvar","nome":"nova","senha":"em-claro-372-srv"}"#)
            .unwrap(),
    );
    let disco = std::fs::read_to_string(dir.join("dblink.json")).unwrap();
    assert!(
        !disco.contains("em-claro-372-srv"),
        "sem a chave, o servidor gravou a credencial nova em TEXTO PURO no \
             cadastro cifrado"
    );
    match salvou {
        Ok(_) => panic!("gravou credencial em claro num cadastro cifrado"),
        Err(e) => assert!(e.to_string().contains(VAR), "{e}"),
    }
    assert!(s.dblink.lock().unwrap().achar("nova").is_err());
}

/// A migracao e DITA na resposta do `dblink_salvar`: quantas ligacoes
/// sairam do texto puro. O cadastro de hoje, e a chave declarada.
#[test]
fn a_migracao_e_dita_na_resposta_do_salvar() {
    let dir = DirTemp::novo("dblink-372-servidor-migra");
    let fora = DirTemp::novo("dblink-372-servidor-migra-chave");
    std::fs::write(
        fora.join("k.hex"),
        "a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0",
    )
    .unwrap();
    std::fs::write(
        dir.join("dblink.json"),
        r#"{"dblink":[{"nome":"loja","senha":"CLARO-372-MIGRA"},{"nome":"erp","senha":"CLARO-372-ERP"}]}"#,
    )
    .unwrap();
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    c.cifra_do_dblink = Config::de_json(
        &Json::analisar(&format!(
            r#"{{"cifra_do_dblink":{{"chave_mestra_arquivo":{:?}}}}}"#,
            fora.join("k.hex").display().to_string()
        ))
        .unwrap(),
    )
    .unwrap()
    .cifra_do_dblink;
    let s = Servidor::novo(c).unwrap();
    // Subir NAO reescreve: a migracao e da primeira gravacao.
    assert!(std::fs::read_to_string(dir.join("dblink.json"))
        .unwrap()
        .contains("CLARO-372-MIGRA"));
    let r = s
        .op_dblink_salvar(
            &Json::analisar(r#"{"op":"dblink_salvar","nome":"loja","descricao":"x"}"#).unwrap(),
        )
        .unwrap();
    let cadastro = r.campo("cadastro").unwrap();
    assert_eq!(
        cadastro
            .campo("ligacoes_cifradas_agora")
            .and_then(Json::inteiro),
        Some(2),
        "{}",
        cadastro.escrever()
    );
    let disco = std::fs::read_to_string(dir.join("dblink.json")).unwrap();
    assert!(!disco.contains("CLARO-372"), "o claro ficou no disco");
    // A senha herdada pela tela continua a mesma, agora selada.
    assert!(
        s.dblink.lock().unwrap().achar("loja").unwrap().senha().ok() == Some("CLARO-372-MIGRA")
    );
}

/// **O DbLink `phxsql` pelo TLS conferido (pedido 572, T6b-2), pelo soquete.**
///
/// O destino tem `"tls": true`, EXIGE cifra e NAO atende o Noise: a ligacao
/// com `pino_tls` abre e faz o `ping` so se falar TLS. O salvar pela tela
/// herda o pino TLS e a ficha so diz `tem_pino_tls`.
#[test]
fn a_ligacao_phxsql_fala_tls_pelo_pino_tls_e_o_salvar_o_herda() {
    let dir_d = DirTemp::novo("dblink-tls-destino");
    let mut c = Config {
        base: dir_d.to_path_buf(),
        log_acessos: dir_d.join("acessos.log"),
        blacklist: dir_d.join("blacklist.json"),
        dblink: dir_d.join("dblink.json"),
        jobs: dir_d.join("jobs.json"),
        token: "TOK".into(),
        caminho: Some(dir_d.join("config.json")),
        ..Config::default()
    };
    c.cifra_fio.arquivo = dir_d.join("chave-do-fio.hex");
    // O Noise DESLIGADO no destino e a exigencia ligada: so o TLS atravessa.
    // Uma ligacao que caisse no Noise (o defeito de ignorar o `pino_tls`)
    // ouviria «aperto recusado» e nao abriria.
    c.cifra_fio.ligada = false;
    c.cifra_fio.exigir = true;
    c.tls.ligado = true;
    let destino = Servidor::novo(c).unwrap();
    destino.preparar_tls_dos_dados().unwrap();
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let d2 = Arc::clone(&destino);
    std::thread::spawn(move || {
        for fluxo in ouvinte.incoming() {
            let Ok(fluxo) = fluxo else { return };
            let Ok(par) = fluxo.peer_addr() else { continue };
            let s = Arc::clone(&d2);
            std::thread::spawn(move || s.atender(fluxo, par));
        }
    });
    let pem = std::fs::read_to_string(dir_d.join("tls-dados-certificado.pem")).unwrap();
    let cert = phxsql_core::x509::blocos_pem(&pem, "CERTIFICATE").unwrap();
    let pino = phxsql_core::tls::pino_em_texto(&phxsql_core::hash::sha256(
        phxsql_core::x509::spki_do_certificado(&cert[0]).unwrap(),
    ));

    let dir = DirTemp::novo("dblink-tls-cadastro");
    let s = servidor(&dir);
    salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"irmao","motor":"phxsql","host":"127.0.0.1",
                 "porta":{porta},"token_remoto":"TOK","cifra":false,"pino_tls":"{pino}"}}"#
        ),
    )
    .unwrap();
    let d = ligacao(&s, "irmao");
    assert!(d.cifra(), "o pino TLS nao conta como cifra efetiva");
    crate::dblink::phx::Conexao::abrir(&d, d.prazo())
        .expect("a ligacao pelo TLS nao abriu contra o destino que exige cifra");

    // A tela salva sem o pino: ele fica, e a ficha so diz que existe.
    let r = salvar(
        &s,
        &format!(
            r#"{{"op":"dblink_salvar","nome":"irmao","motor":"phxsql","host":"127.0.0.1",
                 "porta":{porta},"cifra":false,"descricao":"editada"}}"#
        ),
    )
    .unwrap();
    assert_eq!(
        ligacao(&s, "irmao").pino_tls,
        pino,
        "o salvar apagou o pino TLS"
    );
    let ficha = r.campo("ligacao").unwrap().escrever();
    assert!(ficha.contains("\"tem_pino_tls\":true"), "{ficha}");
    assert!(
        !ficha.contains(&pino),
        "o pino TLS vazou na resposta: {ficha}"
    );
    let lido = crate::dblink::Registro::abrir(&dir.join("dblink.json")).unwrap();
    assert_eq!(
        lido.achar("irmao").unwrap().pino_tls,
        pino,
        "o disco perdeu o pino TLS"
    );

    // O `tls` dos motores de fora no motor phxsql recusa na declaracao:
    // entre dois PhxSql o TLS confere por pino (T6d; antes da T6d, a recusa
    // era a do `pino_tls` no mysql, que agora fala TLS).
    let e = salvar(
        &s,
        r#"{"op":"dblink_salvar","nome":"m","motor":"phxsql","host":"h","tls":"exigir"}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("pino_tls"), "{e}");
}
