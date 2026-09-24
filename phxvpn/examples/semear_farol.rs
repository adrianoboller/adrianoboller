//! Semeia DUAS pastas do programa de mesa com uma rede "Farol" ja pronta:
//! o dono e um segundo membro ja admitido no rol (com o farol marcado nele),
//! para a prova de tela (`provas/farol/tela.mjs`) abrir o `phxvpn mesa` de
//! verdade e ver o selo/botao SEM precisar montar um `netns` com placa TUN
//! so para exercitar HTML/JS -- o mesmo atalho que o teste
//! `mesa::testes::farol_pela_api_dono_autoriza_membro_consente` ja usa
//! (admitir o segundo membro no rol A MAO, sem aperto Noise de verdade).
//!
//! Uso: cargo run --example semear_farol -p phxvpn -- <pasta-dono> <pasta-membro>

use phxvpn::comandos::{self, Opcoes};
use phxvpn::rede_p2p::Rede;
use phxvpn::rol::Membro;

fn opcoes(pares: &[(&str, String)]) -> Opcoes {
    let palavras: Vec<String> = pares
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("/{k}:{v}"))
        .collect();
    Opcoes::de_dos(&palavras)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (pasta_dono, pasta_membro) = match &args[1..] {
        [a, b] => (a.clone(), b.clone()),
        _ => {
            eprintln!("uso: semear_farol <pasta-dono> <pasta-membro>");
            std::process::exit(2);
        }
    };
    std::fs::create_dir_all(&pasta_dono).unwrap();
    std::fs::create_dir_all(&pasta_membro).unwrap();
    let chave_dono = format!("{pasta_dono}/p2p.chave");
    let chave_membro = format!("{pasta_membro}/p2p.chave");
    let arquivo_dono = format!("{pasta_dono}/Farol.p2p");
    let arquivo_membro = format!("{pasta_membro}/Farol.p2p");

    comandos::p2p_criar(&opcoes(&[
        ("rede", "Farol".into()),
        ("ip", "10.78.30.1/24".into()),
        ("modo", "auto".into()),
        ("arquivo", arquivo_dono.clone()),
        ("chave", chave_dono.clone()),
    ]))
    .expect("criar a rede");

    let privada_dono = comandos::identidade(&chave_dono).unwrap();
    let privada_membro = comandos::identidade(&chave_membro).unwrap();
    let chave_publica_membro = phxsql_core::x25519::chave_publica(&privada_membro);

    // Admite o membro no rol do dono -- o mesmo `.com()` que o teste usa.
    let mut ra = Rede::ler(&arquivo_dono).unwrap();
    let rol_com_membro = ra
        .rol
        .as_ref()
        .unwrap()
        .com(
            Membro {
                chave: chave_publica_membro,
                ip: "10.78.30.2".parse().unwrap(),
                nome: Some("notebook-ana".into()),
                farol: None,
            },
            &privada_dono,
        )
        .unwrap();
    ra.rol = Some(rol_com_membro.clone());
    // O par entra em `pares` so para a tela de MESA (desligada) ter o que
    // listar -- `Mesa::redes()` so devolve `r.pares` quando nao ha no P2P
    // vivo, exatamente o caso desta prova de tela.
    ra.pares.push(phxvpn::rede_p2p::Par {
        chave: chave_publica_membro,
        ip: "10.78.30.2".parse().unwrap(),
        endereco: None,
    });
    ra.gravar(&arquivo_dono).unwrap();

    // O arquivo do membro: mesmos campos de `rede_do_convidado`, com o rol
    // ja aprendido (o que a malha teria ensinado ao vivo).
    let mut rm = Rede::ler(&arquivo_dono).unwrap();
    rm.ip = "10.78.30.2".parse().unwrap();
    rm.dono = ra.dono;
    rm.rol = Some(rol_com_membro);
    rm.pares.clear();
    rm.gravar(&arquivo_membro).unwrap();

    // Marca o farol do MEMBRO (endereco publico de mentira -- so para a
    // tela mostrar o selo): mesmo motor de `phxvpn p2p farol`.
    comandos::p2p_farol(&opcoes(&[
        ("rede", "Farol".into()),
        ("arquivo", arquivo_dono.clone()),
        ("chave", chave_dono),
        ("ip", "10.78.30.2".into()),
        ("endereco", "203.0.113.9:51820".into()),
    ]))
    .expect("marcar o farol");

    println!("dono: {arquivo_dono}");
    println!("membro: {arquivo_membro}");
}
