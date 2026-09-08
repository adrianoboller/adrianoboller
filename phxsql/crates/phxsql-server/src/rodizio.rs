//! O rodizio de arquivo por tamanho -- pedido 228.
//!
//! O Profiler ja tinha resolvido este problema (ver `profiler.rs`, secao
//! "Anel em memoria, rodizio em disco"): um arquivo de log sem teto cresce
//! ate encher a particao -- o `perfil.txt` media 345 bytes por pedido e
//! nunca parava, 1,2 GB por hora a mil pedidos por segundo. `diretivas.log`
//! e `acessos.log` tinham o MESMO problema, sem o mesmo remedio.
//!
//! Este modulo isola a parte da logica do Profiler que NAO depende do que
//! esta sendo escrito -- decidir SE a proxima linha estoura o teto, e COMO
//! trocar `arquivo` por `arquivo.1`, `.1` por `.2`... -- para o Profiler
//! (`profiler::girar`/`girar_se_encheu`), o `LogAcessos` (`acesso.rs`) e o
//! `Diario` (`diretivas.rs`) chamarem a MESMA funcao em vez de cada um
//! reescrever a sua. `profiler::com_sufixo` continua sendo o unico lugar que
//! sabe formar o nome do arquivo antigo -- este modulo o reaproveita, nao o
//! duplica.
//!
//! O que cada dono faz DEPOIS de girar continua proprio dele: o Profiler
//! escreve um cabecalho de continuacao no arquivo, porque e ferramenta de
//! diagnostico lida por gente. `diretivas.log` e `acessos.log` sao JSON
//! Lines puro, e uma linha de texto solta so atrapalharia quem le com
//! `Json::analisar` -- por isso eles NAO ganham cabecalho nenhum aqui.

use std::fs::{File, OpenOptions};
use std::path::Path;

// Re-exportado (so dentro do crate, como o proprio `com_sufixo`), e nao so
// importado: `LogAcessos` e `Diario` precisam do mesmo nome de arquivo
// antigo que o Profiler usa, para os testes deles conferirem o `.1`/`.2` sem
// duplicar a formula.
pub(crate) use crate::profiler::com_sufixo;

/// A proxima linha (de `proxima` bytes) precisa de um arquivo novo?
///
/// Mesma formula do `profiler::girar_se_encheu`, reaproveitada e nao
/// reescrita: teto zero nunca gira -- e o comportamento de quem nao
/// configurou nada, e "guarda nova entra pedida" exige que ele continue
/// sendo o padrao. Arquivo vazio (`bytes_no_arquivo == 0`) tambem nunca gira:
/// uma linha maior que o teto inteiro giraria a cada linha, apagando o
/// historico so para gravar uma linha que continuaria nao cabendo.
pub fn deve_girar(bytes_no_arquivo: u64, proxima: u64, teto_do_arquivo: u64) -> bool {
    teto_do_arquivo != 0 && bytes_no_arquivo != 0 && bytes_no_arquivo + proxima > teto_do_arquivo
}

/// Troca `caminho` por `.1`, `.1` por `.2`... com o mais velho saindo
/// primeiro, e reabre `caminho` em append. A MESMA danca do
/// `profiler::girar`, chamada em vez de reescrita.
///
/// # A ordem, e o que acontece quando falha
///
/// Quem chama tem de ter SOLTO o proprio descritor do arquivo ANTES de
/// chamar isto: no Unix renomear um arquivo aberto funciona e as linhas
/// seguintes iriam para o arquivo antigo pelo nome novo; no Windows a
/// renomeacao falha. Soltar primeiro faz os dois se comportarem igual.
///
/// Devolve `(novo_arquivo, deu_errado)`. `novo_arquivo` e `None` so quando
/// reabrir falhou -- quem chama passa a contar a proxima escrita como falha,
/// em vez de escrever num arquivo que nao existe. `deu_errado` conta mesmo
/// quando reabrir deu certo, porque um passo do meio (renomear `.3` para
/// `.4`) pode falhar sem impedir o resto.
pub fn girar(caminho: &Path, manter: usize) -> (Option<File>, bool) {
    let mut deu_errado = false;
    // O mais velho sai primeiro. Sem isto, o `.1 -> .2` de baixo
    // sobrescreveria o `.2` que ainda deveria existir.
    if manter == 0 {
        let _ = std::fs::remove_file(caminho);
    } else {
        let ultimo = com_sufixo(caminho, manter);
        let _ = std::fs::remove_file(&ultimo);
        for n in (1..manter).rev() {
            let de = com_sufixo(caminho, n);
            if de.exists() && std::fs::rename(&de, com_sufixo(caminho, n + 1)).is_err() {
                deu_errado = true;
            }
        }
        if std::fs::rename(caminho, com_sufixo(caminho, 1)).is_err() {
            deu_errado = true;
        }
    }
    let novo = OpenOptions::new()
        .create(true)
        .append(true)
        .open(caminho)
        .ok();
    if novo.is_none() {
        deu_errado = true;
    }
    (novo, deu_errado)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;
    use std::io::Write;

    /// Prova real do defeito que o pedido 228 fecha: sem rodizio (teto e
    /// zero, o padrao de quem nao configurou nada), o arquivo cresce para
    /// sempre -- `deve_girar` nunca manda girar.
    #[test]
    fn teto_zero_nunca_manda_girar() {
        assert!(!deve_girar(1_000_000, 100, 0), "teto 0 tem de ser 'nunca'");
    }

    /// Arquivo vazio nunca gira, mesmo com uma linha maior que o teto --
    /// senao o rodizio giraria a cada linha, apagando o historico para
    /// gravar uma linha que continuaria nao cabendo.
    #[test]
    fn arquivo_vazio_nunca_gira() {
        assert!(!deve_girar(0, 1_000, 100));
    }

    #[test]
    fn manda_girar_quando_a_proxima_linha_estoura_o_teto() {
        assert!(deve_girar(90, 20, 100));
        assert!(!deve_girar(50, 20, 100), "ainda cabe, nao deveria girar");
    }

    #[test]
    fn girar_troca_o_arquivo_e_guarda_o_antigo_com_sufixo() {
        let d = DirTemp::novo("rodizio-girar");
        let alvo = d.join("teste.log");
        std::fs::write(&alvo, b"linha velha\n").unwrap();

        let (novo, deu_errado) = girar(&alvo, 2);
        assert!(!deu_errado);
        let mut f = novo.expect("tinha de reabrir o arquivo");
        writeln!(f, "linha nova").unwrap();
        f.flush().unwrap();

        assert!(com_sufixo(&alvo, 1).exists(), "o antigo vira .1");
        let velho = std::fs::read_to_string(com_sufixo(&alvo, 1)).unwrap();
        assert!(velho.contains("linha velha"));
        let atual = std::fs::read_to_string(&alvo).unwrap();
        assert!(
            atual.contains("linha nova") && !atual.contains("linha velha"),
            "o arquivo corrente comeca vazio: {atual}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn girar_descarta_o_mais_velho_quando_estoura_manter() {
        let d = DirTemp::novo("rodizio-descarta");
        let alvo = d.join("teste.log");
        std::fs::write(&alvo, b"a\n").unwrap();
        let (f, _) = girar(&alvo, 1); // teste.log -> .1
        drop(f);
        std::fs::write(&alvo, b"b\n").unwrap();
        let (f, _) = girar(&alvo, 1); // .1 -> descartado, teste.log -> .1
        drop(f);

        let restante = std::fs::read_to_string(com_sufixo(&alvo, 1)).unwrap();
        assert_eq!(restante.trim(), "b", "com manter=1 so o mais recente fica");
        assert!(!com_sufixo(&alvo, 2).exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
