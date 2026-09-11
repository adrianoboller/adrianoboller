//! Prototipo rodavel do correio nativo do PhxSql. Modelo pedido pelo dono
//! (11/09/2026), provado ponta a ponta so com `std` + a cripto ja conferida
//! contra vetor em phxsql-core (X25519, HKDF, ChaCha20-Poly1305, PBKDF2).
//!
//! O QUE ESTE EXEMPLO PROVA (e o MODELO, nao o formato em disco final):
//!   1. Identidade X25519; a PRIVADA fica cifrada sob a senha -> nao abre sem a senha.
//!   2. So dominios phxsql.com.br e phxmail.com.br podem mandar/receber.
//!   3. Sem CONFIANCA aceita nao ha contato (anti-spam), inclusive no canal de
//!      PEDIDO (repetido recusado, bloqueado nao pede, teto de pendentes).
//!   4. Mensagem cifrada pelo segredo ECDH; cada um decifra com a PROPRIA senha
//!      + a chave publica do outro. Prioridade/tipo AUTENTICADOS.
//!   5. TRES camadas de cripto encaixadas: E2E (sempre) + ALTO SEGREDO (frase por
//!      telefone/SMS) + MASSON (flag X, 3a camada). Ler exige a senha da conta e,
//!      quando ligadas, a frase e a chave Masson.
//!   6. Anexos num armazem A PARTE; a mensagem guarda so a referencia.
//!   7. Moderacao: banir/quarentena/multa com MOTIVO obrigatorio; a DECISAO
//!      (deferir/indeferir) tambem exige motivo.
//!   8. Porta configuravel; padrao 8000.
//!   9. STATUS das solicitacoes, por usuario: confianca com estado
//!      (pendente/aceita/recusada) e moderacao com estado
//!      (aberta/deferida/indeferida). Recusar NAO e bloquear.
//!  10. O FLUXO INTEIRO se encaixa: (A) aceitar a confianca cobrando um pix
//!      (valor sem teto) ou de graca; (B) ligar "seguro alto" = p12 + cada um
//!      a sua senha (E2E); (C) Masson cuja chave da 3a camada E o id da
//!      maconaria — id errado nao abre. Tudo NATIVO, zero-deps, sem OpenSSL.
//!
//! Rodar: cargo run -q --example correio-e2e -p phxsql-core

use phxsql_core::{cifra, hkdf, x25519, CHAVE_LEN, NONCE_LEN, TAG_LEN};

const ITER: u32 = 200_000;
const INFO: &[u8] = b"phxsql-correio-v1";
const TETO_PENDENTES: usize = 50;
const PORTA_PADRAO: u16 = 8000;
const DOMINIOS: [&str; 2] = ["phxsql.com.br", "phxmail.com.br"];

/// O host do endereco tem de SER um dominio permitido ou um subdominio dele.
/// Sufixo exato: "evilphxsql.com.br" NAO passa, "empresa.phxsql.com.br" passa.
fn dominio_permitido(endereco: &str) -> bool {
    let host = match endereco.rsplit_once('@') {
        Some((_, h)) => h,
        None => return false,
    };
    DOMINIOS
        .iter()
        .any(|b| host == *b || host.ends_with(&format!(".{b}")))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prioridade {
    Alta,
    Media,
    Baixa,
}
impl Prioridade {
    fn rotulo(self) -> &'static str {
        match self {
            Prioridade::Alta => "alta",
            Prioridade::Media => "media",
            Prioridade::Baixa => "baixa",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tipo {
    Alerta,
    Aviso,
    Normal,
}
impl Tipo {
    fn rotulo(self) -> &'static str {
        match self {
            Tipo::Alerta => "alerta",
            Tipo::Aviso => "aviso",
            Tipo::Normal => "normal",
        }
    }
    fn icone(self) -> &'static str {
        match self {
            Tipo::Alerta => "🚨",
            Tipo::Aviso => "🔔",
            Tipo::Normal => "  ",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TipoSolicitacao {
    Banir,
    Quarentena,
    Multa,
}
impl TipoSolicitacao {
    fn prejuizo(self) -> &'static str {
        match self {
            TipoSolicitacao::Banir => "alto prejuizo",
            TipoSolicitacao::Quarentena => "baixo prejuizo",
            TipoSolicitacao::Multa => "multa por mau uso",
        }
    }
    fn rotulo(self) -> &'static str {
        match self {
            TipoSolicitacao::Banir => "banir",
            TipoSolicitacao::Quarentena => "quarentena",
            TipoSolicitacao::Multa => "multa",
        }
    }
}

/// Estado de uma solicitacao de confianca. O bool antigo confundia "pendente"
/// com "recusada": quem manda precisa saber se foi RECUSADO ou se so nao houve
/// resposta ainda. Recusar e um "nao, obrigado" (pode-se pedir de novo depois);
/// bloquear e o "nunca mais" — sao coisas diferentes e o status mostra as duas.
#[derive(Clone, Copy, PartialEq, Debug)]
enum EstadoConfianca {
    Pendente,
    Aceita,
    Recusada,
}
impl EstadoConfianca {
    fn rotulo(self) -> &'static str {
        match self {
            EstadoConfianca::Pendente => "pendente",
            EstadoConfianca::Aceita => "aceita",
            EstadoConfianca::Recusada => "recusada",
        }
    }
}

/// Estado de uma solicitacao de moderacao vista pelo lado de quem abriu: o
/// moderador defere ou indefere, e essa decisao tambem carrega motivo.
#[derive(Clone, Copy, PartialEq, Debug)]
enum EstadoModeracao {
    Aberta,
    Deferida,
    Indeferida,
}
impl EstadoModeracao {
    fn rotulo(self) -> &'static str {
        match self {
            EstadoModeracao::Aberta => "aberta",
            EstadoModeracao::Deferida => "deferida",
            EstadoModeracao::Indeferida => "indeferida",
        }
    }
}

fn rnd16() -> [u8; 16] {
    let mut b = [0u8; 16];
    cifra::sortear(&mut b);
    b
}
fn rnd_nonce() -> [u8; NONCE_LEN] {
    let mut b = [0u8; NONCE_LEN];
    cifra::sortear(&mut b);
    b
}

// -------- camadas extras (frase/chave -> PBKDF2 -> ChaCha20-Poly1305) --------
fn selar_camada(segredo: &str, sal: &[u8; 16], aad: &[u8], claro: &[u8]) -> Vec<u8> {
    let k = cifra::chave_de_senha(segredo, sal, ITER);
    let nonce = rnd_nonce();
    let (ct, tag) = cifra::selar(&k, &nonce, aad, claro);
    let mut out = Vec::with_capacity(NONCE_LEN + TAG_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&ct);
    out
}
fn abrir_camada(segredo: &str, sal: &[u8; 16], aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < NONCE_LEN + TAG_LEN {
        return Err("camada extra malformada".into());
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&blob[..NONCE_LEN]);
    let mut tag = [0u8; TAG_LEN];
    tag.copy_from_slice(&blob[NONCE_LEN..NONCE_LEN + TAG_LEN]);
    let ct = &blob[NONCE_LEN + TAG_LEN..];
    let k = cifra::chave_de_senha(segredo, sal, ITER);
    cifra::abrir(&k, &nonce, aad, ct, &tag)
        .map_err(|_| "segredo da camada errado ou ausente".to_string())
}

/// Encaixa as camadas extras ANTES do E2E: Masson por dentro, alto segredo por
/// fora. O E2E (aplicado depois) fica sempre por cima.
fn envelopar(
    claro: &[u8],
    masson: Option<&str>,
    sal_m: &[u8; 16],
    alto: Option<&str>,
    sal_a: &[u8; 16],
) -> Vec<u8> {
    let mut p = claro.to_vec();
    if let Some(mk) = masson {
        p = selar_camada(mk, sal_m, b"masson", &p);
    }
    if let Some(fr) = alto {
        p = selar_camada(fr, sal_a, b"alto-segredo", &p);
    }
    p
}
/// Desfaz na ordem inversa: alto segredo primeiro, Masson por ultimo.
fn desenvelopar(
    mut blob: Vec<u8>,
    alto_on: bool,
    sal_a: &[u8; 16],
    alto: Option<&str>,
    masson_on: bool,
    sal_m: &[u8; 16],
    masson: Option<&str>,
) -> Result<Vec<u8>, String> {
    if alto_on {
        let fr = alto.ok_or("ALTO SEGREDO: precisa da frase passada por telefone/SMS")?;
        blob = abrir_camada(fr, sal_a, b"alto-segredo", &blob)?;
    }
    if masson_on {
        let mk = masson.ok_or("MASSON: precisa da chave da 3a camada")?;
        blob = abrir_camada(mk, sal_m, b"masson", &blob)?;
    }
    Ok(blob)
}

#[derive(Clone)]
struct Usuario {
    endereco: String,
    empresa: String,
    publica: [u8; CHAVE_LEN],
    sal: [u8; 16],
    nonce_priv: [u8; NONCE_LEN],
    priv_cifrada: Vec<u8>,
    tag_priv: [u8; TAG_LEN],
}

fn criar_usuario(endereco: &str, empresa: &str, senha: &str) -> Usuario {
    let privada = x25519::gerar_privada();
    let publica = x25519::chave_publica(&privada);
    let sal = rnd16();
    let ksenha = cifra::chave_de_senha(senha, &sal, ITER);
    let nonce_priv = rnd_nonce();
    let (priv_cifrada, tag_priv) =
        cifra::selar(&ksenha, &nonce_priv, endereco.as_bytes(), &privada);
    Usuario {
        endereco: endereco.into(),
        empresa: empresa.into(),
        publica,
        sal,
        nonce_priv,
        priv_cifrada,
        tag_priv,
    }
}

fn abrir_privada(u: &Usuario, senha: &str) -> Result<[u8; CHAVE_LEN], String> {
    let ksenha = cifra::chave_de_senha(senha, &u.sal, ITER);
    let claro = cifra::abrir(
        &ksenha,
        &u.nonce_priv,
        u.endereco.as_bytes(),
        &u.priv_cifrada,
        &u.tag_priv,
    )
    .map_err(|_| "senha incorreta: nao abre a chave privada".to_string())?;
    let mut p = [0u8; CHAVE_LEN];
    p.copy_from_slice(&claro);
    Ok(p)
}

struct Anexo {
    id: u64,
    nome: String,
    sal_kdf: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
}

struct Mensagem {
    de: String,
    para: String,
    prioridade: Prioridade,
    tipo: Tipo,
    alto_segredo: bool,
    masson: bool,
    sal_extra: [u8; 16],
    sal_masson: [u8; 16],
    sal_kdf: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
    anexos: Vec<u64>,
}

fn aad(de: &str, para: &str, pri: Prioridade, tipo: Tipo) -> String {
    format!("{de}|{para}|{}|{}", pri.rotulo(), tipo.rotulo())
}

fn chave_derivada(segredo: &[u8; CHAVE_LEN], sal: &[u8; 16]) -> Result<[u8; CHAVE_LEN], String> {
    let mut k = [0u8; CHAVE_LEN];
    hkdf::derivar(sal, segredo, INFO, &mut k).map_err(|e| e.to_string())?;
    Ok(k)
}

struct Envio<'a> {
    de: &'a str,
    senha: &'a str,
    para: &'a str,
    corpo: &'a str,
    pri: Prioridade,
    tipo: Tipo,
    anexos: Vec<(&'a str, &'a [u8])>,
    alto_segredo: Option<&'a str>, // frase por telefone/SMS
    masson: Option<&'a str>,       // flag X: 3a camada
}

struct Confianca {
    de: String,
    para: String,
    estado: EstadoConfianca,
    // A) quem aceita pode cobrar para conversar: chave pix + valor, SEM teto.
    pix: Option<(String, f64)>,
    // B) a relacao pediu seguro e criptografia alta (p12 + cada um a sua senha).
    seguro_alto: bool,
}

struct Solicitacao {
    tipo: TipoSolicitacao,
    de: String,
    alvo: String,
    motivo: String,
    link_pagamento: Option<String>,
    estado: EstadoModeracao,
    decisao: Option<String>, // motivo de quem deferiu/indeferiu
}

/// Retrato das solicitacoes de UM usuario, para a tela "Status das solicitacoes".
struct StatusSolicitacoes {
    /// confianca que EU recebi e ainda nao respondi (aprovar/recusar)
    receber: Vec<String>,
    /// confianca que EU pedi, com o estado de cada uma
    pedi: Vec<(String, EstadoConfianca)>,
    /// moderacao que EU abri, com o alvo e o estado
    moderacao: Vec<(TipoSolicitacao, String, EstadoModeracao)>,
}

struct ServerMail {
    dominio_base: String,
    porta: u16,
    usuarios: Vec<Usuario>,
    confiancas: Vec<Confianca>,
    bloqueios: Vec<(String, String)>,
    mensagens: Vec<Mensagem>,
    anexos: Vec<Anexo>,
    solicitacoes: Vec<Solicitacao>,
    proximo_anexo: u64,
}

impl ServerMail {
    fn novo(dominio_base: &str) -> Self {
        ServerMail {
            dominio_base: dominio_base.into(),
            porta: PORTA_PADRAO,
            usuarios: vec![],
            confiancas: vec![],
            bloqueios: vec![],
            mensagens: vec![],
            anexos: vec![],
            solicitacoes: vec![],
            proximo_anexo: 1,
        }
    }
    fn configurar_porta(&mut self, p: u16) {
        self.porta = p;
    }
    fn achar(&self, endereco: &str) -> Option<&Usuario> {
        self.usuarios.iter().find(|u| u.endereco == endereco)
    }
    fn criar_conta(&mut self, local: &str, empresa: &str, senha: &str) -> Result<String, String> {
        let endereco = format!("{local}@{empresa}");
        if !dominio_permitido(&endereco) {
            return Err(format!(
                "RECUSADO: {endereco} nao esta em phxsql.com.br nem phxmail.com.br"
            ));
        }
        self.usuarios.push(criar_usuario(&endereco, empresa, senha));
        println!("   conta criada: {endereco}");
        Ok(endereco)
    }
    fn confia(&self, a: &str, b: &str) -> bool {
        self.confiancas.iter().any(|c| {
            c.estado == EstadoConfianca::Aceita
                && ((c.de == a && c.para == b) || (c.de == b && c.para == a))
        })
    }
    fn bloqueado(&self, dono: &str, quem: &str) -> bool {
        self.bloqueios.iter().any(|x| x.0 == dono && x.1 == quem)
    }
    fn pendentes_para(&self, para: &str) -> usize {
        self.confiancas
            .iter()
            .filter(|c| c.para == para && c.estado == EstadoConfianca::Pendente)
            .count()
    }
    fn mesma_empresa(&self, a: &str, b: &str) -> bool {
        matches!((self.achar(a), self.achar(b)), (Some(x), Some(y)) if x.empresa == y.empresa)
    }

    fn solicitar_confianca(&mut self, de: &str, para: &str) -> Result<(), String> {
        if self.bloqueado(para, de) {
            return Err(format!("RECUSADO: {para} bloqueou {de}"));
        }
        if self.confia(de, para) {
            return Err("ja ha confianca aceita".into());
        }
        if self
            .confiancas
            .iter()
            .any(|c| c.de == de && c.para == para && c.estado == EstadoConfianca::Pendente)
        {
            return Err(format!("RECUSADO: ja ha uma solicitacao pendente de {de}"));
        }
        if self.pendentes_para(para) >= TETO_PENDENTES {
            return Err(format!(
                "RECUSADO: {para} atingiu o teto de solicitacoes pendentes"
            ));
        }
        self.confiancas.push(Confianca {
            de: de.into(),
            para: para.into(),
            estado: EstadoConfianca::Pendente,
            pix: None,
            seguro_alto: false,
        });
        let cruz = if self.mesma_empresa(de, para) {
            "mesma empresa"
        } else {
            "empresas diferentes"
        };
        println!("   solicitacao de confianca: {de} -> {para}  [{cruz}]  (pendente)");
        Ok(())
    }
    fn aceitar_confianca(&mut self, de: &str, para: &str) {
        for c in &mut self.confiancas {
            if c.de == de && c.para == para && c.estado == EstadoConfianca::Pendente {
                c.estado = EstadoConfianca::Aceita;
            }
        }
        println!("   {para} ACEITOU a confianca de {de}");
    }
    /// A) aceitar cobrando (ou nao) e escolhendo seguro alto. O valor do pix
    /// NAO tem teto — pode ser 50,00 ou muito mais.
    fn aceitar_confianca_com(
        &mut self,
        de: &str,
        para: &str,
        pix: Option<(&str, f64)>,
        seguro_alto: bool,
    ) {
        for c in &mut self.confiancas {
            if c.de == de && c.para == para && c.estado == EstadoConfianca::Pendente {
                c.estado = EstadoConfianca::Aceita;
                c.pix = pix.map(|(chave, valor)| (chave.to_string(), valor));
                c.seguro_alto = seguro_alto;
            }
        }
        let cobranca = match pix {
            Some((chave, valor)) => format!("  cobrando pix {chave} R$ {valor:.2}"),
            None => "  sem cobranca".to_string(),
        };
        let sel = if seguro_alto {
            "  [seguro alto: p12]"
        } else {
            ""
        };
        println!("   {para} ACEITOU {de}{cobranca}{sel}");
    }
    fn confianca(&self, a: &str, b: &str) -> Option<&Confianca> {
        self.confiancas.iter().find(|c| {
            c.estado == EstadoConfianca::Aceita
                && ((c.de == a && c.para == b) || (c.de == b && c.para == a))
        })
    }
    /// Recusar e um "nao, obrigado": o pedido sai de pendente e vira recusada,
    /// mas o outro pode pedir de novo depois (diferente de bloquear, que e o
    /// "nunca mais"). O status mostra a diferenca.
    fn recusar_confianca(&mut self, de: &str, para: &str) {
        for c in &mut self.confiancas {
            if c.de == de && c.para == para && c.estado == EstadoConfianca::Pendente {
                c.estado = EstadoConfianca::Recusada;
            }
        }
        println!("   {para} RECUSOU a confianca de {de} (pode pedir de novo)");
    }
    fn bloquear(&mut self, dono: &str, quem: &str) {
        self.bloqueios.push((dono.into(), quem.into()));
        self.confiancas
            .retain(|c| !(c.de == quem && c.para == dono && c.estado == EstadoConfianca::Pendente));
        println!("   {dono} BLOQUEOU {quem}");
    }

    fn solicitar_moderacao(
        &mut self,
        tipo: TipoSolicitacao,
        de: &str,
        alvo: &str,
        motivo: &str,
        link: Option<&str>,
    ) -> Result<usize, String> {
        if motivo.trim().is_empty() {
            return Err(format!(
                "RECUSADO: solicitacao de {} exige motivo",
                tipo.rotulo()
            ));
        }
        if tipo == TipoSolicitacao::Multa && link.is_none() {
            return Err("RECUSADO: multa exige link de pagamento".into());
        }
        self.solicitacoes.push(Solicitacao {
            tipo,
            de: de.into(),
            alvo: alvo.into(),
            motivo: motivo.into(),
            link_pagamento: link.map(|s| s.into()),
            estado: EstadoModeracao::Aberta,
            decisao: None,
        });
        println!(
            "   solicitacao [{}] {de} -> {alvo}  [{}]  motivo: \"{motivo}\"{}",
            tipo.rotulo(),
            tipo.prejuizo(),
            link.map(|l| format!("  link: {l}")).unwrap_or_default()
        );
        Ok(self.solicitacoes.len() - 1)
    }

    /// Decisao do moderador. Deferir ou indeferir, mas SEMPRE com motivo —
    /// a mesma lei do .reason: quem julga tem de dizer por que. E so decide
    /// solicitacao ainda ABERTA (nao se re-julga o que ja foi julgado).
    fn resolver_moderacao(
        &mut self,
        idx: usize,
        deferir: bool,
        motivo_decisao: &str,
    ) -> Result<(), String> {
        if motivo_decisao.trim().is_empty() {
            return Err("RECUSADO: a decisao de moderacao exige motivo".into());
        }
        let s = self
            .solicitacoes
            .get_mut(idx)
            .ok_or("solicitacao inexistente")?;
        if s.estado != EstadoModeracao::Aberta {
            return Err(format!(
                "RECUSADO: solicitacao ja {} — nao se re-julga",
                s.estado.rotulo()
            ));
        }
        s.estado = if deferir {
            EstadoModeracao::Deferida
        } else {
            EstadoModeracao::Indeferida
        };
        s.decisao = Some(motivo_decisao.into());
        println!(
            "   moderacao [{}] {} -> {}  {}  decisao: \"{motivo_decisao}\"",
            s.tipo.rotulo(),
            s.de,
            s.alvo,
            s.estado.rotulo()
        );
        Ok(())
    }

    /// O retrato que a tela "Status das solicitacoes" mostra para UM usuario.
    fn status_solicitacoes(&self, quem: &str) -> StatusSolicitacoes {
        let receber = self
            .confiancas
            .iter()
            .filter(|c| c.para == quem && c.estado == EstadoConfianca::Pendente)
            .map(|c| c.de.clone())
            .collect();
        let pedi = self
            .confiancas
            .iter()
            .filter(|c| c.de == quem)
            .map(|c| (c.para.clone(), c.estado))
            .collect();
        let moderacao = self
            .solicitacoes
            .iter()
            .filter(|s| s.de == quem)
            .map(|s| (s.tipo, s.alvo.clone(), s.estado))
            .collect();
        StatusSolicitacoes {
            receber,
            pedi,
            moderacao,
        }
    }

    fn enviar(&mut self, e: Envio) -> Result<usize, String> {
        if !dominio_permitido(e.de) || !dominio_permitido(e.para) {
            return Err(format!(
                "RECUSADO: so phxsql.com.br/phxmail.com.br podem trocar mensagem ({} -> {})",
                e.de, e.para
            ));
        }
        if !self.confia(e.de, e.para) {
            return Err(format!(
                "RECUSADO: nao ha relacao de confianca aceita entre {} e {}",
                e.de, e.para
            ));
        }
        let ud = self.achar(e.de).ok_or("remetente inexistente")?.clone();
        let up = self
            .achar(e.para)
            .ok_or("destinatario inexistente")?
            .clone();
        let priv_de = abrir_privada(&ud, e.senha)?;
        let segredo = x25519::segredo(&priv_de, &up.publica).map_err(|x| x.to_string())?;

        let sal_extra = rnd16();
        let sal_masson = rnd16();

        let payload = envelopar(
            e.corpo.as_bytes(),
            e.masson,
            &sal_masson,
            e.alto_segredo,
            &sal_extra,
        );
        let sal_kdf = rnd16();
        let kmsg = chave_derivada(&segredo, &sal_kdf)?;
        let nonce = rnd_nonce();
        let a = aad(e.de, e.para, e.pri, e.tipo);
        let (ct, tag) = cifra::selar(&kmsg, &nonce, a.as_bytes(), &payload);

        let mut refs = vec![];
        for (nome, bytes) in &e.anexos {
            let interno = envelopar(bytes, e.masson, &sal_masson, e.alto_segredo, &sal_extra);
            let sal_a = rnd16();
            let ka = chave_derivada(&segredo, &sal_a)?;
            let nonce_a = rnd_nonce();
            let (cta, taga) = cifra::selar(&ka, &nonce_a, nome.as_bytes(), &interno);
            let id = self.proximo_anexo;
            self.proximo_anexo += 1;
            self.anexos.push(Anexo {
                id,
                nome: (*nome).into(),
                sal_kdf: sal_a,
                nonce: nonce_a,
                ct: cta,
                tag: taga,
            });
            refs.push(id);
        }

        self.mensagens.push(Mensagem {
            de: e.de.into(),
            para: e.para.into(),
            prioridade: e.pri,
            tipo: e.tipo,
            alto_segredo: e.alto_segredo.is_some(),
            masson: e.masson.is_some(),
            sal_extra,
            sal_masson,
            sal_kdf,
            nonce,
            ct,
            tag,
            anexos: refs,
        });
        Ok(self.mensagens.len() - 1)
    }

    fn segredo_de(&self, quem: &str, senha: &str, outro: &str) -> Result<[u8; CHAVE_LEN], String> {
        let uq = self.achar(quem).ok_or("leitor inexistente")?;
        let uo = self.achar(outro).ok_or("contraparte inexistente")?;
        let priv_q = abrir_privada(uq, senha)?;
        x25519::segredo(&priv_q, &uo.publica).map_err(|e| e.to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn ler(
        &self,
        idx: usize,
        quem: &str,
        senha: &str,
        frase: Option<&str>,
        masson: Option<&str>,
    ) -> Result<(String, Prioridade, Tipo), String> {
        let m = self.mensagens.get(idx).ok_or("mensagem inexistente")?;
        let outro = if m.de == quem { &m.para } else { &m.de };
        let segredo = self.segredo_de(quem, senha, outro)?;
        let kmsg = chave_derivada(&segredo, &m.sal_kdf)?;
        let a = aad(&m.de, &m.para, m.prioridade, m.tipo);
        let interno = cifra::abrir(&kmsg, &m.nonce, a.as_bytes(), &m.ct, &m.tag)
            .map_err(|_| "nao decifra: chave/senha erradas ou prioridade adulterada".to_string())?;
        let corpo = desenvelopar(
            interno,
            m.alto_segredo,
            &m.sal_extra,
            frase,
            m.masson,
            &m.sal_masson,
            masson,
        )?;
        let texto = String::from_utf8(corpo).map_err(|_| "utf8 invalido".to_string())?;
        Ok((texto, m.prioridade, m.tipo))
    }

    #[allow(clippy::too_many_arguments)]
    fn ler_anexo(
        &self,
        msg_idx: usize,
        anexo_id: u64,
        quem: &str,
        senha: &str,
        frase: Option<&str>,
        masson: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let m = self.mensagens.get(msg_idx).ok_or("mensagem inexistente")?;
        let outro = if m.de == quem { &m.para } else { &m.de };
        let segredo = self.segredo_de(quem, senha, outro)?;
        let anexo = self
            .anexos
            .iter()
            .find(|x| x.id == anexo_id)
            .ok_or("anexo inexistente")?;
        let ka = chave_derivada(&segredo, &anexo.sal_kdf)?;
        let interno = cifra::abrir(
            &ka,
            &anexo.nonce,
            anexo.nome.as_bytes(),
            &anexo.ct,
            &anexo.tag,
        )
        .map_err(|_| "anexo nao decifra".to_string())?;
        desenvelopar(
            interno,
            m.alto_segredo,
            &m.sal_extra,
            frase,
            m.masson,
            &m.sal_masson,
            masson,
        )
    }
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| res.push((c, n.to_string()));

    println!("===== correio PhxSql: dominios, 3 camadas, anexos, moderacao, status =====\n");

    println!("1) server mail (porta padrao) + contas nos dois dominios permitidos:");
    let mut srv = ServerMail::novo("phxsql.com.br");
    ok(srv.porta == 8000, "porta padrao e 8000");
    let a = srv
        .criar_conta(
            "adrianoboller",
            "empresa.phxsql.com.br",
            "senha-do-adriano-9f",
        )
        .unwrap();
    let b = srv
        .criar_conta("joanaprado", "empresa.phxsql.com.br", "senha-da-joana-7k")
        .unwrap();
    let ana = srv
        .criar_conta("ana", "time.phxmail.com.br", "senha-da-ana-5p")
        .unwrap();

    println!("\n2) portao de dominio:");
    ok(
        srv.criar_conta("x", "gmail.com", "z").is_err(),
        "conta fora de phxsql/phxmail e recusada",
    );
    ok(
        !dominio_permitido("golpe@evilphxsql.com.br"),
        "sufixo espertinho (evilphxsql.com.br) NAO passa",
    );

    println!("\n3) confianca e envio adriano -> joana (mesmo dominio):");
    srv.solicitar_confianca(&a, &b).unwrap();
    srv.aceitar_confianca(&a, &b);
    let idx = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &b,
            corpo: "Fechamento de setembro em anexo.",
            pri: Prioridade::Media,
            tipo: Tipo::Normal,
            anexos: vec![("fechamento.xlsx", &[0x50u8; 4096])],
            alto_segredo: None,
            masson: None,
        })
        .expect("devia enviar");
    let m = &srv.mensagens[idx];
    ok(
        m.ct.len() < 200 && m.anexos == vec![1] && srv.anexos.len() == 1,
        "anexo no armazem a parte; registro so com a referencia",
    );
    let (t, _, _) = srv.ler(idx, &b, "senha-da-joana-7k", None, None).unwrap();
    ok(
        t == "Fechamento de setembro em anexo.",
        "joana le com a propria senha",
    );

    println!("\n4) envio para outro dominio (phxmail): tem de exigir confianca e passar:");
    ok(
        srv.enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &ana,
            corpo: "Oi, Ana.",
            pri: Prioridade::Baixa,
            tipo: Tipo::Aviso,
            anexos: vec![],
            alto_segredo: None,
            masson: None,
        })
        .is_err(),
        "sem confianca (mesmo dominio permitido) nao entrega",
    );
    srv.solicitar_confianca(&a, &ana).unwrap();
    srv.aceitar_confianca(&a, &ana);
    let ix_ana = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &ana,
            corpo: "Oi, Ana. phxmail funciona.",
            pri: Prioridade::Baixa,
            tipo: Tipo::Aviso,
            anexos: vec![],
            alto_segredo: None,
            masson: None,
        })
        .expect("devia enviar para phxmail");
    ok(
        srv.ler(ix_ana, &ana, "senha-da-ana-5p", None, None)
            .map(|(t, _, _)| t)
            .as_deref()
            == Ok("Oi, Ana. phxmail funciona."),
        "phxmail.com.br troca mensagem apos confianca",
    );

    println!("\n5) TRES camadas: E2E + alto segredo (frase) + Masson (flag X):");
    let frase = "girassol-42"; // passada por telefone/SMS
    let chave_masson = "masson-77z"; // 3a camada, flag X ligada
    let ix = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &b,
            corpo: "Contrato ultrassecreto: 1,2 mi.",
            pri: Prioridade::Alta,
            tipo: Tipo::Alerta,
            anexos: vec![("contrato.pdf", &[0x9au8; 1024])],
            alto_segredo: Some(frase),
            masson: Some(chave_masson),
        })
        .expect("envia com 3 camadas");

    println!("   so a senha (sem frase, sem Masson):");
    match srv.ler(ix, &b, "senha-da-joana-7k", None, None) {
        Err(e) => {
            println!("   {e}");
            ok(true, "3 camadas: senha sozinha nao le");
        }
        Ok(_) => ok(false, "3 camadas: senha sozinha nao le"),
    }
    println!("   senha + frase, sem Masson:");
    ok(
        srv.ler(ix, &b, "senha-da-joana-7k", Some(frase), None)
            .is_err(),
        "3 camadas: falta a chave Masson -> nao le",
    );
    println!("   senha + Masson, sem a frase:");
    ok(
        srv.ler(ix, &b, "senha-da-joana-7k", None, Some(chave_masson))
            .is_err(),
        "3 camadas: falta a frase -> nao le",
    );
    println!("   senha + frase + Masson (as tres):");
    match srv.ler(ix, &b, "senha-da-joana-7k", Some(frase), Some(chave_masson)) {
        Ok((t2, _, _)) => {
            println!("   \"{t2}\"");
            let ax = srv.ler_anexo(
                ix,
                2,
                &b,
                "senha-da-joana-7k",
                Some(frase),
                Some(chave_masson),
            );
            ok(
                t2.contains("ultrassecreto") && ax.as_deref() == Ok([0x9au8; 1024].as_slice()),
                "3 camadas: com as tres, le texto e anexo",
            );
        }
        Err(e) => {
            println!("   FALHA: {e}");
            ok(false, "3 camadas: com as tres, le texto e anexo");
        }
    }

    println!("\n6) anti-spam do canal de PEDIDO (repetido/bloqueado):");
    let spammer = srv
        .criar_conta("spammer", "golpe.phxsql.com.br", "z")
        .unwrap();
    srv.solicitar_confianca(&spammer, &b).unwrap();
    ok(
        srv.solicitar_confianca(&spammer, &b).is_err(),
        "pedido repetido recusado",
    );
    srv.bloquear(&b, &spammer);
    ok(
        srv.solicitar_confianca(&spammer, &b).is_err(),
        "remetente bloqueado nao pede",
    );

    println!("\n7) moderacao com motivo obrigatorio:");
    ok(
        srv.solicitar_moderacao(TipoSolicitacao::Banir, &b, &spammer, "  ", None)
            .is_err(),
        "solicitacao sem motivo recusada",
    );
    let idx_banir = srv
        .solicitar_moderacao(
            TipoSolicitacao::Banir,
            &b,
            &spammer,
            "phishing com prejuizo",
            None,
        )
        .unwrap();
    let idx_quarentena = srv
        .solicitar_moderacao(
            TipoSolicitacao::Quarentena,
            &b,
            &spammer,
            "flood sem prejuizo direto",
            None,
        )
        .unwrap();
    srv.solicitar_moderacao(
        TipoSolicitacao::Multa,
        &b,
        &spammer,
        "reincidencia apos aviso",
        Some("https://pagar.phxsql.com.br/multa/abc123"),
    )
    .unwrap();
    ok(
        srv.solicitar_moderacao(TipoSolicitacao::Multa, &b, &spammer, "x", None)
            .is_err(),
        "multa sem link recusada",
    );
    for s in &srv.solicitacoes {
        println!(
            "   registrada: [{}] {} -> {}",
            s.tipo.rotulo(),
            s.de,
            s.alvo
        );
    }
    ok(
        srv.solicitacoes.iter().all(|s| !s.motivo.trim().is_empty())
            && srv
                .solicitacoes
                .iter()
                .any(|s| s.tipo == TipoSolicitacao::Multa && s.link_pagamento.is_some()),
        "toda solicitacao tem motivo; multa carrega o link",
    );

    println!("\n8) porta configuravel:");
    srv.configurar_porta(9443);
    ok(srv.porta == 9443, "porta pode ser diferente de 8000");

    println!("\n9) status das solicitacoes (confianca e moderacao, com estado):");
    let carlos = srv
        .criar_conta("carlos", "empresa.phxsql.com.br", "senha-do-carlos-3q")
        .unwrap();
    srv.solicitar_confianca(&carlos, &b).unwrap();
    let sc = srv.status_solicitacoes(&carlos);
    let sj = srv.status_solicitacoes(&b);
    ok(
        sj.receber.iter().any(|d| d == &carlos)
            && sc
                .pedi
                .iter()
                .any(|(p, e)| p == &b && *e == EstadoConfianca::Pendente),
        "pendente aparece: joana recebe, carlos ve como pendente",
    );

    // recusar NAO e bloquear: sai de pendente, vira recusada, e nao entrega.
    // Se recusar fosse no-op, o estado ficaria Pendente e a checagem falha;
    // se recusar "aceitasse", confia() viraria true e a de baixo falha.
    srv.recusar_confianca(&carlos, &b);
    let sc = srv.status_solicitacoes(&carlos);
    let sj = srv.status_solicitacoes(&b);
    ok(
        sc.pedi
            .iter()
            .any(|(p, e)| p == &b && *e == EstadoConfianca::Recusada)
            && !sj.receber.iter().any(|d| d == &carlos)
            && !srv.confia(&carlos, &b),
        "recusar: carlos ve 'recusada', sai da caixa de joana, sem confianca",
    );
    ok(
        srv.enviar(Envio {
            de: &carlos,
            senha: "senha-do-carlos-3q",
            para: &b,
            corpo: "deixa eu passar?",
            pri: Prioridade::Baixa,
            tipo: Tipo::Normal,
            anexos: vec![],
            alto_segredo: None,
            masson: None,
        })
        .is_err(),
        "confianca recusada nao entrega mensagem",
    );

    ok(
        srv.resolver_moderacao(idx_banir, true, "  ").is_err(),
        "decisao de moderacao sem motivo recusada",
    );
    srv.resolver_moderacao(idx_banir, true, "phishing confirmado nos logs")
        .unwrap();
    srv.resolver_moderacao(idx_quarentena, false, "sem evidencia suficiente")
        .unwrap();
    ok(
        srv.resolver_moderacao(idx_banir, false, "mudei de ideia")
            .is_err(),
        "nao se re-julga solicitacao ja decidida",
    );
    let sj = srv.status_solicitacoes(&b);
    ok(
        sj.moderacao
            .iter()
            .any(|(t, _, e)| *t == TipoSolicitacao::Banir && *e == EstadoModeracao::Deferida)
            && sj.moderacao.iter().any(|(t, _, e)| {
                *t == TipoSolicitacao::Quarentena && *e == EstadoModeracao::Indeferida
            })
            && sj
                .moderacao
                .iter()
                .any(|(t, _, e)| *t == TipoSolicitacao::Multa && *e == EstadoModeracao::Aberta),
        "moderacao: banir deferida, quarentena indeferida, multa ainda aberta",
    );
    ok(
        srv.solicitacoes[idx_banir].decisao.as_deref() == Some("phishing confirmado nos logs"),
        "a decisao guarda o motivo de quem julgou",
    );

    // o retrato que a tela "Status das solicitacoes" mostra
    let imprimir = |quem: &str, s: &StatusSolicitacoes| {
        println!("   [{quem}]");
        for d in &s.receber {
            println!("      a aprovar: {d}");
        }
        for (p, e) in &s.pedi {
            println!("      pedi a {p}: {}", e.rotulo());
        }
        for (t, alvo, e) in &s.moderacao {
            println!("      moderacao [{}] {alvo}: {}", t.rotulo(), e.rotulo());
        }
    };
    imprimir(&carlos, &srv.status_solicitacoes(&carlos));
    imprimir(&b, &srv.status_solicitacoes(&b));

    println!("\n10) tudo se encaixa: A) confianca com pix, B) seguro alto (p12), C) Masson = id da maconaria:");
    let juliana = srv
        .criar_conta("juliana", "empresa.phxsql.com.br", "senha-da-juliana-8h")
        .unwrap();

    // A) adriano pede; juliana ACEITA COBRANDO um pix, e liga o seguro alto.
    srv.solicitar_confianca(&a, &juliana).unwrap();
    srv.aceitar_confianca_com(&a, &juliana, Some(("juliana@pix.com.br", 50.00)), true);
    let cj = srv.confianca(&a, &juliana).expect("confianca aceita");
    let pix_ok = match &cj.pix {
        Some((chave, v)) => chave.as_str() == "juliana@pix.com.br" && (*v - 50.00).abs() < 1e-9,
        None => false,
    };
    ok(
        pix_ok && cj.seguro_alto,
        "A: juliana aceitou cobrando pix R$ 50,00 e com seguro alto",
    );
    // sem teto: outra relacao aceita um valor enorme
    srv.solicitar_confianca(&ana, &juliana).unwrap();
    srv.aceitar_confianca_com(
        &ana,
        &juliana,
        Some(("juliana@pix.com.br", 1_000_000.00)),
        false,
    );
    let sem_teto =
        matches!(&srv.confianca(&ana, &juliana).unwrap().pix, Some((_, v)) if *v >= 1_000_000.00);
    ok(
        sem_teto,
        "A: o valor do pix nao tem teto (1.000.000,00 aceito)",
    );
    // e aceitar DE GRACA (sem pix) continua valendo — adriano<->joana da secao 3
    ok(
        srv.confianca(&a, &b)
            .map(|c| c.pix.is_none())
            .unwrap_or(false),
        "A: aceitar de graca (sem cobranca) tambem vale",
    );

    // B) com seguro alto, a mensagem viaja E2E (p12 + cada um a SUA senha).
    let ix_b = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &juliana,
            corpo: "Contrato, seguro alto.",
            pri: Prioridade::Media,
            tipo: Tipo::Normal,
            anexos: vec![],
            alto_segredo: None,
            masson: None,
        })
        .expect("envia com seguro alto");
    let mb = &srv.mensagens[ix_b];
    ok(
        !mb.ct.is_empty() && mb.ct.windows(8).all(|w| w != b"Contrato"),
        "B: registro so com ciphertext (texto claro nao aparece)",
    );
    let (tb, _, _) = srv
        .ler(ix_b, &juliana, "senha-da-juliana-8h", None, None)
        .unwrap();
    ok(
        tb == "Contrato, seguro alto.",
        "B: juliana abre com a PROPRIA senha (p12/ECDH)",
    );
    ok(
        srv.ler(ix_b, &juliana, "senha-ERRADA", None, None).is_err(),
        "B: sem a senha certa, nao abre",
    );

    // C) Masson: a chave da 3a camada E o ID DA MACONARIA. Sem ele, nem a senha abre.
    let masson_id = "LOJA-ADR-4211"; // id da maconaria = chave Masson
    let ix_c = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &juliana,
            corpo: "So para irmaos.",
            pri: Prioridade::Alta,
            tipo: Tipo::Alerta,
            anexos: vec![],
            alto_segredo: None,
            masson: Some(masson_id),
        })
        .expect("envia com Masson (id da maconaria)");
    ok(
        srv.ler(ix_c, &juliana, "senha-da-juliana-8h", None, None)
            .is_err(),
        "C: senha sozinha nao abre a camada Masson",
    );
    ok(
        srv.ler(
            ix_c,
            &juliana,
            "senha-da-juliana-8h",
            None,
            Some("LOJA-ERRADA"),
        )
        .is_err(),
        "C: id da maconaria ERRADO nao abre",
    );
    let (tc, _, _) = srv
        .ler(ix_c, &juliana, "senha-da-juliana-8h", None, Some(masson_id))
        .unwrap();
    ok(
        tc == "So para irmaos.",
        "C: com o id da maconaria CERTO, abre a 3a camada",
    );

    println!("\n===== RESULTADO =====");
    let mut falhas = 0;
    for (c, n) in &res {
        if !c {
            falhas += 1;
        }
        println!("  {}  {}", if *c { " ok  " } else { "FALHA" }, n);
    }
    println!(
        "\n{} checagens, {} ok, {} falha(s).",
        res.len(),
        res.len() - falhas,
        falhas
    );
    println!(
        "{}",
        if falhas == 0 {
            "PROVA VERDE"
        } else {
            "PROVA VERMELHA"
        }
    );
    let _ = (
        &srv.dominio_base,
        Tipo::Aviso.icone(),
        Prioridade::Baixa.rotulo(),
    );
    if falhas != 0 {
        std::process::exit(1);
    }
}
