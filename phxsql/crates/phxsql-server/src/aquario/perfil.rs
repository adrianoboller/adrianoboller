//! O PERFIL VISUAL do aquario (pedido 783): o tom das seis cores nos dois
//! temas, as medidas e a fisica das bolhas, a textura do traco e a letra.
//!
//! # Onde mora, e por que ali
//!
//! No bloco `aquario` do `config.json`, gravado pelo `config_gravar` -- o
//! mesmo caminho das cores do painel da telemetria (`telemetria.cor_*`), que
//! e a preferencia de tela que esta casa ja guardava no servidor. Duas razoes
//! decidiram contra uma tabela `phxsys`:
//!
//! * o portao, a trilha e a escrita atomica ja existem ali: `config_gravar`
//!   exige `administrar` e anota cada campo no diario das diretivas, com o
//!   valor de antes e o de depois. Uma tabela nova pediria os tres de novo, e
//!   a segunda copia de um portao e a que envelhece;
//! * o perfil e do SERVIDOR (a TV de parede e o administrador veem o mesmo
//!   aquario), e nao de um usuario -- preferencia por pessoa ficou no
//!   `localStorage` (a altura da alca), que e onde ela cabe.
//!
//! # O que NAO se configura, e por que
//!
//! * **O significado da cor.** A regra de gravidade do dono (vermelho e
//!   alarme, amarelo e aviso, rosa e encerrando...) e semantica: o perfil
//!   troca o TOM, e o tom tem de continuar na FAMILIA da cor -- a janela de
//!   matiz de [`TONS`]. Um «vermelho» pintado de verde mentiria sobre a
//!   gravidade.
//! * **A forma e o traco.** Circulo, losango, octogono e o padrao do
//!   tracejado sao o sinal que sobra para quem nao distingue a cor; o perfil
//!   so escala o traco ([`MEDIDAS`] `traco_escala`), nunca troca o padrao.
//! * **`msGrande`.** E o `TETO_DO_RAIO_MS` do `anel.rs` (780): a tela e o
//!   servidor contam os aneis pela mesma escala, e um teto configuravel de um
//!   lado so seriam duas formulas.
//! * **`passagens` e `folga`.** Sao a garantia de que bolha nao se sobrepoe,
//!   e nao aparencia.
//!
//! # A gravacao recusa; o arranque avisa
//!
//! Pela tela, valor fora da faixa, contraste abaixo de 3:1 ou fonte fora da
//! lista e RECUSADO com o motivo ([`conferir_secao`]), antes de tocar no
//! arquivo. No arranque, o mesmo valor escrito a mao vira AVISO e cai no de
//! fabrica, campo a campo -- derrubar o servidor por causa de uma cor seria
//! a guarda cobrando mais caro do que aquilo que ela protege (a mesma decisao
//! do `telemetria.cor_*`).

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// O contraste minimo de uma cor de bolha contra o fundo do aquario.
///
/// 3:1 e o piso da WCAG para objeto grafico (1.4.11): a bolha e um contorno,
/// e nao texto -- o texto dentro dela tem contorno proprio no fundo.
pub const CONTRASTE_MINIMO: f64 = 3.0;

/// Os dois fundos do aquario: o `--fundo` dos dois temas do `index.html`.
/// Um teste le os dois de la.
pub const FUNDO_ESCURO: &str = "#010418";
pub const FUNDO_CLARO: &str = "#f7f5f2";

/// Uma cor da regra de gravidade: o nome (a chave do retrato), a variavel do
/// CSS que a pinta de fabrica, os dois tons de fabrica e a janela de matiz
/// que define a FAMILIA dela.
pub struct Tom {
    pub cor: &'static str,
    pub variavel: &'static str,
    pub fabrica_escuro: &'static str,
    pub fabrica_claro: &'static str,
    /// Graus, `(de, ate)`; `de > ate` atravessa o zero (o vermelho).
    pub matiz: (f64, f64),
}

/// As seis cores, na ordem do `CORES` do `ui/aquario.js`.
///
/// As janelas nao se cruzam onde as cores precisam se separar (vermelho x
/// rosa x amarelo); os dois azuis dividem a janela porque o que os separa e
/// a borda dupla e o motivo, nao o matiz.
pub const TONS: &[Tom] = &[
    Tom {
        cor: "verde",
        variavel: "--ok",
        fabrica_escuro: "#6cc98c",
        fabrica_claro: "#2f7a3e",
        matiz: (75.0, 170.0),
    },
    Tom {
        cor: "azul_claro",
        variavel: "--reg",
        fabrica_escuro: "#5fa6e8",
        fabrica_claro: "#1f5c93",
        matiz: (175.0, 260.0),
    },
    Tom {
        cor: "azul_escuro",
        variavel: "--aq-azul-escuro",
        fabrica_escuro: "#9cc3ff",
        fabrica_claro: "#1f3f7a",
        matiz: (175.0, 260.0),
    },
    Tom {
        cor: "amarelo",
        variavel: "--ambar",
        fabrica_escuro: "#ffc43d",
        fabrica_claro: "#a06a00",
        matiz: (25.0, 70.0),
    },
    Tom {
        cor: "vermelho",
        variavel: "--vermelho",
        fabrica_escuro: "#ff5f5f",
        fabrica_claro: "#b71414",
        matiz: (345.0, 20.0),
    },
    Tom {
        cor: "rosa",
        variavel: "--acao-marcar",
        fabrica_escuro: "#ff8fc7",
        fabrica_claro: "#b5257f",
        matiz: (290.0, 344.0),
    },
];

/// Saturacao minima (HSL) para o matiz querer dizer alguma coisa: cinza nao
/// tem familia, e um «vermelho» cinza esconderia a gravidade.
pub const SATURACAO_MINIMA: f64 = 0.25;

/// Uma medida numerica: o campo no `config.json`, o nome no objeto `M` do
/// `ui/aquario.js`, o valor de fabrica e a faixa.
pub struct Medida {
    pub campo: &'static str,
    pub js: &'static str,
    pub fabrica: f64,
    pub min: f64,
    pub max: f64,
    pub inteiro: bool,
}

/// As medidas configuraveis. O de fabrica e o literal do `M` de hoje -- um
/// teste le cada um do `aquario.js` e compara.
///
/// Os tetos tem motivo, um por linha:
pub const MEDIDAS: &[Medida] = &[
    // a recem-nascida precisa ser vista, e nao pode nascer do tamanho da
    // grande
    Medida {
        campo: "raio_min",
        js: "rMin",
        fabrica: 9.0,
        min: 4.0,
        max: 20.0,
        inteiro: true,
    },
    // o teto do 780: no menor tanque da TV (320 px, cinco faixas de 64 px),
    // a maior bolha nao passa de duas faixas -- acima disso ela esconde as
    // vizinhas, e o anel existe justamente para a bolha velha NAO crescer
    Medida {
        campo: "raio_max",
        js: "rMax",
        fabrica: 46.0,
        min: 24.0,
        max: 64.0,
        inteiro: true,
    },
    // acima de 0,6 a caixa fica sem agua para as bolhas se moverem
    Medida {
        campo: "ocupacao",
        js: "ocupacaoMax",
        fabrica: 0.46,
        min: 0.2,
        max: 0.6,
        inteiro: false,
    },
    Medida {
        campo: "cresce",
        js: "cresce",
        fabrica: 2.2,
        min: 0.5,
        max: 6.0,
        inteiro: false,
    },
    Medida {
        campo: "mola_faixa",
        js: "molaFaixa",
        fabrica: 5.0,
        min: 1.0,
        max: 12.0,
        inteiro: false,
    },
    Medida {
        campo: "atrito",
        js: "atrito",
        fabrica: 1.6,
        min: 0.3,
        max: 4.0,
        inteiro: false,
    },
    // 1 seria colisao sem perda nenhuma: as bolhas nunca assentam
    Medida {
        campo: "quique",
        js: "quique",
        fabrica: 0.35,
        min: 0.0,
        max: 0.9,
        inteiro: false,
    },
    // abaixo de 120 ms o estouro nao se ve; acima de 1,5 s a tarefa que
    // acabou parece viva
    Medida {
        campo: "estouro_ms",
        js: "estouroMs",
        fabrica: 380.0,
        min: 120.0,
        max: 1500.0,
        inteiro: true,
    },
    Medida {
        campo: "rotulo_min",
        js: "rotuloMin",
        fabrica: 15.0,
        min: 10.0,
        max: 30.0,
        inteiro: true,
    },
    // a espessura e o tracejado ESCALAM o desenho de fabrica: o padrao
    // (cheio, tracejado, pontilhado, traco longo) e sinal e nao muda
    Medida {
        campo: "espessura",
        js: "espessura",
        fabrica: 1.0,
        min: 0.5,
        max: 2.0,
        inteiro: false,
    },
    Medida {
        campo: "traco_escala",
        js: "tracoEscala",
        fabrica: 1.0,
        min: 0.5,
        max: 2.0,
        inteiro: false,
    },
    // o miolo: acima de 0,4 a bolha vira fundo cheio, e a casa pinta
    // contorno, nunca fundo cheio
    Medida {
        campo: "preenchimento",
        js: "preenchimento",
        fabrica: 0.14,
        min: 0.05,
        max: 0.4,
        inteiro: false,
    },
    Medida {
        campo: "fonte_rotulo_px",
        js: "fonteRotulo",
        fabrica: 10.0,
        min: 8.0,
        max: 16.0,
        inteiro: true,
    },
    Medida {
        campo: "fonte_faixa_px",
        js: "fonteFaixa",
        fabrica: 11.0,
        min: 9.0,
        max: 18.0,
        inteiro: true,
    },
];

/// As letras que o rotulo da bolha e o nome da faixa aceitam. A primeira e a
/// de fabrica. Os TITULOS da tela nao mudam: Exo 2 e a marca.
pub const FONTES: &[&str] = &[
    "Exo 2",
    "IBM Plex Mono",
    "Helvetica Neue",
    "Arial",
    "system-ui",
];

/// O campo da fonte, no bloco `aquario`.
pub const CAMPO_FONTE: &str = "fonte";

/// Os nomes de campo de cor: `cor_<cor>` (tema escuro) e `cor_<cor>_claro`.
pub fn campo_da_cor(cor: &str, claro: bool) -> String {
    if claro {
        format!("cor_{cor}_claro")
    } else {
        format!("cor_{cor}")
    }
}

/// Todos os campos do bloco, na ordem em que a tela os mostra -- a lista que
/// o aviso de campo estranho do `config.rs` usa.
pub const CAMPOS: &[&str] = &[
    "cor_verde",
    "cor_verde_claro",
    "cor_azul_claro",
    "cor_azul_claro_claro",
    "cor_azul_escuro",
    "cor_azul_escuro_claro",
    "cor_amarelo",
    "cor_amarelo_claro",
    "cor_vermelho",
    "cor_vermelho_claro",
    "cor_rosa",
    "cor_rosa_claro",
    "raio_min",
    "raio_max",
    "ocupacao",
    "cresce",
    "mola_faixa",
    "atrito",
    "quique",
    "estouro_ms",
    "rotulo_min",
    "espessura",
    "traco_escala",
    "preenchimento",
    "fonte_rotulo_px",
    "fonte_faixa_px",
    "fonte",
];

/// O perfil lido: tom vazio = o de fabrica, que segue o tema.
#[derive(Debug, Clone, PartialEq)]
pub struct Perfil {
    /// `(escuro, claro)` por cor, na ordem de [`TONS`].
    pub tons: Vec<(String, String)>,
    /// Na ordem de [`MEDIDAS`].
    pub medidas: Vec<f64>,
    pub fonte: String,
}

impl Default for Perfil {
    fn default() -> Perfil {
        Perfil {
            tons: TONS
                .iter()
                .map(|_| (String::new(), String::new()))
                .collect(),
            medidas: MEDIDAS.iter().map(|m| m.fabrica).collect(),
            fonte: FONTES[0].to_string(),
        }
    }
}

// --------------------------------------------------------------- a cor

fn rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let c = |i: usize| {
        u8::from_str_radix(&h[i..i + 2], 16)
            .ok()
            .map(|v| v as f64 / 255.0)
    };
    Some((c(0)?, c(2)?, c(4)?))
}

/// Luminancia relativa da WCAG 2.x.
fn luminancia(hex: &str) -> Option<f64> {
    let (r, g, b) = rgb(hex)?;
    let lin = |c: f64| {
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b))
}

/// A razao de contraste entre duas cores `#rrggbb`.
pub fn contraste(a: &str, b: &str) -> Option<f64> {
    let (la, lb) = (luminancia(a)?, luminancia(b)?);
    let (claro, escuro) = if la > lb { (la, lb) } else { (lb, la) };
    Some((claro + 0.05) / (escuro + 0.05))
}

/// `(matiz em graus, saturacao HSL)`.
pub fn matiz_e_saturacao(hex: &str) -> Option<(f64, f64)> {
    let (r, g, b) = rgb(hex)?;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let l = (max + min) / 2.0;
    if d < 1e-9 {
        return Some((0.0, 0.0));
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    Some((if h < 0.0 { h + 360.0 } else { h }, s))
}

fn na_janela(h: f64, (de, ate): (f64, f64)) -> bool {
    if de <= ate {
        h >= de && h <= ate
    } else {
        h >= de || h <= ate
    }
}

fn virgula(x: f64) -> String {
    format!("{x:.2}").replace('.', ",")
}

/// Por que este tom NAO serve para esta cor neste tema -- ou `None`.
pub fn recusa_do_tom(tom: &Tom, claro: bool, hex: &str) -> Option<String> {
    if hex.is_empty() {
        return None; // o de fabrica, que segue o tema
    }
    let campo = campo_da_cor(tom.cor, claro);
    if !crate::config::cor_valida(hex) {
        return Some(format!("aquario.{campo}: {hex:?} nao e uma cor #rrggbb"));
    }
    let (fundo, tema) = if claro {
        (FUNDO_CLARO, "claro")
    } else {
        (FUNDO_ESCURO, "escuro")
    };
    let razao = contraste(hex, fundo).unwrap_or(0.0);
    if razao < CONTRASTE_MINIMO {
        return Some(format!(
            "aquario.{campo}: contraste {}:1 contra o fundo {fundo} do tema {tema}; \
             o minimo e 3:1, senao a bolha some na agua",
            virgula(razao)
        ));
    }
    let (h, s) = matiz_e_saturacao(hex).unwrap_or((0.0, 0.0));
    if s < SATURACAO_MINIMA || !na_janela(h, tom.matiz) {
        return Some(format!(
            "aquario.{campo}: {hex} nao e da familia do {} (matiz {:.0} graus, \
             saturacao {:.0}%; a familia vai de {:.0} a {:.0} graus, com saturacao de \
             {:.0}% em diante). Configura-se o TOM, e nao o significado da cor",
            tom.cor,
            h,
            s * 100.0,
            tom.matiz.0,
            tom.matiz.1,
            SATURACAO_MINIMA * 100.0
        ));
    }
    None
}

/// Por que este valor NAO serve para esta medida -- ou `None`.
pub fn recusa_da_medida(m: &Medida, v: &Json) -> Option<String> {
    let Some(x) = v.numero() else {
        return Some(format!(
            "aquario.{}: espera um numero, veio {}",
            m.campo,
            v.escrever()
        ));
    };
    if m.inteiro && x.fract() != 0.0 {
        return Some(format!("aquario.{}: espera um inteiro, veio {x}", m.campo));
    }
    if !(m.min..=m.max).contains(&x) {
        return Some(format!(
            "aquario.{}: {x} esta fora da faixa de {} a {}",
            m.campo, m.min, m.max
        ));
    }
    None
}

fn recusa_da_fonte(v: &Json) -> Option<String> {
    match v.texto() {
        Some(f) if FONTES.contains(&f) => None,
        _ => Some(format!(
            "aquario.fonte: {} nao esta na lista ({}); os titulos continuam em Exo 2, que e a marca",
            v.escrever(),
            FONTES.join(", ")
        )),
    }
}

impl Perfil {
    fn medida(&self, campo: &str) -> f64 {
        MEDIDAS
            .iter()
            .position(|m| m.campo == campo)
            .map(|i| self.medidas[i])
            .unwrap_or(0.0)
    }

    /// O que vale entre campos: cada um passou na sua faixa, e o par ainda
    /// pode nao fazer sentido.
    fn recusas_cruzadas(&self) -> Vec<String> {
        let mut fora = Vec::new();
        let (rmin, rmax) = (self.medida("raio_min"), self.medida("raio_max"));
        if rmax < 2.0 * rmin {
            fora.push(format!(
                "aquario.raio_max ({rmax}) tem de ser ao menos o dobro de aquario.raio_min \
                 ({rmin}): senao a bolha de 5 ms e a de 30 s ficam do mesmo tamanho"
            ));
        }
        let rot = self.medida("rotulo_min");
        if rot >= rmax {
            fora.push(format!(
                "aquario.rotulo_min ({rot}) tem de ser menor que aquario.raio_max ({rmax}): \
                 senao nenhuma bolha mostra o nome"
            ));
        }
        fora
    }

    /// Le o bloco, campo a campo, e devolve o perfil e TODAS as recusas.
    ///
    /// O campo recusado fica com o de fabrica: e assim que o arranque segue
    /// de pe, e e o mesmo leitor que a gravacao usa para recusar -- uma regra
    /// so, nao uma no arranque e outra na tela.
    pub fn ler(j: &Json) -> (Perfil, Vec<String>) {
        let mut p = Perfil::default();
        let mut recusas = Vec::new();
        let Some(sec) = j.campo("aquario") else {
            return (p, recusas);
        };
        for (i, tom) in TONS.iter().enumerate() {
            for claro in [false, true] {
                let campo = campo_da_cor(tom.cor, claro);
                let Some(v) = sec.campo(&campo) else { continue };
                let t = v.texto().map(|t| t.trim().to_lowercase());
                let Some(t) = t else {
                    recusas.push(format!("aquario.{campo}: espera uma cor #rrggbb"));
                    continue;
                };
                match recusa_do_tom(tom, claro, &t) {
                    Some(r) => recusas.push(r),
                    None if claro => p.tons[i].1 = t,
                    None => p.tons[i].0 = t,
                }
            }
        }
        for (i, m) in MEDIDAS.iter().enumerate() {
            let Some(v) = sec.campo(m.campo) else {
                continue;
            };
            match recusa_da_medida(m, v) {
                Some(r) => recusas.push(r),
                None => p.medidas[i] = v.numero().unwrap_or(m.fabrica),
            }
        }
        if let Some(v) = sec.campo(CAMPO_FONTE) {
            match recusa_da_fonte(v) {
                Some(r) => recusas.push(r),
                None => p.fonte = v.texto().unwrap_or(FONTES[0]).to_string(),
            }
        }
        let cruzadas = p.recusas_cruzadas();
        if !cruzadas.is_empty() {
            // o par torto volta inteiro ao de fabrica: metade de um par e o
            // que produziria o desenho que a regra recusa
            for c in ["raio_min", "raio_max", "rotulo_min"] {
                if let Some(i) = MEDIDAS.iter().position(|m| m.campo == c) {
                    p.medidas[i] = MEDIDAS[i].fabrica;
                }
            }
            recusas.extend(cruzadas);
        }
        (p, recusas)
    }

    /// O leitor do ARRANQUE: recusa vira aviso, e o campo fica de fabrica.
    pub fn de_json(j: &Json, avisos: &mut Vec<String>) -> Perfil {
        let (p, recusas) = Perfil::ler(j);
        for r in recusas {
            avisos.push(format!("{r}; o de fabrica continua valendo"));
        }
        p
    }

    /// O bloco como ele vai ao `config.json` e a op `config`.
    pub fn para_json(&self) -> Json {
        let mut pares: Vec<(String, Json)> = Vec::new();
        for (tom, (e, c)) in TONS.iter().zip(&self.tons) {
            pares.push((campo_da_cor(tom.cor, false), Json::texto_de(e.as_str())));
            pares.push((campo_da_cor(tom.cor, true), Json::texto_de(c.as_str())));
        }
        for (m, v) in MEDIDAS.iter().zip(&self.medidas) {
            pares.push((m.campo.to_string(), Json::Numero(*v)));
        }
        pares.push((CAMPO_FONTE.to_string(), Json::texto_de(self.fonte.as_str())));
        Json::Objeto(pares)
    }

    /// O perfil como a TELA o le: o valor, a fabrica e a faixa de cada
    /// coisa. A faixa vai junto para o editor nao trazer a sua -- duas faixas
    /// divergem no primeiro teto que alguem mudar de um lado so.
    pub fn para_tela(&self) -> Json {
        let cores: Vec<Json> = TONS
            .iter()
            .zip(&self.tons)
            .map(|(t, (e, c))| {
                Json::objeto(vec![
                    ("cor", Json::texto_de(t.cor)),
                    ("variavel", Json::texto_de(t.variavel)),
                    ("escuro", Json::texto_de(e.as_str())),
                    ("claro", Json::texto_de(c.as_str())),
                    ("fabrica_escuro", Json::texto_de(t.fabrica_escuro)),
                    ("fabrica_claro", Json::texto_de(t.fabrica_claro)),
                ])
            })
            .collect();
        let medidas: Vec<Json> = MEDIDAS
            .iter()
            .zip(&self.medidas)
            .map(|(m, v)| {
                Json::objeto(vec![
                    ("campo", Json::texto_de(m.campo)),
                    ("js", Json::texto_de(m.js)),
                    ("valor", Json::Numero(*v)),
                    ("fabrica", Json::Numero(m.fabrica)),
                    ("min", Json::Numero(m.min)),
                    ("max", Json::Numero(m.max)),
                    ("inteiro", Json::Bool(m.inteiro)),
                ])
            })
            .collect();
        Json::objeto(vec![
            ("versao", Json::texto_de(self.versao())),
            ("cores", Json::Lista(cores)),
            ("medidas", Json::Lista(medidas)),
            ("fonte", Json::texto_de(self.fonte.as_str())),
            (
                "fontes",
                Json::Lista(FONTES.iter().map(|f| Json::texto_de(*f)).collect()),
            ),
            (
                "fundos",
                Json::objeto(vec![
                    ("escuro", Json::texto_de(FUNDO_ESCURO)),
                    ("claro", Json::texto_de(FUNDO_CLARO)),
                ]),
            ),
            ("contraste_minimo", Json::Numero(CONTRASTE_MINIMO)),
        ])
    }

    /// A digital do perfil: FNV-1a 64 do bloco. A tela manda a que tem, e o
    /// retrato so traz o perfil quando ela mudou -- de dois em dois segundos,
    /// nao se reenvia o que nao mudou. Digital e nao contador: um contador
    /// recomecaria no arranque e a TV que ficou aberta acharia que ja tem o
    /// perfil novo.
    pub fn versao(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.para_json().escrever().bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }
}

/// A conferencia da GRAVACAO: o bloco `aquario` da arvore que vai ao disco,
/// recusado com todos os motivos de uma vez.
pub fn conferir_secao(arvore: &Json) -> Result<()> {
    let (_, recusas) = Perfil::ler(arvore);
    if recusas.is_empty() {
        return Ok(());
    }
    Err(PhxError::Esquema(format!(
        "perfil do aquario recusado: {}",
        recusas.join("; ")
    )))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com(bloco: &str) -> Json {
        Json::analisar(&format!(r#"{{"aquario":{bloco}}}"#)).unwrap()
    }

    /// O de fabrica e o desenho de HOJE: cada medida e o literal do `M` do
    /// `ui/aquario.js`. Mudar um lado so derruba este teste nomeando o campo.
    #[test]
    fn a_fabrica_e_o_m_do_aquario_js() {
        let js = include_str!("../../ui/aquario.js");
        let ini = js.find("const M = {").expect("o M sumiu do aquario.js");
        let corpo = &js[ini..ini + js[ini..].find("\n  };").unwrap()];
        for m in MEDIDAS {
            let chave = format!("\n    {}:", m.js);
            let i = corpo
                .find(&chave)
                .unwrap_or_else(|| panic!("{} sumiu do M", m.js));
            let resto = corpo[i + chave.len()..].trim_start();
            let num: String = resto
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            assert_eq!(num.parse::<f64>().ok(), Some(m.fabrica), "{}", m.js);
            assert!(
                (m.min..=m.max).contains(&m.fabrica),
                "{}: o de fabrica fora da propria faixa",
                m.campo
            );
        }
    }

    /// Os tons de fabrica sao os do `index.html` nos dois temas, e passam na
    /// regra que a gravacao aplica -- a regra nao pode recusar o desenho de
    /// hoje.
    #[test]
    fn a_fabrica_e_o_css_do_index_e_passa_na_regra() {
        let html = include_str!("../../ui/index.html");
        let claro_ini = html.find(":root[data-tema=\"claro\"]{").unwrap();
        let (escuro, claro) = html.split_at(claro_ini);
        let valor = |bloco: &str, var: &str| -> String {
            let i = bloco
                .find(&format!("{var}:#"))
                .unwrap_or_else(|| panic!("{var} sumiu"));
            bloco[i + var.len() + 1..i + var.len() + 8].to_lowercase()
        };
        assert_eq!(valor(escuro, "--fundo"), FUNDO_ESCURO);
        assert_eq!(valor(claro, "--fundo"), FUNDO_CLARO);
        for t in TONS {
            assert_eq!(valor(escuro, t.variavel), t.fabrica_escuro, "{}", t.cor);
            assert_eq!(valor(claro, t.variavel), t.fabrica_claro, "{} claro", t.cor);
            assert_eq!(recusa_do_tom(t, false, t.fabrica_escuro), None);
            assert_eq!(recusa_do_tom(t, true, t.fabrica_claro), None);
        }
        // e a reserva `fb` do CORES do aquario.js e o tom escuro de fabrica
        let js = include_str!("../../ui/aquario.js");
        for t in TONS {
            let linha = js
                .lines()
                .find(|l| {
                    l.trim_start().starts_with(&format!("{}:", t.cor)) && l.contains("forma:")
                })
                .unwrap_or_else(|| panic!("{} sumiu do CORES", t.cor));
            assert!(
                linha.contains(&format!("fb: \"{}\"", t.fabrica_escuro)),
                "{linha}"
            );
        }
    }

    /// O comportamento VELHO: sem bloco, o perfil e o de fabrica e nada e
    /// recusado -- config escrito antes do 783 pinta como pintava.
    #[test]
    fn sem_bloco_nada_muda() {
        let (p, r) = Perfil::ler(&Json::analisar("{}").unwrap());
        assert_eq!(p, Perfil::default());
        assert!(r.is_empty());
        assert!(conferir_secao(&Json::analisar("{}").unwrap()).is_ok());
        // e o bloco escrito com os proprios valores de fabrica tambem passa
        let fab = Json::objeto(vec![("aquario", Perfil::default().para_json())]);
        assert!(conferir_secao(&fab).is_ok(), "{:?}", conferir_secao(&fab));
    }

    #[test]
    fn contraste_ruim_e_recusado_nos_dois_temas() {
        // cinza-azulado escuro sobre o #010418: some na agua
        let e = conferir_secao(&com(r##"{"cor_vermelho":"#3a0a0a"}"##)).unwrap_err();
        assert!(e.to_string().contains("contraste"), "{e}");
        assert!(e.to_string().contains("tema escuro"), "{e}");
        // e o rosa claro sobre o papel
        let e = conferir_secao(&com(r##"{"cor_rosa_claro":"#ffd0e8"}"##)).unwrap_err();
        assert!(e.to_string().contains("tema claro"), "{e}");
        // o mesmo tom que passa no escuro e aceito ali
        assert!(conferir_secao(&com(r##"{"cor_vermelho":"#ff3030"}"##)).is_ok());
    }

    #[test]
    fn o_significado_nao_se_configura() {
        // vermelho pintado de verde: contraste otimo, familia errada
        let e = conferir_secao(&com(r##"{"cor_vermelho":"#30ff30"}"##)).unwrap_err();
        assert!(e.to_string().contains("familia do vermelho"), "{e}");
        // cinza nao tem familia
        let e = conferir_secao(&com(r##"{"cor_amarelo":"#a0a0a0"}"##)).unwrap_err();
        assert!(e.to_string().contains("familia do amarelo"), "{e}");
    }

    #[test]
    fn medidas_fora_da_faixa_sao_recusadas() {
        for (bloco, diz) in [
            (r#"{"raio_max":200}"#, "aquario.raio_max: 200"),
            (r#"{"raio_max":65}"#, "fora da faixa"),
            (r#"{"estouro_ms":10}"#, "aquario.estouro_ms"),
            (r#"{"quique":1.5}"#, "aquario.quique"),
            (r#"{"raio_min":9.5}"#, "inteiro"),
            (r#"{"atrito":"muito"}"#, "numero"),
            (r#"{"raio_min":20,"raio_max":30}"#, "dobro"),
            (
                r#"{"rotulo_min":30,"raio_max":28}"#,
                "nenhuma bolha mostra o nome",
            ),
        ] {
            let e = conferir_secao(&com(bloco)).expect_err(bloco);
            assert!(e.to_string().contains(diz), "{bloco}: {e}");
        }
        // os limites sao inclusivos
        assert!(conferir_secao(&com(r#"{"raio_max":64,"raio_min":4,"quique":0}"#)).is_ok());
    }

    #[test]
    fn fonte_fora_da_lista_e_recusada() {
        let e = conferir_secao(&com(r#"{"fonte":"Comic Sans MS"}"#)).unwrap_err();
        assert!(e.to_string().contains("Exo 2"), "{e}");
        assert!(conferir_secao(&com(r#"{"fonte":"IBM Plex Mono"}"#)).is_ok());
    }

    /// O arranque nao cai: o campo recusado vira aviso e fica de fabrica, e o
    /// campo bom do mesmo bloco vale.
    #[test]
    fn no_arranque_a_recusa_vira_aviso() {
        let mut avisos = Vec::new();
        let p = Perfil::de_json(
            &com(r##"{"cor_verde":"#000000","raio_max":30,"raio_min":20}"##),
            &mut avisos,
        );
        assert!(p.tons[0].0.is_empty());
        // o par torto volta inteiro
        assert_eq!(p.medida("raio_max"), 46.0);
        assert_eq!(p.medida("raio_min"), 9.0);
        assert_eq!(avisos.len(), 2, "{avisos:?}");
        let p = Perfil::de_json(
            &com(r##"{"cor_verde":"#33dd66","raio_max":50}"##),
            &mut avisos,
        );
        assert_eq!(p.tons[0].0, "#33dd66");
        assert_eq!(p.medida("raio_max"), 50.0);
    }

    /// A digital muda quando o perfil muda, e so entao.
    #[test]
    fn a_versao_e_a_digital_do_perfil() {
        let a = Perfil::default();
        assert_eq!(a.versao(), Perfil::default().versao());
        let (b, _) = Perfil::ler(&com(r#"{"raio_max":50}"#));
        assert_ne!(a.versao(), b.versao());
    }

    /// A lista de campos e a que o leitor le: um campo que o leitor conhece
    /// e a lista nao, viraria «campo estranho» no aviso do arranque.
    #[test]
    fn a_lista_de_campos_e_a_do_leitor() {
        let lidos: Vec<String> = match Perfil::default().para_json() {
            Json::Objeto(p) => p.into_iter().map(|(k, _)| k).collect(),
            _ => unreachable!(),
        };
        assert_eq!(
            lidos,
            CAMPOS.iter().map(|c| c.to_string()).collect::<Vec<_>>()
        );
        for c in CAMPOS {
            assert!(
                crate::config::campo_editavel(&format!("aquario.{c}")).is_some(),
                "aquario.{c} nao esta em CAMPOS_EDITAVEIS"
            );
        }
        let editaveis = crate::config::CAMPOS_EDITAVEIS
            .iter()
            .filter(|(c, _, _)| c.starts_with("aquario."))
            .count();
        assert_eq!(editaveis, CAMPOS.len());
    }

    #[test]
    fn contraste_confere_com_a_wcag() {
        // vetores conhecidos: preto/branco 21:1, cinza #777 sobre branco 4,48
        assert!((contraste("#000000", "#ffffff").unwrap() - 21.0).abs() < 1e-9);
        assert!((contraste("#777777", "#ffffff").unwrap() - 4.478).abs() < 0.01);
    }
}
