//! Os modelos que existem para esta maquina, e o que se sabe de cada um.
//!
//! De onde vem cada campo importa mais que o campo:
//!   - tamanho, contexto, quantizacao: do CATALOGO (servico local ou arquivo);
//!   - cabe ou nao cabe: CALCULADO com a memoria medida desta maquina;
//!   - velocidade: so de MEDICAO feita aqui (`wx-modelos medir`);
//!   - "inteligencia": nao se mede aqui e nao se inventa -- fica INDISPONIVEL
//!     a menos que o catalogo traga a nota E a fonte dela.
//!
//! A tela da imagem que originou isto mostrava "INTELIGENCIA 10%" num radar
//! bonito. Numero assim decide compra de hardware; ou vem com fonte, ou nao vem.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::json::{analisar, Valor};
use crate::maquina::Maquina;

#[derive(Debug, Clone)]
pub struct Modelo {
    pub id: String,
    pub nome: String,
    pub parametros: Option<String>,
    pub quantizacao: Option<String>,
    pub contexto: Option<u32>,
    pub bytes: Option<u64>,
    pub instalado: bool,
    pub modalidades: Vec<String>,
    /// nota de qualidade SO com fonte declarada no catalogo
    pub nota: Option<(f64, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Couber {
    Cabe,
    Apertado,
    NaoCabe,
    Desconhecido,
}

impl Couber {
    pub fn rotulo(&self) -> &'static str {
        match self {
            Couber::Cabe => "cabe",
            Couber::Apertado => "apertado",
            Couber::NaoCabe => "não cabe",
            Couber::Desconhecido => "INDISPONÍVEL",
        }
    }
}

impl Modelo {
    /// Cabe na memoria desta maquina? So responde com os dois numeros medidos.
    pub fn couber(&self, m: &Maquina) -> Couber {
        let (Some(bytes), Some(orcamento)) = (self.bytes, m.orcamento_de_memoria()) else {
            return Couber::Desconhecido;
        };
        if bytes <= orcamento * 8 / 10 {
            Couber::Cabe
        } else if bytes <= orcamento {
            Couber::Apertado
        } else {
            Couber::NaoCabe
        }
    }

    /// Precisao pela quantizacao: rotulo derivado, nao numero inventado.
    pub fn precisao(&self) -> Option<&'static str> {
        let q = self.quantizacao.as_deref()?.to_ascii_uppercase();
        Some(if q.contains("F16") || q.contains("BF16") {
            "sem perda (16 bits)"
        } else if q.contains("Q8") {
            "muito alta"
        } else if q.contains("Q6") {
            "alta"
        } else if q.contains("Q5") {
            "média"
        } else if q.contains("Q4") {
            "média-baixa"
        } else {
            return None;
        })
    }

    fn do_json(v: &Valor, instalados: &[String]) -> Option<Modelo> {
        let id = v.campo_texto("id").or_else(|| v.campo_texto("name"))?;
        let nota = match (v.campo_numero("nota"), v.campo_texto("nota_fonte")) {
            (Some(n), Some(f)) => Some((n, f)),
            // nota sem fonte e descartada de proposito
            _ => None,
        };
        Some(Modelo {
            nome: v
                .campo_texto("nome")
                .or_else(|| v.campo_texto("display_name"))
                .unwrap_or_else(|| id.clone()),
            parametros: v
                .campo_texto("parametros")
                .or_else(|| v.campo_texto("parameters")),
            quantizacao: v
                .campo_texto("quantizacao")
                .or_else(|| v.campo_texto("quantization")),
            contexto: v
                .campo_numero("contexto")
                .or_else(|| v.campo_numero("context_length"))
                .map(|n| n as u32),
            bytes: v
                .campo_numero("bytes")
                .or_else(|| v.campo_numero("size_bytes"))
                .map(|n| n as u64),
            instalado: v
                .campo("instalado")
                .and_then(|x| match x {
                    Valor::Booleano(b) => Some(*b),
                    _ => None,
                })
                .unwrap_or_else(|| instalados.contains(&id)),
            modalidades: v
                .campo("modalidades")
                .and_then(|x| x.lista())
                .map(|l| {
                    l.iter()
                        .filter_map(|i| i.texto().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            nota,
            id,
        })
    }
}

/// Le o catalogo de um arquivo JSON. Formato: {"modelos": [...]} ou uma lista.
pub fn do_arquivo(caminho: &Path) -> Result<Vec<Modelo>, String> {
    let texto = fs::read_to_string(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    let v = analisar(&texto).map_err(|e| format!("{}: {e}", caminho.display()))?;
    Ok(da_estrutura(&v, &[]))
}

pub fn da_estrutura(v: &Valor, instalados: &[String]) -> Vec<Modelo> {
    let lista = v
        .campo("modelos")
        .and_then(|x| x.lista())
        .or_else(|| v.campo("models").and_then(|x| x.lista()))
        .or_else(|| v.lista());
    lista
        .map(|l| {
            l.iter()
                .filter_map(|i| Modelo::do_json(i, instalados))
                .collect()
        })
        .unwrap_or_default()
}

/// Medicoes de velocidade feitas NESTA maquina, guardadas entre execucoes.
pub fn medicoes(caminho: &Path) -> BTreeMap<String, f64> {
    let mut m = BTreeMap::new();
    let Ok(texto) = fs::read_to_string(caminho) else {
        return m;
    };
    let Ok(v) = analisar(&texto) else { return m };
    if let Valor::Objeto(obj) = v {
        for (k, val) in obj {
            if let Some(n) = val.campo_numero("tokens_por_segundo") {
                m.insert(k, n);
            }
        }
    }
    m
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::maquina::Maquina;

    fn maquina_de(livre: u64) -> Maquina {
        Maquina {
            memoria_livre_bytes: Some(livre),
            ..Default::default()
        }
    }

    #[test]
    fn couber_sai_dos_dois_numeros_medidos() {
        let g = 1_073_741_824u64;
        let mut m =
            Modelo::do_json(&analisar(r#"{"id":"x","bytes":4000000000}"#).unwrap(), &[]).unwrap();
        assert_eq!(m.couber(&maquina_de(16 * g)), Couber::Cabe);
        assert_eq!(m.couber(&maquina_de(6 * g)), Couber::Apertado);
        assert_eq!(m.couber(&maquina_de(2 * g)), Couber::NaoCabe);
        // sem o tamanho do modelo nao ha resposta -- e ela nao se chuta
        m.bytes = None;
        assert_eq!(m.couber(&maquina_de(16 * g)), Couber::Desconhecido);
        // sem a memoria medida, idem
        let m2 = Modelo::do_json(&analisar(r#"{"id":"x","bytes":1}"#).unwrap(), &[]).unwrap();
        assert_eq!(m2.couber(&Maquina::default()), Couber::Desconhecido);
    }

    #[test]
    fn nota_sem_fonte_e_descartada() {
        let com = Modelo::do_json(
            &analisar(r#"{"id":"a","nota":8.1,"nota_fonte":"lmarena 2026-08"}"#).unwrap(),
            &[],
        )
        .unwrap();
        let sem = Modelo::do_json(&analisar(r#"{"id":"b","nota":9.9}"#).unwrap(), &[]).unwrap();
        assert!(com.nota.is_some());
        assert!(sem.nota.is_none(), "nota sem fonte nao pode entrar na tela");
    }

    #[test]
    fn precisao_e_rotulo_da_quantizacao_conhecida() {
        let mk = |q: &str| {
            Modelo::do_json(
                &analisar(&format!(r#"{{"id":"x","quantizacao":"{q}"}}"#)).unwrap(),
                &[],
            )
            .unwrap()
        };
        assert_eq!(mk("Q4_K_M").precisao(), Some("média-baixa"));
        assert_eq!(mk("Q8_0").precisao(), Some("muito alta"));
        assert_eq!(mk("xpto").precisao(), None);
    }

    #[test]
    fn le_catalogo_em_lista_ou_objeto() {
        let a = da_estrutura(&analisar(r#"[{"id":"a"},{"id":"b"}]"#).unwrap(), &[]);
        let b = da_estrutura(&analisar(r#"{"modelos":[{"id":"a"}]}"#).unwrap(), &[]);
        assert_eq!(a.len(), 2);
        assert_eq!(b.len(), 1);
    }
}
