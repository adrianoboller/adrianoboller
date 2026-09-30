//! Chave redigida e politica de origem.
//!
//! Tudo o que decide PARA ONDE a chave vai mora aqui, num lugar so: espalhar a conferencia
//! de origem por quatro provedores seria repetir a decisao quatro vezes, e o provedor que
//! alguem esquecesse mandaria a chave para onde a URL mandasse.

use phxclaw_agent_core::LlmError;
use std::fmt;
use std::net::IpAddr;
use url::Url;

/// Chave de API que nunca se mostra.
///
/// Nao implementa `Display` de proposito: sem ele, `format!("{chave}")` nem compila, e o
/// unico caminho para o texto e `expor`, que se acha por busca.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(chave: String) -> Result<Self, LlmError> {
        let chave = chave.trim().to_owned();
        if chave.is_empty() {
            return Err(LlmError::Credential("chave vazia".into()));
        }
        // Caractere de controle viraria cabecalho invalido -- e o erro do reqwest poderia
        // citar o valor. Recusar aqui garante que a mensagem nunca carrega a chave.
        if chave.chars().any(|c| c.is_control()) {
            return Err(LlmError::Credential(
                "chave com caractere de controle".into(),
            ));
        }
        Ok(Self(chave))
    }

    pub(crate) fn expor(&self) -> &str {
        &self.0
    }

    /// Defesa em profundidade: corpo de erro do provedor e erro de transporte passam por
    /// aqui antes de virar `LlmError`, para o caso de algum eco da chave.
    pub(crate) fn tapar(&self, texto: &str) -> String {
        if self.0.len() < 4 {
            return texto.to_owned();
        }
        texto.replace(&self.0, "[REDACTED]")
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([REDACTED])")
    }
}

/// Onde um provedor de nuvem pode ser chamado.
///
/// O padrao e a origem oficial. Origem diferente (teste local, proxy corporativo) so passa
/// se estiver na lista explicita de permitidas, e `http` so passa em loopback com
/// `permitir_http_loopback`. A lista existe para que uma URL vinda de configuracao ou de
/// ambiente nao consiga, sozinha, desviar a chave para outro servidor.
#[derive(Debug, Clone, Default)]
pub struct Endpoint {
    base: Option<String>,
    origens_permitidas: Vec<String>,
    http_loopback: bool,
}

impl Endpoint {
    pub fn official() -> Self {
        Self::default()
    }

    /// Base customizada; a origem dela tem de estar em `origens_permitidas`
    /// (ex.: `"https://proxy.empresa:8443"`).
    pub fn custom(base_url: impl Into<String>, origens_permitidas: &[&str]) -> Self {
        Self {
            base: Some(base_url.into()),
            origens_permitidas: origens_permitidas.iter().map(|s| s.to_string()).collect(),
            http_loopback: false,
        }
    }

    /// Permite `http://` quando o host e loopback (127.0.0.0/8, ::1, localhost).
    pub fn permitir_http_loopback(mut self) -> Self {
        self.http_loopback = true;
        self
    }

    /// Resolve a base efetiva e diz se ela e loopback (o transporte desliga o proxy do
    /// sistema para loopback, senao o pedido local sairia pelo proxy).
    pub(crate) fn resolver(&self, oficial: &str) -> Result<Base, LlmError> {
        let Some(base) = &self.base else {
            return validar_base(oficial, false);
        };
        let url = analisar(base)?;
        let origem = url.origin().ascii_serialization();
        let oficial_origem = analisar(oficial)?.origin().ascii_serialization();
        let listada = origem == oficial_origem
            || self
                .origens_permitidas
                .iter()
                .filter_map(|o| Url::parse(o).ok())
                .any(|o| o.origin().ascii_serialization() == origem);
        if !listada {
            return Err(LlmError::Denied(format!(
                "origem {origem} nao esta na lista de origens permitidas"
            )));
        }
        validar_base(base, self.http_loopback)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Base {
    /// Sem barra no fim; o provedor acrescenta o caminho.
    pub texto: String,
    pub loopback: bool,
}

fn analisar(texto: &str) -> Result<Url, LlmError> {
    Url::parse(texto).map_err(|e| LlmError::Denied(format!("URL de base invalida: {e}")))
}

/// Confere esquema, credencial embutida e loopback. `http_loopback` e a permissao explicita.
pub(crate) fn validar_base(texto: &str, http_loopback: bool) -> Result<Base, LlmError> {
    let url = analisar(texto)?;
    // Usuario/senha na URL iriam para log de proxy e para mensagens de erro.
    if !url.username().is_empty() || url.password().is_some() {
        return Err(LlmError::Denied(
            "URL de base com credencial embutida".into(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(LlmError::Denied(
            "URL de base nao pode ter consulta nem fragmento".into(),
        ));
    }
    let loopback = eh_loopback(&url);
    match url.scheme() {
        "https" => {}
        "http" if loopback && http_loopback => {}
        "http" if loopback => {
            return Err(LlmError::Denied(
                "http em loopback exige permissao explicita".into(),
            ));
        }
        outro => {
            return Err(LlmError::Denied(format!(
                "esquema {outro} recusado: https e obrigatorio fora de loopback"
            )));
        }
    }
    Ok(Base {
        texto: url.as_str().trim_end_matches('/').to_owned(),
        loopback,
    })
}

pub(crate) fn eh_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// O modelo entra no caminho da URL (Gemini) e no id; um `../` ou `?` ali mudaria o destino.
pub(crate) fn validar_modelo(modelo: &str) -> Result<(), LlmError> {
    let ok = !modelo.is_empty()
        && modelo.len() <= 128
        && !modelo.contains("..")
        && modelo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'));
    if ok {
        Ok(())
    } else {
        Err(LlmError::Denied(format!(
            "nome de modelo recusado: {modelo:?}"
        )))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn chave_nao_aparece_no_debug() {
        let k = ApiKey::new("sk-segredo-123".into()).unwrap();
        let d = format!("{k:?}");
        assert!(!d.contains("segredo"), "{d}");
        assert_eq!(k.tapar("eco sk-segredo-123 fim"), "eco [REDACTED] fim");
    }

    #[test]
    fn origem_oficial_passa_e_estranha_e_recusada() {
        let of = "https://api.exemplo.com";
        assert!(Endpoint::official().resolver(of).is_ok());
        let e = Endpoint::custom("https://ladrao.com", &[]).resolver(of);
        assert!(matches!(e, Err(LlmError::Denied(_))));
        let ok = Endpoint::custom("https://proxy.corp/v", &["https://proxy.corp"]).resolver(of);
        assert_eq!(ok.unwrap().texto, "https://proxy.corp/v");
    }

    #[test]
    fn http_so_em_loopback_e_so_com_permissao() {
        let of = "https://api.exemplo.com";
        let lista = ["http://127.0.0.1:9", "http://10.0.0.5:9"];
        let sem = Endpoint::custom("http://127.0.0.1:9", &lista).resolver(of);
        assert!(matches!(sem, Err(LlmError::Denied(_))));
        let com = Endpoint::custom("http://127.0.0.1:9", &lista)
            .permitir_http_loopback()
            .resolver(of)
            .unwrap();
        assert!(com.loopback);
        // Listada e com permissao, mas nao e loopback: http continua recusado.
        let fora = Endpoint::custom("http://10.0.0.5:9", &lista)
            .permitir_http_loopback()
            .resolver(of);
        assert!(matches!(fora, Err(LlmError::Denied(_))));
    }

    #[test]
    fn credencial_na_url_e_recusada() {
        assert!(validar_base("https://u:p@api.exemplo.com", false).is_err());
    }

    #[test]
    fn modelo_que_mudaria_o_caminho_e_recusado() {
        assert!(validar_modelo("gemini-x.y_z:1").is_ok());
        for ruim in ["", "../x", "a/b", "a?b", "a#b", "a b"] {
            assert!(validar_modelo(ruim).is_err(), "{ruim}");
        }
    }
}
