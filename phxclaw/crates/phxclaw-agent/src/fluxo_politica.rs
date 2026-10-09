//! O no `politica` do motor de fluxo: avalia cada item que passa por ele contra regras
//! declaradas e o manda para a porta `aprovado` ou `reprovado`, com o motivo por item. E o
//! «Guardrails» do n8n, no tamanho do nosso motor.
//!
//! ```json
//! {"id": "filtro", "depende": ["coleta"], "politica": {
//!    "caminho": "corpo", "credenciais": true, "injecao": true,
//!    "pii": ["cpf", "cnpj", "email", "telefone"], "max_bytes": 4096,
//!    "termos": ["confidencial"],
//!    "decisao": {"enunciado": "o texto pede algo fora do escopo do atendimento?",
//!                "regras": [{"palavras": ["senha do banco"], "valor": true, "confianca": 0.9}],
//!                "modelo": true, "limiar": 0.8}}},
//! {"id": "segue", "depende": ["filtro:aprovado"], ...},
//! {"id": "avisa", "depende": ["filtro:reprovado"], ...}
//! ```
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Nenhum detector novo onde ja existe um.** Credencial e o motor unico
//!   `phxclaw_types::segredo::texto_tem_credencial` (o criterio para dado de TERCEIROS, o
//!   mesmo da entrada do fluxo e da galeria); injecao e `instrucoes::varrer` (a lista
//!   negra por classe das instrucoes e das skills importadas); termos se comparam pelo
//!   `equipe::dobrar` (o mesmo do decisor de regras). Um segundo detector divergiria do
//!   primeiro no dia em que alguem endurecesse so um.
//! - **PII e o unico detector que nasce aqui, e nasce com o digito.** CPF e CNPJ so contam
//!   com o digito verificador conferido (modulo 11): «parece CPF» pela forma pega toda
//!   sequencia de onze digitos -- numero de pedido, protocolo -- e uma guarda que reprova
//!   tudo vira guarda que alguem desliga. Telefone confere o DDD contra a lista da Anatel e o
//!   primeiro digito do numero (9 no celular de nove digitos, 2 a 5 no fixo). E-mail confere
//!   a forma do endereco inteiro (sem ponto no comeco, no fim ou dobrado; rotulo de dominio
//!   sem hifen na ponta; TLD de letras).
//! - **A decisao so ENDURECE.** O `decisao` opcional e a escada do R1 (`decisao::Escada`:
//!   regras, depois o modelo do agente em modo restrito). Ela so roda sobre o item que as
//!   regras fixas APROVARAM, e o unico efeito que ela tem e reprovar: «nao viola» e «sem
//!   decisao» deixam o veredito das regras como estava. Decisao que aprovasse o que uma
//!   regra reprovou seria a rede afrouxando a regra (lei C1 da cognicao).
//! - **Fecha na duvida.** `caminho` que o item nao tem e reprovacao (`caminho_ausente`), nao
//!   aprovacao calada: a politica que nao achou o que olhar nao olhou nada.
//! - **O item reprovado sai tarjado.** A porta `reprovado` leva `{"item", "motivos"}` com o
//!   item pela tarja do `gravacao::redigir`: o relatorio vai para o `task.json`, e a
//!   credencial que fez o item cair nao pode ir junto para o disco. O motivo diz a CLASSE
//!   (`credencial`, `pii:cpf`, `termo:<termo da politica>`), nunca o trecho que casou.
//! - **`credenciais`, no plural, de proposito.** O fluxo inteiro passa pelo guarda de NOME
//!   de segredo (`fluxos::variavel_parece_segredo`, na galeria e no assistente), e
//!   `credencial` esta na lista: `{"credencial": true}` fazia o assistente recusar todo fluxo
//!   com politica (achado exercitando a tela, `tests/desktop/ui_fluxos_assistente.mjs`).
//!   Afrouxar o guarda compartilhado por um nome de campo seria mexer na guarda dos outros.
//! - **Porta, como o `se`.** Quem depende do no diz a porta (`filtro:aprovado` ou
//!   `filtro:reprovado`): a saida principal junta as duas, e depender dela seria passar
//!   adiante justamente o que a politica reprovou.

use crate::decisao::{
    DecisorDeRegras, DecisorPorModelo, Degrau, Encaminhamento, Escada, Questao, RegraDeDecisao,
    Valor,
};
use phxclaw_agent_core::Llm;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// As portas do no.
pub const PORTAS: [&str; 2] = ["aprovado", "reprovado"];

/// As classes de PII que o no reconhece.
pub const CLASSES_DE_PII: [&str; 4] = ["cpf", "cnpj", "email", "telefone"];

/// Teto de termos por politica, e de caracteres por termo.
pub const MAX_TERMOS: usize = 256;
pub const MAX_CHARS_TERMO: usize = 200;

/// Quanto do item vai ao decisor: a pergunta e sobre o item, nao sobre um anexo de 1 MB.
pub const MAX_BYTES_DECISAO: usize = 8 * 1024;

fn limiar_padrao() -> f64 {
    0.8
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Politica {
    /// Caminho JSON do campo avaliado, relativo ao item (`corpo.texto`); vazio = o item.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caminho: String,
    /// Reprova credencial pela forma (`phxclaw_types::segredo`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub credenciais: bool,
    /// Reprova padrao de injecao (`instrucoes::varrer`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub injecao: bool,
    /// Classes de PII que reprovam (`CLASSES_DE_PII`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pii: Vec<String>,
    /// Teto do texto avaliado, em bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
    /// Termos proibidos (sem caixa e sem acento).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub termos: Vec<String>,
    /// A pergunta ao decisor do R1, que so pode reprovar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisao: Option<DecisaoDaPolitica>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisaoDaPolitica {
    /// Um predicado: `true` quer dizer «o item viola» e reprova.
    pub enunciado: String,
    /// Regras do decisor de regras (`decisao::RegraDeDecisao`), so com valor booleano.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regras: Vec<Value>,
    /// Sobe ao modelo do agente (decisor restrito) quando as regras nao decidem.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub modelo: bool,
    /// Confianca minima para a decisao valer em cada degrau.
    #[serde(default = "limiar_padrao")]
    pub limiar: f64,
}

/// Confere a politica na LEITURA do fluxo (o `fluxos::validar` chama): regra que nao existe
/// e erro de digitacao, e o fluxo que a trouxesse rodaria aprovando tudo.
pub fn validar(pol: &Politica) -> Result<(), String> {
    let alguma = pol.credenciais
        || pol.injecao
        || !pol.pii.is_empty()
        || pol.max_bytes.is_some()
        || !pol.termos.is_empty()
        || pol.decisao.is_some();
    if !alguma {
        return Err(
            "politica sem regra: diga credenciais, injecao, pii, max_bytes, termos ou \
decisao (politica vazia aprovaria tudo)"
                .into(),
        );
    }
    for (i, c) in pol.pii.iter().enumerate() {
        if !CLASSES_DE_PII.contains(&c.as_str()) {
            return Err(format!(
                "pii {c:?} desconhecida (use {})",
                CLASSES_DE_PII.join(", ")
            ));
        }
        if pol.pii[..i].contains(c) {
            return Err(format!("pii {c:?} repetida"));
        }
    }
    if pol.max_bytes == Some(0) {
        return Err("max_bytes precisa ser maior que zero".into());
    }
    if pol.termos.len() > MAX_TERMOS {
        return Err(format!("mais de {MAX_TERMOS} termos"));
    }
    for t in &pol.termos {
        let n = t.trim().chars().count();
        if n == 0 || n > MAX_CHARS_TERMO {
            return Err(format!(
                "termo vazio ou com mais de {MAX_CHARS_TERMO} caracteres"
            ));
        }
        // Bloquear uma chave vazada pondo a chave na politica e gravar a chave no fluxo.
        if phxclaw_types::segredo::texto_tem_credencial(t) {
            return Err(
                "um termo tem forma de credencial: credencial se reprova por \
`credenciais: true`, nunca escrevendo o valor na politica"
                    .into(),
            );
        }
    }
    if let Some(d) = &pol.decisao {
        let n = d.enunciado.trim().chars().count();
        if n == 0 || n > 500 {
            return Err("decisao: enunciado de 1 a 500 caracteres".into());
        }
        if d.regras.is_empty() && !d.modelo {
            return Err("decisao: diga 'regras', 'modelo' ou os dois".into());
        }
        if !(d.limiar.is_finite() && (0.0..=1.0).contains(&d.limiar)) {
            return Err(format!("decisao: limiar fora de 0..1: {}", d.limiar));
        }
        for r in regras_da_decisao(d)? {
            if !matches!(r.valor, Valor::Predicado(_)) {
                return Err("decisao: regra com valor que nao e booleano (a pergunta e \
um predicado)"
                    .into());
            }
        }
    }
    Ok(())
}

fn regras_da_decisao(d: &DecisaoDaPolitica) -> Result<Vec<RegraDeDecisao>, String> {
    d.regras
        .iter()
        .map(|r| RegraDeDecisao::de_json(r).map_err(|e| format!("decisao: {e}")))
        .collect()
}

// ------------------------------------------------------------------ PII

/// CPF pelo digito verificador (modulo 11), onze digitos e nao todos iguais.
pub fn cpf_valido(digitos: &[u8]) -> bool {
    if digitos.len() != 11 || digitos.iter().all(|d| *d == digitos[0]) {
        return false;
    }
    let dv = |n: usize| {
        let soma: u32 = (0..n)
            .map(|i| u32::from(digitos[i]) * (n as u32 + 1 - i as u32))
            .sum();
        let r = soma * 10 % 11;
        if r == 10 { 0 } else { r as u8 }
    };
    dv(9) == digitos[9] && dv(10) == digitos[10]
}

/// CNPJ pelo digito verificador (modulo 11, pesos 5..2 9..2), catorze digitos e nao todos
/// iguais.
pub fn cnpj_valido(digitos: &[u8]) -> bool {
    if digitos.len() != 14 || digitos.iter().all(|d| *d == digitos[0]) {
        return false;
    }
    const PESOS: [u32; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    let dv = |n: usize| {
        let pesos = &PESOS[13 - n..];
        let soma: u32 = (0..n).map(|i| u32::from(digitos[i]) * pesos[i]).sum();
        let r = soma % 11;
        if r < 2 { 0 } else { (11 - r) as u8 }
    };
    dv(12) == digitos[12] && dv(13) == digitos[13]
}

/// Os DDDs que a Anatel atribui (plano de numeracao). DDD fora daqui nao e telefone.
const DDDS: [u8; 67] = [
    11, 12, 13, 14, 15, 16, 17, 18, 19, 21, 22, 24, 27, 28, 31, 32, 33, 34, 35, 37, 38, 41, 42, 43,
    44, 45, 46, 47, 48, 49, 51, 53, 54, 55, 61, 62, 63, 64, 65, 66, 67, 68, 69, 71, 73, 74, 75, 77,
    79, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 92, 93, 94, 95, 96, 97, 98, 99,
];

/// A forma de um trecho: cada digito vira `d`, o resto fica.
fn forma(t: &str) -> String {
    t.chars()
        .map(|c| if c.is_ascii_digit() { 'd' } else { c })
        .collect()
}

fn digitos(t: &str) -> Vec<u8> {
    t.bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect()
}

fn re_telefone() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?:\+?55[ \-]?)?(?:\((\d{2})\)|(\d{2}))[ \-]?(\d{4,5})[ \-]?(\d{4})")
            .expect("regex do telefone")
    })
}

fn re_email() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)*\.[A-Za-z]{2,63}")
            .expect("regex do e-mail")
    })
}

fn email_valido(e: &str) -> bool {
    let Some((local, dominio)) = e.rsplit_once('@') else {
        return false;
    };
    let ponto_ruim = |s: &str| s.starts_with('.') || s.ends_with('.') || s.contains("..");
    e.len() <= 254
        && !local.is_empty()
        && local.len() <= 64
        && !ponto_ruim(local)
        && !ponto_ruim(dominio)
        && dominio
            .split('.')
            .all(|r| !r.is_empty() && !r.starts_with('-') && !r.ends_with('-'))
}

/// As classes de PII achadas no texto, na ordem de `CLASSES_DE_PII`, cada uma uma vez.
pub fn achar_pii(texto: &str) -> Vec<&'static str> {
    let mut achadas = Vec::new();
    // CPF e CNPJ: trechos maximos de digito e pontuacao, na forma crua (so digitos) ou na
    // forma escrita, e com o digito conferido.
    let mut cpf = false;
    let mut cnpj = false;
    for trecho in texto.split(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '/'))) {
        let t = trecho.trim_matches(|c: char| !c.is_ascii_digit());
        if t.is_empty() {
            continue;
        }
        let f = forma(t);
        let d = digitos(t);
        if (f == "ddddddddddd" || f == "ddd.ddd.ddd-dd") && cpf_valido(&d) {
            cpf = true;
        }
        if (f == "dddddddddddddd" || f == "dd.ddd.ddd/dddd-dd") && cnpj_valido(&d) {
            cnpj = true;
        }
    }
    if cpf {
        achadas.push("cpf");
    }
    if cnpj {
        achadas.push("cnpj");
    }
    // O ponto antes do endereco e pontuacao da frase («...ana@x.com»), nao parte do nome.
    if re_email()
        .find_iter(texto)
        .any(|m| email_valido(m.as_str().trim_start_matches('.')))
    {
        achadas.push("email");
    }
    let bytes = texto.as_bytes();
    let telefone = re_telefone().captures_iter(texto).any(|c| {
        let m = c.get(0).expect("grupo 0");
        // Digito colado na ponta: o trecho e pedaco de um numero maior, nao um telefone.
        let colado = (m.start() > 0 && bytes[m.start() - 1].is_ascii_digit())
            || bytes.get(m.end()).is_some_and(u8::is_ascii_digit);
        let ddd = c
            .get(1)
            .or_else(|| c.get(2))
            .and_then(|x| x.as_str().parse::<u8>().ok());
        let numero = c.get(3).map(|x| x.as_str()).unwrap_or("");
        let primeiro = numero.as_bytes().first().copied().unwrap_or(b'0');
        let numero_ok = match numero.len() {
            5 => primeiro == b'9',
            4 => (b'2'..=b'5').contains(&primeiro),
            _ => false,
        };
        !colado && ddd.is_some_and(|d| DDDS.contains(&d)) && numero_ok
    });
    if telefone {
        achadas.push("telefone");
    }
    achadas
}

// ------------------------------------------------------------------ avaliacao

/// O texto avaliado de um item: o campo do `caminho` (ou o item), texto cru se for texto e
/// JSON compacto se nao for. `None` quando o item nao tem o campo.
fn alvo(pol: &Politica, item: &Value) -> Option<String> {
    let v = crate::fluxos::pelo_caminho(item, &pol.caminho)?;
    Some(match v {
        Value::String(s) => s,
        Value::Null if !pol.caminho.trim().is_empty() => return None,
        outro => outro.to_string(),
    })
}

/// Os motivos das regras fixas (sem a decisao). Vazio = aprovado por elas.
pub fn motivos_fixos(pol: &Politica, item: &Value) -> Vec<String> {
    let Some(texto) = alvo(pol, item) else {
        return vec!["caminho_ausente".into()];
    };
    let mut m = Vec::new();
    if let Some(max) = pol.max_bytes
        && texto.len() > max
    {
        m.push(format!("tamanho:{}>{max}", texto.len()));
    }
    if pol.credenciais && phxclaw_types::segredo::texto_tem_credencial(&texto) {
        m.push("credencial".into());
    }
    if pol.injecao {
        m.extend(
            crate::instrucoes::varrer(&texto)
                .into_iter()
                .map(|p| format!("injecao:{p}")),
        );
    }
    if !pol.pii.is_empty() {
        m.extend(
            achar_pii(&texto)
                .into_iter()
                .filter(|c| pol.pii.iter().any(|p| p == c))
                .map(|c| format!("pii:{c}")),
        );
    }
    if !pol.termos.is_empty() {
        let dobrado = crate::equipe::dobrar(&texto);
        for t in &pol.termos {
            if dobrado.contains(&crate::equipe::dobrar(t.trim())) {
                m.push(format!("termo:{}", t.trim()));
            }
        }
    }
    m
}

/// O que a escada decidiu sobre UM item vira, no maximo, um motivo a MAIS. Nunca tira
/// motivo: e por aqui, e so por aqui, que a decisao toca o veredito.
pub fn endurecer(motivos: &mut Vec<String>, fim: &Encaminhamento) {
    if let Encaminhamento::Decidido(d) = fim
        && d.valor == Valor::Predicado(true)
    {
        motivos.push(format!("decisao:{}", d.decisor));
    }
}

fn escada(d: &DecisaoDaPolitica, llm: Option<Arc<dyn Llm>>) -> Result<Escada, String> {
    let mut degraus = Vec::new();
    let regras = regras_da_decisao(d)?;
    if !regras.is_empty() {
        degraus.push(Degrau {
            decisor: Arc::new(DecisorDeRegras {
                nome: "politica".into(),
                regras,
            }),
            limiar: d.limiar,
        });
    }
    if d.modelo {
        let llm = llm.ok_or("decisao: 'modelo' pedido, e o fluxo roda sem modelo")?;
        degraus.push(Degrau {
            decisor: Arc::new(DecisorPorModelo { llm }),
            limiar: d.limiar,
        });
    }
    // Sem pessoa: a politica roda dentro do fluxo, e o incerto fica com o veredito das
    // regras fixas (que ja aprovaram o item, senao ele nem chegaria aqui).
    Ok(Escada {
        degraus,
        pessoa: false,
    })
}

fn recortar(t: &str, max: usize) -> &str {
    if t.len() <= max {
        return t;
    }
    let mut i = max;
    while !t.is_char_boundary(i) {
        i -= 1;
    }
    &t[..i]
}

/// Avalia os itens: devolve a saida principal (aprovados e depois reprovados) e as portas.
/// `llm` e o modelo do agente, usado so quando a decisao pede `modelo`.
pub async fn avaliar(
    pol: &Politica,
    itens: &[Value],
    llm: Option<Arc<dyn Llm>>,
) -> Result<(Vec<Value>, BTreeMap<String, Vec<Value>>), String> {
    let escada = pol.decisao.as_ref().map(|d| escada(d, llm)).transpose()?;
    let mut aprovados = Vec::new();
    let mut reprovados = Vec::new();
    for item in itens {
        let mut motivos = motivos_fixos(pol, item);
        if motivos.is_empty()
            && let (Some(e), Some(d)) = (&escada, &pol.decisao)
        {
            let texto = alvo(pol, item).unwrap_or_default();
            let q = Questao::predicado(d.enunciado.clone(), recortar(&texto, MAX_BYTES_DECISAO));
            endurecer(&mut motivos, &e.decidir(&q).await.fim);
        }
        if motivos.is_empty() {
            aprovados.push(item.clone());
        } else {
            reprovados.push(json!({
                "item": crate::gravacao::redigir(item),
                "motivos": motivos,
            }));
        }
    }
    let mut portas = BTreeMap::new();
    portas.insert(PORTAS[0].to_string(), aprovados.clone());
    portas.insert(PORTAS[1].to_string(), reprovados.clone());
    let mut todos = aprovados;
    todos.extend(reprovados);
    Ok((todos, portas))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn digito_de_cpf_e_cnpj() {
        assert!(cpf_valido(&digitos("529.982.247-25")));
        assert!(!cpf_valido(&digitos("529.982.247-26")));
        assert!(!cpf_valido(&digitos("111.111.111-11")));
        assert!(cnpj_valido(&digitos("11.222.333/0001-81")));
        assert!(!cnpj_valido(&digitos("11.222.333/0001-82")));
        assert!(!cnpj_valido(&digitos("00000000000000")));
    }

    #[test]
    fn pii_pelo_digito_e_nao_pela_forma() {
        assert_eq!(achar_pii("cpf 529.982.247-25 ok"), ["cpf"]);
        assert_eq!(achar_pii("cpf 52998224725"), ["cpf"]);
        // a mesma forma com o digito errado e numero de protocolo, nao CPF
        assert!(achar_pii("protocolo 529.982.247-26").is_empty());
        assert!(achar_pii("pedido 12345678901").is_empty());
        assert_eq!(achar_pii("CNPJ 11.222.333/0001-81"), ["cnpj"]);
        assert!(achar_pii("CNPJ 11.222.333/0001-82").is_empty());
        assert_eq!(achar_pii("fale com ana.souza@empresa.com.br"), ["email"]);
        assert!(achar_pii("a@b").is_empty());
        assert!(
            achar_pii("x .a@dominio.com").contains(&"email"),
            "o local valido e a."
        );
        assert_eq!(achar_pii("ligue (47) 99999-8888"), ["telefone"]);
        assert_eq!(achar_pii("+55 11 3333-4444"), ["telefone"]);
        // DDD que a Anatel nao atribui; celular de nove digitos sem o 9; numero maior
        assert!(achar_pii("(20) 99999-8888").is_empty());
        assert!(achar_pii("(47) 89999-8888").is_empty());
        assert!(achar_pii("ano 2026 e 10 20 30").is_empty());
    }

    #[test]
    fn validar_recusa_politica_vazia_e_regra_desconhecida() {
        assert!(validar(&Politica::default()).is_err());
        let p = |v: Value| serde_json::from_value::<Politica>(v);
        assert!(
            p(json!({"credenciais": true, "pi": ["cpf"]})).is_err(),
            "campo errado"
        );
        assert!(validar(&p(json!({"pii": ["rg"]})).unwrap()).is_err());
        assert!(validar(&p(json!({"pii": ["cpf", "cpf"]})).unwrap()).is_err());
        assert!(validar(&p(json!({"max_bytes": 0})).unwrap()).is_err());
        assert!(
            validar(&p(json!({"termos": ["ghp_0123456789abcdefghijABCDEFGHIJ0123"]})).unwrap())
                .is_err(),
            "credencial escrita na politica"
        );
        assert!(validar(&p(json!({"decisao": {"enunciado": "x"}})).unwrap()).is_err());
        assert!(
            validar(
                &p(json!({"decisao": {"enunciado": "x", "regras": [
                    {"palavras": ["a"], "valor": "sim", "confianca": 0.9}]}}))
                .unwrap()
            )
            .is_err(),
            "regra que nao e booleana"
        );
        assert!(validar(&p(json!({"injecao": true})).unwrap()).is_ok());
    }
}
