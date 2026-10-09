//! O decisor Laya: um modelo de decisao «System 1» servido pelo `laya-serve` (Convai
//! Innovations, Apache-2.0, github.com/NandhaKishorM/laya), pelo protocolo `POST
//! /v1/systemone` do Jev. Um forward pass por pergunta, sem geracao: a resposta ja vem na
//! forma (`choice`/`score`) e com a probabilidade de cada opcao.
//!
//! O que vale saber antes de mexer:
//!
//! - **A confianca e `answer_confidence`, nunca `confidence`.** No Laya o `confidence` de
//!   `choice`/`score` e 1 - entropia normalizada (quao concentrada esta a distribuicao), e o
//!   README avisa que limiar herdado do Jev nao transfere. O `answer_confidence` e a
//!   probabilidade da resposta dada -- o numero que a calibracao deles mede e o unico
//!   comparavel com um limiar. Resposta sem ele (servidor em `LAYA_JEV_STRICT`) e
//!   `SemDecisao`: cair para o `confidence` poria a entropia no lugar da probabilidade com o
//!   mesmo nome, e o limiar do degrau passaria a medir outra coisa calado.
//! - **O pedido sai pelo no HTTP da casa** (`fluxo_http::executar`): a mesma politica de
//!   saida (SSRF fechado; loopback e rede privada so com `liberar` no `http.json`), a mesma
//!   credencial por NOME (declarada no `http.json`, segredo no broker de `credenciais/`), o
//!   mesmo teto de bytes e a mesma tarja. Um segundo cliente HTTP aqui seria uma segunda
//!   decisao sobre o que pode sair da maquina.
//! - **Toda falha e `SemDecisao` com o motivo, e a escada sobe.** Rede, prazo, 401, 413,
//!   422, 5xx, JSON que nao se le, opcao fora da lista: nada vira palpite. O teto de 100
//!   opcoes do servidor (`MAX_CHOICE_OPTIONS`) e conferido ANTES de sair, para nao pagar a
//!   ida so para ouvir o 413.
//! - **A saida passa pela `conferir` da `decisao.rs`**, a mesma de todo decisor: o Laya
//!   devolve a chave da opcao que recebeu, e qualquer outra coisa morre ali.
//! - **Nao cobra o orcamento da tarefa, e isso e dito.** O `laya-serve` nao devolve uso em
//!   tokens nem tem linha na tabela de precos: o degrau dele, na rota, fica fora da conta
//!   (`orcamento.rs`), ao contrario do degrau `modelo`, que cobra. Quem servir o Laya por
//!   um provedor que cobra por chamada tem de trazer o numero antes de ele entrar na conta.

use super::{Decidido, Decisor, Desfecho, Forma, Questao, Valor, conferir};
use phxclaw_agent_core::BoxFut;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// As chaves do `config.json` (catalogo em `phxclaw-config-runtime`).
pub const CHAVE_URL: &str = "decisao.laya.url";
pub const CHAVE_MODELO: &str = "decisao.laya.modelo";
/// O NOME da credencial (declarada no `http.json`), nunca o segredo. Nao se chama
/// `credencial` porque o `config.json` recusa valor em chave com nome de segredo -- e com
/// razao: o nome nao distingue o que guarda o nome do que guarda o valor.
pub const CHAVE_CREDENCIAL: &str = "decisao.laya.credencial_nome";
pub const CHAVE_LIMIAR: &str = "decisao.laya.limiar";
pub const CHAVE_PRAZO: &str = "decisao.laya.prazo_ms";

/// Os checkpoints que o `laya-serve` serve pelo nome.
pub const MODELOS: [&str; 3] = ["english", "multilingual", "typed-decisions"];
/// O `MAX_CHOICE_OPTIONS` do `laya/serve.py`: acima disso o servidor responde 413.
pub const MAX_OPCOES: usize = 100;
/// O padrao do limiar quando o operador nao diz. O checkpoint multilingual sai SEM
/// temperatura ajustada (README do Laya, Calibration): o numero e conservador de proposito
/// e deve ser refeito sobre dado proprio, no numero de opcoes que a carga usa.
pub const LIMIAR_PADRAO: f64 = 0.8;
pub const PRAZO_PADRAO_MS: u64 = 5_000;
const PRAZO_MAX_MS: u64 = 60_000;
/// A resposta e um objeto pequeno; o teto so impede um servidor errado de mandar megas.
const TETO_BYTES: usize = 256 * 1024;
/// A chave da unica pergunta de cada pedido.
const ID: &str = "q";

/// O predicado vai como escolha de duas opcoes (contrato do R1); as etiquetas sao texto
/// que o modelo le, por isso em portugues e com acento.
pub const SIM: &str = "sim";
pub const NAO: &str = "não";

/// A nota (0..1) vai como `score` de cinco niveis. O Laya exige descricao em todo nivel
/// (nivel nulo e 422); o `score` volta como o indice ESPERADO (0..4) e vira nota dividindo
/// por 4.
pub const NIVEIS_DA_NOTA: [&str; 5] = [
    "0 — nada, nenhum",
    "0,25 — pouco",
    "0,5 — médio, metade",
    "0,75 — muito",
    "1 — totalmente",
];

/// O cliente do `laya-serve`.
#[derive(Debug, Clone)]
pub struct DecisorLaya {
    /// A base (`http://127.0.0.1:8000`); o caminho `/v1/systemone` e acrescentado aqui.
    pub url: String,
    pub modelo: String,
    /// O nome da credencial no `http.json` (vira `Authorization: Bearer` pelo broker).
    pub credencial: Option<String>,
    pub prazo: Duration,
    /// A raiz do agente: onde moram o `http.json` e o broker das credenciais.
    pub raiz: PathBuf,
}

impl DecisorLaya {
    /// Monta conferindo a URL e o modelo (configuracao errada e erro da montagem, nao
    /// `SemDecisao` em toda pergunta para sempre).
    pub fn novo(
        url: &str,
        modelo: &str,
        credencial: Option<String>,
        prazo: Duration,
        raiz: &Path,
    ) -> Result<Self, String> {
        let u = reqwest::Url::parse(url).map_err(|e| format!("{CHAVE_URL} {url:?}: {e}"))?;
        if u.scheme() != "http" && u.scheme() != "https" {
            return Err(format!("{CHAVE_URL} {url:?}: so http e https"));
        }
        if !u.username().is_empty() || u.password().is_some() {
            return Err(format!(
                "{CHAVE_URL}: url com usuario/senha; a credencial entra por {CHAVE_CREDENCIAL}"
            ));
        }
        if !MODELOS.contains(&modelo) {
            return Err(format!(
                "{CHAVE_MODELO} {modelo:?} (use {})",
                MODELOS.join(", ")
            ));
        }
        let ms = prazo.as_millis();
        if ms == 0 || ms > u128::from(PRAZO_MAX_MS) {
            return Err(format!("{CHAVE_PRAZO} de 1 a {PRAZO_MAX_MS}"));
        }
        Ok(Self {
            url: url.trim_end_matches('/').to_string(),
            modelo: modelo.to_string(),
            credencial: credencial.filter(|c| !c.trim().is_empty()),
            prazo,
            raiz: raiz.to_path_buf(),
        })
    }

    /// O decisor e o limiar do degrau pelo `config.json`. Sem `decisao.laya.url` o Laya
    /// esta desligado (`None`): e o padrao.
    pub fn do_config(raiz: &Path) -> Result<Option<(Self, f64)>, String> {
        let Some(url) = crate::config::texto(CHAVE_URL)?.filter(|u| !u.trim().is_empty()) else {
            return Ok(None);
        };
        let modelo = crate::config::texto(CHAVE_MODELO)?.unwrap_or_else(|| "multilingual".into());
        let credencial = crate::config::texto(CHAVE_CREDENCIAL)?;
        let limiar = match crate::config::valor(CHAVE_LIMIAR)? {
            None | Some(Value::Null) => LIMIAR_PADRAO,
            Some(v) => v
                .as_f64()
                .ok_or_else(|| format!("{CHAVE_LIMIAR}: numero de 0 a 1"))?,
        };
        if !(limiar.is_finite() && (0.0..=1.0).contains(&limiar)) {
            return Err(format!("{CHAVE_LIMIAR} fora de 0..1: {limiar}"));
        }
        let prazo = match crate::config::valor(CHAVE_PRAZO)? {
            None | Some(Value::Null) => PRAZO_PADRAO_MS,
            Some(v) => v
                .as_u64()
                .ok_or_else(|| format!("{CHAVE_PRAZO}: inteiro de 1 a {PRAZO_MAX_MS}"))?,
        };
        let d = Self::novo(
            &url,
            &modelo,
            credencial,
            Duration::from_millis(prazo),
            raiz,
        )?;
        Ok(Some((d, limiar)))
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/systemone", self.url)
    }

    /// O corpo do pedido para a questao, ou o motivo de nao haver pedido.
    pub fn corpo(&self, q: &Questao) -> Result<Value, String> {
        let pergunta = match &q.forma {
            Forma::Escolha { opcoes } => {
                if opcoes.is_empty() {
                    return Err("escolha sem opcoes".into());
                }
                if opcoes.len() > MAX_OPCOES {
                    return Err(format!(
                        "{} opcoes passam do teto de {MAX_OPCOES} do laya-serve",
                        opcoes.len()
                    ));
                }
                json!({"type": "choice", "instructions": q.enunciado, "criteria": opcoes})
            }
            Forma::Predicado => {
                json!({"type": "choice", "instructions": q.enunciado, "criteria": [SIM, NAO]})
            }
            Forma::Nota => {
                json!({"type": "score", "instructions": q.enunciado, "criteria": NIVEIS_DA_NOTA})
            }
        };
        Ok(json!({
            "model": self.modelo,
            "state": q.contexto,
            "questions": {ID: pergunta},
        }))
    }

    /// A resposta do servidor (status e corpo) virada desfecho, ANTES da `conferir`.
    fn ler(&self, q: &Questao, status: u64, corpo: &Value) -> Desfecho {
        let sem = |m: String| Desfecho::sem(self.id(), m);
        if status != 200 {
            let detalhe = corpo
                .get("detail")
                .map(|d| match d {
                    Value::String(s) => s.clone(),
                    outro => outro.to_string(),
                })
                .unwrap_or_default();
            let detalhe: String = detalhe.chars().take(200).collect();
            return sem(match status {
                413 => format!("laya-serve recusou pelo tamanho (413): {detalhe}"),
                422 => format!("laya-serve recusou a pergunta (422): {detalhe}"),
                401 => "laya-serve recusou a credencial (401)".to_string(),
                s => format!("laya-serve respondeu {s}: {detalhe}"),
            });
        }
        let Some(r) = corpo.get("answers").and_then(|a| a.get(ID)) else {
            return sem("resposta do laya-serve sem answers.q".into());
        };
        // `answer_confidence` e so ele (ver o cabecalho). Booleano nao e confianca.
        let Some(confianca) = r
            .get("answer_confidence")
            .filter(|v| v.is_number())
            .and_then(Value::as_f64)
        else {
            return sem(
                "resposta sem 'answer_confidence' (servidor em LAYA_JEV_STRICT?); o 'confidence' de entropia nao substitui"
                    .into(),
            );
        };
        let tipo = r.get("type").and_then(Value::as_str).unwrap_or("");
        let valor = match &q.forma {
            Forma::Escolha { .. } | Forma::Predicado if tipo != "choice" => {
                return sem(format!("resposta do tipo {tipo:?}, pedido choice"));
            }
            Forma::Nota if tipo != "score" => {
                return sem(format!("resposta do tipo {tipo:?}, pedido score"));
            }
            Forma::Escolha { .. } => match r.get("choice").and_then(Value::as_str) {
                Some(c) => Valor::Escolha(c.to_string()),
                None => return sem("resposta choice sem 'choice' de texto".into()),
            },
            Forma::Predicado => match r.get("choice").and_then(Value::as_str) {
                Some(SIM) => Valor::Predicado(true),
                Some(NAO) => Valor::Predicado(false),
                outro => return sem(format!("opcao fora da lista: {outro:?}")),
            },
            Forma::Nota => match r.get("score").and_then(Value::as_f64) {
                Some(s) => Valor::Nota(s / (NIVEIS_DA_NOTA.len() - 1) as f64),
                None => return sem("resposta score sem 'score' numerico".into()),
            },
        };
        Desfecho::Decidido(Decidido {
            valor,
            confianca,
            decisor: self.id(),
        })
    }
}

impl Decisor for DecisorLaya {
    fn id(&self) -> String {
        format!("laya:{}", self.modelo)
    }

    fn decidir<'a>(&'a self, q: &'a Questao) -> BoxFut<'a, Desfecho> {
        Box::pin(async move {
            let corpo = match self.corpo(q) {
                Ok(c) => c,
                Err(m) => return Desfecho::sem(self.id(), m),
            };
            let ms = self.prazo.as_millis() as u64;
            let mut pedido = json!({
                "metodo": "POST",
                "url": self.endpoint(),
                "corpo": {"json": corpo},
                // Um segundo a mais que o prazo de fora: quem corta e o `timeout` abaixo, e o
                // motivo sai o mesmo qualquer que seja a fase em que o tempo acabou.
                "teto_ms": ms + 1_000,
                "teto_bytes": TETO_BYTES,
                // 413/422 tem de chegar aqui com o `detail`, nao como erro do no.
                "aceitar_erro": true,
                "resposta": "completa",
            });
            if let Some(c) = &self.credencial {
                pedido["credencial"] = json!(c);
            }
            // O prazo do no vale so para o pedido HTTP; este cobre tambem a resolucao do
            // nome e a credencial.
            let itens = match tokio::time::timeout(
                self.prazo,
                crate::fluxo_http::executar(&self.raiz, &pedido),
            )
            .await
            {
                Err(_) => {
                    return Desfecho::sem(self.id(), format!("prazo de {ms} ms esgotado"));
                }
                Ok(Err(e)) => return Desfecho::sem(self.id(), format!("laya-serve: {e}")),
                Ok(Ok(i)) => i,
            };
            let Some(item) = itens.first() else {
                return Desfecho::sem(self.id(), "laya-serve sem resposta");
            };
            let status = item.get("status").and_then(Value::as_u64).unwrap_or(0);
            let corpo = item.get("corpo").cloned().unwrap_or(Value::Null);
            conferir(q, self.ler(q, status, &corpo))
        })
    }
}
