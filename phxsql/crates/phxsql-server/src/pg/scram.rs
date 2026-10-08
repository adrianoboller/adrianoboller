//! SCRAM-SHA-256 (RFC 5802 + RFC 7677), do lado do CLIENTE.
//!
//! E o que o PostgreSQL(R) 10 em diante usa por padrao (`scram-sha-256` no
//! `pg_hba.conf`). Nao precisou de tijolo novo: SHA-256, HMAC-SHA256 e PBKDF2
//! ja estavam escritos no `phxsql-core` por causa do hash de senha do
//! `config.json`, e sao exatamente as tres pecas que o SCRAM pede.
//!
//! # A ideia, em uma frase
//!
//! A senha nunca vai na rede, e o servidor tambem nao a tem: os dois provam um
//! ao outro que conhecem a mesma senha derivada, usando um nonce que muda a
//! cada conexao. Repetir um pacote gravado nao autentica ninguem.
//!
//! # A conversa
//!
//! ```text
//! cliente -> n,,n=user,r=<nonce do cliente>
//! servidor -> r=<nonce do cliente + do servidor>,s=<sal>,i=<iteracoes>
//! cliente -> c=biws,r=<nonce inteiro>,p=<prova>
//! servidor -> v=<assinatura do servidor>
//! ```
//!
//! O `c=biws` e o `n,,` do inicio em Base64 -- vai de volta para o servidor
//! conferir que ninguem trocou o cabecalho no meio do caminho.
//!
//! # Conferido contra o RFC 7677
//!
//! A secao 3 do RFC traz uma troca inteira com valores fixos. O teste deste
//! modulo refaz aquela troca e compara a prova do cliente e a assinatura do
//! servidor byte a byte. Nada aqui foi aceito por parecer certo.

use phxsql_core::base64;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::{hmac_sha256, iguais_em_tempo_constante, pbkdf2_sha256, sha256};

fn erro(msg: String) -> PhxError {
    PhxError::Autorizacao(msg)
}

/// O vinculo ao canal pedido no SCRAM (RFC 5802 §6, RFC 5929 §4).
pub enum Vinculo {
    SemTls,
    ServidorSemPlus,
    /// O `tls-server-end-point`: o resumo do certificado do servidor.
    PontaDoServidor(Vec<u8>),
}

/// O `tls-server-end-point` de um certificado (RFC 5929 §4.1): o resumo do
/// certificado inteiro com o hash da ASSINATURA dele -- e SHA-256 quando ela
/// e MD5 ou SHA-1. `None` quando o algoritmo nao diz o hash (Ed25519): a RFC
/// nao o define, e sem vinculo definido o cliente cai no `y,,`.
pub fn ponta_do_servidor(cert: &[u8]) -> Option<Vec<u8>> {
    use phxsql_core::rsa::Resumo;
    let c = phxsql_core::cadeia::Certificado::analisar(cert).ok()?;
    let r = match c.alg.as_slice() {
        [1, 2, 840, 113549, 1, 1, 12] | [1, 2, 840, 10045, 4, 3, 3] => Resumo::Sha384,
        [1, 2, 840, 113549, 1, 1, 13] | [1, 2, 840, 10045, 4, 3, 4] => Resumo::Sha512,
        // SHA-256, e os de MD5/SHA-1 que a §4.1 sobe para SHA-256.
        [1, 2, 840, 113549, 1, 1, 4 | 5 | 11]
        | [1, 2, 840, 10045, 4, 1]
        | [1, 2, 840, 10045, 4, 3, 2] => Resumo::Sha256,
        _ => return None,
    };
    Some(r.calcular(cert))
}

/// O estado de uma negociacao SCRAM em andamento.
pub struct Scram {
    /// `n=user,r=nonce` -- a primeira mensagem SEM o cabecalho `n,,`.
    primeira_sem_cabecalho: String,
    /// O cabecalho GS2 e o dado do vinculo, que voltam juntos no `c=` da
    /// ultima mensagem (RFC 5802 §7: `c=` e o Base64 de `gs2-header ||
    /// cbind-data`).
    cabecalho_gs2: String,
    dado_do_vinculo: Vec<u8>,
    nonce_cliente: String,
    /// Guardada entre a segunda e a terceira mensagem, para a assinatura.
    chave_do_servidor: Vec<u8>,
    assinatura_esperada: Vec<u8>,
}

impl Scram {
    /// Comeca a conversa. Devolve o estado e a `client-first-message`.
    pub fn comecar(nonce_cliente: &str) -> (Scram, String) {
        Scram::comecar_com(nonce_cliente, Vinculo::SemTls)
    }

    /// Comeca a conversa com o vinculo ao canal decidido (pedido 572, T6d):
    /// `n,,` sem TLS; `y,,` com TLS e servidor sem o `-PLUS` (diz ao servidor
    /// que o cliente sabe vincular, e um intermediario que apagou o `-PLUS`
    /// da lista e pego por isso -- RFC 5802 §6); `p=tls-server-end-point,,`
    /// com o resumo do certificado do servidor.
    pub fn comecar_com(nonce_cliente: &str, vinculo: Vinculo) -> (Scram, String) {
        let (cabecalho_gs2, dado_do_vinculo) = match vinculo {
            Vinculo::SemTls => ("n,,".to_string(), Vec::new()),
            Vinculo::ServidorSemPlus => ("y,,".to_string(), Vec::new()),
            Vinculo::PontaDoServidor(d) => ("p=tls-server-end-point,,".to_string(), d),
        };
        // O nome do usuario vai VAZIO de proposito: no PostgreSQL(R) quem diz
        // quem esta entrando e o campo `user` da mensagem de startup, e o RFC
        // 5802 manda ignorar o `n=` quando o transporte ja carrega a
        // identidade. Mandar o nome duas vezes so abriria a chance de as duas
        // discordarem.
        let sem_cabecalho = format!("n=,r={nonce_cliente}");
        let primeira = format!("{cabecalho_gs2}{sem_cabecalho}");
        (
            Scram {
                primeira_sem_cabecalho: sem_cabecalho,
                cabecalho_gs2,
                dado_do_vinculo,
                nonce_cliente: nonce_cliente.to_string(),
                chave_do_servidor: Vec::new(),
                assinatura_esperada: Vec::new(),
            },
            primeira,
        )
    }

    /// Recebe a `server-first-message` e devolve a `client-final-message`.
    pub fn responder(&mut self, senha: &str, servidor_primeira: &str) -> Result<String> {
        let (nonce, sal, iteracoes) = analisar_primeira_do_servidor(servidor_primeira)?;

        // O nonce do servidor TEM de comecar pelo nosso. Sem esta conferencia,
        // um intermediario poderia impor um nonce que ele escolheu -- e nonce
        // escolhido pelo atacante e o fim da protecao contra repeticao.
        if !nonce.starts_with(&self.nonce_cliente) {
            return Err(erro(
                "o servidor devolveu um nonce que nao comeca pelo nosso: \
                 a conexao pode estar sendo intermediada"
                    .into(),
            ));
        }

        let mut senha_salgada = [0u8; 32];
        pbkdf2_sha256(senha.as_bytes(), &sal, iteracoes, &mut senha_salgada);

        let chave_cliente = hmac_sha256(&senha_salgada, b"Client Key");
        let chave_guardada = sha256(&chave_cliente);
        self.chave_do_servidor = hmac_sha256(&senha_salgada, b"Server Key").to_vec();

        // O cabecalho GS2 (e o vinculo, quando ha) volta para o servidor
        // conferir que ninguem o trocou no caminho. Sem TLS, `biws` -- o
        // Base64 de "n,,".
        let mut c = self.cabecalho_gs2.as_bytes().to_vec();
        c.extend_from_slice(&self.dado_do_vinculo);
        let final_sem_prova = format!("c={},r={nonce}", base64::codificar(&c));
        let mensagem = format!(
            "{},{},{}",
            self.primeira_sem_cabecalho, servidor_primeira, final_sem_prova
        );

        let assinatura_cliente = hmac_sha256(&chave_guardada, mensagem.as_bytes());
        let prova: Vec<u8> = chave_cliente
            .iter()
            .zip(assinatura_cliente.iter())
            .map(|(a, b)| a ^ b)
            .collect();

        self.assinatura_esperada =
            hmac_sha256(&self.chave_do_servidor, mensagem.as_bytes()).to_vec();

        Ok(format!("{final_sem_prova},p={}", base64::codificar(&prova)))
    }

    /// Confere a `server-final-message`.
    ///
    /// # Por que conferir, se o servidor ja nos aceitou
    ///
    /// Porque a autenticacao do SCRAM e MUTUA. Sem esta conferencia, qualquer
    /// um que se ponha no meio poderia dizer "pode entrar" sem conhecer a
    /// senha -- e o cliente mandaria a consulta seguinte para ele.
    pub fn conferir_servidor(&self, servidor_final: &str) -> Result<()> {
        for campo in servidor_final.split(',') {
            if let Some(erro_do_servidor) = campo.strip_prefix("e=") {
                return Err(erro(format!(
                    "o servidor recusou a autenticacao: {erro_do_servidor}"
                )));
            }
            if let Some(v) = campo.strip_prefix("v=") {
                let vinda = base64::decodificar(v)
                    .map_err(|e| erro(format!("assinatura do servidor ilegivel: {e}")))?;
                if !iguais_em_tempo_constante(&vinda, &self.assinatura_esperada) {
                    return Err(erro(
                        "a assinatura do servidor nao confere: ele NAO conhece a senha \
                         (conexao intermediada, ou credencial trocada)"
                            .into(),
                    ));
                }
                return Ok(());
            }
        }
        Err(erro(format!(
            "a mensagem final do servidor nao traz assinatura: {servidor_final:?}"
        )))
    }
}

/// O teto das iteracoes que o PAR pode pedir -- pedido 547.
///
/// O `i=` vem do outro lado, e cada iteracao e um PBKDF2 que ESTE processo
/// paga. Sem teto, o `u32` inteiro custava ~57 min de uma CPU por tentativa
/// (medido em 30/09/2026, release: 1.000.000 iteracoes em 763-841 ms, e o
/// `u32` e 4.295 vezes isso). Nao ha convergencia a seguir -- o `libpq` aceita
/// qualquer contagem --, e o numero e nosso: 1.000.000, cerca de 0,8 s, que e
/// 244 vezes o padrao do PostgreSQL (4096) e 4,8 vezes as 210.000 que esta
/// casa usa para as proprias senhas. Quem configurou o servidor acima disso
/// recebe a recusa dizendo o numero.
pub const TETO_DE_ITERACOES_DO_PAR: u32 = 1_000_000;

/// `r=<nonce>,s=<sal em base64>,i=<iteracoes>`
fn analisar_primeira_do_servidor(m: &str) -> Result<(String, Vec<u8>, u32)> {
    let (mut nonce, mut sal, mut iteracoes) = (None, None, None);
    for campo in m.split(',') {
        if let Some(v) = campo.strip_prefix("r=") {
            nonce = Some(v.to_string());
        } else if let Some(v) = campo.strip_prefix("s=") {
            sal = Some(base64::decodificar(v).map_err(|e| erro(format!("sal: {e}")))?);
        } else if let Some(v) = campo.strip_prefix("i=") {
            iteracoes = Some(
                v.parse::<u32>()
                    .map_err(|_| erro(format!("contagem de iteracoes ilegivel: {v:?}")))?,
            );
        }
    }
    if let Some(i) = iteracoes.filter(|&i| i > TETO_DE_ITERACOES_DO_PAR) {
        return Err(erro(format!(
            "o servidor pediu {i} iteracoes de PBKDF2, acima do teto de \
             {TETO_DE_ITERACOES_DO_PAR} (cerca de 0,8 s de CPU) -- recusado antes de \
             derivar. Se o servidor e seu, baixe o `scram_iterations` dele"
        )));
    }
    match (nonce, sal, iteracoes) {
        (Some(n), Some(s), Some(i)) if i > 0 => Ok((n, s, i)),
        _ => Err(erro(format!(
            "a primeira mensagem do servidor esta incompleta: {m:?}"
        ))),
    }
}

/// Nonce do cliente: 24 caracteres imprimiveis, sem virgula.
///
/// Sem virgula porque a virgula separa os campos da mensagem -- um nonce que a
/// contivesse cortaria a mensagem ao meio. E a mesma razao de o RFC chamar
/// isso de `printable`.
pub fn nonce() -> String {
    const ALFABETO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    phxsql_core::senha::bytes_aleatorios(24)
        .iter()
        .map(|b| ALFABETO[*b as usize % ALFABETO.len()] as char)
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// RFC 7677, secao 3: a troca inteira com valores fixos.
    ///
    /// Este teste substitui o nonce sorteado pelo do RFC. E o unico jeito de
    /// conferir uma prova contra um vetor: com nonce aleatorio, a prova muda a
    /// cada rodada e nao ha o que comparar.
    #[test]
    fn troca_do_rfc_7677() {
        let nonce_cliente = "rOprNGfwEbeRWgbNEkqO";
        let servidor_primeira = "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
                                 s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096";

        // O vetor do RFC usa `n=user`; a nossa `comecar` manda `n=` vazio
        // (ver o comentario la). Para conferir contra o vetor, o estado e
        // montado com o `n=user` do RFC.
        let mut s = Scram {
            cabecalho_gs2: "n,,".into(),
            dado_do_vinculo: Vec::new(),
            primeira_sem_cabecalho: format!("n=user,r={nonce_cliente}"),
            nonce_cliente: nonce_cliente.to_string(),
            chave_do_servidor: Vec::new(),
            assinatura_esperada: Vec::new(),
        };

        let final_do_cliente = s.responder("pencil", servidor_primeira).unwrap();
        assert_eq!(
            final_do_cliente,
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
             p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ=",
            "a prova do cliente nao bate com o RFC 7677"
        );

        s.conferir_servidor("v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
            .expect("a assinatura do servidor do RFC devia conferir");
    }

    /// Assinatura errada NAO pode passar: e ela que prova que o outro lado
    /// conhece a senha.
    #[test]
    fn assinatura_do_servidor_errada_recusa() {
        let nonce_cliente = "rOprNGfwEbeRWgbNEkqO";
        let mut s = Scram {
            cabecalho_gs2: "n,,".into(),
            dado_do_vinculo: Vec::new(),
            primeira_sem_cabecalho: format!("n=user,r={nonce_cliente}"),
            nonce_cliente: nonce_cliente.to_string(),
            chave_do_servidor: Vec::new(),
            assinatura_esperada: Vec::new(),
        };
        s.responder(
            "pencil",
            "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
             s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096",
        )
        .unwrap();

        let e = s
            .conferir_servidor("v=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
            .unwrap_err();
        assert!(format!("{e}").contains("nao confere"), "{e}");
    }

    /// Nonce que nao comeca pelo nosso e conexao intermediada.
    #[test]
    fn nonce_que_nao_estende_o_nosso_recusa() {
        let (mut s, primeira) = Scram::comecar("cliente-aqui");
        assert!(primeira.starts_with("n,,n=,r=cliente-aqui"));
        let e = s
            .responder("x", "r=outroNonce,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096")
            .unwrap_err();
        assert!(format!("{e}").contains("nonce"), "{e}");
    }

    /// Pedido 547: o par que pede iteracoes demais e recusado ANTES de o
    /// PBKDF2 rodar. O teste pede o teto MAIS UM, e nao `u32::MAX`: com o
    /// teto tirado, ele cai em segundos pelo relogio e pela mensagem, em vez
    /// de derivar por 57 minutos -- prova que so termina por prazo nao prova.
    #[test]
    fn iteracoes_acima_do_teto_recusam_antes_de_derivar() {
        let (mut s, _) = Scram::comecar("rOprNGfwEbeRWgbNEkqO");
        let inicio = std::time::Instant::now();
        let e = s
            .responder(
                "x",
                &format!(
                    "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
                     s=W22ZaJ0SNY7soEsUEjb6gQ==,i={}",
                    TETO_DE_ITERACOES_DO_PAR + 1
                ),
            )
            .expect_err("uma iteracao acima do teto tem de recusar");
        assert!(e.to_string().contains("teto"), "{e}");
        assert!(inicio.elapsed() < std::time::Duration::from_secs(1));
        // No teto ainda aceita: o limite e inclusivo.
        let (mut s, _) = Scram::comecar("rOprNGfwEbeRWgbNEkqO");
        assert!(analisar_primeira_do_servidor(&format!(
            "r=x,s=W22ZaJ0SNY7soEsUEjb6gQ==,i={TETO_DE_ITERACOES_DO_PAR}"
        ))
        .is_ok());
        let _ = &mut s;
    }

    #[test]
    fn primeira_do_servidor_incompleta_recusa() {
        let (mut s, _) = Scram::comecar("abc");
        assert!(s.responder("x", "r=abcdef,i=4096").is_err());
        assert!(s.responder("x", "").is_err());
    }

    #[test]
    fn erro_do_servidor_na_final_vira_erro_nosso() {
        let (s, _) = Scram::comecar("abc");
        let e = s.conferir_servidor("e=invalid-proof").unwrap_err();
        assert!(format!("{e}").contains("invalid-proof"), "{e}");
    }

    #[test]
    fn o_nonce_nao_tem_virgula_e_muda() {
        let a = nonce();
        let b = nonce();
        assert_eq!(a.len(), 24);
        assert!(!a.contains(','));
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(a, b, "dois nonces iguais quebrariam a protecao");
    }
}
