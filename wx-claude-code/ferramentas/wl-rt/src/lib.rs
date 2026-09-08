//! wl-rt: o runtime minimo que o codigo Rust convertido do WLanguage precisa para
//! reproduzir a semantica do legado -- e nao a do f64, do `to_uppercase` ou do `==`.
//!
//! Cada funcao cita a pagina do Help WLanguage (corpus 12k, `identificador`) de onde
//! a semantica foi lida, e cada teste usa os EXEMPLOS dessa pagina como vetor. O que
//! o Help nao diz esta escrito como limite, nao adivinhado.
//!
//! Achados do exemplo ESTOQUE que motivaram isto: `currency` e ponto fixo (17 + 6
//! digitos, pagina 1514043), `Round` e metade para longe do zero (3050063), e `=`
//! entre strings e ESTRITO -- `"Dupond" = "DUPOND"` e falso; quem ignora caixa,
//! acento e espacos das pontas e `~=` (1512006).
pub mod data;
pub mod moeda;
pub mod numero;
pub mod texto;

pub use data::{data_hoje, data_mais_dias, data_valida, diferenca_de_datas};
pub use moeda::Moeda;
pub use numero::{num_para_texto, parte_inteira, round};
pub use texto::{
    comeca_com, igual, igual_flexivel, igual_muito_flexivel, left, length, middle, no_space,
    repeat_string, replace, right, truncate, upper, val,
};
