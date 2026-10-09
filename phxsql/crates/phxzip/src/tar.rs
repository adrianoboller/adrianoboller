//! `.tar` POSIX: ustar e pax, leitura e escrita (pedido 454, fatia Z4).
//!
//! # Por que tar
//!
//! A web do PhxZip devolve varias entradas de uma vez num `.tar`
//! (`docs/PHXZIP-WEB.md` §3.4): o ZIP e recusa nomeada do PhxZip, e o tar nao
//! comprime -- nao ha segunda compressao para decidir.
//!
//! # A norma
//!
//! IEEE Std 1003.1-2017 (POSIX.1), volume XCU, utilitario `pax`, secao
//! «EXTENDED DESCRIPTION»:
//!
//! * **«ustar Interchange Format»** -- o cabecalho de 512 bytes, os campos
//!   numericos em octal, o `chksum` e o nome partido em `prefix` + `name`.
//! * **«pax Interchange Format»** -- o cabecalho estendido (`typeflag` `x` para
//!   a proxima entrada, `g` global), registros `"%d %s=%s\n"` em que o
//!   comprimento conta o registro inteiro, e as chaves `path`, `linkpath`,
//!   `size` e `mtime`, que valem POR CIMA do campo do ustar.
//!
//! O que se le alem da norma, e por que: o `typeflag` `L`/`K` do GNU tar (nome
//! e alvo longos numa entrada falsa `././@LongLink`), a assinatura antiga
//! `"ustar  \0"` e o numero em base 256 (bit alto no primeiro byte) -- e o que
//! o `tar` do GNU grava no formato padrao dele, e um leitor de tar que nao abre
//! o tar mais comum do Linux nao serve. Escrever, so a norma.
//!
//! # O que o leitor recusa
//!
//! Igual ao leitor do 7z: TODO nome passa por [`conferir_nome`] e o arquivo
//! com um nome perigoso ou repetido nao abre -- em vez de abrir e confiar que
//! cada extrator lembre de pular a entrada ruim. Link e informacao: o leitor o
//! lista com o alvo, e [`extrair_em`] recusa o arquivo inteiro antes de gravar
//! o primeiro byte.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::erro::Erro;
use crate::leitor::{chave_de_colisao, conferir_nome, e_a_raiz};

/// O tamanho do bloco do tar: cabecalho e dados andam em multiplos dele.
pub const BLOCO: usize = 512;

/// O maior numero que cabe num campo octal de 12 bytes (11 digitos + NUL):
/// `size` e `mtime`. Acima disso o escritor manda o valor num registro pax.
const OCTAL_11: u64 = 0o777_7777_7777;

/// O que uma entrada do tar e (`typeflag`, ustar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoTar {
    /// `'0'`, NUL (o ustar antigo) ou `'7'` (contiguo, que a norma manda ler
    /// como regular).
    Arquivo,
    /// `'5'`.
    Pasta,
    /// `'2'`. O alvo vem em [`EntradaTar::alvo`].
    LinkSimbolico,
    /// `'1'`. O alvo vem em [`EntradaTar::alvo`].
    LinkFisico,
    /// Dispositivo (`'3'`, `'4'`), FIFO (`'6'`) ou qualquer outro byte. Os
    /// dados que o `size` declara sao pulados, e o extrator recusa.
    Outro(u8),
}

impl TipoTar {
    fn de(byte: u8) -> TipoTar {
        match byte {
            b'0' | 0 | b'7' => TipoTar::Arquivo,
            b'5' => TipoTar::Pasta,
            b'2' => TipoTar::LinkSimbolico,
            b'1' => TipoTar::LinkFisico,
            outro => TipoTar::Outro(outro),
        }
    }

    /// A norma: link e pasta nao tem registro de dados, seja o que for que o
    /// `size` diga (ustar, descricao do `typeflag`).
    fn tem_dados(self) -> bool {
        !matches!(
            self,
            TipoTar::Pasta | TipoTar::LinkSimbolico | TipoTar::LinkFisico
        )
    }

    /// Como a recusa do extrator nomeia o tipo.
    pub fn descricao(self) -> String {
        match self {
            TipoTar::Arquivo => String::from("arquivo"),
            TipoTar::Pasta => String::from("pasta"),
            TipoTar::LinkSimbolico => String::from("link simbolico"),
            TipoTar::LinkFisico => String::from("link fisico"),
            TipoTar::Outro(b) if b.is_ascii_graphic() => format!("tipo {:?}", b as char),
            TipoTar::Outro(b) => format!("tipo 0x{b:02x}"),
        }
    }
}

/// Uma entrada lida, com o conteudo emprestado da fatia de entrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntradaTar<'a> {
    /// Caminho relativo com `/`, ja conferido por [`conferir_nome`] e sem a
    /// barra final de pasta.
    pub nome: String,
    pub tipo: TipoTar,
    /// O alvo de um link (`linkname` ou `linkpath`). Informacao, nunca ordem.
    pub alvo: Option<String>,
    /// O `mode` gravado. Informacao: o extrator nao o aplica.
    pub modo: u32,
    /// Segundos Unix (`mtime`, ou o `mtime` do pax, arredondado para baixo).
    pub modificado: i64,
    pub conteudo: &'a [u8],
}

// ---------------------------------------------------------------- leitura

fn tar(onde: &str) -> Erro {
    Erro::Tar(String::from(onde))
}

/// Um campo de texto do cabecalho: ate o primeiro NUL.
fn campo_texto(h: &[u8], ini: usize, tam: usize) -> &[u8] {
    let c = &h[ini..ini + tam];
    let fim = c.iter().position(|&b| b == 0).unwrap_or(tam);
    &c[..fim]
}

fn utf8(bytes: &[u8], oque: &str) -> Result<String, Erro> {
    core::str::from_utf8(bytes).map(String::from).map_err(|_| {
        Erro::Tar(format!(
            "{oque} nao e UTF-8; o PhxZip nao adivinha a codificacao"
        ))
    })
}

/// Um campo numerico (ustar: octal, com espacos ou zeros a esquerda, fechado
/// por espaco ou NUL). Bit alto no primeiro byte e a base 256 do GNU tar, so
/// positiva: um tamanho negativo nao tem leitura honesta.
fn campo_numero(h: &[u8], ini: usize, tam: usize, oque: &str) -> Result<u64, Erro> {
    let c = &h[ini..ini + tam];
    if c[0] & 0x80 != 0 {
        if c[0] & 0x40 != 0 {
            return Err(Erro::Tar(format!("{oque} negativo em base 256")));
        }
        let mut v: u64 = (c[0] & 0x3F) as u64;
        for &b in &c[1..] {
            v = v
                .checked_mul(256)
                .map(|v| v | b as u64)
                .ok_or_else(|| Erro::Tar(format!("{oque} nao cabe em 64 bits")))?;
        }
        return Ok(v);
    }
    let mut i = 0;
    while i < tam && c[i] == b' ' {
        i += 1;
    }
    let mut v: u64 = 0;
    let mut digitos = 0;
    while i < tam && (b'0'..=b'7').contains(&c[i]) {
        v = (v << 3) | (c[i] - b'0') as u64;
        digitos += 1;
        i += 1;
    }
    // Depois dos digitos so pode vir o fecho. Campo todo vazio (sem digito) e
    // zero -- e o que varios tar gravam em `devmajor` e companhia.
    if c[i..].iter().any(|&b| b != b' ' && b != 0) {
        return Err(Erro::Tar(format!("{oque} nao e octal")));
    }
    if digitos > 22 {
        return Err(Erro::Tar(format!("{oque} nao cabe em 64 bits")));
    }
    Ok(v)
}

/// A soma do cabecalho com o `chksum` (148..156) contado como oito espacos.
/// Devolve a sem sinal (a da norma) e a com sinal (a de tar historico).
fn somas(h: &[u8]) -> (u64, i64) {
    let mut sem = 0u64;
    let mut com = 0i64;
    for (i, &b) in h[..BLOCO].iter().enumerate() {
        let b = if (148..156).contains(&i) { b' ' } else { b };
        sem += b as u64;
        com += b as i8 as i64;
    }
    (sem, com)
}

/// O que um cabecalho pax (ou o `L`/`K` do GNU) muda na entrada seguinte.
#[derive(Default, Clone)]
struct Sobrepoe {
    caminho: Option<String>,
    alvo: Option<String>,
    tamanho: Option<u64>,
    modificado: Option<i64>,
}

impl Sobrepoe {
    /// `self` por cima de `base`: o da entrada vence o global.
    fn sobre(&self, base: &Sobrepoe) -> Sobrepoe {
        Sobrepoe {
            caminho: self.caminho.clone().or_else(|| base.caminho.clone()),
            alvo: self.alvo.clone().or_else(|| base.alvo.clone()),
            tamanho: self.tamanho.or(base.tamanho),
            modificado: self.modificado.or(base.modificado),
        }
    }
}

/// `mtime` do pax: decimal com sinal e fracao opcionais. Arredonda para baixo.
fn mtime_pax(v: &str) -> Result<i64, Erro> {
    let ruim = || tar("mtime do pax nao e numero");
    let (neg, resto) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v),
    };
    let (inteiro, fracao) = match resto.split_once('.') {
        Some((i, f)) => (i, f),
        None => (resto, ""),
    };
    if inteiro.is_empty()
        || !inteiro.bytes().all(|b| b.is_ascii_digit())
        || !fracao.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ruim());
    }
    let n: i64 = inteiro.parse().map_err(|_| ruim())?;
    let tem_fracao = fracao.bytes().any(|b| b != b'0');
    Ok(if neg { -n - i64::from(tem_fracao) } else { n })
}

/// Os registros de um cabecalho pax: `"%d %s=%s\n"`, em que o `%d` conta o
/// registro inteiro, ele mesmo incluido (pax Interchange Format, «pax Extended
/// Header»). Valor vazio APAGA a chave -- devolvido como `Some("")`, e quem
/// aplica decide o que apagar quer dizer.
fn registros_pax(mut d: &[u8], s: &mut Sobrepoe) -> Result<(), Erro> {
    while !d.is_empty() {
        let espaco = d
            .iter()
            .position(|&b| b == b' ')
            .ok_or_else(|| tar("registro pax sem o comprimento"))?;
        let comp = core::str::from_utf8(&d[..espaco])
            .ok()
            .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|t| t.parse::<usize>().ok())
            .ok_or_else(|| tar("comprimento do registro pax nao e decimal"))?;
        if comp <= espaco + 1 || comp > d.len() || d[comp - 1] != b'\n' {
            return Err(tar("registro pax com o comprimento errado"));
        }
        let corpo = &d[espaco + 1..comp - 1];
        let igual = corpo
            .iter()
            .position(|&b| b == b'=')
            .ok_or_else(|| tar("registro pax sem '='"))?;
        let chave = &corpo[..igual];
        let valor = &corpo[igual + 1..];
        match chave {
            b"path" => s.caminho = Some(utf8(valor, "path do pax")?),
            b"linkpath" => s.alvo = Some(utf8(valor, "linkpath do pax")?),
            b"size" => {
                s.tamanho = if valor.is_empty() {
                    None
                } else {
                    Some(
                        core::str::from_utf8(valor)
                            .ok()
                            .filter(|t| t.bytes().all(|b| b.is_ascii_digit()))
                            .and_then(|t| t.parse().ok())
                            .ok_or_else(|| tar("size do pax nao e decimal"))?,
                    )
                }
            }
            b"mtime" => {
                s.modificado = if valor.is_empty() {
                    None
                } else {
                    Some(mtime_pax(&utf8(valor, "mtime do pax")?)?)
                }
            }
            // atime, uid, uname, charset, comment, SCHILY.*, LIBARCHIVE.*:
            // nada disso vira dado nem caminho, e a norma manda ignorar a
            // chave que nao se conhece.
            _ => {}
        }
        d = &d[comp..];
    }
    // Vazio quer dizer «use o campo do ustar»: o mesmo que nao ter vindo.
    if s.caminho.as_deref() == Some("") {
        s.caminho = None;
    }
    if s.alvo.as_deref() == Some("") {
        s.alvo = None;
    }
    Ok(())
}

/// Le o `.tar` inteiro. Toda entrada sai com o nome conferido; o arquivo com
/// nome perigoso, nome repetido, soma errada ou dado cortado nao abre.
pub fn ler(dados: &[u8]) -> Result<Vec<EntradaTar<'_>>, Erro> {
    let mut entradas = Vec::new();
    let mut vistos = BTreeSet::new();
    let mut global = Sobrepoe::default();
    let mut proxima = Sobrepoe::default();
    let mut pos = 0usize;
    loop {
        let h = dados
            .get(pos..pos + BLOCO)
            .ok_or_else(|| tar("acabou sem o fim do arquivo (o bloco zerado)"))?;
        // O fim sao dois blocos zerados; o GNU tar so avisa quando vem um, e o
        // que vem depois e enchimento do registro (10.240 bytes).
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let magica = &h[257..265];
        let posix = magica == b"ustar\x0000";
        if !posix && magica != b"ustar  \x00" {
            return Err(tar("cabecalho sem a assinatura ustar"));
        }
        let gravada = campo_numero(h, 148, 8, "chksum")?;
        let (sem, com) = somas(h);
        if gravada != sem && gravada as i64 != com {
            return Err(Erro::Corrompido(format!(
                "a soma do cabecalho do tar no byte {pos} nao bate"
            )));
        }
        let byte_tipo = h[156];
        let tipo = TipoTar::de(byte_tipo);
        let mut tamanho = campo_numero(h, 124, 12, "size")?;
        let tem_dados = match byte_tipo {
            b'x' | b'g' | b'L' | b'K' => true,
            _ => tipo.tem_dados(),
        };
        if !matches!(byte_tipo, b'x' | b'g' | b'L' | b'K') {
            if let Some(t) = proxima.sobre(&global).tamanho {
                tamanho = t;
            }
        }
        // Link e pasta com `size` diferente de zero: a norma diz que nao tem
        // dado, o GNU tar pula o `size` declarado, e outros leitores nao --
        // dois leitores vendo arquivos DIFERENTES no mesmo tar e o contrabando
        // (um cabecalho escondido no «dado» do link, revisao SEC da Z9).
        // Recusar e a unica leitura que nao escolhe um dos dois lados.
        if !tem_dados && tamanho != 0 {
            return Err(tar(
                "link ou pasta com size diferente de zero: os leitores de tar divergem \
                 sobre pular esse dado, e e por ai que se esconde uma entrada",
            ));
        }
        let tam_dados = if tem_dados { tamanho } else { 0 };
        let tam_dados = usize::try_from(tam_dados).map_err(|_| Erro::NaoCabe {
            oque: "entrada do tar",
            valor: tam_dados,
        })?;
        let ini = pos + BLOCO;
        let conteudo = ini
            .checked_add(tam_dados)
            .and_then(|fim| dados.get(ini..fim))
            .ok_or_else(|| tar("o dado de uma entrada passa do fim do arquivo"))?;
        let blocos = tam_dados.div_ceil(BLOCO);
        pos = ini + blocos * BLOCO;

        match byte_tipo {
            b'x' => {
                registros_pax(conteudo, &mut proxima)?;
                continue;
            }
            b'g' => {
                registros_pax(conteudo, &mut global)?;
                continue;
            }
            b'L' | b'K' => {
                // GNU: o nome longo e o dado, fechado por NUL.
                let fim = conteudo
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(conteudo.len());
                let texto = utf8(&conteudo[..fim], "nome longo do GNU tar")?;
                if byte_tipo == b'L' {
                    proxima.caminho = Some(texto);
                } else {
                    proxima.alvo = Some(texto);
                }
                continue;
            }
            _ => {}
        }

        let s = proxima.sobre(&global);
        proxima = Sobrepoe::default();
        let nome_bruto = match s.caminho {
            Some(c) => c,
            None => {
                let nome = utf8(campo_texto(h, 0, 100), "nome")?;
                // O `prefix` so existe na assinatura POSIX: no GNU antigo o
                // mesmo lugar guarda atime e ctime.
                let prefixo = if posix {
                    utf8(campo_texto(h, 345, 155), "prefixo")?
                } else {
                    String::new()
                };
                if prefixo.is_empty() {
                    nome
                } else {
                    format!("{prefixo}/{nome}")
                }
            }
        };
        // Tar antigo marca pasta so pela barra final.
        let tipo = if tipo == TipoTar::Arquivo && nome_bruto.ends_with('/') {
            TipoTar::Pasta
        } else {
            tipo
        };
        // A pasta `./` do `tar -C pasta -cf x.tar .` e o proprio destino.
        if tipo == TipoTar::Pasta && e_a_raiz(&nome_bruto) {
            continue;
        }
        let nome = conferir_nome(&nome_bruto)?;
        if !vistos.insert(chave_de_colisao(&nome)) {
            return Err(Erro::NomeRepetido(nome));
        }
        let alvo = match tipo {
            TipoTar::LinkSimbolico | TipoTar::LinkFisico => Some(match s.alvo {
                Some(a) => a,
                None => utf8(campo_texto(h, 157, 100), "alvo do link")?,
            }),
            _ => None,
        };
        let modificado = match s.modificado {
            Some(m) => m,
            None => campo_numero(h, 136, 12, "mtime")? as i64,
        };
        let modo = campo_numero(h, 100, 8, "mode")? as u32;
        entradas.push(EntradaTar {
            nome,
            tipo,
            alvo,
            modo,
            modificado,
            conteudo: if tipo == TipoTar::Arquivo || matches!(tipo, TipoTar::Outro(_)) {
                conteudo
            } else {
                &[]
            },
        });
    }
    Ok(entradas)
}

// ---------------------------------------------------------------- escrita

/// Escreve `valor` em octal, com zeros a esquerda, ocupando `tam - 1` digitos
/// e fechando com NUL (ustar: «leading zero-filled octal numbers ... terminated
/// by one or more space or NUL characters»).
fn por_octal(h: &mut [u8], ini: usize, tam: usize, mut valor: u64) {
    let c = &mut h[ini..ini + tam];
    c[tam - 1] = 0;
    for i in (0..tam - 1).rev() {
        c[i] = b'0' + (valor & 7) as u8;
        valor >>= 3;
    }
}

/// O registro pax `"%d chave=valor\n"`, com o comprimento contando a si mesmo.
fn registro_pax(saida: &mut Vec<u8>, chave: &str, valor: &str) {
    let resto = 1 + chave.len() + 1 + valor.len() + 1;
    let mut comp = resto + 1;
    // O numero de digitos do comprimento entra no comprimento: ate fechar.
    loop {
        let total = resto + format!("{comp}").len();
        if total == comp {
            break;
        }
        comp = total;
    }
    saida.extend_from_slice(format!("{comp} {chave}={valor}\n").as_bytes());
}

/// Parte o nome em `prefix` (ate 155) e `name` (ate 100) numa barra, como o
/// ustar preve; `None` quando nao ha barra que sirva.
fn partir(nome: &str) -> Option<(&str, &str)> {
    if nome.len() <= 100 {
        return Some(("", nome));
    }
    nome.match_indices('/').map(|(i, _)| i).find_map(|i| {
        let (p, n) = (&nome[..i], &nome[i + 1..]);
        (p.len() <= 155 && !n.is_empty() && n.len() <= 100).then_some((p, n))
    })
}

/// O maior prefixo de `s` com ate `max` bytes que termina numa fronteira de
/// caractere.
fn cortar(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut fim = max;
    while !s.is_char_boundary(fim) {
        fim -= 1;
    }
    &s[..fim]
}

/// Monta um `.tar` (POSIX pax: ustar, com cabecalho `x` so quando o ustar nao
/// alcanca) na memoria.
///
/// O que se grava de proposito, e o que NAO: modo `0644`/`0755`, dono 0 e sem
/// nome de dono -- o tar leva dado e caminho, nao permissao nem identidade de
/// quem gravou, pela mesma razao por que o extrator nao aplica `mode`.
pub struct EscritorTar {
    saida: Vec<u8>,
    nomes: BTreeSet<String>,
}

impl Default for EscritorTar {
    fn default() -> EscritorTar {
        EscritorTar::novo()
    }
}

impl EscritorTar {
    pub fn novo() -> EscritorTar {
        EscritorTar {
            saida: Vec::new(),
            nomes: BTreeSet::new(),
        }
    }

    /// Acrescenta um arquivo. `modificado` em segundos Unix.
    pub fn arquivo(&mut self, nome: &str, conteudo: &[u8], modificado: i64) -> Result<(), Erro> {
        self.acrescentar(nome, TipoTar::Arquivo, conteudo, modificado)
    }

    /// Acrescenta uma pasta.
    pub fn pasta(&mut self, nome: &str, modificado: i64) -> Result<(), Erro> {
        self.acrescentar(nome, TipoTar::Pasta, &[], modificado)
    }

    fn acrescentar(
        &mut self,
        nome: &str,
        tipo: TipoTar,
        conteudo: &[u8],
        modificado: i64,
    ) -> Result<(), Erro> {
        // A mesma conferencia do leitor: o escritor nao grava o que o leitor
        // recusaria abrir.
        let nome = conferir_nome(nome)?;
        if !self.nomes.insert(chave_de_colisao(&nome)) {
            return Err(Erro::NomeRepetido(nome));
        }
        let caminho = if tipo == TipoTar::Pasta {
            format!("{nome}/")
        } else {
            nome
        };
        let tamanho = conteudo.len() as u64;
        let mtime_cabe = (0..=OCTAL_11 as i64).contains(&modificado);

        // O ustar so carrega ASCII de forma portavel (a norma restringe o
        // nome ao conjunto portavel); fora dele, e fora do tamanho, vai pax.
        let partido = if caminho.is_ascii() {
            partir(&caminho)
        } else {
            None
        };
        let mut pax = Vec::new();
        if partido.is_none() {
            registro_pax(&mut pax, "path", &caminho);
        }
        if tamanho > OCTAL_11 {
            registro_pax(&mut pax, "size", &format!("{tamanho}"));
        }
        if !mtime_cabe {
            registro_pax(&mut pax, "mtime", &format!("{modificado}"));
        }
        let mtime_ustar = if mtime_cabe { modificado as u64 } else { 0 };

        if !pax.is_empty() {
            // O nome do cabecalho `x` nao e lido por quem entende pax; quem
            // nao entende o extrai como arquivo comum, e por isso ele mora numa
            // pasta propria e nunca no lugar da entrada.
            let base = caminho.trim_end_matches('/');
            let base = base.rsplit('/').next().unwrap_or(base);
            let base: String = base
                .chars()
                .filter(|c| c.is_ascii_graphic())
                .take(80)
                .collect();
            let nome_x = format!("PaxHeaders/{base}");
            self.cabecalho(("", &nome_x), b'x', &pax, 0o644, mtime_ustar);
            self.dados(&pax);
        }
        let (prefixo, curto) = match partido {
            Some(p) => p,
            None => ("", cortar(&caminho, 100)),
        };
        let (byte, modo) = match tipo {
            TipoTar::Pasta => (b'5', 0o755),
            _ => (b'0', 0o644),
        };
        self.cabecalho((prefixo, curto), byte, conteudo, modo, mtime_ustar);
        self.dados(conteudo);
        Ok(())
    }

    fn cabecalho(
        &mut self,
        (prefixo, nome): (&str, &str),
        tipo: u8,
        dados: &[u8],
        modo: u32,
        mtime: u64,
    ) {
        let mut h = [0u8; BLOCO];
        h[..nome.len()].copy_from_slice(nome.as_bytes());
        por_octal(&mut h, 100, 8, modo as u64);
        por_octal(&mut h, 108, 8, 0);
        por_octal(&mut h, 116, 8, 0);
        let tam = dados.len() as u64;
        por_octal(&mut h, 124, 12, if tam > OCTAL_11 { 0 } else { tam });
        por_octal(&mut h, 136, 12, mtime);
        h[156] = tipo;
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        por_octal(&mut h, 329, 8, 0);
        por_octal(&mut h, 337, 8, 0);
        h[345..345 + prefixo.len()].copy_from_slice(prefixo.as_bytes());
        // `chksum`: seis digitos octais, NUL e espaco -- a forma que todo tar
        // grava, somada com o proprio campo como oito espacos.
        let (soma, _) = somas(&h);
        por_octal(&mut h, 148, 7, soma);
        h[155] = b' ';
        self.saida.extend_from_slice(&h);
    }

    fn dados(&mut self, d: &[u8]) {
        self.saida.extend_from_slice(d);
        let resto = d.len() % BLOCO;
        if resto != 0 {
            self.saida.resize(self.saida.len() + BLOCO - resto, 0);
        }
    }

    /// Fecha com os dois blocos zerados e devolve os bytes.
    pub fn terminar(mut self) -> Vec<u8> {
        self.saida.resize(self.saida.len() + 2 * BLOCO, 0);
        self.saida
    }
}

// --------------------------------------------------------------- extracao

/// Extrai o `.tar` inteiro em `raiz` e devolve quantas entradas gravou.
///
/// Recusa o arquivo INTEIRO antes de gravar o primeiro byte se houver link,
/// dispositivo, FIFO ou tipo desconhecido ([`Erro::EntradaEspecial`]) -- meio
/// arquivo extraido e pior que nenhum, porque quem olha a pasta acha que
/// acabou. O resto da protecao e do [`crate::disco::Destino`]: nada segue
/// link que ja esteja no destino e nada sobrescreve.
#[cfg(feature = "std")]
pub fn extrair_em(dados: &[u8], raiz: &std::path::Path) -> Result<usize, Erro> {
    let entradas = ler(dados)?;
    if let Some(e) = entradas
        .iter()
        .find(|e| !matches!(e.tipo, TipoTar::Arquivo | TipoTar::Pasta))
    {
        return Err(Erro::EntradaEspecial {
            nome: e.nome.clone(),
            tipo: e.tipo.descricao(),
        });
    }
    crate::disco::Destino::conferir_lote(
        entradas
            .iter()
            .map(|e| (e.nome.as_str(), matches!(e.tipo, TipoTar::Pasta))),
    )?;
    let destino = crate::disco::Destino::novo(raiz)?;
    for e in &entradas {
        match e.tipo {
            TipoTar::Pasta => destino.pasta(&e.nome)?,
            _ => destino.arquivo(&e.nome, e.conteudo, Some(e.modificado))?,
        }
    }
    Ok(entradas.len())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn registro_pax_conta_a_si_mesmo() {
        for valor in ["", "a", "abcdef", &"x".repeat(90), &"y".repeat(995)] {
            let mut v = Vec::new();
            registro_pax(&mut v, "path", valor);
            let texto = core::str::from_utf8(&v).unwrap();
            let (n, _) = texto.split_once(' ').unwrap();
            assert_eq!(n.parse::<usize>().unwrap(), v.len(), "{valor:?}");
        }
    }

    #[test]
    fn octal_e_base_256() {
        let mut h = [0u8; 12];
        por_octal(&mut h, 0, 12, 0o1234);
        assert_eq!(&h, b"00000001234\0");
        assert_eq!(campo_numero(&h, 0, 12, "x").unwrap(), 0o1234);
        assert_eq!(campo_numero(b"  644 \0\0", 0, 8, "x").unwrap(), 0o644);
        assert_eq!(campo_numero(&[0; 8], 0, 8, "x").unwrap(), 0);
        let mut b256 = [0u8; 12];
        b256[0] = 0x80;
        b256[11] = 0x01;
        b256[10] = 0x02;
        assert_eq!(campo_numero(&b256, 0, 12, "x").unwrap(), 0x0201);
        assert!(campo_numero(b"12a4\0\0\0\0", 0, 8, "x").is_err());
        assert!(campo_numero(b"128\0\0\0\0\0", 0, 8, "x").is_err());
    }

    #[test]
    fn mtime_do_pax_arredonda_para_baixo() {
        assert_eq!(mtime_pax("1790000000").unwrap(), 1_790_000_000);
        assert_eq!(mtime_pax("1790000000.999").unwrap(), 1_790_000_000);
        assert_eq!(mtime_pax("-5.5").unwrap(), -6);
        assert_eq!(mtime_pax("-5.000").unwrap(), -5);
        assert!(mtime_pax("1e9").is_err());
        assert!(mtime_pax(".5").is_err());
    }
}
