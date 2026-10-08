//! Pedido 599 (a): o arbitro do cluster que se rebaixa e NAO consegue gravar
//! o papel diz isso, nos dois caminhos que rebaixam -- e o mesmo motor do
//! `registrar` do 597.
//!
//! O disco recusa pelo mesmo truque do `cluster.rs`: um DIRETORIO no lugar do
//! temporario do `cluster.estado.json`, entao o `gravar_privado` falha sem
//! mexer em permissao (que o root do conteiner ignoraria).
use super::*;
use crate::cluster::{EstadoCluster, PapelVivo, PulsoDeNo};
use crate::usuarios::Cadastro;

/// Um no MASTER (`papel: source`) de um cluster de tres, sem thread
/// nenhuma: o que se prova e a RODADA do arbitro, chamada a mao.
fn master(nome: &str) -> (Arc<Servidor>, Arc<EstadoCluster>, DirTemp) {
    let dir = DirTemp::novo(&format!("rebaixar-sem-disco-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
  "token": "t",
  "bind": "127.0.0.1:0",
  "base": "{}",
  "replicacao": {{"papel": "source", "id_servidor": "no1", "imagem_da_linha": true}},
  "cluster": {{
    "id": "no1",
    "janela_inatividade_s": 30,
    "pulso_s": 1,
    "nos": [
      {{"id": "no1", "endereco": "127.0.0.1", "porta": 5397}},
      {{"id": "no2", "endereco": "127.0.0.1", "porta": 5398}},
      {{"id": "no3", "endereco": "127.0.0.1", "porta": 5399}}
    ]
  }}
}}
"#,
            dir.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    c.cadastro = Cadastro::default();
    let s = Servidor::novo(c).unwrap();
    let estado = s.cluster.clone().expect("cluster");
    assert_eq!(estado.papel(), PapelVivo::Master);
    (s, estado, dir)
}

fn pulso_de_master(estado: &EstadoCluster, id: &str, epoca: u64, posicao: u64) {
    estado
        .registrar(
            id,
            PulsoDeNo {
                papel: PapelVivo::Master,
                epoca,
                posicao,
                incompleta: false,
                prioridade: 0,
                quando_ms: crate::agora_ms(),
                por_tabela: None,
            },
        )
        .unwrap();
}

fn quebrar_o_disco(dir: &DirTemp) {
    let tmp = crate::config::temporario_de(&dir.join("dados").join("cluster.estado.json"));
    std::fs::create_dir_all(tmp.join("ocupado")).unwrap();
}

fn diz_que_nao_gravou(motivos: &[String]) -> bool {
    motivos.iter().any(|m| m.contains("NAO foi ao disco"))
}

/// Epoca maior no ar: rebaixa, e o motivo diz que o papel nao foi ao
/// disco. Defeito reposto (guarda `arbitro-engole-o-rebaixar`): o
/// `rebaixar_dizendo` volta a engolir, e a rodada cala.
#[test]
fn rebaixar_pela_epoca_maior_sem_disco_se_diz() {
    let (s, estado, dir) = master("epoca");
    pulso_de_master(&estado, "no2", estado.epoca() + 1, 0);
    quebrar_o_disco(&dir);
    let motivos = s.rodada_do_arbitro(&estado, crate::agora_ms());
    assert_eq!(
        estado.papel(),
        PapelVivo::Replica,
        "nao rebaixou: {motivos:?}"
    );
    assert!(
        diz_que_nao_gravou(&motivos),
        "o papel nao foi ao disco e a rodada calou: {motivos:?}"
    );
}

/// Desempate perdido na mesma epoca: o irmao do de cima, pelo mesmo motor.
#[test]
fn rebaixar_pelo_desempate_sem_disco_se_diz() {
    let (s, estado, dir) = master("desempate");
    // Posicao maior: o `vencedor` escolhe o no2.
    pulso_de_master(&estado, "no2", estado.epoca(), 1_000);
    quebrar_o_disco(&dir);
    let motivos = s.rodada_do_arbitro(&estado, crate::agora_ms());
    assert_eq!(
        estado.papel(),
        PapelVivo::Replica,
        "nao rebaixou: {motivos:?}"
    );
    assert!(
        diz_que_nao_gravou(&motivos),
        "o papel nao foi ao disco e a rodada calou: {motivos:?}"
    );
}

/// O comportamento VELHO: com o disco bom, rebaixar nao inventa aviso.
#[test]
fn rebaixar_com_disco_nao_inventa_aviso() {
    let (s, estado, _dir) = master("disco-bom");
    pulso_de_master(&estado, "no2", estado.epoca() + 1, 0);
    let motivos = s.rodada_do_arbitro(&estado, crate::agora_ms());
    assert_eq!(estado.papel(), PapelVivo::Replica);
    assert!(!diz_que_nao_gravou(&motivos), "{motivos:?}");
}
