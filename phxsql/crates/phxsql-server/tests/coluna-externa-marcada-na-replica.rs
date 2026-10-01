//! Pedido 344: a coluna EXTERNA marcada (`Bin`) replicada entre DOIS
//! servidores de verdade, pelo soquete.
//!
//! # Por que dois processos
//!
//! O cofre e global ao PROCESSO (`cofre::definir`). Dois `Servidor` no mesmo
//! processo dividiriam a mesma senha e o mesmo estado, e o caso 344-1 -- a
//! replica SEM cofre -- nao existiria. Entao a ORIGEM e o `phxsqld` de verdade,
//! com a cifra ligada no `config.json` dele, e a REPLICA roda aqui dentro, com
//! o cofre que cada teste escolhe (por isso a trava: o cofre daqui e um so).
//!
//! # O que se mede: o CONTEUDO, nunca o veredito
//!
//! O defeito do 344-1 e justamente responder `ok`: a replica gravava os bytes
//! selados pela origem como se fossem o anexo. Entao a prova le o `Bin` na
//! replica e compara byte a byte com o que a origem recebeu. E o 344-2 mede a
//! mesma coisa com a MESMA senha dos dois lados, que antes parava a replicacao
//! acusando adulteracao que nao houve.
#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};
use phxsql_store::cofre;

static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

const TOKEN: &str = "coluna-externa-marcada-na-replica";
const SENHA: &str = "a mesma senha nos dois lados";
const ITERACOES: u32 = 10_000;
/// O anexo marcado, em hexadecimal: e o que tem de chegar igual na replica.
const ANEXO: &str = "4c6175646f20636f6e666964656e6369616c20646f2066756c616e6f";
/// O mesmo anexo em texto, para procurar nos arquivos da replica.
const ANEXO_TEXTO: &str = "Laudo confidencial do fulano";

/// Sobe a origem: o `phxsqld`, com a cifra ligada pelo `config.json`.
fn subir_origem(dir: &Path) -> (Filho, u16) {
    let config = dir.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "cifra": {{ "ligada": true, "senha": "{SENHA}", "iteracoes": {ITERACOES} }},
                "cifra_fio": {{ "exigir": false, "arquivo": {chave:?} }},
                "web": {{ "ligado": false }},
                "replicacao": {{ "papel": "source", "id_servidor": "origem-344",
                                 "imagem_da_linha": true }}
            }}"#,
            base = dir.join("dados").display().to_string(),
            chave = dir.join("chave-do-fio.hex").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = dir.join("stderr.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(dir)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    (filho, porta)
}

/// A replica, aqui dentro, puxando da origem pelo tunel da §7 -- o fio que o
/// pedido 342 exige para tabela com coluna marcada.
fn subir_replica(base: &Path, porta_origem: u16) -> (Arc<Servidor>, u16) {
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
    c.somente_leitura = true;
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-344".into();
    c.replicacao.origens = vec![Origem {
        nome: "origem".into(),
        host: "127.0.0.1".into(),
        porta: porta_origem,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: true,
        chave_do_fio: String::new(),
    }];
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    Json::analisar(&resposta)
        .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

/// A origem com a tabela de anexo MARCADO e uma linha.
fn encher_origem(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"laudos",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"anexo","tipo":"Bin","dado_pessoal":"sensivel"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    exigir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"laudos",
               "linha":{{"id":1,"anexo":"{ANEXO}"}}"#
        ),
    );
    // Controle positivo: na origem o anexo volta inteiro.
    assert_eq!(
        anexos(porta),
        vec![ANEXO.to_string()],
        "a origem nao gravou"
    );
}

/// Os anexos que um servidor devolve, em hexadecimal, na ordem.
fn anexos(porta: u16) -> Vec<String> {
    let r = pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"laudos","max":10"#,
    );
    let Some(res) = r.campo("resultado") else {
        return Vec::new();
    };
    res.campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| l.texto_ou("anexo", "").to_string())
        .collect()
}

/// O que a replica diz da replicacao: e onde a recusa aparece.
fn estado(porta: u16) -> String {
    pedir(porta, r#""op":"replicacao_estado""#).escrever()
}

/// Espera a linha chegar na replica, ou 20 s. Devolve os anexos lidos.
fn esperar_a_linha(porta: u16) -> Vec<String> {
    let ate = Instant::now() + Duration::from_secs(20);
    loop {
        let lidos = anexos(porta);
        if !lidos.is_empty() || Instant::now() > ate {
            return lidos;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Algum arquivo debaixo de `raiz` contem `agulha`?
fn algum_arquivo_contem(raiz: &Path, agulha: &[u8]) -> bool {
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(p) = pilha.pop() {
        if p.is_dir() {
            pilha.extend(std::fs::read_dir(&p).unwrap().flatten().map(|i| i.path()));
        } else if let Ok(b) = std::fs::read(&p) {
            if b.windows(agulha.len()).any(|j| j == agulha) {
                return true;
            }
        }
    }
    false
}

/// **344-1.** Replica SEM cofre: o anexo chega IGUAL, ou nao chega.
///
/// # O vermelho
///
/// Com o defeito reposto (a origem mandando o externo selado e a replica
/// decidindo pelo proprio estado), a replica respondia `ok` e o `varrer`
/// devolvia 68 bytes que nao sao o anexo: `[nonce 24][cifrado][etiqueta 16]`.
/// Dado errado, calado, e o `spare_promover` promoveria esse banco.
#[test]
fn replica_sem_cofre_nao_grava_o_selado_como_anexo() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let dir_o = DirTemp::novo("344-sem-cofre-origem");
    let dir_r = DirTemp::novo("344-sem-cofre-replica");
    let (_origem, porta_o) = subir_origem(&dir_o);
    encher_origem(porta_o);

    let (_replica, porta_r) = subir_replica(&dir_r, porta_o);
    let lidos = esperar_a_linha(porta_r);
    assert_eq!(
        lidos,
        vec![ANEXO.to_string()],
        "a replica sem cofre gravou outra coisa no lugar do anexo (estado: {})",
        estado(porta_r)
    );
}

/// **344-2.** A MESMA senha dos dois lados replica, e a replica sela com a
/// chave DELA.
///
/// # O vermelho
///
/// Com o defeito reposto, a replica parava no primeiro evento com
/// `a etiqueta nao confere -- ou o dado foi alterado, ou a chave de "cifra"
/// nao e a que gravou este arquivo`: nada foi alterado e a senha e a mesma; o
/// sal e por arquivo, e a chave derivada la nao e a daqui.
#[test]
fn mesma_senha_replica_a_coluna_externa_marcada() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    cofre::definir(SENHA, ITERACOES).unwrap();
    let dir_o = DirTemp::novo("344-mesma-senha-origem");
    let dir_r = DirTemp::novo("344-mesma-senha-replica");
    let (_origem, porta_o) = subir_origem(&dir_o);
    encher_origem(porta_o);

    let (_replica, porta_r) = subir_replica(&dir_r, porta_o);
    let lidos = esperar_a_linha(porta_r);
    let est = estado(porta_r);
    cofre::desligar();
    assert_eq!(
        lidos,
        vec![ANEXO.to_string()],
        "com a mesma senha o anexo nao chegou igual (estado: {est})"
    );
    // A replica guardou o anexo SELADO com a chave dela: abrir no fio nao
    // pode virar texto claro em disco do outro lado.
    assert!(
        !algum_arquivo_contem(&dir_r, ANEXO_TEXTO.as_bytes()),
        "o anexo marcado ficou em claro num arquivo da replica com cofre"
    );
}

/// **O controle: sem coluna marcada, nada muda.** O mesmo anexo numa coluna
/// SEM marca, da origem com cofre para a replica sem cofre, chega igual --
/// como sempre chegou. Sem este teste, um conserto que quebrasse a
/// replicacao inteira passaria pelos dois de cima como «nao gravou o selado».
#[test]
fn sem_coluna_marcada_o_anexo_replica_como_sempre() {
    let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    cofre::desligar();
    let dir_o = DirTemp::novo("344-controle-origem");
    let dir_r = DirTemp::novo("344-controle-replica");
    let (_origem, porta_o) = subir_origem(&dir_o);
    exigir(porta_o, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta_o,
        r#""op":"criar_tabela","database":"loja","tabela":"laudos",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"anexo","tipo":"Bin"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    exigir(
        porta_o,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"laudos",
               "linha":{{"id":1,"anexo":"{ANEXO}"}}"#
        ),
    );
    let (_replica, porta_r) = subir_replica(&dir_r, porta_o);
    let lidos = esperar_a_linha(porta_r);
    assert_eq!(
        lidos,
        vec![ANEXO.to_string()],
        "a coluna sem marca deixou de replicar (estado: {})",
        estado(porta_r)
    );
}
