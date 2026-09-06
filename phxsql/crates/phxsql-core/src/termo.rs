//! Como um texto vira TERMOS, para o indice de texto (`.fts`).
//!
//! O desenho esta em `docs/FTS.md`. Aqui mora so a peca que todo desenho de
//! indice precisa, e que nao depende de nenhuma decisao ainda aberta: partir o
//! texto em palavras e **dobrar** cada uma.
//!
//! # A dobra nao e enfeite, e o medidor provou
//!
//! `custo-da-busca-de-palavra` mediu: a busca de hoje **nao** dobra acento --
//! procurar `fenix` acha **0** das linhas que tem so `fenix` com circunflexo.
//! Entao um indice sem dobra acharia MENOS que a varredura de hoje em qualquer
//! palavra acentuada, e indice que acha menos que a varredura e pior que nao
//! ter indice. Por isso a dobra nasce ligada (`FTS.md` §5.1).
//!
//! # A tabela de dobra tem um dono so
//!
//! [`crate::paginacao::sem_acento`], escrita a mao para a particao
//! alfanumerica. **Nao se copia**: duas copias divergiriam, e a divergencia
//! apareceria como o balde do `.pag` e o termo do `.fts` discordando sobre a
//! mesma palavra.

use crate::paginacao::sem_acento;

/// Dobra uma palavra: sem acento e em minuscula.
///
/// Minuscula tambem, e pelo mesmo motivo da dobra: quem digita `Fenix` na
/// ficha e `fenix` na busca espera achar. O `.pag` ja tinha tomado esta
/// decisao para o balde, e o indice a repete de proposito.
pub fn dobrar(palavra: &str) -> String {
    palavra
        .chars()
        .map(sem_acento)
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Quebra um texto em termos dobrados, SEM repetir.
///
/// # O que conta como separador
///
/// Tudo o que nao e letra nem digito. Isso inclui o hifen, e a escolha e
/// consciente: `nota-fiscal` vira dois termos, `nota` e `fiscal`, e quem
/// procurar qualquer um dos dois acha a linha. Manter o hifen faria
/// `nota-fiscal` so ser achado por quem digitasse o hifen no mesmo lugar.
///
/// # Por que SEM repetir
///
/// Uma palavra que aparece cinco vezes na mesma linha vira **uma** chave no
/// indice, e nao cinco. A chave e `(termo, rowid)`, entao cinco copias seriam
/// cinco chaves identicas -- e a arvore ja recusa duplicata em indice unico e
/// desperdicaria espaco num livre. O preco esta escrito no `FTS.md` §5: sem a
/// contagem, nao ha como ordenar por relevancia.
///
/// A ordem de saida e a de PRIMEIRA aparicao, e nao a alfabetica. Quem precisa
/// delas ordenadas ordena -- e quem grava no `.ndx` ja ordena o lote, que e a
/// licao do pedido 113.
pub fn termos(texto: &str) -> Vec<String> {
    quebrar(texto, dobrar)
}

/// A mesma quebra, mas SEM dobrar -- para o indice que pediu `"dobrar": false`.
///
/// Ela existe porque a dobra e uma escolha por indice, e o par de funcoes tem
/// de partir o texto exatamente igual: se uma quebrasse no hifen e a outra
/// nao, dois indices da mesma tabela achariam conjuntos diferentes de linhas
/// para o mesmo texto, e o motivo nao apareceria em lugar nenhum. Por isso as
/// duas chamam a MESMA quebra, e so trocam o que fazem com cada pedaco.
pub fn termos_sem_dobrar(texto: &str) -> Vec<String> {
    quebrar(texto, |p| p.to_string())
}

/// A quebra, com o que fazer com cada palavra passado de fora.
///
/// Um so lugar decide o que e separador. Duas copias divergiriam no dia em que
/// alguem acrescentasse um caractere a uma delas.
fn quebrar(texto: &str, como: impl Fn(&str) -> String) -> Vec<String> {
    let mut fora: Vec<String> = Vec::new();
    for bruta in texto.split(|c: char| !c.is_alphanumeric()) {
        if bruta.is_empty() {
            continue;
        }
        let t = como(bruta);
        if t.is_empty() || fora.contains(&t) {
            continue;
        }
        fora.push(t);
    }
    fora
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn dobra_tira_acento_e_baixa_a_caixa() {
        assert_eq!(dobrar("Fênix"), "fenix");
        assert_eq!(dobrar("ÁLVARO"), "alvaro");
        assert_eq!(dobrar("coração"), "coracao");
    }

    /// A PROVA REAL da dobra, nos dois sentidos.
    ///
    /// Com a dobra reposta -- isto e, comparando as palavras cruas -- as duas
    /// grafias sao textos DIFERENTES e o indice nao as juntaria. E exatamente
    /// o que o `custo-da-busca-de-palavra` mediu na varredura de hoje: 0 de
    /// 200. Este teste falha se alguem tirar a dobra do caminho.
    #[test]
    fn as_duas_grafias_caem_no_mesmo_termo() {
        assert_ne!("fênix", "fenix", "as grafias CRUAS sao diferentes");
        assert_eq!(
            dobrar("fênix"),
            dobrar("fenix"),
            "dobradas, tem de ser o mesmo termo -- senao o indice acha menos \
             que a varredura de hoje, e ai e pior que nao ter indice"
        );
    }

    #[test]
    fn quebra_por_tudo_que_nao_e_letra_nem_digito() {
        assert_eq!(
            termos("nota-fiscal 123, e/ou outra."),
            vec!["nota", "fiscal", "123", "e", "ou", "outra"]
        );
    }

    #[test]
    fn palavra_repetida_vira_um_termo_so() {
        assert_eq!(termos("pedido pedido PEDIDO Pédido"), vec!["pedido"]);
    }

    #[test]
    fn texto_vazio_e_texto_so_de_pontuacao_nao_dao_termo() {
        assert!(termos("").is_empty());
        assert!(termos("  ...,;  ").is_empty());
    }

    /// A ordem e a de primeira aparicao, e o teste trava isso porque alguem
    /// que "melhorasse" ordenando aqui mudaria o que o gravador recebe.
    #[test]
    fn a_ordem_e_a_de_primeira_aparicao() {
        assert_eq!(
            termos("zebra alfa zebra beta"),
            vec!["zebra", "alfa", "beta"]
        );
    }

    /// As duas funcoes quebram IGUAL, e so o que fazem com o pedaco muda.
    ///
    /// Se uma quebrasse no hifen e a outra nao, dois indices da mesma tabela
    /// achariam conjuntos diferentes para o mesmo texto -- e sem motivo
    /// visivel em lugar nenhum.
    #[test]
    fn dobrando_ou_nao_a_quebra_e_a_mesma() {
        let t = "nota-fiscal 123, Fênix/ou outra.";
        assert_eq!(termos(t).len(), termos_sem_dobrar(t).len());
        assert_eq!(
            termos_sem_dobrar(t)[3],
            "Fênix",
            "sem dobrar mantem a grafia"
        );
        assert_eq!(termos(t)[3], "fenix", "dobrando, vira o termo do indice");
    }

    /// O que a tabela de dobra NAO cobre continua passando inteiro, e isso e
    /// decisao: cair fora da tabela deixa a palavra buscavel como ela e, em
    /// vez de virar um termo mutilado que ninguem consegue digitar.
    #[test]
    fn o_que_a_tabela_nao_cobre_atravessa_sem_mutilar() {
        assert_eq!(dobrar("Ярославль"), "ярославль");
        assert_eq!(termos("東京 tokyo"), vec!["東京", "tokyo"]);
    }
}
