//! Texto atravessando a fronteira C: ler o que o aplicativo mandou e escrever
//! em buffer de tamanho fixo sem jamais passar do fim.
//!
//! A decisao de quanto copiar e uma funcao PURA (`fatiar`), separada do
//! `unsafe` que toca o ponteiro. E de proposito: o truncamento e a parte do
//! driver que mais quebra aplicativo quando sai errada -- copiar demais
//! estoura o buffer do cliente, copiar de menos sem avisar corrompe o dado em
//! silencio -- e funcao pura se prova com teste de tabela, sem soquete e sem
//! ponteiro.

use crate::tipos::*;

/// O que couber de `dado` num buffer de `cap` bytes com NUL no fim.
///
/// Devolve quantos bytes copiar e se truncou. `cap` <= 0 nao cabe nem o NUL,
/// entao copia zero e conta como truncado -- o chamador decide o erro.
pub fn fatiar(dado: usize, cap: SqlLen) -> (usize, bool) {
    if cap <= 0 {
        return (0, dado > 0);
    }
    let util = (cap as usize) - 1;
    if dado <= util {
        (dado, false)
    } else {
        (util, true)
    }
}

/// Le a string que o aplicativo passou: `len` em bytes ou `SQL_NTS`.
///
/// Bytes invalidos de UTF-8 viram U+FFFD em vez de derrubar a chamada: a
/// string vai para o servidor, e e ele quem recusa o que nao entender --
/// com uma mensagem melhor que "texto invalido" daqui.
///
/// # Safety
///
/// `ptr` precisa apontar para `len` bytes validos (ou uma string com NUL,
/// quando `len == SQL_NTS`).
pub unsafe fn ler_texto(ptr: *const SqlChar, len: SqlInteger) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let bytes = if len == SQL_NTS {
        let mut fim = 0usize;
        while *ptr.add(fim) != 0 {
            fim += 1;
        }
        std::slice::from_raw_parts(ptr, fim)
    } else if len < 0 {
        return String::new();
    } else {
        std::slice::from_raw_parts(ptr, len as usize)
    };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Escreve `dado` como string C no buffer do aplicativo.
///
/// Devolve `(escritos, truncou)`. Buffer nulo escreve nada e serve para o
/// aplicativo que so quer o tamanho (pergunta com NULL, aloca, pergunta de
/// novo -- o vai-e-vem classico do ODBC).
///
/// # Safety
///
/// `buf`, quando nao nulo, precisa ter `cap` bytes de espaco.
pub unsafe fn escrever_texto(dado: &[u8], buf: *mut SqlChar, cap: SqlLen) -> (usize, bool) {
    if buf.is_null() {
        return (0, !dado.is_empty());
    }
    let (n, truncou) = fatiar(dado.len(), cap);
    if cap > 0 {
        std::ptr::copy_nonoverlapping(dado.as_ptr(), buf, n);
        *buf.add(n) = 0;
    }
    (n, truncou)
}

/// Le um parametro/buffer `SQL_C_WCHAR`: UTF-16 na ORDEM NATIVA que o gestor
/// de drivers entrega -> UTF-8 (pedido 238). `tamanho` e o comprimento em
/// BYTES -- a convencao WCHAR do ODBC, NUNCA em unidades de 16 bits -- ou
/// `SQL_NTS`.
///
/// Par substituto invalido (alto sem baixo, baixo solto) devolve a POSICAO
/// (o indice da unidade UTF-16, 0-based) em vez de calar virando U+FFFD ou de
/// desistir sem dizer onde: um cliente WCHAR que manda lixo tem bug no
/// CLIENTE, e apontar o lugar e mais barato para quem depura do que um erro
/// generico. `char::decode_utf16` (da `std`) faz a decodificacao; a posicao
/// vem de somar `char::len_utf16()` a cada acerto -- um erro consome sempre
/// UMA unidade so (a invalida), entao o acumulado na hora do erro E a
/// posicao dela.
///
/// # Safety
///
/// `ptr` precisa apontar para `tamanho` bytes validos (arredondado para
/// baixo em unidades de 2 bytes -- um byte impar sobrando e o proprio
/// aplicativo mandando um tamanho que a especificacao nunca produz) ou
/// terminar numa unidade `0x0000` quando `tamanho == SQL_NTS`. O alinhamento
/// de `u16` NAO e garantido pela ABI, e por isso toda leitura e
/// `read_unaligned` -- a mesma cautela de `ler_texto`.
pub unsafe fn ler_texto_utf16(ptr: *const SqlWchar, tamanho: SqlInteger) -> Result<String, usize> {
    if ptr.is_null() {
        return Ok(String::new());
    }
    let unidades: Vec<u16> = if tamanho == SQL_NTS {
        let mut v = Vec::new();
        loop {
            let u = std::ptr::read_unaligned(ptr.add(v.len()));
            if u == 0 {
                break;
            }
            v.push(u);
        }
        v
    } else if tamanho < 0 {
        return Ok(String::new());
    } else {
        let n_unidades = (tamanho as usize) / 2;
        (0..n_unidades)
            .map(|i| std::ptr::read_unaligned(ptr.add(i)))
            .collect()
    };

    let mut saida = String::new();
    let mut posicao = 0usize;
    for r in char::decode_utf16(unidades.iter().copied()) {
        match r {
            Ok(c) => {
                posicao += c.len_utf16();
                saida.push(c);
            }
            Err(_) => return Err(posicao),
        }
    }
    Ok(saida)
}

/// Quantos BYTES `dado` ocupa como UTF-16 -- sem o NUL final. E a unidade do
/// INDICADOR de saida em `SQL_C_WCHAR`: a convencao do ODBC e bytes, mesmo
/// quando o tipo C e de 16 bits, e por isso nao vale reusar `dado.len()`
/// (que e byte de UTF-8, outra contagem).
pub fn bytes_utf16(dado: &str) -> usize {
    dado.chars().map(char::len_utf16).sum::<usize>() * 2
}

/// Escreve `dado` (UTF-8) como UTF-16 no buffer do aplicativo -- o espelho de
/// `escrever_texto` para `SQL_C_WCHAR` (pedido 238).
///
/// `cap` e o espaco do buffer em BYTES (a convencao WCHAR: duas por unidade),
/// nunca em caracteres nem em unidades. Devolve `(consumidos, truncou)`, e
/// `consumidos` e BYTES de `dado` NA CODIFICACAO DE ORIGEM (UTF-8) -- o que o
/// chamador soma ao deslocamento para a proxima chamada continuar no lugar
/// certo, exatamente como `escrever_texto` devolve bytes da fonte e nao do
/// destino.
///
/// A fatia nunca corta uma unidade nem um par substituto ao meio: cada
/// CARACTERE entra inteiro ou fica de fora inteiro inteiro -- um caractere
/// pela metade nao e truncamento, e corrupcao.
///
/// # Safety
///
/// `buf`, quando nao nulo, precisa ter `cap` bytes de espaco.
pub unsafe fn escrever_utf16(dado: &str, buf: *mut SqlWchar, cap: SqlLen) -> (usize, bool) {
    if buf.is_null() {
        return (0, !dado.is_empty());
    }
    // Espaco em UNIDADES utf-16, menos uma para o NUL final -- o mesmo "-1"
    // de `fatiar`, só que em unidades de 2 bytes em vez de 1.
    let util_unidades = if cap < 2 { 0 } else { (cap as usize) / 2 - 1 };
    let mut unidades_usadas = 0usize;
    let mut bytes_fonte = 0usize;
    let mut truncou = false;
    for c in dado.chars() {
        let n = c.len_utf16();
        if unidades_usadas + n > util_unidades {
            truncou = true;
            break;
        }
        unidades_usadas += n;
        bytes_fonte += c.len_utf8();
    }
    if cap >= 2 {
        for (i, u) in dado[..bytes_fonte].encode_utf16().enumerate() {
            std::ptr::write_unaligned(buf.add(i), u);
        }
        std::ptr::write_unaligned(buf.add(unidades_usadas), 0u16);
    }
    (bytes_fonte, truncou)
}

#[cfg(test)]
mod testes {
    use super::*;

    // A tabela cobre as tres bordas que ja quebraram driver alheio: buffer
    // justo (cabe com o NUL), buffer um byte menor (trunca UM byte) e buffer
    // sem espaco nem para o NUL.
    #[test]
    fn fatiar_respeita_o_nul() {
        assert_eq!(fatiar(5, 6), (5, false)); // justo: 5 bytes + NUL
        assert_eq!(fatiar(5, 5), (4, true)); // um a menos: trunca
        assert_eq!(fatiar(5, 1), (0, true)); // so o NUL: copia nada
        assert_eq!(fatiar(0, 1), (0, false)); // vazio cabe em 1
        assert_eq!(fatiar(5, 0), (0, true)); // sem espaco algum
        assert_eq!(fatiar(0, 0), (0, false)); // nada em nada nao trunca
    }

    #[test]
    fn escrever_poe_o_nul_no_fim() {
        let mut buf = [0xAAu8; 8];
        let (n, truncou) = unsafe { escrever_texto(b"abc", buf.as_mut_ptr(), 8) };
        assert_eq!((n, truncou), (3, false));
        assert_eq!(&buf[..4], b"abc\0");
    }

    #[test]
    fn escrever_truncado_avisa_e_termina_em_nul() {
        let mut buf = [0xAAu8; 4];
        let (n, truncou) = unsafe { escrever_texto(b"abcdef", buf.as_mut_ptr(), 4) };
        assert_eq!((n, truncou), (3, true));
        assert_eq!(&buf[..4], b"abc\0");
    }

    #[test]
    fn ler_com_nts_e_com_tamanho() {
        let c = b"ola\0resto";
        assert_eq!(unsafe { ler_texto(c.as_ptr(), SQL_NTS) }, "ola");
        assert_eq!(unsafe { ler_texto(c.as_ptr(), 3) }, "ola");
        assert_eq!(unsafe { ler_texto(std::ptr::null(), SQL_NTS) }, "");
    }

    // Pedido 238: o buffer UTF-16 montado A MAO, na ordem nativa (o que o
    // gestor de drivers entrega) -- "Sao Joao" com acento (uma unidade cada,
    // dentro do BMP) e um emoji (fora do BMP, PAR SUBSTITUTO de duas
    // unidades). E o caso que prova as duas metades: acento cabe numa
    // unidade so, emoji nao cabe em nenhuma.
    #[test]
    fn wchar_le_bmp_e_par_substituto_pelo_tamanho_em_bytes() {
        let texto = "São João 😀";
        let unidades: Vec<u16> = texto.encode_utf16().collect();
        assert!(
            unidades.len() > texto.chars().count(),
            "o emoji tem de render mais unidades que caracteres"
        );
        let bytes = (unidades.len() * 2) as SqlInteger;
        let lido = unsafe { ler_texto_utf16(unidades.as_ptr(), bytes) };
        assert_eq!(lido, Ok(texto.to_string()));
    }

    #[test]
    fn wchar_le_por_sql_nts() {
        let texto = "São João 😀";
        let mut unidades: Vec<u16> = texto.encode_utf16().collect();
        unidades.push(0); // o NUL que o SQL_NTS pede para achar o fim.
        let lido = unsafe { ler_texto_utf16(unidades.as_ptr(), SQL_NTS) };
        assert_eq!(lido, Ok(texto.to_string()));
    }

    // O substituto invalido recusa nomeando a POSICAO (pedido 238) -- nunca
    // vira U+FFFD calado. Alto solto (sem o baixo que devia seguir) na
    // unidade 2, depois de duas unidades boas.
    #[test]
    fn wchar_substituto_invalido_recusa_nomeando_a_posicao() {
        let mut unidades: Vec<u16> = "ok".encode_utf16().collect(); // 2 unidades boas
        unidades.push(0xD800); // alto solto, sem baixo -- unidade invalida na posicao 2
        let bytes = (unidades.len() * 2) as SqlInteger;
        assert_eq!(unsafe { ler_texto_utf16(unidades.as_ptr(), bytes) }, Err(2));
    }

    // Ponteiro nulo nao e erro: e o mesmo comportamento de `ler_texto`, e
    // existe para o caminho em que o aplicativo nao ligou parametro nenhum.
    #[test]
    fn wchar_ponteiro_nulo_e_texto_vazio() {
        assert_eq!(
            unsafe { ler_texto_utf16(std::ptr::null(), SQL_NTS) },
            Ok(String::new())
        );
    }

    // O espelho de `escrever_poe_o_nul_no_fim`/`escrever_truncado_avisa_e_
    // termina_em_nul`, agora em UTF-16: ida e volta com acento e emoji, e o
    // truncamento nunca corta uma unidade nem um par substituto ao meio.
    #[test]
    fn escrever_utf16_ida_e_volta_sem_truncar() {
        let texto = "São João 😀";
        let unidades_esperadas: Vec<u16> = texto.encode_utf16().collect();
        let mut buf = vec![0xAAAAu16; unidades_esperadas.len() + 1];
        let cap = (buf.len() * 2) as SqlLen;
        let (consumidos, truncou) = unsafe { escrever_utf16(texto, buf.as_mut_ptr(), cap) };
        assert!(!truncou);
        assert_eq!(
            consumidos,
            texto.len(),
            "bytes da FONTE (utf-8), nao do destino"
        );
        assert_eq!(&buf[..unidades_esperadas.len()], &unidades_esperadas[..]);
        assert_eq!(buf[unidades_esperadas.len()], 0, "NUL no fim");
        // E o mesmo buffer relido pelo conversor de entrada bate com o texto.
        let bytes = (unidades_esperadas.len() * 2) as SqlInteger;
        assert_eq!(
            unsafe { ler_texto_utf16(buf.as_ptr(), bytes) },
            Ok(texto.to_string())
        );
    }

    // O truncamento so pode cortar CARACTERE inteiro: um buffer que cabe so
    // a primeira metade do par substituto do emoji tem de deixar o emoji de
    // fora INTEIRO, nunca escrever so a metade alta do par.
    #[test]
    fn escrever_utf16_nunca_corta_o_par_substituto_ao_meio() {
        let texto = "ab😀"; // "ab" (2 unidades) + emoji (2 unidades) = 4
                            // cap para 3 unidades (a+NUL cabe, mas o par do emoji nao cabe
                            // inteiro): so "ab" pode entrar, nunca "ab" + a metade alta.
        let mut buf = [0xAAAAu16; 4];
        let cap = (3 * 2) as SqlLen; // 3 unidades de espaco (2 uteis + NUL)
        let (consumidos, truncou) = unsafe { escrever_utf16(texto, buf.as_mut_ptr(), cap) };
        assert!(truncou);
        assert_eq!(
            consumidos, 2,
            "so 'ab' (2 bytes utf-8), o emoji fica de fora inteiro"
        );
        assert_eq!(&buf[..2], &[b'a' as u16, b'b' as u16]);
        assert_eq!(buf[2], 0, "NUL logo apos 'ab', sem lixo da metade do par");
    }

    #[test]
    fn escrever_utf16_buffer_nulo_so_pergunta_o_tamanho() {
        let (n, truncou) = unsafe { escrever_utf16("x", std::ptr::null_mut(), 0) };
        assert_eq!((n, truncou), (0, true));
        let (n, truncou) = unsafe { escrever_utf16("", std::ptr::null_mut(), 0) };
        assert_eq!((n, truncou), (0, false));
    }

    #[test]
    fn bytes_utf16_conta_par_substituto_como_quatro_bytes() {
        assert_eq!(bytes_utf16("ab"), 4); // 2 unidades x 2 bytes
        assert_eq!(bytes_utf16("😀"), 4); // par substituto: 2 unidades x 2 bytes
        assert_eq!(bytes_utf16(""), 0);
    }
}
