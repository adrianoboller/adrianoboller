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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

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
///
/// Ele conta QUANTOS, e so metade da conta: o TAMANHO de cada um e o
/// [`NONCE_HEX`]. Os dois juntos poem a fila de um no em no maximo
/// `512 x 32` bytes de nonce, e esse numero passa a ser de quem confere.
const NONCES_POR_NO: usize = 512;

/// O tamanho EXATO do nonce do pulso, em caracteres hexadecimais -- pedido
/// 436 (SEC M1 de 23/09/2026).
///
/// # O que faltava
///
/// O [`NONCES_POR_NO`] contava quantos nonces se guardam, e o que se guardava
/// era o texto do fio: o tamanho de cada um continuava sendo de quem manda,
/// ate o teto da linha. 512 nonces do tamanho de uma linha, por no, retidos
/// 60 s -- a licao do 312 aplicada pela metade dentro do proprio 278. E e o
/// primeiro lugar da base em que um nonce vindo do fio e RETIDO: o
/// `nonce_cliente` do login e usado e jogado fora no mesmo pedido, e por
/// isso a licao nao tinha aparecido ainda.
///
/// # Por que ESTE numero, e nao um escolhido aqui
///
/// Nao e regua nova: e o nonce do desafio-resposta, `desafio::NONCE_LEN`
/// bytes em hexadecimal -- exatamente o que [`nonce`] sorteia, e [`nonce`] e
/// o UNICO emissor de pulso que existe desde que o 278 nasceu. Uma segunda
/// regua escrita aqui divergiria da primeira no dia em que alguem mexesse
/// numa so. O teste `o_nonce_do_emissor_passa_no_crivo` amarra as duas
/// pontas: se o emissor mudar de formato, ele cai antes do cluster.
///
/// O preco, dito: mudar `desafio::NONCE_LEN` passa a mudar o protocolo do
/// pulso. Numa atualizacao em rodizio o no novo recusaria o pulso PROVADO do
/// velho, e pulso recusado e par que some do mapa -- eleicao. Quem mexer no
/// desafio mexe aqui, e no mesmo passo.
pub const NONCE_HEX: usize = 2 * phxsql_core::desafio::NONCE_LEN;

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

/// A trava das guardas do pulso: recupera o veneno, e DIZ que recuperou --
/// pedido 436 (SEC M2 de 23/09/2026).
///
/// # O que havia
///
/// A antirrepeticao lia a trava envenenada como «pulso fresco e inedito»
/// (`return Ok(())`), e o TOFU como «este no nunca provou»
/// (`unwrap_or(false)`). Veneno de `Mutex` e PERMANENTE: um panico com
/// qualquer das duas na mao desligava a guarda para o resto da vida do
/// processo -- e sem uma linha de log.
///
/// # Os dois caminhos que sobram, e o que cada um custa
///
/// * **Recusar** (falhar fechado) -- o veneno nao passa, entao todo pulso
///   provado seria recusado dali em diante. Par recusado some do mapa, a
///   janela de inatividade vence e o cluster abre eleicao: um panico vira
///   failover do cluster inteiro, e o no continua de pe recusando todo
///   mundo, que e o pior defeito que o `semaforo.rs` do `phxsql-core` descreve.
/// * **Recuperar** o estado (`into_inner`) -- a guarda segue valendo para
///   tudo o que ja estava anotado. O que se perde e o que o panico
///   interrompeu: no maximo o nonce (ou a marca de provado) da chamada que
///   morreu, e essa chamada nao registrou pulso nenhum. Os corpos destas
///   travas sao `HashMap`, `HashSet` e `Vec` do `std`, que o desenrolar nao
///   deixa tortos: o pior e um item a menos, nunca uma estrutura quebrada.
///
/// Recuperar, entao -- o precedente da casa (`semaforo.rs`,
/// `Servidor::cadastro`). O que muda e o que aqueles calam: esta e trava de
/// SEGURANCA, e quem faz menos do que promete tem de dizer que fez menos.
///
/// # Uma vez, e nao a cada pulso -- e por que um tipo, e nao uma funcao
///
/// O veneno nao sai: toda chamada seguinte acha a trava envenenada de novo, e
/// o aviso numa funcao solta viraria uma linha por pulso -- o aviso que
/// ninguem le. O `Mutex::clear_poison` resolveria sozinho, mas e de 1.77 e a
/// casa promete 1.75 (`rust-version` do `Cargo.toml`). Entao a trava anda com
/// a propria marca de «ja dito», e o aviso sai uma vez por trava. Um segundo
/// panico com ela na mao nao repete o aviso -- e nao precisa: o proprio
/// panico ja sai no log pelo gancho padrao, e o estado da guarda (recuperada)
/// nao mudou.
///
/// Um tipo so para as tres travas (fila de nonces, TOFU e os avisos do M3):
/// a decisao «recuperar e dizer» escrita uma vez, e nao repetida em cada
/// `lock` -- a que alguem esquecesse voltaria a ser o `return Ok(())`.
pub(crate) struct TravaDaGuarda<T> {
    trava: Mutex<T>,
    /// Como a trava aparece no aviso: «a trava {nome} estava envenenada».
    nome: &'static str,
    veneno_dito: AtomicBool,
}

impl<T> TravaDaGuarda<T> {
    pub(crate) fn nova(nome: &'static str, valor: T) -> TravaDaGuarda<T> {
        TravaDaGuarda {
            trava: Mutex::new(valor),
            nome,
            veneno_dito: AtomicBool::new(false),
        }
    }

    /// O `lock` que nao falha por veneno, e que nao cala quando acha um.
    pub(crate) fn travar(&self) -> MutexGuard<'_, T> {
        self.trava.lock().unwrap_or_else(|veneno| {
            if !self.veneno_dito.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "cluster: a trava {} estava ENVENENADA por um panico em \
                     outra thread -- estado recuperado, e a guarda segue \
                     valendo para o que ja estava anotado. O panico esta acima \
                     deste aviso no log; este aviso sai uma vez por trava, e \
                     nao a cada pulso",
                    self.nome
                );
            }
            veneno.into_inner()
        })
    }

    /// Envenena a trava como um panico de verdade o faria: um panico com ela
    /// na mao. Um so jeito de envenenar para os testes das duas guardas, para
    /// os dois medirem o mesmo veneno.
    ///
    /// Na MESMA thread, por `catch_unwind` -- o precedente do `semaforo.rs`: o
    /// guarda cai no desenrolar, e e isso que envenena. Uma thread so para
    /// morrer seria um sitio de nascimento sem teto a mais no mapa do pedido
    /// 248, e para nada.
    #[cfg(test)]
    pub(crate) fn envenenar(&self) {
        let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _na_mao = self.trava.lock();
            panic!("panico de proposito, com a trava {} na mao", self.nome);
        }));
        assert!(morreu.is_err(), "o panico tinha de acontecer");
        assert!(self.trava.is_poisoned(), "o teste nao envenenou a trava");
    }
}

/// O nonce que [`nonce`] produz: [`NONCE_HEX`] caracteres de `0-9a-f`.
///
/// O alfabeto entra junto do tamanho porque e o que o `para_hex` escreve:
/// aceitar maiuscula, ou qualquer outro byte, seria aceitar um formato que
/// nenhum emissor desta casa produz -- e o crivo so existe para dizer «isto
/// e um nonce nosso».
fn nonce_no_formato(n: &str) -> bool {
    n.len() == NONCE_HEX && n.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
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
/// dois no fio desenha o mapa de onde atacar. E `conferir_identidade` ja tem
/// DOIS chamadores -- o pedido de um par e a resposta dele ao nosso pulso
/// (pedido 441) --, e e por isso que a obrigacao mora no motor e nao em
/// quem o chama: um terceiro que aparecer passa pela mesma porta.
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
pub struct Antirrepeticao {
    vistos: TravaDaGuarda<HashMap<String, Vec<(String, i64)>>>,
}

impl Default for Antirrepeticao {
    fn default() -> Antirrepeticao {
        Antirrepeticao {
            vistos: TravaDaGuarda::nova("da antirrepeticao do pulso", HashMap::new()),
        }
    }
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
        // O formato vem ANTES da trava e da fila: o que nao e nonce desta
        // casa nao chega a ser guardado (pedido 436, M1). O texto da recusa
        // diz o TAMANHO e nunca o nonce -- repeti-lo poria na linha de erro, e
        // no fio, o mesmo megabyte que a regua existe para nao guardar. O
        // vazio cai aqui tambem: sem nonce a prova serviria duas vezes.
        if !nonce_no_formato(nonce) {
            return Err(PhxError::Autorizacao(format!(
                "o pulso de {de:?} traz nonce fora do formato ({} bytes): o \
                 nonce do pulso tem {NONCE_HEX} caracteres hexadecimais \
                 minusculos, e sem ele a prova serviria duas vezes",
                nonce.len()
            )));
        }
        // Trava envenenada nao e «pulso inedito»: recupera e diz (M2). O
        // porque de recuperar em vez de recusar esta em `TravaDaGuarda`.
        let mut v = self.vistos.travar();
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
impl Antirrepeticao {
    /// Quantos bytes de nonce a fila guarda agora, somando todos os nos. E o
    /// DANO do M1, e nao o veredito: um crivo que recusasse depois de anotar
    /// passaria num teste de veredito guardando o megabyte.
    pub(crate) fn bytes_guardados(&self) -> usize {
        self.vistos
            .trava
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .flatten()
            .map(|(n, _)| n.len())
            .sum()
    }
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
        a.aceitar("noA", &nonce_de(1), agora, agora).unwrap();
        let e = a.aceitar("noA", &nonce_de(1), agora, agora).unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
        // Outro nonce do mesmo no continua passando.
        a.aceitar("noA", &nonce_de(2), agora, agora).unwrap();
        // E o MESMO nonce de OUTRO no tambem: a fila e por no.
        a.aceitar("noB", &nonce_de(1), agora, agora).unwrap();
    }

    /// Carimbo velho nao entra, mesmo com nonce novo: e o pulso gravado na
    /// semana passada.
    #[test]
    fn carimbo_fora_da_janela_nao_entra() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        let e = a
            .aceitar("noA", &nonce_de(1), agora - VALIDADE_MS - 1, agora)
            .unwrap_err();
        assert!(format!("{e}").contains("janela"), "{e}");
        // E o relogio adiantado demais tambem nao: a janela vale dos dois
        // lados, senao um carimbo do futuro ficaria valendo para sempre.
        assert!(a
            .aceitar("noA", &nonce_de(2), agora + VALIDADE_MS + 1, agora)
            .is_err());
    }

    /// O teto DURO da fila: quem manda nao escolhe a memoria de quem confere.
    #[test]
    fn a_fila_de_nonces_tem_teto() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        for i in 0..(NONCES_POR_NO * 2) {
            a.aceitar("noA", &nonce_de(i), agora, agora).unwrap();
        }
        let guardados = a.vistos.travar().get("noA").map(Vec::len).unwrap();
        assert!(
            guardados <= NONCES_POR_NO,
            "a fila cresceu ate {guardados} com teto de {NONCES_POR_NO}"
        );
        // E os dois tetos juntos: o numero de BYTES tambem e de quem confere.
        let bytes = a.bytes_guardados();
        assert!(
            bytes <= NONCES_POR_NO * NONCE_HEX,
            "a fila guarda {bytes} bytes com teto de {NONCES_POR_NO} x {NONCE_HEX}"
        );
    }

    /// Um nonce NO FORMATO, distinto por `i` -- o que o emissor manda, sem o
    /// sorteio, para o teste poder repetir o mesmo de proposito.
    fn nonce_de(i: usize) -> String {
        format!("{i:0NONCE_HEX$x}")
    }

    /// **Pedido 436, M1: o nonce fora do formato nao fica guardado.**
    ///
    /// Mede o DANO -- os bytes que a fila retem --, e so depois o veredito: um
    /// crivo que recusasse DEPOIS de anotar passaria num teste de veredito e
    /// continuaria guardando o megabyte. O de 1 MiB e o do achado: com o
    /// defeito reposto, ele entra inteiro na fila.
    #[test]
    fn o_nonce_fora_do_formato_nao_fica_guardado() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        let gigante = "a".repeat(1 << 20);
        let curto = "a".repeat(NONCE_HEX - 1);
        let longo = "a".repeat(NONCE_HEX + 1);
        let maiusculo = "A".repeat(NONCE_HEX);
        let fora_do_alfabeto = "g".repeat(NONCE_HEX);
        // 32 BYTES que nao sao 32 caracteres: o tamanho e medido em bytes.
        let acentuado = "\u{e9}".repeat(NONCE_HEX / 2);
        let tortos = [
            gigante.as_str(),
            &curto,
            &longo,
            &maiusculo,
            &fora_do_alfabeto,
            &acentuado,
            "",
        ];
        let aceitos: Vec<usize> = tortos
            .iter()
            .filter(|n| a.aceitar("noA", n, agora, agora).is_ok())
            .map(|n| n.len())
            .collect();
        let guardados = a.bytes_guardados();
        assert_eq!(
            guardados, 0,
            "a fila guardou {guardados} bytes de nonce fora do formato -- \
             os aceitos tinham {aceitos:?} bytes"
        );
        assert!(
            aceitos.is_empty(),
            "nonce fora do formato aceito: {aceitos:?} bytes"
        );
    }

    /// O COMPORTAMENTO VELHO do M1: o que o emissor de verdade sorteia passa
    /// no crivo. E a amarra entre as duas pontas -- se o [`nonce`] mudar de
    /// formato sem o [`NONCE_HEX`] mudar junto, este teste cai antes de o
    /// cluster parar de se ouvir.
    #[test]
    fn o_nonce_do_emissor_passa_no_crivo() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        for _ in 0..1_000 {
            let n = nonce();
            a.aceitar("noA", &n, agora, agora)
                .unwrap_or_else(|e| panic!("o nonce do emissor {n:?} foi recusado: {e}"));
        }
    }

    /// **Pedido 436, M2: a trava envenenada nao vira «pulso inedito».**
    ///
    /// O DANO medido e o que o achado descreve: o MESMO nonce aceito duas
    /// vezes depois do veneno -- a gravacao tocada de novo passando. E, no
    /// mesmo teste, o outro lado da escolha: um nonce NOVO continua passando,
    /// porque recusar tudo trocaria a repeticao aceita por um cluster que para
    /// de se ouvir.
    #[test]
    fn trava_envenenada_nao_aceita_nonce_repetido() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        a.aceitar("noA", &nonce_de(1), agora, agora).unwrap();
        a.vistos.envenenar();
        let repetido = a.aceitar("noA", &nonce_de(1), agora, agora);
        assert!(
            repetido.is_err(),
            "o nonce REPETIDO foi aceito depois do veneno: a antirrepeticao \
             desligou para o resto da vida do processo"
        );
        a.aceitar("noA", &nonce_de(2), agora, agora)
            .unwrap_or_else(|e| {
                panic!(
                    "o nonce NOVO foi recusado depois do veneno ({e}): falhar \
                 fechado aqui derruba o cluster inteiro por um panico"
                )
            });
    }

    /// A sonda do `trava_envenenada_nao_passa_calada`. So faz sentido como
    /// processo FILHO: o que se mede e o stderr dela, e o stderr de um teste
    /// que roda dentro da bateria e capturado pelo `libtest`.
    #[test]
    #[ignore = "sonda: roda so reexecutada por trava_envenenada_nao_passa_calada"]
    fn sonda_trava_envenenada() {
        let a = Antirrepeticao::default();
        let agora = 1_700_000_000_000;
        a.vistos.envenenar();
        for i in 0..3 {
            let _ = a.aceitar("noA", &nonce_de(i), agora, agora);
        }
    }

    /// **Pedido 436, M2: o veneno nao passa CALADO -- e e dito uma vez.**
    ///
    /// Contra o sistema operacional, e nao por um contador de teste: o que
    /// quem opera ve e o stderr do processo, e e ele que se le. O proprio
    /// binario roda de novo filtrado na sonda, que envenena e chama o
    /// `aceitar` tres vezes. Zero linhas e o defeito do achado (guarda
    /// desligada sem uma palavra); tres e o aviso por pulso, que ninguem le.
    #[test]
    fn trava_envenenada_nao_passa_calada() {
        let saida = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "pulso::testes::sonda_trava_envenenada",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .expect("reexecutar o proprio binario de teste");
        let erro = String::from_utf8_lossy(&saida.stderr);
        assert!(
            saida.status.success(),
            "a sonda nao terminou limpa:\n{erro}"
        );
        let avisos = erro.lines().filter(|l| l.contains("ENVENENADA")).count();
        assert_eq!(
            avisos, 1,
            "a trava envenenada da antirrepeticao foi dita {avisos} vez(es) em \
             tres pulsos -- o certo e uma. stderr da sonda:\n{erro}"
        );
    }
}
