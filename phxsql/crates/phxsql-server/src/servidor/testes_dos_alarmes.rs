//! A3 do pedido 707: as marcas de alarme na ORIGEM, pelo caminho real.
//!
//! Cada prova chama a origem verdadeira (`travar_dados`, o `siga`, o
//! `anotar`) com uma atividade amarrada, como a conexao faz, e le o bit que
//! ficou. Nenhuma chama o `sinal` direto: a prova e de que a ORIGEM marca.

use super::*;
use crate::aquario::Alarme;
use crate::telemetria::Atividade;

fn servidor(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("alarme-{nome}"));
    let config = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    (Servidor::novo(config).unwrap(), dir)
}

/// Um pedido inteiro como a porta de dados o faz: entra, amarra, comeca,
/// roda `corpo`, termina e anota o desfecho no sumidouro. Devolve a
/// mascara de alarmes que ficou na tarefa.
fn pedido_com(
    s: &Arc<Servidor>,
    chave: &str,
    corpo: impl FnOnce(&Arc<Servidor>, &Atividade) -> Result<()>,
) -> u32 {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar(chave, "dados", "127.0.0.1", 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("inserir", "root", "b", "t", agora);
    let desfecho = corpo(s, &a);
    a.terminou_pedido("root");
    s.anotar(&Acesso {
        quando_ms: agora,
        ip: "127.0.0.1".into(),
        porta_origem: 1,
        op: "inserir".into(),
        usuario: "root".into(),
        autenticado: true,
        ok: desfecho.is_ok(),
        duracao_ms: 0,
        erro: desfecho.as_ref().err().map(|e| e.to_string()),
        database: "b".into(),
        tabela: "t".into(),
        codigo: desfecho.as_ref().err().map(PhxError::codigo).unwrap_or(0),
        ..Acesso::default()
    });
    a.alarmes()
}

fn grupos(mascara: u32) -> Vec<&'static str> {
    Alarme::da_mascara(mascara)
        .map(|a| a.grupo().nome())
        .collect()
}

/// **O RED da A3 (`plano-0.21.md`).** A trava pedida duas vezes e o dado
/// corrompido saem do servidor com o MESMO codigo, 1001. So o bit posto em
/// `trava_reentrante()` as separa.
///
/// Vermelho: sem a linha do `sinal` em `trava_reentrante()`, o sumidouro ve
/// um 1001 sem marca de trava e o chama de dado corrompido -- as duas
/// tarefas saem com a mesma mascara, a mesma classe, e o teste cai.
#[test]
fn reentrante_e_corrompido_saem_em_classes_diferentes() {
    let (s, _dir) = servidor("reentrante");
    let reentrante = pedido_com(&s, "dados:1", |s, _| {
        let primeira = s.travar_dados()?;
        let segunda = s.travar_dados().map(|_| ());
        drop(primeira);
        segunda
    });
    let corrompido = pedido_com(&s, "dados:2", |_, _| {
        Err(PhxError::Corrompido("pagina com CRC errado".into()))
    });
    assert_ne!(
        reentrante, corrompido,
        "as duas tarefas empataram: o 1001 nao separa as causas sozinho"
    );
    assert_eq!(grupos(reentrante), vec!["lock"], "{reentrante:#b}");
    assert_eq!(grupos(corrompido), vec!["dado"], "{corrompido:#b}");
}

/// A trava envenenada sem reparo tambem e 1001, e tambem e LOCK.
///
/// O veneno e posto por fora do `travar_dados` de proposito: por dentro, o
/// `Drop` da ficha repara e a trava volta a servir -- e o alarme e da trava
/// que RECUSA. Vermelho: sem a linha do `sinal` em
/// `trava_de_dados_sem_reparo()`, a tarefa sai como dado corrompido.
#[test]
fn a_trava_envenenada_e_lock_e_nao_dado() {
    let (s, _dir) = servidor("veneno");
    let copia = Arc::clone(&s);
    let _ = std::thread::spawn(move || {
        let _g = copia.dados.write().unwrap();
        panic!("veneno de teste da A3 (pedido 707)");
    })
    .join();
    assert!(s.dados.is_poisoned(), "a prova nao envenenou a trava");
    let m = pedido_com(&s, "dados:3", |s, _| s.travar_dados().map(|_| ()));
    let lidos: Vec<Alarme> = Alarme::da_mascara(m).collect();
    assert_eq!(lidos, vec![Alarme::TravaEnvenenada], "{m:#b}");
}

/// O 6001 e o mesmo para o prazo e para o encerrar manual. So o prazo e
/// alarme. Vermelho: sem a linha do `sinal_em` no `siga`, a tarefa que
/// estourou o STATEMENT TIMEOUT sai sem alarme nenhum.
#[test]
fn o_prazo_marca_e_o_encerrar_manual_nao() {
    let (s, _dir) = servidor("prazo");
    let prazo = pedido_com(&s, "dados:4", |_, a| {
        a.definir_prazo(crate::agora_ms() - 1);
        a.siga(1)
    });
    assert_eq!(
        Alarme::da_mascara(prazo).collect::<Vec<_>>(),
        vec![Alarme::PrazoEstourado],
        "{prazo:#b}"
    );
    let manual = pedido_com(&s, "dados:5", |_, a| {
        a.encerrar("root");
        let r = a.siga(1);
        // Sem o 6001 de verdade, o zero abaixo nao provaria nada.
        assert!(matches!(r, Err(PhxError::Cancelado(_))), "{r:?}");
        r
    });
    assert_eq!(manual, 0, "o encerrar manual virou alarme: {manual:#b}");
}

/// O erro de E/S marca no gancho que ja existia no sumidouro. Vermelho: sem
/// a linha do `sinal` dentro do `if acesso.codigo == CODIGO_DE_ES`, nada.
#[test]
fn o_erro_de_disco_marca_na_tarefa() {
    let (s, _dir) = servidor("es");
    let m = pedido_com(&s, "dados:6", |_, _| {
        Err(PhxError::Io(std::io::Error::other(
            "No space left on device",
        )))
    });
    assert_eq!(
        Alarme::da_mascara(m).collect::<Vec<_>>(),
        vec![Alarme::ErroDeDisco],
        "{m:#b}"
    );
}

/// O alarme e da TAREFA: o pedido seguinte da mesma conexao nasce limpo.
/// Vermelho: sem o `alarmes.store(0)` do `comecou_pedido`, o pedido depois
/// da reentrancia sai pintado de LOCK.
#[test]
fn o_pedido_seguinte_nasce_sem_o_alarme_do_anterior() {
    let (s, _dir) = servidor("limpo");
    let m = pedido_com(&s, "dados:7", |s, _| {
        let _p = s.travar_dados()?;
        s.travar_dados().map(|_| ())
    });
    assert_ne!(
        m, 0,
        "a reentrancia nao marcou -- a prova nao exercita nada"
    );
    let seguinte = pedido_com(&s, "dados:7", |_, _| Ok(()));
    assert_eq!(seguinte, 0, "{seguinte:#b}");
}

/// O caminho comum nao marca nada: pedido que deu certo, e erro que nao e
/// de nenhuma origem marcada («nao achei», 3001), saem com zero.
///
/// A recusa de integridade (3006) era o exemplo de «erro sem marca» ate o
/// pedido 779 -- e era o defeito: o alarme `IntegridadeRecusada` existia, a
/// tela o traduzia, e nada o acendia. Agora ela marca o bit DELA, e so ele.
#[test]
fn sem_alarme_nada_muda() {
    let (s, _dir) = servidor("comum");
    assert_eq!(pedido_com(&s, "dados:8", |_, _| Ok(())), 0);
    let nao_achei = pedido_com(&s, "dados:10", |_, _| {
        Err(PhxError::NaoEncontrado("linha 7".into()))
    });
    assert_eq!(nao_achei, 0, "{nao_achei:#b}");
    let integridade = pedido_com(&s, "dados:9", |_, _| {
        Err(PhxError::Integridade("filha sem mae".into()))
    });
    assert_eq!(
        integridade,
        crate::aquario::Alarme::IntegridadeRecusada.bit(),
        "{integridade:#b}"
    );
}
