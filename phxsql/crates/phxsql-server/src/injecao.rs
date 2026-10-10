//! O gancho do observador de injecao (pedido 495, fatia F3).
//!
//! Desenho em `docs/propostas/ia-495-496-desenho.md` (linha F3). As quatro
//! classes sao as do `phxsql_sql::sinais` (F1); a ocorrencia sai pelo
//! produtor unico `telemetria::sinal_com_sinais` (F2). O que este arquivo
//! decide, e por que:
//!
//! * **Quem analisa entrega; o gancho so recolhe.** A op `sql` e os tres
//!   campos de expressao (`expressao_do_pedido`, `tendo_do_pedido` e o
//!   `expressao` do `op_consultar`) ja leem o texto. Eles entregam aqui as
//!   classes dos MESMOS simbolos que leram, e o gancho em
//!   `executar_e_contar_escrita_local` as recolhe no fim. Ler de novo no
//!   gancho seria uma passada do lexico a mais por pedido -- e o teste
//!   `o_observador_nao_acrescenta_passada_do_lexico` reprova isso.
//! * **No sucesso E no erro.** O gancho recolhe depois do `executar`, seja
//!   qual for o desfecho. O 215 so olhava o `is_err()`, e a tautologia, que
//!   da CERTO e devolve as linhas, sumia (parecer SEC do 495, §2, item 4).
//! * **Uma ocorrencia por pedido de fora.** O mesmo gancho roda para cada
//!   passo derivado, entao conta a profundidade: so o primeiro nivel abre e
//!   recolhe. O passo derivado ENTREGA para ele -- o `de` de um `consultar`
//!   e o lado B de um `juntar` sao texto do cliente, so que um nivel abaixo.
//! * **A op `sql` fecha a entrega depois da dela.** O `varrer` que ela
//!   traduz traz o mesmo texto ja normalizado (seria acusar duas vezes), e o
//!   corpo de uma rotina chama a op `sql` por dentro (seria acusar o codigo
//!   do dono como se fosse o pedido).
//!
//! # O preco
//!
//! O portao e o `bool` `seguranca.observar_injecao_sql` -- ou o
//! `protecao.bloquear_por_codigo` do 766 (P8), que conta as mesmas classes
//! para bloquear e por isso abre a mesma vez --, lido ANTES de tudo
//! no gancho: desligado, nada daqui roda e a profundidade fica em zero, entao
//! a op `sql` e os campos de expressao tambem nao classificam nada (uma
//! leitura de `thread_local` cada). Ligado, o pedido comum paga a
//! classificacao sobre simbolos prontos -- medida no exemplo
//! `custo-do-observador` do `phxsql-sql` --, e so o pedido acusado monta
//! o JSON da ocorrencia.

use std::cell::Cell;

use phxsql_core::expressao::{Expressao, Peca};
use phxsql_core::Result;
use phxsql_sql::{Comparador, Simbolo, Sinais, Token};

#[derive(Clone, Copy)]
struct Estado {
    /// Quantos `executar_e_contar_escrita_local` estao abertos nesta thread.
    profundidade: u32,
    /// O pedido de fora ainda aceita entrega (a op `sql` fecha depois da
    /// dela).
    aberto: bool,
    visto: Sinais,
}

thread_local! {
    static ESTADO: Cell<Estado> = const {
        Cell::new(Estado {
            profundidade: 0,
            aberto: false,
            visto: Sinais::NENHUM,
        })
    };
}

fn mudar(f: impl FnOnce(&mut Estado)) {
    ESTADO.with(|c| {
        let mut e = c.get();
        f(&mut e);
        c.set(e);
    });
}

/// A vez de um pedido no gancho. Desce a profundidade no `Drop` -- tambem
/// num panico do `executar`, senao a thread da conexao ficaria contando um
/// nivel a mais para sempre e nunca mais observaria nada.
pub(crate) struct Vez {
    de_fora: bool,
}

/// Abre a vez de um pedido. O de fora (profundidade 1) comeca limpo.
pub(crate) fn abrir() -> Vez {
    let mut de_fora = false;
    mudar(|e| {
        e.profundidade += 1;
        if e.profundidade == 1 {
            de_fora = true;
            e.aberto = true;
            e.visto = Sinais::NENHUM;
        }
    });
    Vez { de_fora }
}

impl Vez {
    /// As classes que o pedido de fora e os passos dele entregaram. Vazio
    /// para o passo derivado: quem recolhe e o primeiro nivel.
    pub(crate) fn fechar(self) -> Sinais {
        if !self.de_fora {
            return Sinais::NENHUM;
        }
        let mut visto = Sinais::NENHUM;
        mudar(|e| {
            visto = e.visto;
            e.visto = Sinais::NENHUM;
            e.aberto = false;
        });
        visto
    }
}

impl Drop for Vez {
    fn drop(&mut self) {
        mudar(|e| e.profundidade = e.profundidade.saturating_sub(1));
    }
}

/// O pedido de fora quer as classes? E o portao de quem analisa: falso com o
/// observador desligado (ninguem abriu) ou depois que a op `sql` ja
/// entregou.
pub(crate) fn observando() -> bool {
    ESTADO.with(|c| {
        let e = c.get();
        e.profundidade >= 1 && e.aberto
    })
}

/// Soma as classes ao pedido de fora. Nao faz nada quando ninguem observa.
pub(crate) fn entregar(s: Sinais) {
    if s.vazio() {
        return;
    }
    mudar(|e| {
        if e.profundidade >= 1 && e.aberto {
            e.visto = e.visto.com(s);
        }
    });
}

/// A op `sql` ja entregou: o que roda dentro dela (o corpo de uma rotina, que
/// chama a op `sql` de novo sem passar pelo gancho) nao e o pedido de fora.
pub(crate) fn encerrar_entrega() {
    mudar(|e| e.aberto = false);
}

/// `Expressao::analisar`, entregando as classes dos simbolos que a propria
/// analise produziu. E a porta dos tres campos de expressao.
pub(crate) fn analisar_expressao(texto: &str) -> Result<Expressao> {
    if !observando() {
        return Expressao::analisar(texto);
    }
    Expressao::analisar_vendo(
        texto,
        Some(&mut |pecas| entregar(phxsql_sql::sinais(&simbolos_das_pecas(pecas)))),
    )
}

/// Os simbolos da expressao no vocabulario do `phxsql_sql`, para as quatro
/// classes lerem. O literal entra VAZIO: as classes perguntam «e literal?»,
/// nunca «qual?», e assim nenhum valor do pedido e copiado aqui.
fn simbolos_das_pecas(pecas: &mut dyn Iterator<Item = Peca<'_>>) -> Vec<Simbolo> {
    pecas
        .filter_map(|p| {
            let token = match p {
                Peca::Numero => Token::Numero(String::new()),
                Peca::Texto => Token::Texto(String::new()),
                Peca::Palavra(t) => Token::Palavra {
                    texto: t.to_string(),
                    citado: false,
                },
                Peca::Op(o) => token_do_operador(o)?,
                Peca::Abre => Token::AbreParen,
                Peca::Fecha => Token::FechaParen,
                Peca::Virgula => Token::Virgula,
            };
            Some(Simbolo { token, posicao: 0 })
        })
        .collect()
}

fn token_do_operador(o: &str) -> Option<Token> {
    Some(match o {
        "=" => Token::Comparador(Comparador::Igual),
        "<>" => Token::Comparador(Comparador::Diferente),
        "<" => Token::Comparador(Comparador::Menor),
        "<=" => Token::Comparador(Comparador::MenorIgual),
        ">" => Token::Comparador(Comparador::Maior),
        ">=" => Token::Comparador(Comparador::MaiorIgual),
        "+" => Token::Mais,
        "-" => Token::Menos,
        "*" => Token::Asterisco,
        "/" => Token::Barra,
        // Operador que o lexico da expressao aprender depois e que este mapa
        // nao conhece: fica fora, e as classes leem o resto. Nenhuma das
        // quatro depende de operador aritmetico.
        _ => return None,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Cada operador que o lexico da expressao produz tem token aqui -- o
    /// `_` do mapa nao pode engolir um comparador, senao `OR 1 = 1` deixa
    /// de ser constante comparada a constante.
    #[test]
    fn todo_operador_da_expressao_tem_token() {
        let mut vistos = Vec::new();
        Expressao::analisar_vendo(
            "a = 1 OR b <> 2 OR c != 3 OR d < 4 OR e <= 5 OR f > 6 OR g >= 7 OR h + 1 - 2 * 3 / 4 = 0",
            Some(&mut |p| {
                for x in p {
                    if let Peca::Op(o) = x {
                        vistos.push(o);
                    }
                }
            }),
        )
        .unwrap();
        assert!(vistos.len() >= 11, "{vistos:?}");
        for o in vistos {
            assert!(token_do_operador(o).is_some(), "{o}");
        }
    }

    /// A profundidade: o passo derivado entrega ao pedido de fora e nao
    /// recolhe; so o primeiro nivel recolhe; a vez desce no `Drop`; e depois
    /// do `encerrar_entrega` nada mais entra.
    #[test]
    fn so_o_pedido_de_fora_recolhe() {
        assert!(!observando(), "ninguem abriu");
        entregar(Sinais::EMPILHADO);
        let fora = abrir();
        assert!(observando());
        assert!(visto_vazio(), "o que veio antes de abrir nao conta");
        {
            let dentro = abrir();
            entregar(Sinais::CONSTANTE_SOB_OR);
            assert!(dentro.fechar().vazio(), "o derivado nao recolhe");
        }
        assert!(observando(), "o Drop desceu a profundidade");
        encerrar_entrega();
        entregar(Sinais::UNIAO_DE_SONDAGEM);
        assert_eq!(fora.fechar(), Sinais::CONSTANTE_SOB_OR);
        assert!(!observando());
    }

    fn visto_vazio() -> bool {
        ESTADO.with(|c| c.get().visto.vazio())
    }
}
