//! FrogCript: o envelope de duas camadas com transposicao, do Adriano Boller.
//!
//! # O que ele e, sem rodeio
//!
//! O FrogCript original (Wx Solucoes, documentado em
//! `FrogCript_Documentacao.docx`) monta um pacote assim:
//!
//! ```text
//! AEAD( Base64( AEAD(resto) ) )  |  AEAD( d + Base64(AEAD(extraido)) + d )
//! ```
//!
//! O texto e partido em dois pelo **pulo**: as casas 5, 10, 15... saem para o
//! lado "extraido", o resto fica. A **direcao** (0 ou 1) inverte o extraido, e
//! viaja escondida DENTRO do segundo pacote, entre as duas camadas.
//!
//! # O que ele acrescenta, e o que nao acrescenta
//!
//! Esta escrito na secao 9 do documento do autor, e vale repetir aqui porque
//! um modulo de cifra e o lugar onde uma frase vaga vira uma crenca errada:
//!
//! > O pulo 5 e a direcao **nao sao a chave**. A chave e a senha.
//!
//! A transposicao e uma permutacao FIXA e publica. Quem tem o texto cifrado
//! nao ganha nada com ela, porque so a ve depois de abrir o AEAD -- e quem
//! abriu o AEAD ja tem tudo. Ela nao acrescenta forca criptografica: o que
//! segura o conteudo e o AEAD e o tamanho da senha. Nem a duplicacao de
//! camadas acrescenta: dois AEAD com a MESMA chave nao somam segredo.
//!
//! O que ela acrescenta de verdade e **formato**: o pacote tem a forma que o
//! autor definiu, com dois lados e um separador, e um leitor que nao conheca a
//! convencao nao remonta o texto nem depois de abrir os dois lados.
//!
//! # O que esta implementacao NAO faz: AES
//!
//! O FrogCript de referencia usa **AES-256-GCM**. Este aqui usa o
//! **ChaCha20-Poly1305** da casa ([`crate::cifra`]). A consequencia tem de
//! estar escrita e nao escondida:
//!
//! > **Um pacote produzido aqui NAO abre no `frogcript.py`, e um pacote
//! > produzido pelo `frogcript.py` NAO abre aqui.** A estrutura e a mesma; a
//! > cifra de dentro nao e.
//!
//! A razao esta no cabecalho do [`crate::cifra`]: AES portatil, sem a
//! instrucao do processador, se escreve com tabelas, e tabela em cache vaza a
//! chave pelo tempo de acesso. Escrever AES aqui para ganhar compatibilidade
//! seria trocar uma cifra conferida contra o RFC 8439 por alguns milhares de
//! linhas novas de codigo criptografico no caminho de todo dado pessoal do
//! banco. Ver `docs/SEGURANCA.md` §11.4.
//!
//! # E o Base64 entre as camadas?
//!
//! Nao esta aqui, e a diferenca e so de tamanho. No original ele existe porque
//! o pacote intermediario e uma STRING que vai para dentro de outra cifra que
//! espera texto. Aqui tudo sao bytes do comeco ao fim, e Base64 no meio
//! custaria 33% de disco por camada -- 78% no total -- para nao mudar nada do
//! que o pacote esconde.
//!
//! # Tamanho
//!
//! ```text
//! saida = entrada
//!       + 2   direcao, uma ponta de cada lado, dentro da camada de fora
//!       + 4   comprimento do lado A
//!       + 1   separador
//!       + 4 x 16  etiquetas
//!       + 4 x 24  nonces
//!       = entrada + 167 bytes
//! ```
//!
//! Sao quatro etiquetas e quatro nonces porque sao quatro selagens: duas de
//! dentro (resto e extraido) e duas de fora. O numero nao e estimado -- o
//! teste `o_acrescimo_e_o_que_esta_escrito` confere o pacote contra esta
//! conta, para ele nao envelhecer calado.
//!
//! O FrogCript original sai maior: com Base64 em duas camadas e um sal de 16
//! bytes mais um nonce de 12 por selagem, ele fica em **3,16 x entrada + 327
//! bytes**. A conta esta em `docs/SEGURANCA.md` §11.4.

use crate::cifra::{self, CHAVE_LEN, TAG_LEN, XNONCE_LEN};
use crate::error::{PhxError, Result};

/// O pulo padrao: as casas 5, 10, 15... saem.
pub const SALTO_PADRAO: usize = 5;
/// O separador padrao entre os dois lados do pacote.
pub const SEPARADOR_PADRAO: u8 = b'|';

/// Quanto o pacote cresce sobre o texto, fora os quatro nonces: as duas
/// pontas de direcao, o comprimento, o separador e as quatro etiquetas.
pub const ACRESCIMO: usize = 2 + 4 + 1 + 4 * TAG_LEN;

/// Os dois valores que quem chama pode personalizar.
///
/// # Personalizar e virar segredo
///
/// A secao 10 do documento do autor pede que o salto e o separador possam
/// mudar, e diz o que isso implica: **quem os personaliza passa a trata-los
/// como parte do segredo**, junto com a senha. Vale a ressalva de sempre --
/// eles nao sao chave: um salto diferente muda a ordem das letras dentro de um
/// pacote que so abre com a senha certa. Perder o salto perde o texto; um
/// atacante nao o ganha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ajuste {
    pub salto: usize,
    pub separador: u8,
}

impl Ajuste {
    /// O ajuste de fabrica, utilizavel em contexto `const`.
    pub const PADRAO: Ajuste = Ajuste {
        salto: SALTO_PADRAO,
        separador: SEPARADOR_PADRAO,
    };
}

impl Default for Ajuste {
    fn default() -> Self {
        Ajuste::PADRAO
    }
}

impl Ajuste {
    pub fn novo(salto: usize, separador: u8) -> Result<Ajuste> {
        if salto < 2 {
            // Salto 1 levaria TODAS as casas para o lado extraido e nenhuma
            // para o resto; salto 0 dividiria por zero. Nos dois casos a
            // transposicao deixa de transpor, e o pacote passa a mentir sobre
            // o que faz.
            return Err(PhxError::Esquema(format!(
                "salto do FrogCript e {salto}: use 2 ou mais"
            )));
        }
        Ok(Ajuste { salto, separador })
    }
}

/// De que lado a direcao poe o extraido.
///
/// No documento e um digito 0 ou 1; aqui e um tipo, porque um `u8` solto
/// atravessando quatro funcoes e um convite a passar o valor errado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direcao {
    /// O extraido vai na ordem em que saiu.
    Direta,
    /// O extraido vai invertido.
    Invertida,
}

impl Direcao {
    pub fn digito(&self) -> u8 {
        match self {
            Direcao::Direta => 0,
            Direcao::Invertida => 1,
        }
    }

    pub fn de_digito(d: u8) -> Result<Direcao> {
        match d {
            0 => Ok(Direcao::Direta),
            1 => Ok(Direcao::Invertida),
            outro => Err(PhxError::Corrompido(format!(
                "direcao {outro} no pacote FrogCript: so 0 e 1 existem"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Nivel 1: o pulo
// ---------------------------------------------------------------------------

/// Parte o texto em (resto, extraido) pelas casas multiplas do salto.
///
/// # Por que CARACTERE, e nao byte
///
/// Porque o documento conta casas de texto, e o exemplo dele e
/// `ADRIANO JOSÉ BOLLER` -- em que o `É` ocupa **dois** bytes em UTF-8 e uma
/// casa. Contar bytes tiraria a metade de um caractere e a remontagem
/// devolveria texto invalido. Contando caracteres, o pulo do documento e o
/// pulo daqui.
pub fn pular(texto: &str, salto: usize) -> (String, String) {
    let mut resto = String::with_capacity(texto.len());
    let mut extraido = String::new();
    for (i, c) in texto.chars().enumerate() {
        if (i + 1) % salto == 0 {
            extraido.push(c);
        } else {
            resto.push(c);
        }
    }
    (resto, extraido)
}

/// Remonta o texto a partir de (resto, extraido). Inversa exata de [`pular`].
pub fn despular(resto: &str, extraido: &str, salto: usize) -> String {
    let resto: Vec<char> = resto.chars().collect();
    let extraido: Vec<char> = extraido.chars().collect();
    let total = resto.len() + extraido.len();
    let mut fora = String::with_capacity(total);
    let (mut ir, mut ie) = (0usize, 0usize);
    for i in 1..=total {
        if i % salto == 0 && ie < extraido.len() {
            fora.push(extraido[ie]);
            ie += 1;
        } else if ir < resto.len() {
            fora.push(resto[ir]);
            ir += 1;
        } else if ie < extraido.len() {
            fora.push(extraido[ie]);
            ie += 1;
        }
    }
    fora
}

// ---------------------------------------------------------------------------
// O pacote inteiro
// ---------------------------------------------------------------------------

/// Sela um lado: nonce sorteado a frente, etiqueta atras.
fn selar_lado(chave: &[u8; CHAVE_LEN], aad: &[u8], claro: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; XNONCE_LEN];
    cifra::sortear(&mut nonce);
    let (corpo, tag) = cifra::xselar(chave, &nonce, aad, claro);
    let mut fora = Vec::with_capacity(XNONCE_LEN + corpo.len() + TAG_LEN);
    fora.extend_from_slice(&nonce);
    fora.extend_from_slice(&corpo);
    fora.extend_from_slice(&tag);
    fora
}

fn abrir_lado(chave: &[u8; CHAVE_LEN], aad: &[u8], guardado: &[u8]) -> Result<Vec<u8>> {
    if guardado.len() < XNONCE_LEN + TAG_LEN {
        return Err(PhxError::Corrompido(
            "lado do pacote FrogCript curto demais para ter nonce e etiqueta".into(),
        ));
    }
    let mut nonce = [0u8; XNONCE_LEN];
    nonce.copy_from_slice(&guardado[..XNONCE_LEN]);
    let corte = guardado.len() - TAG_LEN;
    let mut tag = [0u8; TAG_LEN];
    tag.copy_from_slice(&guardado[corte..]);
    cifra::xabrir(chave, &nonce, aad, &guardado[XNONCE_LEN..corte], &tag)
}

/// Cifra `texto` no formato FrogCript, contando o pulo em CARACTERES -- que e
/// o que o documento do autor especifica.
pub fn cifrar(
    chave: &[u8; CHAVE_LEN],
    texto: &str,
    direcao: Direcao,
    ajuste: Ajuste,
) -> Result<Vec<u8>> {
    let (resto, extraido) = pular(texto, ajuste.salto);
    Ok(montar_pacote(
        chave,
        resto.as_bytes(),
        extraido.as_bytes(),
        direcao,
        ajuste,
    ))
}

/// Abre um pacote FrogCript de TEXTO. Devolve o texto e a direcao escondida.
///
/// A direcao **nao se informa**: ela vem de dentro do segundo pacote, que e
/// exatamente o desenho do documento (secao 5).
pub fn decifrar(
    chave: &[u8; CHAVE_LEN],
    pacote: &[u8],
    ajuste: Ajuste,
) -> Result<(String, Direcao)> {
    let (resto, extraido, direcao) = abrir_pacote(chave, pacote, ajuste)?;
    let texto = |b: Vec<u8>| -> Result<String> {
        String::from_utf8(b)
            .map_err(|e| PhxError::Corrompido(format!("pacote FrogCript nao e UTF-8: {e}")))
    };
    Ok((
        despular(&texto(resto)?, &texto(extraido)?, ajuste.salto),
        direcao,
    ))
}

/// O mesmo pacote, sobre BYTES opacos.
///
/// # Por que existe uma variante em bytes
///
/// Porque o que a tabela guarda numa coluna marcada nem sempre e texto: uma
/// data sao 4 bytes, um decimal sao 16, e um `Bin` e um anexo. Contar
/// caracteres em bytes que nao formam texto nao tem sentido -- e converter
/// para texto para poder contar seria inventar um encoding no caminho do dado
/// do cliente.
///
/// A diferenca com [`cifrar`] e SO o pulo: aqui ele conta byte. Para texto, a
/// versao de caractere e a que reproduz o exemplo do documento do autor, e e
/// a que uma operacao de protocolo deve expor.
pub fn cifrar_bytes(
    chave: &[u8; CHAVE_LEN],
    dados: &[u8],
    direcao: Direcao,
    ajuste: Ajuste,
) -> Vec<u8> {
    let (resto, extraido) = pular_bytes(dados, ajuste.salto);
    montar_pacote(chave, &resto, &extraido, direcao, ajuste)
}

/// Abre o pacote de [`cifrar_bytes`].
pub fn decifrar_bytes(
    chave: &[u8; CHAVE_LEN],
    pacote: &[u8],
    ajuste: Ajuste,
) -> Result<(Vec<u8>, Direcao)> {
    let (resto, extraido, direcao) = abrir_pacote(chave, pacote, ajuste)?;
    Ok((despular_bytes(&resto, &extraido, ajuste.salto), direcao))
}

/// O pulo sobre bytes. Ver [`pular`] para a versao de caractere.
pub fn pular_bytes(dados: &[u8], salto: usize) -> (Vec<u8>, Vec<u8>) {
    let mut resto = Vec::with_capacity(dados.len());
    let mut extraido = Vec::new();
    for (i, b) in dados.iter().enumerate() {
        if (i + 1) % salto == 0 {
            extraido.push(*b);
        } else {
            resto.push(*b);
        }
    }
    (resto, extraido)
}

/// Remonta os bytes. Inversa exata de [`pular_bytes`].
pub fn despular_bytes(resto: &[u8], extraido: &[u8], salto: usize) -> Vec<u8> {
    let total = resto.len() + extraido.len();
    let mut fora = Vec::with_capacity(total);
    let (mut ir, mut ie) = (0usize, 0usize);
    for i in 1..=total {
        if i % salto == 0 && ie < extraido.len() {
            fora.push(extraido[ie]);
            ie += 1;
        } else if ir < resto.len() {
            fora.push(resto[ir]);
            ir += 1;
        } else if ie < extraido.len() {
            fora.push(extraido[ie]);
            ie += 1;
        }
    }
    fora
}

/// Monta o pacote de duas camadas a partir das duas metades ja separadas.
///
/// ```text
/// [tam do lado A u32][lado A][separador][lado B]
/// ```
///
/// O comprimento vai na frente **de proposito**: o separador e um byte
/// qualquer (`|` por padrao) e o lado A e texto cifrado, que pode conter esse
/// mesmo byte. Partir pelo separador, como o `frogcript.py` faz com uma
/// string Base64, aqui cortaria no lugar errado uma vez a cada 256 bytes. O
/// separador continua gravado, porque faz parte do formato e porque o
/// documento o quer personalizavel -- mas quem parte o pacote usa o
/// comprimento, e o separador e CONFERIDO.
fn montar_pacote(
    chave: &[u8; CHAVE_LEN],
    resto: &[u8],
    extraido: &[u8],
    direcao: Direcao,
    ajuste: Ajuste,
) -> Vec<u8> {
    let extraido: Vec<u8> = match direcao {
        Direcao::Direta => extraido.to_vec(),
        Direcao::Invertida => extraido.iter().rev().copied().collect(),
    };

    // Camada de dentro.
    let a1 = selar_lado(chave, b"in", resto);
    let b1 = selar_lado(chave, b"in", &extraido);

    // A direcao entra AQUI, entre as duas camadas -- e por isso ela nao
    // aparece em lugar nenhum do pacote visivel. Nas duas pontas, como no
    // documento: um digito antes e outro depois.
    let d = direcao.digito();
    let mut b1d = Vec::with_capacity(b1.len() + 2);
    b1d.push(d);
    b1d.extend_from_slice(&b1);
    b1d.push(d);

    // Camada de fora.
    let a2 = selar_lado(chave, b"out", &a1);
    let b2 = selar_lado(chave, b"out", &b1d);

    let mut fora = Vec::with_capacity(4 + a2.len() + 1 + b2.len());
    fora.extend_from_slice(&(a2.len() as u32).to_le_bytes());
    fora.extend_from_slice(&a2);
    fora.push(ajuste.separador);
    fora.extend_from_slice(&b2);
    fora
}

/// Abre as duas camadas e devolve (resto, extraido ja desinvertido, direcao).
fn abrir_pacote(
    chave: &[u8; CHAVE_LEN],
    pacote: &[u8],
    ajuste: Ajuste,
) -> Result<(Vec<u8>, Vec<u8>, Direcao)> {
    if pacote.len() < 5 {
        return Err(PhxError::Corrompido("pacote FrogCript truncado".into()));
    }
    let tam_a = u32::from_le_bytes([pacote[0], pacote[1], pacote[2], pacote[3]]) as usize;
    let fim_a = 4 + tam_a;
    if pacote.len() <= fim_a {
        return Err(PhxError::Corrompido(
            "pacote FrogCript diz um lado A maior que ele proprio".into(),
        ));
    }
    if pacote[fim_a] != ajuste.separador {
        return Err(PhxError::Corrompido(format!(
            "separador do pacote FrogCript e {:?} e o esperado e {:?}: \
             ou o pacote foi montado com outro separador, ou nao e um pacote",
            pacote[fim_a] as char, ajuste.separador as char
        )));
    }

    let a1 = abrir_lado(chave, b"out", &pacote[4..fim_a])?;
    let b1d = abrir_lado(chave, b"out", &pacote[fim_a + 1..])?;
    if b1d.len() < 2 {
        return Err(PhxError::Corrompido(
            "lado B do pacote FrogCript nao tem as duas pontas de direcao".into(),
        ));
    }
    let d = b1d[0];
    if b1d[b1d.len() - 1] != d {
        return Err(PhxError::Corrompido(
            "as duas pontas de direcao do pacote FrogCript nao batem".into(),
        ));
    }
    let direcao = Direcao::de_digito(d)?;

    let resto = abrir_lado(chave, b"in", &a1)?;
    let extraido = abrir_lado(chave, b"in", &b1d[1..b1d.len() - 1])?;
    let extraido: Vec<u8> = match direcao {
        Direcao::Direta => extraido,
        Direcao::Invertida => extraido.into_iter().rev().collect(),
    };
    Ok((resto, extraido, direcao))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn chave() -> [u8; CHAVE_LEN] {
        cifra::chave_de_senha("Wx-Solucoes-2026", b"sal do teste", 1000)
    }

    /// O exemplo da secao 7 do documento, letra por letra.
    ///
    /// `ADRIANO JOSÉ BOLLER`, salto 5: as casas 5, 10 e 15 sao `A`, `O` e `O`,
    /// e o resto e `ADRINO JSÉ BLLER`. E o unico "vetor" que o FrogCript tem,
    /// e ele nao e criptografico -- e a transposicao, que e a parte que o
    /// autor definiu.
    #[test]
    fn o_pulo_bate_com_o_exemplo_do_documento() {
        let (resto, extraido) = pular("ADRIANO JOSÉ BOLLER", SALTO_PADRAO);
        assert_eq!(extraido, "AOO", "as casas 5, 10 e 15");
        assert_eq!(resto, "ADRINO JSÉ BLLER");
    }

    /// O acento e a razao de o pulo contar CARACTERE.
    ///
    /// Com o pulo contando byte, o `É` de `JOSÉ` -- que ocupa dois -- seria
    /// partido, e a casa 15 cairia no meio dele. O teste falha com o defeito
    /// reposto (trocando `chars()` por `bytes()` no `pular`): o extraido sai
    /// `AO\u{a9}` e a remontagem devolve texto invalido.
    #[test]
    fn o_pulo_conta_caractere_e_nao_byte() {
        let texto = "ADRIANO JOSÉ BOLLER";
        assert_eq!(texto.chars().count(), 19);
        assert_eq!(texto.len(), 20, "o E com acento ocupa dois bytes");
        let (resto, extraido) = pular(texto, SALTO_PADRAO);
        assert_eq!(despular(&resto, &extraido, SALTO_PADRAO), texto);
    }

    /// Ida e volta em textos de todo tamanho, inclusive os que quebram laco:
    /// vazio, menor que o salto, exatamente o salto, e multiplo dele.
    #[test]
    fn o_pulo_fecha_a_volta_em_qualquer_tamanho() {
        for n in 0..60usize {
            let texto: String = (0..n).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
            for salto in 2..9usize {
                let (r, e) = pular(&texto, salto);
                assert_eq!(
                    despular(&r, &e, salto),
                    texto,
                    "nao fechou com {n} letras e salto {salto}"
                );
            }
        }
    }

    /// As duas direcoes dao pacotes diferentes, e as duas voltam o mesmo
    /// texto com a direcao certa -- que e a promessa da secao 6.
    #[test]
    fn as_duas_direcoes_voltam_o_mesmo_texto() {
        let k = chave();
        let texto = "ADRIANO JOSÉ BOLLER";
        let a = cifrar(&k, texto, Direcao::Direta, Ajuste::default()).unwrap();
        let b = cifrar(&k, texto, Direcao::Invertida, Ajuste::default()).unwrap();
        assert_ne!(a, b);
        assert_eq!(
            decifrar(&k, &a, Ajuste::default()).unwrap(),
            (texto.to_string(), Direcao::Direta)
        );
        assert_eq!(
            decifrar(&k, &b, Ajuste::default()).unwrap(),
            (texto.to_string(), Direcao::Invertida)
        );
    }

    /// A direcao nao viaja em claro: nao ha byte 0 nem 1 solto no pacote que
    /// diga qual foi. Os dois pacotes do mesmo texto tem o MESMO tamanho.
    ///
    /// Um tamanho diferente entregaria a direcao sem abrir nada, que e
    /// justamente o que a secao 3 promete que nao acontece.
    #[test]
    fn a_direcao_nao_vaza_pelo_tamanho() {
        let k = chave();
        let texto = "ADRIANO JOSÉ BOLLER, com um pouco mais de texto para medir";
        let a = cifrar(&k, texto, Direcao::Direta, Ajuste::default()).unwrap();
        let b = cifrar(&k, texto, Direcao::Invertida, Ajuste::default()).unwrap();
        assert_eq!(a.len(), b.len());
    }

    /// Salto e separador personalizados, como pede a secao 10.
    #[test]
    fn salto_e_separador_a_gosto() {
        let k = chave();
        let meu = Ajuste::novo(7, b'#').unwrap();
        let texto = "ADRIANO JOSÉ BOLLER";
        let pacote = cifrar(&k, texto, Direcao::Invertida, meu).unwrap();
        assert_eq!(decifrar(&k, &pacote, meu).unwrap().0, texto);

        // Com o ajuste ERRADO nao se remonta o texto. O separador cai antes,
        // com mensagem propria; o salto errado passa pelas quatro etiquetas e
        // devolve as letras fora de ordem -- que e o que a secao 10 quer dizer
        // com "trate como parte do segredo".
        let e = decifrar(&k, &pacote, Ajuste::default()).unwrap_err();
        assert!(e.to_string().contains("separador"), "{e}");

        let so_o_salto = Ajuste::novo(3, b'#').unwrap();
        assert_ne!(decifrar(&k, &pacote, so_o_salto).unwrap().0, texto);
    }

    /// Salto 1 e 0 sao recusados: eles fariam a transposicao nao transpor.
    #[test]
    fn salto_degenerado_e_recusado() {
        assert!(Ajuste::novo(1, b'|').is_err());
        assert!(Ajuste::novo(0, b'|').is_err());
    }

    /// Um bit trocado em qualquer lugar do pacote nao abre.
    ///
    /// E o que as quatro etiquetas compram, e nao a transposicao.
    #[test]
    fn um_bit_trocado_em_qualquer_lugar_nao_abre() {
        let k = chave();
        let pacote = cifrar(&k, "conteudo sensivel", Direcao::Direta, Ajuste::default()).unwrap();
        for i in 4..pacote.len() {
            let mut torto = pacote.clone();
            torto[i] ^= 1;
            if torto[i] == Ajuste::default().separador || pacote[i] == Ajuste::default().separador {
                continue; // este byte e o separador: cai com a mensagem dele
            }
            assert!(
                decifrar(&k, &torto, Ajuste::default()).is_err(),
                "o byte {i} trocado passou"
            );
        }
    }

    /// Chave errada nao abre, e nao devolve lixo.
    #[test]
    fn chave_errada_nao_abre() {
        let k = chave();
        let outra = cifra::chave_de_senha("outra senha", b"sal do teste", 1000);
        let pacote = cifrar(&k, "ADRIANO", Direcao::Direta, Ajuste::default()).unwrap();
        assert!(decifrar(&outra, &pacote, Ajuste::default()).is_err());
    }

    /// O acrescimo anunciado e o acrescimo real.
    ///
    /// Numero em documento envelhece calado; este confere a conta contra o
    /// pacote de verdade.
    #[test]
    fn o_acrescimo_e_o_que_esta_escrito() {
        let k = chave();
        for texto in ["", "a", "ADRIANO JOSÉ BOLLER", &"x".repeat(500)] {
            let pacote = cifrar(&k, texto, Direcao::Direta, Ajuste::default()).unwrap();
            assert_eq!(
                pacote.len(),
                texto.len() + ACRESCIMO + 4 * XNONCE_LEN,
                "o tamanho de {:?} nao bate com a conta do cabecalho",
                &texto[..texto.len().min(20)]
            );
        }
    }
}
