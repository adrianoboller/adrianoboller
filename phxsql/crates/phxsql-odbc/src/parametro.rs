//! Os parametros de instrucao preparada: contar os `?` do texto.
//!
//! A contagem e uma funcao PURA, pelo mesmo motivo do `fatiar` do `texto.rs`:
//! ela decide quantos valores o aplicativo tem de ligar, e errar isso faz o
//! driver recusar uma execucao legitima (ou mandar ao servidor uma lista de
//! tamanho que ele recusa). Funcao pura se prova com tabela, sem soquete e
//! sem ponteiro.

/// Quantos `?` valem parametro em `sql`.
///
/// O criterio e o do LEXICO DO SERVIDOR (`crates/phxsql-sql/src/lexico.rs`), e
/// nao so o das aspas simples: la, quatro trechos engolem caractere sem que
/// ele vire simbolo -- `'texto'` (com `''` valendo uma aspa dentro),
/// `"identificador"` (com `""`), o comentario de linha `-- ate o fim` e o de
/// bloco `/* ... */`. Um `?` dentro de qualquer um dos quatro e dado ou
/// comentario, nao pergunta.
///
/// Por que a conta se refaz aqui em vez de chamar o lexico: o driver nao
/// depende do `phxsql-sql` (ele manda o texto INTEIRO para o servidor, que e
/// onde mora o parser), e o lexico de hoje nem sequer aceita `?` -- ele
/// recusa o caractere. Mas a duplicacao tem um preco nomeado: **contexto novo
/// que o lexico aprender a engolir tem de chegar aqui junto**, ou o driver
/// passa a contar um `?` que o servidor nao ve, e a recusa vai apontar para
/// uma pergunta que nunca existiu.
///
/// Citacao ou comentario aberto e nao fechado vale ate o fim do texto: o erro
/// de sintaxe e do servidor, e o que o driver precisa e nao contar o que esta
/// dentro.
pub fn contar_interrogacoes(sql: &str) -> usize {
    // Percorrer por BYTE e seguro aqui e nao e economia: os cinco
    // delimitadores sao ASCII, e byte ASCII nunca aparece no meio de um
    // caractere UTF-8 de varios bytes -- entao acento em literal ou em
    // comentario nao desalinha a varredura.
    let b = sql.as_bytes();
    let mut i = 0usize;
    let mut quantos = 0usize;
    while i < b.len() {
        match b[i] {
            b'\'' => i = fim_da_citacao(b, i, b'\''),
            b'"' => i = fim_da_citacao(b, i, b'"'),
            // Os comentarios vem antes dos demais porque `--` comeca com `-` e
            // `/*` com `/`; o `-` e o `/` sozinhos sao operadores e nao
            // engolem nada.
            b'-' if b.get(i + 1) == Some(&b'-') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            b'?' => {
                quantos += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    quantos
}

/// A posicao logo depois do trecho citado que comeca em `inicio`.
///
/// A aspa DOBRADA vale uma aspa dentro, como no lexico do servidor -- e e por
/// isso que a varredura nao pode ser um simples "alterna a cada aspa": em
/// `'a''?''b'` a aspa do meio nao fecha nada, e o `?` continua dentro do
/// literal.
fn fim_da_citacao(b: &[u8], inicio: usize, aspa: u8) -> usize {
    let mut i = inicio + 1;
    while i < b.len() {
        if b[i] == aspa {
            if b.get(i + 1) == Some(&aspa) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    b.len()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn conta_as_perguntas_de_fora() {
        assert_eq!(contar_interrogacoes("SELECT * FROM c"), 0);
        assert_eq!(contar_interrogacoes("SELECT * FROM c WHERE id = ?"), 1);
        assert_eq!(
            contar_interrogacoes("SELECT * FROM c WHERE id = ? AND nome = ?"),
            2
        );
    }

    // O caso que o pedido nomeia: `?` dentro de aspas simples e DADO. Contar
    // aqui faria o driver exigir uma ligacao para um parametro que nao
    // existe, e recusar um SELECT perfeitamente valido.
    #[test]
    fn pergunta_dentro_de_aspas_nao_conta() {
        assert_eq!(contar_interrogacoes("SELECT * FROM c WHERE nome = '?'"), 0);
        assert_eq!(
            contar_interrogacoes("SELECT * FROM c WHERE nome = 'e ai?' AND id = ?"),
            1
        );
        // A aspa dobrada nao fecha o literal: o `?` do meio continua dentro.
        assert_eq!(contar_interrogacoes("SELECT 'a''?''b' FROM c"), 0);
        assert_eq!(contar_interrogacoes("SELECT 'a''b' FROM c WHERE x = ?"), 1);
    }

    // Os tres irmaos do caso das aspas simples: o lexico do servidor engole os
    // quatro trechos, e quem contasse so um deles divergiria dele em silencio.
    #[test]
    fn identificador_citado_e_comentarios_tambem_engolem() {
        assert_eq!(contar_interrogacoes(r#"SELECT "co?una" FROM c"#), 0);
        assert_eq!(contar_interrogacoes(r#"SELECT "a""?""b" FROM c"#), 0);
        assert_eq!(contar_interrogacoes("SELECT 1 -- e ai?\nFROM c"), 0);
        assert_eq!(
            contar_interrogacoes("SELECT 1 -- e ai?\nFROM c WHERE x = ?"),
            1
        );
        assert_eq!(contar_interrogacoes("SELECT /* ? */ 1 FROM c"), 0);
        assert_eq!(
            contar_interrogacoes("SELECT /* ? */ 1 FROM c WHERE x = ?"),
            1
        );
    }

    // Citacao aberta engole ate o fim: o erro de sintaxe e do servidor, e o
    // driver so nao pode inventar um parametro dentro dela.
    #[test]
    fn citacao_aberta_engole_o_resto() {
        assert_eq!(
            contar_interrogacoes("SELECT * FROM c WHERE n = 'sem fim ?"),
            0
        );
        assert_eq!(contar_interrogacoes("SELECT 1 /* sem fim ?"), 0);
    }

    // Acento antes do `?` nao desalinha a varredura por byte.
    #[test]
    fn acento_no_literal_nao_desalinha() {
        assert_eq!(contar_interrogacoes("SELECT * FROM c WHERE d = 'ação?'"), 0);
        assert_eq!(
            contar_interrogacoes("SELECT * FROM c WHERE d = 'ação' AND x = ?"),
            1
        );
    }
}
