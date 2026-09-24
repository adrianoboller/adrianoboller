//! Os ganchos do rol assinado (`rol.rs`) e da descoberta na LAN
//! (`descoberta.rs`) no no P2P. Mora num arquivo proprio, filho de `p2p`
//! (como `perfuracao.rs`), porque precisa dos campos privados do `No` -- e
//! para que frentes paralelas no `p2p.rs` encontrem ali so as chamadas.

use super::{Estado, No, Par, Via, CONTROLE, CONTROLE_ROL, RELER_ROL};
use crate::rede_p2p::Rede;
use crate::{descoberta, rol};
use std::net::SocketAddr;
use std::time::Instant;

/// O rol do disco, se for mais novo que o de `rede` E assinado pelo dono.
/// O arquivo e 0600, mas quem o grava e outro processo: conferir aqui custa
/// uma verificacao Ed25519 a cada `RELER_ROL`, e poupa confiar no disco.
pub(super) fn rol_do_disco_se_mais_novo(rede: &Rede, caminho: &str) -> Option<rol::Rol> {
    let dono = rede.dono?;
    let atual = rede.rol.as_ref().map_or(0, |r| r.versao);
    let d = Rede::ler(caminho).ok()?.rol?;
    match rol::avaliar(&d.para_bytes(), &dono, &rede.nome, atual) {
        rol::Veredito::Novo(r) => Some(r),
        _ => None,
    }
}

/// Tira da malha quem nao passa em `fica`, e conserta o mapa de indices de
/// sessao (que aponta para a POSICAO do par). As sessoes do par que sai
/// morrem com ele: pacote dele nao acha mais indice.
fn reter_pares(e: &mut Estado, fica: impl Fn(&Par) -> bool) {
    let mut nova_posicao = Vec::with_capacity(e.pares.len());
    let mut n = 0;
    for p in &e.pares {
        if fica(p) {
            nova_posicao.push(Some(n));
            n += 1;
        } else {
            nova_posicao.push(None);
        }
    }
    if n == e.pares.len() {
        return;
    }
    let mut i = 0;
    e.pares.retain(|_| {
        let fica = nova_posicao[i].is_some();
        i += 1;
        fica
    });
    e.indices = e
        .indices
        .drain()
        .filter_map(|(ind, pos)| nova_posicao.get(pos).copied().flatten().map(|np| (ind, np)))
        .collect();
}

impl No {
    /// A rede tem dono (rol assinado)?
    pub(super) fn assinada(&self) -> bool {
        self.rede
            .lock()
            .expect("rede")
            .as_ref()
            .is_some_and(|(r, _)| r.dono.is_some())
    }

    /// Em rede de rol assinado, com um rol ja aceito, a chave nao esta nele.
    /// Sem rol ainda (o convidado antes do primeiro), vale a lista -- que so
    /// tem o anfitriao do convite.
    pub(super) fn fora_do_rol(&self, chave: &[u8; 32]) -> bool {
        self.rede
            .lock()
            .expect("rede")
            .as_ref()
            .is_some_and(|(r, _)| {
                r.dono.is_some()
                    && r.rol
                        .as_ref()
                        .is_some_and(|rol| rol.membro(chave).is_none())
            })
    }

    pub(super) fn mensagem_do_rol(&self) -> Option<Vec<u8>> {
        let r = self.rede.lock().expect("rede");
        let rol = r.as_ref()?.0.rol.as_ref()?;
        let mut m = vec![CONTROLE, CONTROLE_ROL];
        m.extend_from_slice(&rol.para_bytes());
        Some(m)
    }

    /// Um par autenticado mandou um rol.
    pub(super) fn receber_rol(&self, e: &mut Estado, corpo: &[u8], de: usize) {
        let (veredito, atual) = {
            let r = self.rede.lock().expect("rede");
            let Some((rede, _)) = r.as_ref() else {
                return;
            };
            let Some(dono) = rede.dono else {
                return;
            };
            let atual = rede.rol.as_ref().map_or(0, |r| r.versao);
            (rol::avaliar(corpo, &dono, &rede.nome, atual), atual)
        };
        match veredito {
            rol::Veredito::Novo(novo) => {
                if let Some((rede, _)) = self.rede.lock().expect("rede").as_mut() {
                    rede.rol = Some(novo);
                }
                self.aplicar_rol(e);
            }
            // Quem mandou esta atrasado: recebe o nosso no proximo tique.
            // E assim que o rol chega a quem estava fora do ar.
            rol::Veredito::Velho(v) if v < atual => e.pares[de].ultimo_rol = None,
            _ => {}
        }
    }

    /// Faz a malha refletir o rol aceito: quem saiu dele sai da malha (e as
    /// sessoes com ele caem), quem entrou vira par (o endereco vem da lista
    /// de pares ou da descoberta), e o IP de cada um e o que o rol diz.
    pub(super) fn aplicar_rol(&self, e: &mut Estado) {
        use std::sync::atomic::Ordering;
        let Some(rol) = self
            .rede
            .lock()
            .expect("rede")
            .as_ref()
            .and_then(|(r, _)| r.dono.and(r.rol.clone()))
        else {
            return;
        };
        if rol.versao == self.rol_aplicado.load(Ordering::Relaxed) {
            return;
        }
        reter_pares(e, |p| rol.membro(&p.publica).is_some());
        for m in &rol.membros {
            if m.chave == self.minha_publica {
                continue;
            }
            match e.pares.iter_mut().find(|p| p.publica == m.chave) {
                Some(p) => p.ip = m.ip,
                None => {
                    if !e.pares.iter().any(|p| p.ip == m.ip) {
                        e.pares.push(No::par_novo(m.chave, m.ip, None));
                    }
                }
            }
        }
        if rol.membro(&self.minha_publica).is_none() {
            eprintln!(
                "phxvpn: este computador saiu do rol da rede (versao {})",
                rol.versao
            );
        }
        self.rol_aplicado.store(rol.versao, Ordering::Relaxed);
        No::avisar_malha(e);
        self.persistir(e);
    }

    /// A cada `RELER_ROL`, adota o rol do disco se for mais novo (o `p2p
    /// remover` grava ali), e aplica o que ainda nao foi aplicado.
    pub(super) fn sincronizar_rol(&self, e: &mut Estado) {
        if !self.assinada() {
            return;
        }
        {
            let mut lido = self.rol_lido.lock().expect("rol lido");
            if lido.map_or(true, |t| t.elapsed() >= RELER_ROL) {
                *lido = Some(Instant::now());
                let mut r = self.rede.lock().expect("rede");
                if let Some((rede, caminho)) = r.as_mut() {
                    if let Some(d) = rol_do_disco_se_mais_novo(rede, caminho) {
                        rede.rol = Some(d);
                    }
                }
            }
        }
        self.aplicar_rol(e);
    }

    /// Manda o anuncio na LAN, se a descoberta esta ligada e ja deu a hora.
    pub(super) fn anunciar(&self) {
        let p = match self.descoberta.lock().expect("descoberta").as_mut() {
            Some(d) => d.anuncio_se_ja_e_hora(&self.minha_publica),
            None => return,
        };
        let (Some(p), Ok(local)) = (p, self.udp.local_addr()) else {
            return;
        };
        for destino in descoberta::destinos(local.port()) {
            self.enviar(destino, &p);
        }
    }

    /// Um anuncio chegou: se e de um par conhecido sem sessao viva, o endereco
    /// dele na LAN e onde o aperto vai. O interruptor e o primeiro a olhar.
    pub(super) fn receber_anuncio(&self, dado: &[u8], de: SocketAddr) {
        let chave = match self.descoberta.lock().expect("descoberta").as_mut() {
            Some(d) => d.abrir(dado, de),
            None => return,
        };
        let Some(chave) = chave.filter(|k| *k != self.minha_publica) else {
            return;
        };
        let mut e = self.estado.lock().expect("estado");
        let Some(i) = e.pares.iter().position(|p| p.publica == chave) else {
            return;
        };
        if self.fora_do_rol(&chave) {
            return;
        }
        let par = &mut e.pares[i];
        if par
            .atual
            .as_ref()
            .is_some_and(|s| s.confirmada && !s.expirada())
        {
            return;
        }
        par.endereco = Some(de);
        par.via = Some(Via::Direta(de));
        par.tentativas_diretas = 0;
        self.iniciar_aperto(&mut e, i);
    }
}

#[cfg(test)]
mod testes {
    use super::super::*;
    use crate::rede_p2p;

    fn ip(origem: [u8; 4], destino: [u8; 4], carga: &[u8]) -> Vec<u8> {
        let mut p = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0];
        p.extend_from_slice(&origem);
        p.extend_from_slice(&destino);
        p.extend_from_slice(carga);
        p
    }

    fn bombear(no: &No) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; 2048];
        let (n, de) = no.udp.recv_from(&mut buf).ok()?;
        no.da_rede(&buf[..n], de)
    }

    fn tentados(no: &No) -> u64 {
        no.apertos_tentados
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Um no ligado a um arquivo de rede numa pasta temporaria propria.
    struct Montado {
        no: No,
        end: SocketAddr,
        publica: [u8; 32],
    }

    fn pasta(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "phxvpn-{nome}-{}-{}",
            std::process::id(),
            phxsql_core::cifra::sortear_u64()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// `k`: identidade; `ip`: o proprio; `conhece`: (publica, ip, endereco)
    /// dos pares que o no ja tem na lista; `dono`/`rol`: None = rede antiga.
    fn no_com_rede(
        dir: &std::path::Path,
        k: [u8; 32],
        ip: &str,
        conhece: &[([u8; 32], &str, Option<SocketAddr>)],
        dono: Option<[u8; 32]>,
        rol: Option<rol::Rol>,
    ) -> Montado {
        let u = UdpSocket::bind("127.0.0.1:0").unwrap();
        u.set_read_timeout(Some(Duration::from_millis(60))).unwrap();
        let end = u.local_addr().unwrap();
        let mut rede = Rede::nova("R", ip.parse().unwrap(), 24, 0);
        rede.dono = dono;
        rede.rol = rol;
        let caminho = dir
            .join(format!("{}.p2p", ip.replace('.', "_")))
            .to_str()
            .unwrap()
            .to_string();
        rede.gravar(&caminho).unwrap();
        let pares = conhece
            .iter()
            .map(|(p, i, e)| ParConfig {
                publica: *p,
                ip: i.parse().unwrap(),
                endereco: *e,
            })
            .collect();
        let no = No::novo(
            k,
            psk_da_rede("R", "s", 1_000),
            ip.parse().unwrap(),
            u,
            pares,
        )
        .com_rede(rede, caminho);
        Montado {
            publica: x25519::chave_publica(&k),
            no,
            end,
        }
    }

    fn membro(k: &[u8; 32], ip: &str) -> rol::Membro {
        rol::Membro {
            chave: x25519::chave_publica(k),
            ip: ip.parse().unwrap(),
            nome: None,
        }
    }

    /// Bombeia todos ate `feito` valer (ou desistir).
    fn girar(nos: &[&No], feito: &dyn Fn() -> bool) -> bool {
        for _ in 0..80 {
            if feito() {
                return true;
            }
            for n in nos {
                bombear(n);
            }
        }
        feito()
    }

    fn sessao_com(no: &No, publica: &[u8; 32]) -> bool {
        no.estado
            .lock()
            .unwrap()
            .pares
            .iter()
            .any(|p| p.publica == *publica && p.atual.as_ref().is_some_and(|s| s.confirmada))
    }

    fn conhece(no: &No, publica: &[u8; 32]) -> bool {
        no.estado
            .lock()
            .unwrap()
            .pares
            .iter()
            .any(|p| p.publica == *publica)
    }

    fn versao(no: &No) -> u64 {
        no.rede
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|(r, _)| r.rol.as_ref().map(|x| x.versao))
            .unwrap_or(0)
    }

    /// Estar na lista de pares nao basta em rede de rol assinado: a chave
    /// que nao esta no rol aceito nao fecha aperto (o no ainda nem limpou a
    /// lista -- e a conferencia da admissao que segura).
    #[test]
    fn par_fora_do_rol_nao_fecha_aperto() {
        let dir = pasta("fora-do-rol");
        let (ka, kb, kx) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let dono = rol::publica_do_dono(&ka, "R");
        let r = rol::Rol::primeiro("R", membro(&ka, "10.78.0.1"), &ka)
            .unwrap()
            .com(membro(&kb, "10.78.0.2"), &ka)
            .unwrap();
        let px = x25519::chave_publica(&kx);
        let b = no_com_rede(
            &dir,
            kb,
            "10.78.0.2",
            &[(px, "10.78.0.9", None)],
            Some(dono),
            Some(r),
        );
        let x = no_com_rede(
            &dir,
            kx,
            "10.78.0.9",
            &[(b.publica, "10.78.0.2", Some(b.end))],
            None,
            None,
        );
        x.no.da_placa(&ip([10, 78, 0, 9], [10, 78, 0, 2], b"oi"));
        bombear(&b.no);
        assert_eq!(tentados(&b.no), 1, "o INICIO de X tinha de chegar ao Noise");
        assert!(
            bombear(&x.no).is_none() && !sessao_com(&x.no, &b.publica),
            "par fora do rol fechou aperto"
        );
        // E no primeiro tique a lista se alinha ao rol: X sai dela.
        b.no.tique();
        assert!(!conhece(&b.no, &px));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Em rede de rol assinado, a lista de pares de um membro nao apresenta
    /// ninguem: so ensina endereco de quem ja esta no rol.
    #[test]
    fn lista_de_pares_nao_injeta_par_em_rede_assinada() {
        let dir = pasta("injeta");
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let dono = rol::publica_do_dono(&ka, "R");
        let r = rol::Rol::primeiro("R", membro(&ka, "10.78.0.1"), &ka)
            .unwrap()
            .com(membro(&kb, "10.78.0.2"), &ka)
            .unwrap();
        let b = no_com_rede(&dir, kb, "10.78.0.2", &[], Some(dono), Some(r));
        let intruso = rede_p2p::Par {
            chave: [0x55; 32],
            ip: "10.78.0.66".parse().unwrap(),
            endereco: Some("127.0.0.1:9".into()),
        };
        let lista = rede_p2p::pares_json(&[intruso]).escrever();
        let mut e = b.no.estado.lock().unwrap();
        assert!(!b.no.aprender_rol(&mut e, lista.as_bytes()));
        assert!(
            !e.pares.iter().any(|p| p.publica == [0x55; 32]),
            "a lista injetou par"
        );
        drop(e);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Comportamento VELHO: rede sem dono (criada antes do rol) continua com
    /// a confianca transitiva -- a lista de um membro apresenta outro.
    #[test]
    fn rede_sem_dono_aprende_par_pela_lista_como_antes() {
        let dir = pasta("velha");
        let kb = x25519::gerar_privada();
        let b = no_com_rede(&dir, kb, "10.78.0.2", &[], None, None);
        let novo = rede_p2p::Par {
            chave: [0x44; 32],
            ip: "10.78.0.7".parse().unwrap(),
            endereco: None,
        };
        let lista = rede_p2p::pares_json(&[novo]).escrever();
        let mut e = b.no.estado.lock().unwrap();
        assert!(b.no.aprender_rol(&mut e, lista.as_bytes()));
        assert!(e.pares.iter().any(|p| p.publica == [0x44; 32]));
        drop(e);
        // E a mensagem do rol nao existe numa rede sem dono.
        assert!(b.no.mensagem_do_rol().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// O dono admite pelo convite e assina o rol com o convidado (e o
    /// apelido dele); o convidado recebe o rol pela malha e o adota.
    #[test]
    fn convite_em_rede_assinada_poe_o_convidado_no_rol() {
        let dir = pasta("convite-rol");
        let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let dono = rol::publica_do_dono(&ka, "R");
        let r = rol::Rol::primeiro("R", membro(&ka, "10.78.0.1"), &ka).unwrap();
        let a = no_com_rede(&dir, ka, "10.78.0.1", &[], Some(dono), Some(r));
        let caminho_a = a.no.rede.lock().unwrap().as_ref().unwrap().1.clone();
        let mut ra = Rede::ler(&caminho_a).unwrap();
        let psk = psk_da_rede("R", "s", 1_000);
        let codigo = rede_p2p::convidar(&mut ra, &psk, a.publica, None, 60).unwrap();
        ra.gravar(&caminho_a).unwrap();
        let c = rede_p2p::abrir_convite(&codigo, &psk).unwrap();
        assert_eq!(c.dono, Some(dono), "o convite leva a chave do dono");
        let ub = UdpSocket::bind("127.0.0.1:0").unwrap();
        ub.set_read_timeout(Some(Duration::from_millis(60)))
            .unwrap();
        let mut rb = rede_p2p::rede_do_convidado(&c, 0);
        rb.apelido = Some("filial".into());
        let b = No::novo(
            kb,
            psk,
            c.ip_convidado,
            ub,
            vec![ParConfig {
                publica: a.publica,
                ip: "10.78.0.1".parse().unwrap(),
                endereco: Some(a.end),
            }],
        )
        .com_rede(rb, dir.join("B.p2p").to_str().unwrap().to_string());
        b.tique();
        assert!(girar(&[&a.no, &b], &|| versao(&b) > 0
            && versao(&b) == versao(&a.no)));
        let rol_b = b
            .rede
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .0
            .rol
            .clone()
            .unwrap();
        let eu = rol_b.membro(&x25519::chave_publica(&kb)).expect("B no rol");
        assert_eq!(eu.nome.as_deref(), Some("filial"));
        assert_eq!(eu.ip, c.ip_convidado);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Tres membros; o dono remove C. O rol novo chega a B pela malha, e B
    /// derruba C: o dado de C nao entra mais na placa de B, e a sessao
    /// A-B continua.
    #[test]
    fn rol_sem_o_membro_derruba_o_tunel_com_ele() {
        let dir = pasta("remover");
        let (ka, kb, kc) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let dono = rol::publica_do_dono(&ka, "R");
        let r = rol::Rol::primeiro("R", membro(&ka, "10.78.0.1"), &ka)
            .unwrap()
            .com(membro(&kb, "10.78.0.2"), &ka)
            .unwrap()
            .com(membro(&kc, "10.78.0.3"), &ka)
            .unwrap();
        let (pa, pb, pc) = (
            x25519::chave_publica(&ka),
            x25519::chave_publica(&kb),
            x25519::chave_publica(&kc),
        );
        let b = no_com_rede(
            &dir,
            kb,
            "10.78.0.2",
            &[(pa, "10.78.0.1", None), (pc, "10.78.0.3", None)],
            Some(dono),
            Some(r.clone()),
        );
        let a = no_com_rede(
            &dir,
            ka,
            "10.78.0.1",
            &[(pb, "10.78.0.2", Some(b.end)), (pc, "10.78.0.3", None)],
            Some(dono),
            Some(r.clone()),
        );
        let c = no_com_rede(
            &dir,
            kc,
            "10.78.0.3",
            &[(pa, "10.78.0.1", None), (pb, "10.78.0.2", Some(b.end))],
            Some(dono),
            Some(r.clone()),
        );
        let nos = [&a.no, &b.no, &c.no];
        a.no.da_placa(&ip([10, 78, 0, 1], [10, 78, 0, 2], b"a"));
        c.no.da_placa(&ip([10, 78, 0, 3], [10, 78, 0, 2], b"c"));
        assert!(girar(&nos, &|| sessao_com(&b.no, &pa) && sessao_com(&b.no, &pc)));
        // O dono grava o rol sem C (o que `p2p remover` faz) e o no o le.
        let caminho_a = a.no.rede.lock().unwrap().as_ref().unwrap().1.clone();
        let mut ra = Rede::ler(&caminho_a).unwrap();
        ra.rol = Some(r.sem(&pc, &ka).unwrap());
        ra.gravar(&caminho_a).unwrap();
        a.no.tique();
        assert!(!conhece(&a.no, &pc), "o dono nao tirou C");
        a.no.tique(); // manda o rol a B
        assert!(
            girar(&nos, &|| !conhece(&b.no, &pc)),
            "B nao recebeu o rol novo"
        );
        // O dado de C nao entra mais em B; o de A entra.
        let depois = ip([10, 78, 0, 3], [10, 78, 0, 2], b"depois");
        c.no.da_placa(&depois);
        for _ in 0..5 {
            assert!(
                bombear(&b.no).as_deref() != Some(&depois[..]),
                "C removido ainda entrou na placa de B"
            );
        }
        let ida = ip([10, 78, 0, 1], [10, 78, 0, 2], b"a-depois");
        a.no.da_placa(&ida);
        let mut chegou = false;
        for _ in 0..10 {
            if bombear(&b.no).as_deref() == Some(&ida[..]) {
                chegou = true;
                break;
            }
        }
        assert!(chegou, "a sessao A-B caiu junto");
        // Anti-rollback: o rol velho (com C), reapresentado, nao o traz de volta.
        let mut e = b.no.estado.lock().unwrap();
        let velho = r.para_bytes();
        b.no.receber_rol(&mut e, &velho, 0);
        drop(e);
        b.no.tique();
        assert!(!conhece(&b.no, &pc), "o rol velho ressuscitou C");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Descoberta: o anuncio de A (aqui mandado direto, no lugar do
    /// broadcast) leva B a fazer o aperto no endereco de onde ele veio. Com
    /// a descoberta desligada em B, nada acontece.
    #[test]
    fn anuncio_leva_ao_aperto_e_desligada_nao() {
        for ligada in [true, false] {
            let (ka, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
            let (ua, ub) = (
                UdpSocket::bind("127.0.0.1:0").unwrap(),
                UdpSocket::bind("127.0.0.1:0").unwrap(),
            );
            for u in [&ua, &ub] {
                u.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
            }
            let eb = ub.local_addr().unwrap();
            let psk = psk_da_rede("R", "s", 1_000);
            let a = No::novo(
                ka,
                psk,
                "10.78.0.1".parse().unwrap(),
                ua,
                vec![ParConfig {
                    publica: x25519::chave_publica(&kb),
                    ip: "10.78.0.2".parse().unwrap(),
                    endereco: None,
                }],
            )
            .com_descoberta(true);
            let b = No::novo(
                kb,
                psk,
                "10.78.0.2".parse().unwrap(),
                ub,
                vec![ParConfig {
                    publica: x25519::chave_publica(&ka),
                    ip: "10.78.0.1".parse().unwrap(),
                    endereco: None,
                }],
            )
            .com_descoberta(ligada);
            let anuncio = a
                .descoberta
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .anuncio_se_ja_e_hora(&a.minha_publica)
                .unwrap();
            a.udp.send_to(&anuncio, eb).unwrap();
            bombear(&b); // B ouve o anuncio
            let ok = girar(&[&a, &b], &|| sessao_com(&b, &a.minha_publica));
            assert_eq!(ok, ligada, "descoberta ligada={ligada}");
            if !ligada {
                assert_eq!(tentados(&a), 0, "B desligado respondeu ao anuncio");
            }
        }
    }

    /// Anuncio de chave que nao e par (quem tem a senha mas nao esta na
    /// lista, ou saiu do rol) nao provoca aperto nenhum.
    #[test]
    fn anuncio_de_estranho_nao_provoca_resposta() {
        let (kx, kb) = (x25519::gerar_privada(), x25519::gerar_privada());
        let psk = psk_da_rede("R", "s", 1_000);
        let ux = UdpSocket::bind("127.0.0.1:0").unwrap();
        ux.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let ub = UdpSocket::bind("127.0.0.1:0").unwrap();
        ub.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let eb = ub.local_addr().unwrap();
        let x = No::novo(kx, psk, "10.78.0.9".parse().unwrap(), ux, vec![]).com_descoberta(true);
        let b = No::novo(kb, psk, "10.78.0.2".parse().unwrap(), ub, vec![]).com_descoberta(true);
        let anuncio = x
            .descoberta
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .anuncio_se_ja_e_hora(&x.minha_publica)
            .unwrap();
        x.udp.send_to(&anuncio, eb).unwrap();
        bombear(&b);
        let mut buf = [0u8; 512];
        assert!(
            x.udp.recv_from(&mut buf).is_err(),
            "B respondeu a um estranho"
        );
    }
}
