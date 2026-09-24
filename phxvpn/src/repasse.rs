//! O servidor intermediario (repasse) do modo P2P.
//!
//! Decisao do dono (23/09/2026): quando os dois lados estao atras de CGNAT e
//! nao ha caminho direto, o trafego passa por um servidor NOSSO -- que so
//! carrega pacote que ja vem cifrado de ponta a ponta pelo Noise dos pares.
//! O repasse nao tem a PSK nem as chaves de sessao: ve quem fala com quem
//! (as chaves publicas) e o tamanho, nunca o conteudo.
//!
//! # Pacotes (alem dos do `transporte.rs`)
//!
//! ```text
//! REGISTRO [7,0,0,0] chave_do_no:32 carimbo:12 mac:32
//! PARA     [8,0,0,0] chave_destino:32 | pacote do par (INICIO/RESPOSTA/DADOS)
//! DE       [9,0,0,0] chave_origem:32  | pacote do par
//! ```
//!
//! # Por que o REGISTRO prova a chave sem ida-e-volta
//!
//! O `mac` e HMAC-SHA256 com a chave `DH(no, repasse)`: so quem tem a privada
//! do no (ou a do repasse) calcula. Sem isso, qualquer um se registraria com a
//! chave publica de outro e receberia o trafego dele -- cifrado, mas desviado
//! (negacao de servico). O carimbo crescente impede reenviar um REGISTRO
//! gravado de outro endereco.
//!
//! # Contas com usuario e senha (`--contas`)
//!
//! O repasse e a unica porta do phxvpn exposta na internet sem senha nenhuma:
//! qualquer um com o programa podia usar o servidor da empresa. Com
//! `--contas`, o REGISTRO leva tambem um usuario e um HMAC com a CREDENCIAL
//! dele -- `PBKDF2(senha, "phxvpn-repasse:" + usuario)`, derivada uma vez no
//! no. A senha nunca viaja; o repasse guarda so a credencial (0600) e confere
//! com um HMAC. A conferencia barata vem ANTES do Diffie-Hellman (86 us):
//! quem nao tem conta e recusado sem custar DH. Erros contam no mesmo
//! limitador do painel (`guarda.rs`), por usuario e por IP. Sem `--contas`,
//! o repasse continua aberto -- e diz isso ao ligar.
//!
//! # Por que o repasse nao confia na chave que o PARA declara
//!
//! A origem sai do ENDERECO que registrou, nao do pacote: um no nao consegue
//! mandar em nome de outro. E o no que recebe ainda confere que a chave de
//! origem bate com a do aperto -- o repasse mentiroso nao engana o Noise.
//!
//! # Apresentador da perfuracao de NAT
//!
//! O repasse tambem responde APRESENTAR (ver `p2p/perfuracao.rs`): conta a
//! cada lado de um pedido MUTUO o endereco publico do outro, para os dois
//! tentarem o caminho direto. O segredo do registro fica guardado para que
//! essa conferencia custe um HMAC, nunca um Diffie-Hellman.

use crate::guarda::Limitador;
use crate::p2p::perfuracao;
use phxsql_core::hash::{de_hex, hmac_sha256, iguais_em_tempo_constante, para_hex, pbkdf2_sha256};
use phxsql_core::x25519;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub const TIPO_REGISTRO: u8 = 7;
pub const TIPO_PARA: u8 = 8;
pub const TIPO_DE: u8 = 9;

pub const REGISTRO_LEN: usize = 4 + 32 + 12 + 32;
/// O no renova o registro nesse ritmo -- tambem mantem aberto o furo do NAT.
pub const RENOVAR_REGISTRO: Duration = Duration::from_secs(20);
/// Registro sem renovacao nesse prazo sai da tabela.
pub const VALIDADE_REGISTRO: Duration = Duration::from_secs(60);
/// Teto de nos na tabela: registro valido custa um DH, mas ocupa memoria.
pub const TETO_NOS: usize = 100_000;

/// Custo da credencial de conta (derivada uma vez, no no e ao cadastrar).
pub const ITERACOES_CONTA: u32 = 310_000;

/// Uma conta do repasse, do lado do no: o usuario e a credencial derivada.
#[derive(Clone)]
pub struct Conta {
    pub usuario: String,
    pub credencial: [u8; 32],
}

pub fn credencial(usuario: &str, senha: &str, iteracoes: u32) -> [u8; 32] {
    let mut c = [0u8; 32];
    let sal = format!("phxvpn-repasse:{usuario}");
    pbkdf2_sha256(senha.as_bytes(), sal.as_bytes(), iteracoes, &mut c);
    c
}

pub fn validar_usuario(u: &str) -> Result<(), String> {
    let ok = (2..=32).contains(&u.len())
        && u.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b));
    if ok {
        Ok(())
    } else {
        Err("usuario do repasse: de 2 a 32 caracteres, so a-z, 0-9, ponto, _ e -".into())
    }
}

fn mac_conta(
    credencial: &[u8; 32],
    chave: &[u8; 32],
    carimbo: &[u8; 12],
    usuario: &str,
) -> [u8; 32] {
    let mut m = b"phxvpn-repasse-conta".to_vec();
    m.extend_from_slice(chave);
    m.extend_from_slice(carimbo);
    m.extend_from_slice(usuario.as_bytes());
    hmac_sha256(credencial, &m)
}

/// O arquivo de contas: `usuario credencial_hex` por linha.
pub fn ler_contas(caminho: &str) -> Result<HashMap<String, [u8; 32]>, String> {
    let t = std::fs::read_to_string(caminho).map_err(|e| format!("{caminho}: {e}"))?;
    let mut contas = HashMap::new();
    for (n, linha) in t.lines().enumerate() {
        let linha = linha.trim();
        if linha.is_empty() || linha.starts_with('#') {
            continue;
        }
        let (u, c) = linha
            .split_once(' ')
            .ok_or_else(|| format!("{caminho}:{}: linha torta", n + 1))?;
        let c: [u8; 32] = de_hex(c.trim())
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| format!("{caminho}:{}: credencial torta", n + 1))?;
        contas.insert(u.to_string(), c);
    }
    Ok(contas)
}

/// Inclui ou troca a senha de uma conta no arquivo (0600).
pub fn gravar_conta(caminho: &str, usuario: &str, senha: &str) -> Result<(), String> {
    validar_usuario(usuario)?;
    if senha.chars().count() < 10 {
        return Err("senha do repasse: no minimo 10 caracteres".into());
    }
    let mut contas = if std::path::Path::new(caminho).exists() {
        ler_contas(caminho)?
    } else {
        HashMap::new()
    };
    contas.insert(
        usuario.to_string(),
        credencial(usuario, senha, ITERACOES_CONTA),
    );
    let mut linhas: Vec<String> = contas
        .iter()
        .map(|(u, c)| format!("{u} {}", para_hex(c)))
        .collect();
    linhas.sort();
    let tmp = format!("{caminho}.tmp");
    let _ = std::fs::remove_file(&tmp);
    crate::comandos::gravar_secreto(&tmp, (linhas.join("\n") + "\n").as_bytes(), true)?;
    std::fs::rename(&tmp, caminho).map_err(|e| format!("{caminho}: {e}"))
}

fn mac(segredo: &[u8; 32], chave: &[u8; 32], carimbo: &[u8; 12]) -> [u8; 32] {
    let mut m = b"phxvpn-repasse-registro".to_vec();
    m.extend_from_slice(chave);
    m.extend_from_slice(carimbo);
    hmac_sha256(segredo, &m)
}

/// O REGISTRO que o no manda ao repasse. Com conta, acrescenta
/// `tamanho:1 usuario mac_conta:32` depois dos 80 bytes de sempre.
pub fn registro(
    privada_no: &[u8; 32],
    publica_repasse: &[u8; 32],
    carimbo: [u8; 12],
    conta: Option<&Conta>,
) -> Result<Vec<u8>, String> {
    let segredo = x25519::segredo(privada_no, publica_repasse).map_err(|e| e.to_string())?;
    let chave = x25519::chave_publica(privada_no);
    let mut p = vec![TIPO_REGISTRO, 0, 0, 0];
    p.extend_from_slice(&chave);
    p.extend_from_slice(&carimbo);
    p.extend_from_slice(&mac(&segredo, &chave, &carimbo));
    if let Some(c) = conta {
        p.push(c.usuario.len() as u8);
        p.extend_from_slice(c.usuario.as_bytes());
        p.extend_from_slice(&mac_conta(&c.credencial, &chave, &carimbo, &c.usuario));
    }
    Ok(p)
}

pub fn embrulhar_para(destino: &[u8; 32], pacote: &[u8]) -> Vec<u8> {
    let mut p = vec![TIPO_PARA, 0, 0, 0];
    p.extend_from_slice(destino);
    p.extend_from_slice(pacote);
    p
}

/// `DE` -> (chave de origem, pacote do par).
pub fn desembrulhar_de(p: &[u8]) -> Option<([u8; 32], &[u8])> {
    (p.len() > 36 && p[..4] == [TIPO_DE, 0, 0, 0])
        .then(|| (p[4..36].try_into().expect("32"), &p[36..]))
}

struct Registro {
    endereco: SocketAddr,
    carimbo: [u8; 12],
    visto: Instant,
    /// `DH(no, repasse)`, ja pago no registro: prova o APRESENTAR por HMAC.
    segredo: [u8; 32],
}

pub struct Repasse {
    privada: [u8; 32],
    nos: HashMap<[u8; 32], Registro>,
    por_endereco: HashMap<SocketAddr, [u8; 32]>,
    /// Com lista, so repassa entre chaves dela (repasse fechado da empresa).
    permitidas: Option<HashSet<[u8; 32]>>,
    /// Com contas, so registra quem prova usuario e senha.
    contas: Option<HashMap<String, [u8; 32]>>,
    tentativas: Limitador,
    /// Pedidos de apresentacao (perfuracao de NAT).
    pub mesa: perfuracao::Mesa,
}

impl Repasse {
    pub fn novo(privada: [u8; 32], permitidas: Option<HashSet<[u8; 32]>>) -> Repasse {
        Repasse {
            privada,
            nos: HashMap::new(),
            por_endereco: HashMap::new(),
            permitidas,
            contas: None,
            tentativas: Limitador::default(),
            mesa: perfuracao::Mesa::default(),
        }
    }

    /// Liga o controle por usuario e senha.
    pub fn com_contas(mut self, contas: HashMap<String, [u8; 32]>) -> Repasse {
        self.contas = Some(contas);
        self
    }

    pub fn publica(&self) -> [u8; 32] {
        x25519::chave_publica(&self.privada)
    }

    fn permitida(&self, k: &[u8; 32]) -> bool {
        self.permitidas.as_ref().map_or(true, |l| l.contains(k))
    }

    fn vivo(&self, k: &[u8; 32]) -> Option<&Registro> {
        self.nos
            .get(k)
            .filter(|r| r.visto.elapsed() < VALIDADE_REGISTRO)
    }

    /// Trata um datagrama. Devolve o que mandar e para onde.
    pub fn tratar(&mut self, dado: &[u8], de: SocketAddr) -> Option<(SocketAddr, Vec<u8>)> {
        match *dado.first()? {
            TIPO_REGISTRO => {
                self.registrar(dado, de);
                None
            }
            TIPO_PARA => {
                if dado.len() <= 36 {
                    return None;
                }
                let origem = *self.por_endereco.get(&de)?;
                self.vivo(&origem).filter(|r| r.endereco == de)?;
                let destino: [u8; 32] = dado[4..36].try_into().ok()?;
                if !self.permitida(&destino) {
                    return None;
                }
                let alvo = self.vivo(&destino)?.endereco;
                let mut saida = vec![TIPO_DE, 0, 0, 0];
                saida.extend_from_slice(&origem);
                saida.extend_from_slice(&dado[36..]);
                Some((alvo, saida))
            }
            perfuracao::TIPO_APRESENTAR => {
                // Mesma atribuicao do PARA: a origem sai do endereco que
                // registrou, e a resposta so volta para ele.
                let origem = *self.por_endereco.get(&de)?;
                let segredo = self.vivo(&origem).filter(|r| r.endereco == de)?.segredo;
                let par = perfuracao::par_do_pedido(dado)?;
                if !self.permitida(&par) {
                    return None;
                }
                let alvo = self.vivo(&par)?.endereco;
                let resposta = self.mesa.pedir(dado, origem, &segredo, alvo)?;
                Some((de, resposta))
            }
            _ => None,
        }
    }

    fn registrar(&mut self, dado: &[u8], de: SocketAddr) {
        if dado.len() < REGISTRO_LEN || dado[..4] != [TIPO_REGISTRO, 0, 0, 0] {
            return;
        }
        let chave: [u8; 32] = dado[4..36].try_into().expect("32");
        let carimbo: [u8; 12] = dado[36..48].try_into().expect("12");
        if !self.permitida(&chave) {
            return;
        }
        if let Some(contas) = &self.contas {
            // Conta primeiro: e so um HMAC; quem nao a tem nao custa DH.
            let resto = &dado[REGISTRO_LEN..];
            let Some((&n, resto)) = resto.split_first() else {
                return;
            };
            if resto.len() != n as usize + 32 {
                return;
            }
            let Ok(usuario) = std::str::from_utf8(&resto[..n as usize]) else {
                return;
            };
            let chave_ip = format!("ip:{}", de.ip());
            let chave_u = format!("usuario:{usuario}");
            if self
                .tentativas
                .antes_de_todas(&[&chave_ip, &chave_u])
                .is_err()
            {
                return;
            }
            let bate = contas.get(usuario).is_some_and(|cred| {
                iguais_em_tempo_constante(
                    &mac_conta(cred, &chave, &carimbo, usuario),
                    &resto[n as usize..],
                )
            });
            if !bate {
                self.tentativas.falhou(&chave_ip);
                self.tentativas.falhou(&chave_u);
                return;
            }
            self.tentativas.acertou(&chave_u);
        } else if dado.len() != REGISTRO_LEN && dado.len() < REGISTRO_LEN + 1 + 2 + 32 {
            return;
        }
        let Ok(segredo) = x25519::segredo(&self.privada, &chave) else {
            return;
        };
        if !iguais_em_tempo_constante(&mac(&segredo, &chave, &carimbo), &dado[48..80]) {
            return;
        }
        if let Some(r) = self.nos.get(&chave) {
            if carimbo <= r.carimbo {
                return;
            }
        } else if self.nos.len() >= TETO_NOS {
            self.nos
                .retain(|_, r| r.visto.elapsed() < VALIDADE_REGISTRO);
            if self.nos.len() >= TETO_NOS {
                return;
            }
        }
        if let Some(velho) = self.nos.get(&chave) {
            self.por_endereco.remove(&velho.endereco);
        }
        self.por_endereco.insert(de, chave);
        self.nos.insert(
            chave,
            Registro {
                endereco: de,
                carimbo,
                visto: Instant::now(),
                segredo,
            },
        );
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn end(p: u16) -> SocketAddr {
        SocketAddr::from(([198, 51, 100, 1], p))
    }

    fn carimbo(n: u8) -> [u8; 12] {
        let mut c = [0u8; 12];
        c[11] = n;
        c
    }

    #[test]
    fn repassa_entre_registrados_com_a_origem_do_endereco() {
        let r_priv = x25519::gerar_privada();
        let mut r = Repasse::novo(r_priv, None);
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (pa, pb) = (x25519::chave_publica(&a), x25519::chave_publica(&b));
        r.tratar(
            &registro(&a, &r.publica(), carimbo(1), None).unwrap(),
            end(1),
        );
        r.tratar(
            &registro(&b, &r.publica(), carimbo(1), None).unwrap(),
            end(2),
        );
        let (alvo, saida) = r.tratar(&embrulhar_para(&pb, b"oi"), end(1)).unwrap();
        assert_eq!(alvo, end(2));
        assert_eq!(desembrulhar_de(&saida), Some((pa, &b"oi"[..])));
        // Endereco nao registrado nao manda nada.
        assert!(r.tratar(&embrulhar_para(&pb, b"oi"), end(9)).is_none());
    }

    #[test]
    fn registro_com_chave_alheia_ou_repetido_e_recusado() {
        let mut r = Repasse::novo(x25519::gerar_privada(), None);
        let (a, b, m) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let pb = x25519::chave_publica(&b);
        r.tratar(
            &registro(&b, &r.publica(), carimbo(5), None).unwrap(),
            end(2),
        );
        // O atacante M tenta se registrar como B: poe a chave de B com o mac dele.
        let mut falso = registro(&m, &r.publica(), carimbo(9), None).unwrap();
        falso[4..36].copy_from_slice(&pb);
        r.tratar(&falso, end(66));
        // Reenvio do registro legitimo (carimbo velho) de outro endereco.
        r.tratar(
            &registro(&b, &r.publica(), carimbo(5), None).unwrap(),
            end(67),
        );
        r.tratar(
            &registro(&a, &r.publica(), carimbo(1), None).unwrap(),
            end(1),
        );
        let (alvo, _) = r.tratar(&embrulhar_para(&pb, b"x"), end(1)).unwrap();
        assert_eq!(alvo, end(2), "o trafego de B continua indo para B");
    }

    fn conta(u: &str, senha: &str) -> Conta {
        Conta {
            usuario: u.into(),
            credencial: credencial(u, senha, 1_000),
        }
    }

    /// Com contas: sem conta, com senha errada e com usuario inventado nao
    /// registra; com a conta certa, registra e repassa.
    #[test]
    fn contas_exigem_usuario_e_senha() {
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let pb = x25519::chave_publica(&b);
        let mut contas = HashMap::new();
        contas.insert(
            "filial".to_string(),
            credencial("filial", "senha-do-repasse", 1_000),
        );
        let mut r = Repasse::novo(x25519::gerar_privada(), None).com_contas(contas);
        let rp = r.publica();
        r.tratar(
            &registro(
                &b,
                &rp,
                carimbo(1),
                Some(&conta("filial", "senha-do-repasse")),
            )
            .unwrap(),
            end(2),
        );
        // A tenta de tres jeitos errados:
        r.tratar(&registro(&a, &rp, carimbo(1), None).unwrap(), end(1));
        assert!(
            r.tratar(&embrulhar_para(&pb, b"x"), end(1)).is_none(),
            "sem conta registrou"
        );
        r.tratar(
            &registro(&a, &rp, carimbo(2), Some(&conta("filial", "errada"))).unwrap(),
            end(1),
        );
        assert!(
            r.tratar(&embrulhar_para(&pb, b"x"), end(1)).is_none(),
            "senha errada registrou"
        );
        r.tratar(
            &registro(&a, &rp, carimbo(3), Some(&conta("inventado", "x"))).unwrap(),
            end(1),
        );
        assert!(
            r.tratar(&embrulhar_para(&pb, b"x"), end(1)).is_none(),
            "usuario inventado registrou"
        );
        r.tratar(
            &registro(
                &a,
                &rp,
                carimbo(4),
                Some(&conta("filial", "senha-do-repasse")),
            )
            .unwrap(),
            end(1),
        );
        assert!(
            r.tratar(&embrulhar_para(&pb, b"x"), end(1)).is_some(),
            "a conta certa nao registrou"
        );
    }

    #[test]
    fn chutar_a_senha_bloqueia() {
        let mut contas = HashMap::new();
        contas.insert(
            "filial".to_string(),
            credencial("filial", "senha-do-repasse", 1_000),
        );
        let mut r = Repasse::novo(x25519::gerar_privada(), None).com_contas(contas);
        let rp = r.publica();
        let a = x25519::gerar_privada();
        for i in 0..8u8 {
            r.tratar(
                &registro(
                    &a,
                    &rp,
                    carimbo(i + 1),
                    Some(&conta("filial", &format!("chute{i}"))),
                )
                .unwrap(),
                end(1),
            );
        }
        // Agora nem a senha certa passa deste IP: o bloqueio vale.
        r.tratar(
            &registro(
                &a,
                &rp,
                carimbo(20),
                Some(&conta("filial", "senha-do-repasse")),
            )
            .unwrap(),
            end(1),
        );
        let b = x25519::gerar_privada();
        r.tratar(
            &registro(
                &b,
                &rp,
                carimbo(1),
                Some(&conta("filial", "senha-do-repasse")),
            )
            .unwrap(),
            end(2),
        );
        assert!(r
            .tratar(&embrulhar_para(&x25519::chave_publica(&b), b"x"), end(1))
            .is_none());
    }

    #[test]
    fn arquivo_de_contas_ida_e_volta() {
        let c = std::env::temp_dir().join(format!("phxvpn-contas-{}", std::process::id()));
        let c = c.to_str().unwrap();
        let _ = std::fs::remove_file(c);
        assert!(
            gravar_conta(c, "Filial", "senha-do-repasse").is_err(),
            "maiuscula"
        );
        assert!(gravar_conta(c, "filial", "curta").is_err());
        gravar_conta(c, "filial", "senha-do-repasse").unwrap();
        let l = ler_contas(c).unwrap();
        assert_eq!(
            l["filial"],
            credencial("filial", "senha-do-repasse", ITERACOES_CONTA)
        );
        assert!(!std::fs::read_to_string(c)
            .unwrap()
            .contains("senha-do-repasse"));
        let _ = std::fs::remove_file(c);
    }

    #[test]
    fn lista_de_permitidas_fecha_o_repasse() {
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let pa = x25519::chave_publica(&a);
        let mut r = Repasse::novo(x25519::gerar_privada(), Some([pa].into_iter().collect()));
        r.tratar(
            &registro(&a, &r.publica(), carimbo(1), None).unwrap(),
            end(1),
        );
        r.tratar(
            &registro(&b, &r.publica(), carimbo(1), None).unwrap(),
            end(2),
        );
        assert!(r
            .tratar(&embrulhar_para(&x25519::chave_publica(&b), b"x"), end(1))
            .is_none());
    }
}
