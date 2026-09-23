//! A PROVA de identidade do pulso do cluster -- pedido 278 (SEC A1).
//!
//! # O buraco que isto fecha
//!
//! O pulso dizia quem era num campo `"id"` do corpo, e nada mais. A credencial
//! da replicacao e UMA so para o cluster inteiro (`docs/CLUSTER.md` §2.2),
//! entao qualquer um que a tivesse -- um no legitimo inclusive -- podia se
//! declarar OUTRO no e mandar a epoca que quisesse. Medido pelo soquete: uma
//! linha com `"id":"noB","papel":"master","epoca":9` rebaixou o master de
//! verdade em meio segundo e gravou `{"papel":"replica","epoca":9}` no
//! `cluster.estado.json`, que GANHA do `config.json` no arranque seguinte.
//!
//! # Por que uma prova DENTRO do pulso, e nao outro aperto de mao
//!
//! Decisao do dono, 17/09/2026: o aperto do fio e Noise **NX**, e no NX so o
//! respondedor apresenta estatica -- quem manda o pulso e o iniciador, e o
//! iniciador e anonimo por desenho (`docs/CIFRA-DO-FIO.md` §1). Trocar o
//! padrao para XX ou IK reabriria aquela decisao. O material, porem, ja esta
//! todo aqui: `cluster.nos[].chave_do_fio` e a chave PUBLICA de cada no --
//! um `known_hosts` do cluster --, e cada no tem a propria privada estatica.
//! Entao o no assina o pulso com a chave que sai do Diffie-Hellman entre a
//! estatica DELE e a publica do DESTINATARIO, e o destinatario refaz a mesma
//! conta do lado dele. Nao ha chave nova para distribuir, nao ha ida-e-volta
//! a mais, e o aperto continua NX.
//!
//! # O que a prova amarra, e por que cada pedaco esta na mensagem
//!
//! * **`de` e `para`** -- a prova vale para UM par. Um pulso legitimo que o
//!   no B recebeu de A nao serve contra C: a chave do par (A,B) nao e a do
//!   par (A,C), e o `para` diz de quem era.
//! * **`quando` e `nonce`** -- frescor. Sem eles, gravar um pulso legitimo do
//!   master e repeti-lo depois renovaria "vi o master agora" para sempre, e
//!   nenhuma eleicao legitima abriria. A janela e a mesma do desafio-resposta
//!   ([`VALIDADE_MS`]), e o nonce nao se repete dentro dela.
//! * **A transcricao do tunel**, quando ha tunel -- a mesma amarracao ao canal
//!   do `desafio.rs`: um pulso gravado numa conexao nao vale em outra.
//!   `None` deixa a mensagem **byte a byte igual** a de quem fala em claro.
//! * **Os campos que DECIDEM** -- papel, epoca, posicao, incompleta e
//!   prioridade. Assinar so o `id` deixaria o forjador reusar uma prova
//!   legitima trocando a epoca, que e exatamente o ataque.
//!
//! # A primitiva e de norma, e ja estava aqui
//!
//! X25519 (RFC 7748), SHA-256 (FIPS 180-4) e HMAC-SHA256 (RFC 2104/4231),
//! escritos nesta casa e conferidos contra vetor oficial. Nada de cifra nova.

use std::collections::HashMap;
use std::sync::Mutex;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::{de_hex, hmac_sha256, iguais_em_tempo_constante, para_hex, sha256};
use phxsql_core::x25519;

/// O rotulo que separa esta chave de qualquer outra derivada da MESMA
/// estatica. Sem ele, a chave do pulso seria o segredo cru do X25519 -- o
/// mesmo material que outro uso da estatica poderia derivar amanha.
const ROTULO: &[u8] = b"phxsql-pulso-do-cluster-v1";

/// O cabecalho da mensagem assinada. Muda quando o formato da prova mudar, e
/// uma prova do formato antigo deixa de fechar -- que e o que se quer.
const CABECALHO: &str = "phxsql-pulso-v1";

/// Quanto tempo um pulso vale, em milissegundos.
///
/// A mesma janela do desafio-resposta (`phxsql_core::desafio::VALIDADE_MS`),
/// e pelo mesmo motivo: e o maximo de desacerto de relogio que se tolera
/// entre dois nos antes de a prova de frescor deixar de valer. Com `pulso_s`
/// de fabrica, cabem dezenas de pulsos dentro dela.
pub const VALIDADE_MS: i64 = 60_000;

/// Quantos nonces se guardam por no antes de podar os mais velhos.
///
/// Teto DURO, e nao so a poda pela janela: sem ele, a memoria de quem confere
/// seria escolhida por quem manda -- o mesmo defeito do pedido 312, de outro
/// jeito. Com a janela de 60 s e o pulso de fabrica, um no legitimo nao chega
/// perto disto.
const NONCES_POR_NO: usize = 512;

/// Os campos do pulso que entram na prova.
///
/// Struct em vez de oito argumentos: o que assina e o que confere tem de
/// montar a MESMA mensagem, e duas listas de argumentos posicionais iguais
/// sao o jeito mais facil de trocar dois campos de lugar sem ninguem ver.
pub struct Assinado<'a> {
    pub de: &'a str,
    pub para: &'a str,
    pub papel: &'a str,
    pub epoca: u64,
    pub posicao: u64,
    pub incompleta: bool,
    pub prioridade: i64,
    pub quando: i64,
    pub nonce: &'a str,
}

impl Assinado<'_> {
    /// A mensagem canonica. Nao e o JSON: dois JSONs com os mesmos campos em
    /// ordens diferentes sao o mesmo pulso, e amarrar a prova ao texto do
    /// pedido faria a prova depender de como o outro lado serializa.
    fn mensagem(&self, canal: Option<&[u8]>) -> Vec<u8> {
        let mut m = Vec::with_capacity(160);
        m.extend_from_slice(CABECALHO.as_bytes());
        for campo in [
            self.de.to_string(),
            self.para.to_string(),
            self.papel.to_string(),
            self.epoca.to_string(),
            self.posicao.to_string(),
            self.incompleta.to_string(),
            self.prioridade.to_string(),
            self.quando.to_string(),
            self.nonce.to_string(),
        ] {
            m.push(b'\n');
            m.extend_from_slice(campo.as_bytes());
        }
        // So quando HA tunel. Sem ela a mensagem fica igual a de sempre, e e
        // por isso que um no que fala em claro prova do mesmo jeito.
        if let Some(c) = canal {
            m.push(b'\n');
            m.extend_from_slice(c);
        }
        m
    }
}

/// A chave que os DOIS lados do par derivam sozinhos.
///
/// O X25519 e simetrico: `DH(priv_A, pub_B) == DH(priv_B, pub_A)`. E por isso
/// que nao ha nada a distribuir -- o que A usa para assinar e o que B usa para
/// conferir, e ninguem mais no cluster chega nele.
fn chave_do_par(privada: &[u8; 32], publica_do_outro: &[u8; 32]) -> Result<[u8; 32]> {
    Ok(chave_do_segredo(&x25519::segredo(
        privada,
        publica_do_outro,
    )?))
}

/// O segredo cru do X25519 nunca vira chave de HMAC direto: passa pelo hash
/// com rotulo, como o `MixKey` do aperto faz com o dele.
///
/// Separada do `chave_do_par` so para a forja do teste do 435 passar pela
/// MESMA derivacao da producao: uma segunda copia desta conta no teste
/// divergiria no dia em que o rotulo mudasse, a forja deixaria de fechar, e o
/// teste passaria pelo motivo errado.
fn chave_do_segredo(segredo: &[u8; 32]) -> [u8; 32] {
    let mut m = Vec::with_capacity(ROTULO.len() + 32);
    m.extend_from_slice(ROTULO);
    m.extend_from_slice(segredo);
    sha256(&m)
}

/// **A forja que o pino cego do 435 admite**, montada so com dado PUBLICO.
///
/// `x25519::segredo(privada, BASE)` e a propria chave publica de quem confere,
/// e essa publica todo par do cluster tem -- e o `chave_do_fio` que ele guarda.
/// Entao a prova conferida contra o ponto-base FECHA para qualquer membro,
/// sem privada nenhuma. Existe so em teste, e so para provar que a recusa
/// incondicional do no sem pino, no `cluster.rs`, e a porta e nao enfeite.
#[cfg(test)]
pub(crate) fn forjar_contra_o_pino_cego(
    publica_de_quem_confere: &[u8; 32],
    campos: &Assinado<'_>,
    canal: Option<&[u8]>,
) -> String {
    let k = chave_do_segredo(publica_de_quem_confere);
    para_hex(&hmac_sha256(&k, &campos.mensagem(canal)))
}

/// A prova, em hexadecimal.
pub fn assinar(
    privada: &[u8; 32],
    publica_do_destino: &[u8; 32],
    campos: &Assinado<'_>,
    canal: Option<&[u8]>,
) -> Result<String> {
    let k = chave_do_par(privada, publica_do_destino)?;
    Ok(para_hex(&hmac_sha256(&k, &campos.mensagem(canal))))
}

/// Confere a prova. `Ok(())` = quem mandou tem a privada que corresponde ao
/// `chave_do_fio` daquele no.
///
/// A comparacao e em tempo constante pelo mesmo motivo de sempre: uma
/// comparacao que sai no primeiro byte diferente conta, pelo relogio, quantos
/// bytes o palpite acertou.
///
/// **O texto destes erros e DIAGNOSTICO LOCAL e nao vai ao fio** -- pedido 435
/// (SEC A2). Quem chama e `EstadoCluster::conferir_identidade`, que os manda
/// para o log deste processo por `recusa_da_prova` e devolve ao remetente uma
/// frase unica. O motivo esta la: «nao fecha» quer dizer «ha pino aqui», e
/// «nao ha pino» quer dizer «este no ainda passa sem prova» -- distinguir os
/// dois no fio desenha o mapa de onde atacar. Segundo chamador que aparecer
/// tem a mesma obrigacao.
pub fn conferir(
    privada: &[u8; 32],
    publica_do_remetente: &[u8; 32],
    campos: &Assinado<'_>,
    canal: Option<&[u8]>,
    prova_hex: &str,
) -> Result<()> {
    let Some(veio) = de_hex(prova_hex.trim()) else {
        return Err(PhxError::Autorizacao(
            "a prova do pulso nao esta em hexadecimal".into(),
        ));
    };
    let k = chave_do_par(privada, publica_do_remetente)?;
    let esperada = hmac_sha256(&k, &campos.mensagem(canal));
    if !iguais_em_tempo_constante(&veio, &esperada) {
        // Sem o `de`: quem chama ja nomeia o no na linha do log, e detalhe que
        // nao repete o id e detalhe que vaza menos se um dia escapar ao fio.
        return Err(PhxError::Autorizacao(
            "a prova nao fecha: quem mandou nao tem a chave que corresponde ao \
             chave_do_fio deste no"
                .into(),
        ));
    }
    Ok(())
}

/// O frescor: janela de tempo e nonce que nao se repete.
///
/// Mora separado da prova de propósito -- a prova diz QUEM mandou, e esta diz
/// que nao e uma gravacao de antes. Sem as duas, um pulso legitimo do master,
/// gravado uma vez, renovaria "vi o master agora" para sempre e nenhuma
/// eleicao legitima abriria.
#[derive(Default)]
pub struct Antirrepeticao {
    vistos: Mutex<HashMap<String, Vec<(String, i64)>>>,
}

impl Antirrepeticao {
    /// `Ok(())` = pulso fresco e inedito; o nonce fica anotado.
    pub fn aceitar(&self, de: &str, nonce: &str, quando: i64, agora: i64) -> Result<()> {
        if (agora - quando).abs() > VALIDADE_MS {
            return Err(PhxError::Autorizacao(format!(
                "o pulso de {de:?} esta fora da janela de {VALIDADE_MS} ms \
                 (carimbo {quando}, aqui sao {agora}): gravacao antiga ou \
                 relogio dos dois nos muito longe um do outro"
            )));
        }
        if nonce.trim().is_empty() {
            return Err(PhxError::Autorizacao(format!(
                "o pulso de {de:?} veio sem nonce: sem ele a prova serve \
                 duas vezes"
            )));
        }
        let Ok(mut v) = self.vistos.lock() else {
            // Trava envenenada e falha deste lado, nao do outro: recusar aqui
            // derrubaria o cluster inteiro por um panico alheio.
            return Ok(());
        };
        let fila = v.entry(de.to_string()).or_default();
        // Poda ANTES de procurar: o que saiu da janela ja foi recusado pelo
        // carimbo, entao guarda-lo so gastaria memoria.
        fila.retain(|(_, q)| (agora - *q).abs() <= VALIDADE_MS);
        if fila.iter().any(|(n, _)| n == nonce) {
            return Err(PhxError::Autorizacao(format!(
                "o pulso de {de:?} repete um nonce ja usado nesta janela: e \
                 uma gravacao sendo tocada de novo"
            )));
        }
        if fila.len() >= NONCES_POR_NO {
            fila.remove(0);
        }
        fila.push((nonce.to_string(), quando));
        Ok(())
    }
}

/// Um nonce novo, em hexadecimal -- o mesmo do desafio-resposta.
pub fn nonce() -> String {
    phxsql_core::desafio::nonce()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campos<'a>(de: &'a str, para: &'a str, epoca: u64, nonce: &'a str) -> Assinado<'a> {
        Assinado {
            de,
            para,
            papel: "master",
            epoca,
            posicao: 42,
            incompleta: false,
            prioridade: 0,
            quando: 1_700_000_000_000,
            nonce,
        }
    }

    fn par() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
        let pa = x25519::gerar_privada();
        let pb = x25519::gerar_privada();
        let ua = x25519::chave_publica(&pa);
        let ub = x25519::chave_publica(&pb);
        (pa, ua, pb, ub)
    }

    /// O caminho feliz, e a simetria que o faz existir: A assina com a
    /// privada dele e a publica de B; B confere com a privada dele e a
    /// publica de A, e chega na MESMA chave.
    #[test]
    fn o_que_a_assina_b_confere() {
        let (pa, ua, pb, ub) = par();
        let c = campos("noA", "noB", 3, "abcd");
        let prova = assinar(&pa, &ub, &c, None).unwrap();
        conferir(&pb, &ua, &c, None, &prova).expect("a prova do par tinha de fechar");
    }

    /// **O ataque do 278, na unidade.** Um TERCEIRO no do cluster -- que tem
    /// a credencial e esta na lista -- nao consegue assinar como A.
    #[test]
    fn um_terceiro_no_nao_consegue_se_passar_por_a() {
        let (_pa, ua, pb, ub) = par();
        let pc = x25519::gerar_privada();
        let c = campos("noA", "noB", 9, "abcd");
        // C monta o pulso dizendo que e A, e assina com o que ele tem.
        let forjada = assinar(&pc, &ub, &c, None).unwrap();
        let e = conferir(&pb, &ua, &c, None, &forjada).unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    }

    /// Trocar a EPOCA de um pulso legitimo quebra a prova -- se so o `id`
    /// entrasse na mensagem, o forjador reusaria a prova de verdade com a
    /// epoca dele.
    #[test]
    fn mexer_na_epoca_quebra_a_prova() {
        let (pa, ua, pb, ub) = par();
        let legitimo = campos("noA", "noB", 3, "abcd");
        let prova = assinar(&pa, &ub, &legitimo, None).unwrap();
        let mexido = campos("noA", "noB", 9, "abcd");
        assert!(conferir(&pb, &ua, &mexido, None, &prova).is_err());
    }

    /// A prova vale para UM par: o pulso que A mandou a B nao serve contra C.
    #[test]
    fn a_prova_de_um_par_nao_serve_para_outro() {
        let (pa, ua, _pb, ub) = par();
        let pc = x25519::gerar_privada();
        let c = campos("noA", "noB", 3, "abcd");
        let prova = assinar(&pa, &ub, &c, None).unwrap();
        assert!(
            conferir(&pc, &ua, &c, None, &prova).is_err(),
            "o pulso de A para B fechou no C: a chave nao esta presa ao par"
        );
    }

    /// A amarracao ao canal: a mesma prova nao atravessa dois tuneis.
    #[test]
    fn a_prova_nao_atravessa_dois_tuneis() {
        let (pa, ua, pb, ub) = par();
        let c = campos("noA", "noB", 3, "abcd");
        let prova = assinar(&pa, &ub, &c, Some(&[1u8; 32])).unwrap();
        conferir(&pb, &ua, &c, Some(&[1u8; 32]), &prova).unwrap();
        assert!(
            conferir(&pb, &ua, &c, Some(&[2u8; 32]), &prova).is_err(),
            "a prova de um tunel fechou em outro"
        );
        assert!(
            conferir(&pb, &ua, &c, None, &prova).is_err(),
            "a prova de um tunel fechou em claro"
        );
    }

    /// Sem tunel, a mensagem e a de sempre -- e e isso que deixa um no que
    /// fala em claro provar do mesmo jeito.
    #[test]
    fn sem_tunel_a_mensagem_nao_ganha_nada() {
        let c = campos("noA", "noB", 3, "abcd");
        let sem = c.mensagem(None);
        let com = c.mensagem(Some(&[7u8; 32]));
        assert_eq!(&com[..sem.len()], &sem[..]);
        assert_eq!(com.len(), sem.len() + 33);
    }

    /// Gravar e repetir: o mesmo nonce nao conta duas vezes.
    #[test]
    fn o_mesmo_nonce_nao_conta_duas_vezes() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        a.aceitar("noA", "n1", agora, agora).unwrap();
        let e = a.aceitar("noA", "n1", agora, agora).unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
        // Outro nonce do mesmo no continua passando.
        a.aceitar("noA", "n2", agora, agora).unwrap();
        // E o MESMO nonce de OUTRO no tambem: a fila e por no.
        a.aceitar("noB", "n1", agora, agora).unwrap();
    }

    /// Carimbo velho nao entra, mesmo com nonce novo: e o pulso gravado na
    /// semana passada.
    #[test]
    fn carimbo_fora_da_janela_nao_entra() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        let e = a
            .aceitar("noA", "n1", agora - VALIDADE_MS - 1, agora)
            .unwrap_err();
        assert!(format!("{e}").contains("janela"), "{e}");
        // E o relogio adiantado demais tambem nao: a janela vale dos dois
        // lados, senao um carimbo do futuro ficaria valendo para sempre.
        assert!(a
            .aceitar("noA", "n2", agora + VALIDADE_MS + 1, agora)
            .is_err());
    }

    /// O teto DURO da fila: quem manda nao escolhe a memoria de quem confere.
    #[test]
    fn a_fila_de_nonces_tem_teto() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        for i in 0..(NONCES_POR_NO * 2) {
            a.aceitar("noA", &format!("n{i}"), agora, agora).unwrap();
        }
        let guardados = a.vistos.lock().unwrap().get("noA").map(Vec::len).unwrap();
        assert!(
            guardados <= NONCES_POR_NO,
            "a fila cresceu ate {guardados} com teto de {NONCES_POR_NO}"
        );
    }
}
