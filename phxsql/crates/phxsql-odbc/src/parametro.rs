//! Os parametros de instrucao preparada: contar os `?` do texto.
//!
//! A contagem e uma funcao PURA, pelo mesmo motivo do `fatiar` do `texto.rs`:
//! ela decide quantos valores o aplicativo tem de ligar, e errar isso faz o
//! driver recusar uma execucao legitima (ou mandar ao servidor uma lista de
//! tamanho que ele recusa). Funcao pura se prova com tabela, sem soquete e
//! sem ponteiro.

use crate::registro::Parametro;
use crate::texto::ler_texto;
use crate::tipos::*;
use phxsql_core::json::Json;

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

/// O motivo de recusa do tipo C, ou `None` quando o driver sabe le-lo.
///
/// A recusa acontece na LIGACAO e nao na execucao, e e a mesma decisao que a
/// casa ja tomou no `ao_excluir`: uma ligacao se DECLARA uma vez e se EXECUTA
/// muitas. Recusar cedo custa um erro lido enquanto se escreve a ligacao;
/// recusar tarde custa um laco de mil execucoes que morre na primeira, longe
/// de quem a escreveu.
pub fn recusa_do_tipo_c(tipo_c: SqlSmallint) -> Option<String> {
    match tipo_c {
        // SQL_C_DEFAULT quer dizer "o tipo C padrao do tipo SQL", e o tipo SQL
        // que este driver declara no SQLDescribeParam e SQL_VARCHAR -- cujo
        // padrao e SQL_C_CHAR. Tratar como texto aqui nao e chute: e a
        // consequencia do que o proprio driver responde.
        SQL_C_CHAR | SQL_C_DEFAULT | SQL_C_SSHORT | SQL_C_SHORT | SQL_C_SLONG | SQL_C_LONG
        | SQL_C_SBIGINT | SQL_C_DOUBLE | SQL_C_FLOAT | SQL_C_BIT => None,
        SQL_C_WCHAR => Some(
            "SQL_C_WCHAR (UTF-16) nao serve de parametro neste driver, que e ANSI \
             (docs/ODBC.md, secao 2): ligue o valor como SQL_C_CHAR em UTF-8"
                .into(),
        ),
        outro => Some(format!(
            "tipo C {outro} nao serve de parametro neste driver; ele le SQL_C_CHAR, \
             SQL_C_SSHORT, SQL_C_SLONG, SQL_C_SBIGINT, SQL_C_DOUBLE, SQL_C_FLOAT e SQL_C_BIT"
        )),
    }
}

/// O inteiro como o protocolo o quer.
///
/// Ate 2^53 vai como NUMERO, que e a forma escrita no contrato
/// (`"parametros": [1]`). Dali para cima vai como TEXTO, e nao por gosto: o
/// `Json` desta casa guarda numero num `f64`, e um id de dezenove digitos
/// voltaria ARREDONDADO -- quer dizer, a linha errada, sem erro nenhum. O
/// `json_para_valor` do servidor aceita inteiro em texto exatamente por causa
/// deste caminho, e o comentario esta la nomeando o ODBC.
pub fn json_de_inteiro(n: i64) -> Json {
    if n.unsigned_abs() < phxsql_core::json::INTEIRO_EXATO_MAX {
        Json::de_i64(n)
    } else {
        Json::texto_de(n.to_string())
    }
}

/// O numero fracionario como o protocolo o quer: TEXTO, sempre.
///
/// E a regra da casa inteira, nao uma escolha local. O `literal_para_json` do
/// tradutor de SQL manda TODO literal numerico como texto para o decimal nao
/// passar por `f64`, e o `json_para_valor` do servidor RECUSA decimal que
/// chegue como numero -- «decimal precisa vir como texto ("12.34"), para nao
/// perder centavo em f64». Um parametro fracionario que fosse `Json::Numero`
/// seria recusado numa coluna Decimal e aceito numa Real: duas respostas para
/// a mesma ligacao.
///
/// O `{}` do Rust escreve a forma mais curta que releia o MESMO `f64` -- o
/// driver nao inventa digito, nem perde o que o aplicativo ja tinha.
///
/// `None` para NaN e infinito: nao existe literal para eles no JSON nem no
/// lexico do servidor, e transforma-los em nulo seria mentir sobre o dado.
pub fn json_de_real(n: f64) -> Option<Json> {
    if !n.is_finite() {
        return None;
    }
    Some(Json::texto_de(format!("{n}")))
}

/// Le o valor que a ligacao aponta, ja no JSON que vai no `parametros`.
///
/// Devolve `(SQLSTATE, mensagem)` na recusa. E o UNICO lugar do driver que
/// desreferencia ponteiro de parametro.
///
/// # Safety
///
/// So se chama de dentro de um `SQLExecute`/`SQLExecDirect`: e a unica janela
/// em que o contrato da ABI promete que o ponteiro do aplicativo ainda vale.
pub unsafe fn ler(p: &Parametro) -> Result<Json, (&'static str, String)> {
    let n = p.numero;
    let indicador = if p.indicador == 0 {
        None
    } else {
        Some(std::ptr::read_unaligned(p.indicador as *const SqlLen))
    };

    // O indicador manda, e vem ANTES do buffer: em NULL o ponteiro de valor
    // pode ser nulo de direito, e olhar o buffer primeiro seria ler o que nao
    // existe.
    if indicador == Some(SQL_NULL_DATA) {
        return Ok(Json::Nulo);
    }
    if let Some(i) = indicador {
        // SQL_DATA_AT_EXEC e a familia SQL_LEN_DATA_AT_EXEC(n) (<= -100) dizem
        // "o valor vem depois, por SQLPutData" -- que este driver nao tem.
        // Reconhecer e recusar NOMEANDO e melhor que ler o buffer como se o
        // valor estivesse la: ali ha lixo, e lixo vira linha gravada.
        if i == SQL_DATA_AT_EXEC || i <= -100 {
            return Err((
                "HYC00",
                format!(
                    "parametro {n}: valor entregue em pedacos (SQLPutData) nao \
                     implementado neste driver"
                ),
            ));
        }
        if i < 0 && i != SQL_NTS as SqlLen {
            return Err((
                "HY090",
                format!("parametro {n}: indicador {i} nao e um tamanho valido"),
            ));
        }
    }
    if p.buf == 0 {
        return Err((
            "HY009",
            format!(
                "parametro {n}: ponteiro de valor nulo sem SQL_NULL_DATA no \
                 indicador -- NULL se manda pelo indicador"
            ),
        ));
    }

    match p.tipo_c {
        SQL_C_CHAR | SQL_C_DEFAULT => {
            // Quem diz quantos bytes valem e o INDICADOR, nao o tamanho do
            // buffer: a especificacao manda ignorar o `BufferLength` em
            // parametro de entrada de tipo caractere. Sem indicador, ou com
            // SQL_NTS, o texto vai ate o NUL.
            let tamanho = match indicador {
                Some(i) if i >= 0 => i.min(SqlInteger::MAX as SqlLen) as SqlInteger,
                _ => SQL_NTS,
            };
            Ok(Json::texto_de(ler_texto(p.buf as *const SqlChar, tamanho)))
        }
        SQL_C_SSHORT | SQL_C_SHORT => Ok(json_de_inteiro(i64::from(std::ptr::read_unaligned(
            p.buf as *const i16,
        )))),
        SQL_C_SLONG | SQL_C_LONG => Ok(json_de_inteiro(i64::from(std::ptr::read_unaligned(
            p.buf as *const i32,
        )))),
        SQL_C_SBIGINT => Ok(json_de_inteiro(std::ptr::read_unaligned(
            p.buf as *const i64,
        ))),
        SQL_C_DOUBLE => json_de_real(std::ptr::read_unaligned(p.buf as *const f64)).ok_or((
            "22003",
            format!("parametro {n}: NaN ou infinito nao tem literal nesta linguagem"),
        )),
        SQL_C_FLOAT => json_de_real(f64::from(std::ptr::read_unaligned(p.buf as *const f32)))
            .ok_or((
                "22003",
                format!("parametro {n}: NaN ou infinito nao tem literal nesta linguagem"),
            )),
        // Um byte, e qualquer coisa diferente de zero e verdadeiro -- o mesmo
        // criterio do `json_para_valor` do servidor para Bool.
        SQL_C_BIT => Ok(Json::Bool(
            std::ptr::read_unaligned(p.buf as *const u8) != 0,
        )),
        // Nao se chega aqui pelo caminho normal: o `recusa_do_tipo_c` ja
        // barrou na ligacao. A linha existe porque a barreira e o leitor podem
        // divergir num commit futuro, e divergir para o lado de LER lixo seria
        // o pior dos dois.
        outro => Err((
            "HYC00",
            format!("parametro {n}: tipo C {outro} nao serve de parametro neste driver"),
        )),
    }
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

    // A recusa do tipo C acontece na ligacao, e o par trava os dois sentidos:
    // o que o driver le passa, e o que ele nao le recusa NOMEANDO o tipo -- um
    // "nao suportado" sem numero manda o programador adivinhar qual ligacao
    // errou.
    #[test]
    fn o_tipo_c_que_o_driver_le_passa_e_o_resto_recusa_nomeando() {
        for bom in [
            SQL_C_CHAR,
            SQL_C_DEFAULT,
            SQL_C_SLONG,
            SQL_C_LONG,
            SQL_C_SSHORT,
            SQL_C_SBIGINT,
            SQL_C_DOUBLE,
            SQL_C_FLOAT,
            SQL_C_BIT,
        ] {
            assert!(recusa_do_tipo_c(bom).is_none(), "tipo C {bom} devia passar");
        }
        let w = recusa_do_tipo_c(SQL_C_WCHAR).expect("UTF-16 tem de recusar neste driver ANSI");
        assert!(
            w.contains("SQL_C_CHAR"),
            "a recusa tem de dizer o que usar: {w}"
        );
        let outro = recusa_do_tipo_c(-99).expect("tipo C inventado tem de recusar");
        assert!(
            outro.contains("-99"),
            "a recusa tem de nomear o tipo: {outro}"
        );
    }

    fn liga(tipo_c: SqlSmallint, buf: usize, indicador: usize) -> Parametro {
        Parametro {
            numero: 1,
            tipo_c,
            buf,
            indicador,
        }
    }

    // O indicador manda, e manda ANTES do buffer: em NULL o ponteiro de valor
    // pode ser nulo de direito.
    #[test]
    fn nulo_pelo_indicador_nem_olha_o_buffer() {
        let mut ind: SqlLen = SQL_NULL_DATA;
        let p = liga(SQL_C_CHAR, 0, &mut ind as *mut SqlLen as usize);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::Nulo);
    }

    // Sem SQL_NULL_DATA, ponteiro nulo e erro e nao string vazia: gravar ""
    // onde o aplicativo queria NULL e uma mentira sobre o dado.
    #[test]
    fn ponteiro_nulo_sem_indicador_de_nulo_recusa() {
        let mut ind: SqlLen = 4;
        let p = liga(SQL_C_CHAR, 0, &mut ind as *mut SqlLen as usize);
        assert_eq!(unsafe { ler(&p) }.unwrap_err().0, "HY009");
    }

    #[test]
    fn texto_pelo_indicador_e_pelo_nul() {
        let dado = b"Blumenau\0lixo depois";
        // Com tamanho no indicador: le exatamente o que ele diz.
        let mut ind: SqlLen = 8;
        let p = liga(
            SQL_C_CHAR,
            dado.as_ptr() as usize,
            &mut ind as *mut SqlLen as usize,
        );
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::texto_de("Blumenau"));
        // Com SQL_NTS: vai ate o NUL, e o lixo depois nao entra.
        let mut ind: SqlLen = SQL_NTS as SqlLen;
        let p = liga(
            SQL_C_CHAR,
            dado.as_ptr() as usize,
            &mut ind as *mut SqlLen as usize,
        );
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::texto_de("Blumenau"));
        // Sem indicador nenhum: a especificacao manda tratar como NTS.
        let p = liga(SQL_C_CHAR, dado.as_ptr() as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::texto_de("Blumenau"));
    }

    #[test]
    fn inteiros_de_cada_largura() {
        let mut curto: i16 = -7;
        let p = liga(SQL_C_SSHORT, &mut curto as *mut i16 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::de_i64(-7));
        let mut medio: i32 = 1234;
        let p = liga(SQL_C_SLONG, &mut medio as *mut i32 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::de_i64(1234));
        let mut longo: i64 = -900_000;
        let p = liga(SQL_C_SBIGINT, &mut longo as *mut i64 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::de_i64(-900_000));
    }

    // O caso que decide a forma: 2^53 + 1 nao existe em `f64`. Mandado como
    // Json::Numero ele volta 2^53 -- a LINHA ERRADA, sem erro nenhum. A
    // conferencia e sobre o JSON ESCRITO, porque e ele que viaja.
    #[test]
    fn inteiro_maior_que_o_exato_vai_como_texto() {
        let mut enorme: i64 = 9_007_199_254_740_993;
        let p = liga(SQL_C_SBIGINT, &mut enorme as *mut i64 as usize, 0);
        let j = unsafe { ler(&p) }.unwrap();
        assert_eq!(j.escrever(), "\"9007199254740993\"");
        // E o controle: o mesmo numero como `Json::Numero` PERDE o digito.
        assert_eq!(Json::de_i64(enorme).escrever(), "9007199254740992");
    }

    // Fracionario vai como texto pela regra da casa (o `literal_para_json` do
    // tradutor e o `json_para_valor` do servidor, que RECUSA decimal em
    // numero). A forma curta do Rust nao inventa digito.
    #[test]
    fn fracionario_vai_como_texto_com_os_digitos_que_tinha() {
        let mut preco: f64 = 4200.5;
        let p = liga(SQL_C_DOUBLE, &mut preco as *mut f64 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::texto_de("4200.5"));
        let mut curto: f32 = 1.5;
        let p = liga(SQL_C_FLOAT, &mut curto as *mut f32 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::texto_de("1.5"));
    }

    #[test]
    fn nan_e_infinito_recusam_em_vez_de_virar_nulo() {
        let mut nao: f64 = f64::NAN;
        let p = liga(SQL_C_DOUBLE, &mut nao as *mut f64 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap_err().0, "22003");
        let mut inf: f64 = f64::INFINITY;
        let p = liga(SQL_C_DOUBLE, &mut inf as *mut f64 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap_err().0, "22003");
    }

    #[test]
    fn bit_vira_booleano() {
        let mut um: u8 = 1;
        let p = liga(SQL_C_BIT, &mut um as *mut u8 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::Bool(true));
        let mut zero: u8 = 0;
        let p = liga(SQL_C_BIT, &mut zero as *mut u8 as usize, 0);
        assert_eq!(unsafe { ler(&p) }.unwrap(), Json::Bool(false));
    }

    // SQLPutData nao existe aqui. Ler o buffer assim mesmo pegaria LIXO, e
    // lixo vira linha gravada -- recusar nomeando e a unica saida honesta.
    #[test]
    fn valor_em_pedacos_recusa_em_vez_de_ler_lixo() {
        let mut nada: u8 = 0xAA;
        for pedido in [SQL_DATA_AT_EXEC, -100, -1234] {
            let mut ind: SqlLen = pedido;
            let p = liga(
                SQL_C_CHAR,
                &mut nada as *mut u8 as usize,
                &mut ind as *mut SqlLen as usize,
            );
            assert_eq!(unsafe { ler(&p) }.unwrap_err().0, "HYC00");
        }
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
