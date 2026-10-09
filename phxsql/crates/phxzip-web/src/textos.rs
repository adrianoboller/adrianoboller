//! Os textos da tela do PhxZip, resolvidos pela maquina de idiomas do core
//! (contrato §5, fatia Z6).
//!
//! # Uma tabela, uma maquina
//!
//! A TABELA e o `ui/textos.json` -- um arquivo so, que o tradutor edita e que
//! o roteiro do navegador tambem le. A MAQUINA e a do `phxsql_core::idiomas`:
//! a lista [`IDIOMAS`], os degraus da queda (`resolver`), a conferencia da
//! tabela (`defeitos_da_tabela`) e o laco (`conferir_laco`). Daqui sai so o
//! que e deste produto: ler o JSON para o tipo do core.
//!
//! O JSON vira [`TextoDeFabrica`] UMA vez, no primeiro uso, e as frases ficam
//! na memoria do processo ate ele acabar -- e o tipo que a maquina recebe
//! (`&'static str`), e e o mesmo tempo de vida do `include_str!` de onde
//! vieram. Sao 173 chaves × 6 idiomas: tamanho fixo, conhecido no build.

use std::collections::HashSet;
use std::sync::OnceLock;

use phxsql_core::idiomas::{
    self, chaves_no_fonte, conferir_laco, defeitos_da_tabela, Laco, TextoDeFabrica, IDIOMAS,
    QUANTOS,
};
use phxsql_core::json::Json;

/// O dicionario, embutido. Nao e servido cru (contrato §5).
pub const FONTE: &str = include_str!("../ui/textos.json");

/// Os dois fontes da tela que pedem chave. O laco os varre juntos.
pub const TELA: [&str; 2] = [
    include_str!("../ui/index.html"),
    include_str!("../ui/phxzip.js"),
];

/// O prefixo de toda chave desta tela.
pub const PREFIXO: &str = "zip.";

fn eterno(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Le o dicionario para a tabela da maquina. Recusa coluna que nao esta em
/// [`IDIOMAS`] -- chaveado pelo NOME da coluna, um idioma escrito errado
/// sumiria calado, caindo no portugues pelo degrau 2 --, chave sem portugues
/// e qualquer defeito que o `defeitos_da_tabela` do core acuse.
pub fn carregar(fonte: &str) -> Result<Vec<TextoDeFabrica>, String> {
    let raiz = Json::analisar(fonte).map_err(|e| format!("textos.json: {e}"))?;
    let Some(Json::Objeto(pares)) = raiz.campo("textos") else {
        return Err("textos.json: falta o objeto \"textos\"".into());
    };
    let mut tabela = Vec::with_capacity(pares.len());
    for (chave, celulas) in pares {
        let Json::Objeto(cels) = celulas else {
            return Err(format!("textos.json: {chave} nao e objeto"));
        };
        let mut textos: [&'static str; QUANTOS] = [""; QUANTOS];
        for (coluna, valor) in cels {
            let Some(i) = IDIOMAS.iter().position(|c| c == coluna) else {
                return Err(format!(
                    "textos.json: {chave} traz a coluna {coluna:?}, que nao esta em IDIOMAS"
                ));
            };
            let Some(t) = valor.texto() else {
                return Err(format!("textos.json: {chave}/{coluna} nao e texto"));
            };
            textos[i] = eterno(t);
        }
        tabela.push(TextoDeFabrica {
            nome: eterno(chave),
            textos,
        });
    }
    let defeitos = defeitos_da_tabela(&tabela, PREFIXO);
    if !defeitos.is_empty() {
        return Err(format!("textos.json: {}", defeitos.join("; ")));
    }
    Ok(tabela)
}

static TABELA: OnceLock<Result<Vec<TextoDeFabrica>, String>> = OnceLock::new();

/// A tabela embutida. O `Servidor::escutar` a pede antes de abrir a porta, e
/// a porta nao sobe com ela torta.
pub fn tabela() -> Result<&'static [TextoDeFabrica], &'static str> {
    match TABELA.get_or_init(|| carregar(FONTE)) {
        Ok(t) => Ok(t.as_slice()),
        Err(e) => Err(e.as_str()),
    }
}

/// O corpo do `GET /api/idiomas`: o idioma resolvido (desconhecido cai no
/// portugues, degrau do core) e cada texto JA resolvido nele. A pagina nunca
/// recebe as seis colunas.
pub fn resposta(tabela: &[TextoDeFabrica], pedido: &str) -> Json {
    let idioma = idiomas::indice_do_idioma(pedido);
    let textos = tabela
        .iter()
        .map(|f| {
            (
                f.nome.to_string(),
                Json::texto_de(idiomas::resolver(None, f, idioma)),
            )
        })
        .collect();
    Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("idioma", Json::texto_de(IDIOMAS[idioma])),
        (
            "idiomas",
            Json::Lista(IDIOMAS.iter().map(|i| Json::texto_de(*i)).collect()),
        ),
        ("textos", Json::Objeto(textos)),
    ])
}

/// As chaves que a tela pede, pelo varredor do core.
pub fn pedidas_pela_tela(fontes: &[&str]) -> HashSet<String> {
    let mut todas = HashSet::new();
    for f in fontes {
        todas.extend(chaves_no_fonte(f, PREFIXO));
    }
    todas
}

/// O laco entre a tela embutida e uma tabela.
pub fn laco(tabela: &'static [TextoDeFabrica]) -> Laco {
    conferir_laco(tabela, &pedidas_pela_tela(&TELA))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O laco da tela (contrato §5.5): toda chave que a tela pede existe no
    /// dicionario, e toda chave do dicionario alguem pede.
    #[test]
    fn o_laco_da_tela_fecha() {
        let t = tabela().expect("o textos.json embutido carrega");
        let l = laco(t);
        assert!(
            l.faltando.is_empty(),
            "a tela pede e falta: {:?}",
            l.faltando
        );
        assert!(l.mortas.is_empty(), "chave morta: {:?}", l.mortas);
        assert!(
            t.len() > 150,
            "{} chaves -- a varredura achou a tela?",
            t.len()
        );
    }

    /// RED do laco: tirar do dicionario UMA chave que a tela pede faz o laco
    /// acusar -- senao o teste de cima passaria com qualquer dicionario.
    #[test]
    fn chave_tirada_do_dicionario_faz_o_laco_cair() {
        let t = tabela().unwrap();
        let tirada = "zip.compactar_e_baixar";
        assert!(t.iter().any(|f| f.nome == tirada));
        let menor: Vec<TextoDeFabrica> = t
            .iter()
            .filter(|f| f.nome != tirada)
            .map(|f| TextoDeFabrica {
                nome: f.nome,
                textos: f.textos,
            })
            .collect();
        let menor: &'static [TextoDeFabrica] = Box::leak(menor.into_boxed_slice());
        let l = laco(menor);
        assert_eq!(l.faltando.iter().collect::<Vec<_>>(), [tirada]);
        // E o sentido contrario: chave a mais no dicionario e chave morta.
        let mut maior: Vec<TextoDeFabrica> = t
            .iter()
            .map(|f| TextoDeFabrica {
                nome: f.nome,
                textos: f.textos,
            })
            .collect();
        maior.push(TextoDeFabrica {
            nome: "zip.ninguem_pede",
            textos: ["x"; QUANTOS],
        });
        let maior: &'static [TextoDeFabrica] = Box::leak(maior.into_boxed_slice());
        assert_eq!(
            laco(maior).mortas.iter().collect::<Vec<_>>(),
            [&"zip.ninguem_pede"]
        );
    }

    #[test]
    fn coluna_fora_de_idiomas_recusa_o_dicionario() {
        let ruim = r#"{"textos":{"zip.a":{"Portugues":"a","Klingon":"b"}}}"#;
        assert!(carregar(ruim).err().unwrap().contains("Klingon"));
        let sem_pt = r#"{"textos":{"zip.a":{"Ingles":"a"}}}"#;
        assert!(carregar(sem_pt).is_err());
        let bom = r#"{"textos":{"zip.a":{"Portugues":"a","Ingles":"b"}}}"#;
        assert_eq!(carregar(bom).unwrap().len(), 1);
    }

    #[test]
    fn idioma_desconhecido_cai_no_portugues_e_o_pedido_sai_resolvido() {
        let t = tabela().unwrap();
        let de = resposta(t, "Alemao");
        assert_eq!(de.campo("idioma").and_then(Json::texto), Some("Alemao"));
        let txt = de.campo("textos").unwrap();
        assert_eq!(
            txt.campo("zip.aba_compactar").and_then(Json::texto),
            Some("Komprimieren")
        );
        let k = resposta(t, "Klingon");
        assert_eq!(k.campo("idioma").and_then(Json::texto), Some("Portugues"));
        assert_eq!(
            k.campo("textos")
                .unwrap()
                .campo("zip.aba_compactar")
                .and_then(Json::texto),
            Some("Compactar")
        );
        assert_eq!(
            de.campo("idiomas").and_then(Json::lista).map(|l| l.len()),
            Some(QUANTOS)
        );
    }
}
