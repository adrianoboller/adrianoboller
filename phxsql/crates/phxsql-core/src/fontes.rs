//! As fontes da marca, EMBUTIDAS no binario: Exo 2 (a voz da marca) e IBM
//! Plex Mono (o dado). Licenca SIL OFL 1.1, em `marca/fontes/`.
//!
//! # Por que embutir
//!
//! Ate 24/09/2026 as duas vinham do Google Fonts. Servidor de banco sem
//! internet e o caso NORMAL, e ali a marca sumia: a pilha de reserva assumia
//! e a tela saia em Arial. Embutir e a regra de instalacao simples aplicada a
//! tela -- um binario que abre igual com e sem rede, e sem pedir nada a
//! terceiro (nem a fonte, nem o registro do IP de quem abriu a tela).
//!
//! Custo medido: 86.112 bytes de woff2 (so o subconjunto latino, que cobre os
//! seis idiomas da fabrica), 114.816 em base64 dentro da pagina.
//!
//! # Um motor so
//!
//! O PhxSql e o PhxZip web chamam [`css_das_fontes`]; nenhum dos dois monta o
//! seu `@font-face`. Fonte nova entra aqui e aparece nos dois.

use std::sync::OnceLock;

const EXO2: &[u8] = include_bytes!("../../../marca/fontes/exo2-latin-var.woff2");
const MONO_400: &[u8] = include_bytes!("../../../marca/fontes/ibmplexmono-latin-400.woff2");
const MONO_500: &[u8] = include_bytes!("../../../marca/fontes/ibmplexmono-latin-500.woff2");
const MONO_600: &[u8] = include_bytes!("../../../marca/fontes/ibmplexmono-latin-600.woff2");

/// O mesmo `unicode-range` latino que o Google Fonts serve para as duas.
const LATINO: &str = "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, \
U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD";

fn face(familia: &str, peso: &str, dados: &[u8]) -> String {
    format!(
        "@font-face{{font-family:'{familia}';font-style:normal;font-weight:{peso};\
         font-display:swap;src:url(data:font/woff2;base64,{}) format('woff2');\
         unicode-range:{LATINO}}}\n",
        crate::base64::codificar(dados)
    )
}

/// O CSS com os quatro `@font-face` em `data:` -- calculado uma vez por
/// processo. Quem serve a pagina precisa de `font-src data:` na CSP.
pub fn css_das_fontes() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let mut s = String::new();
        s.push_str(&face("Exo 2", "400 700", EXO2));
        s.push_str(&face("IBM Plex Mono", "400", MONO_400));
        s.push_str(&face("IBM Plex Mono", "500", MONO_500));
        s.push_str(&face("IBM Plex Mono", "600", MONO_600));
        s
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_fontes_sao_woff2_de_verdade_e_estao_no_css() {
        for f in [EXO2, MONO_400, MONO_500, MONO_600] {
            assert_eq!(&f[..4], b"wOF2", "nao e woff2");
        }
        let css = css_das_fontes();
        assert_eq!(css.matches("@font-face").count(), 4);
        assert!(css.contains("font-family:'Exo 2'"));
        assert!(!css.contains("http"), "fonte embutida nao busca nada fora");
    }
}
