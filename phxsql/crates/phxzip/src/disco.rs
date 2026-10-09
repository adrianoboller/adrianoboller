//! Gravar entradas no disco sem seguir link e sem sair da raiz.
//!
//! E o UNICO lugar da crate que toca o sistema de arquivos, e so existe com o
//! recurso `std`. Um modulo e nao uma funcao do tar porque a mesma pergunta --
//! «posso gravar este nome debaixo desta raiz?» -- e a do PhxZipCmd extraindo
//! um 7z (fatia Z9): duas respostas escritas em dois lugares divergem, e a que
//! alguem esquecer e a porta (petrea «funcao e comando vem do mesmo motor»).
//!
//! # As defesas, e o que cada uma fecha
//!
//! 1. **O nome** passa de novo por [`conferir_nome`], que o devolve CANONICO
//!    (sem `.`, sem vazio): absoluto, `..`, letra de unidade e dispositivo do
//!    Windows nao chegam ao `join`. Quem chama ja conferiu; conferir de novo
//!    custa um laco sobre o nome e fecha o chamador que esqueceu.
//! 2. **O nome, pela PLATAFORMA**: cada componente tem de ser um
//!    `Component::Normal` igual ao proprio texto. O [`conferir_nome`] decide
//!    por texto; o `Path` e o que o disco vai obedecer -- o zip-slip so passa
//!    se os dois errarem juntos.
//! 3. **Cada pasta do caminho** e olhada com `symlink_metadata`, que NAO segue
//!    link: um `sub -> /etc` plantado no destino e recusado em vez de
//!    atravessado. A pasta que falta e criada com `create_dir` (modo `0755`,
//!    estreitado pelo `umask`), que falha se algo apareceu no lugar.
//! 4. **O arquivo** abre com `create_new` (`O_CREAT|O_EXCL`), que falha se o
//!    nome ja existe -- inclusive como link pendurado: nada se grava atraves
//!    de link e nada se sobrescreve calado. Sobrescrever e PEDIDO
//!    ([`Destino::sobrescrevendo`]), e o que se apaga antes e o NOME -- o
//!    arquivo, ou o link ele mesmo --, nunca o alvo; a abertura continua
//!    `create_new`.
//! 5. **A colisao arquivo/pasta** (`a` arquivo e `a/b` no mesmo lote) recusa o
//!    lote inteiro ANTES do primeiro byte ([`Destino::conferir_lote`]).
//! 6. **Quem mais escreve no caminho** (Unix): ver «A corrida» abaixo.
//!
//! **Link nunca nasce aqui.** Nao ha caminho neste modulo que crie link: o 7z
//! que traz o modo Unix de link nos atributos ([`marcado_como_link`]) vira
//! arquivo comum com aqueles bytes. O aviso disso e de tela, e fica com quem
//! chama.
//!
//! # A corrida (TOCTOU), e por que ela e fechada pelo DONO do caminho
//!
//! Entre olhar uma pasta e criar dentro dela, quem pode escrever no caminho
//! troca a pasta por um link -- e o `create_new` segue link nos componentes do
//! MEIO. A revisao SEC da Z9 MEDIU que isso e exploravel, e nao teorico: com a
//! troca atomica (`renameat2(RENAME_EXCHANGE)`), 5 de 9 extracoes escreveram
//! fora do destino. A frase que estava aqui («o atacante que escreve nele ja
//! tem o que queria») era falsa: ele pode ter escrita SO no destino e querer
//! escrever fora dele, com o uid de quem extrai.
//!
//! Fechar por componente pede `openat` com `O_NOFOLLOW`, que a `std` nao
//! expoe sem `unsafe` nem crate. As duas saidas so com `std` eram:
//!
//! * **(a) extrair numa pasta `0700` propria e mover no fim** -- fecha a
//!   corrida DENTRO da pasta propria, mas o mover para o destino e uma segunda
//!   descida pelo mesmo caminho, com a mesma janela; e juntar com pasta que ja
//!   existe no destino (o caso comum de extrair em `.`) volta a precisar dela.
//!   Morreu: muda a janela de lugar em vez de fecha-la.
//! * **(b) recusar o caminho onde OUTRO usuario pode escrever** -- a corrida
//!   so existe para quem pode trocar um componente, e quem pode trocar e o
//!   dono da pasta-mae ou quem tem escrita nela. Se toda pasta do caminho e de
//!   quem extrai (ou do root), sem escrita para outros (grupo so o proprio), o
//!   unico que troca e quem extrai -- e esse nao precisa do PhxZip para
//!   escrever onde quiser. **Esta e a escolhida.**
//!
//! A regra, conferida no [`Destino::novo`] (os ancestrais e a raiz) e a cada
//! pasta que ja existia ao descer: dono = quem extrai ou root; sem escrita
//! para outros; escrita de grupo so com o grupo de quem extrai. Ancestral com
//! escrita para todos e aceito so com o bit pegajoso (`/tmp`): ali ninguem
//! renomeia o que nao e seu. Quem extrai e medido criando e apagando uma
//! pasta-sonda na raiz -- a `std` nao tem `geteuid`, e o dono do que o
//! proprio processo cria e o uid efetivo por definicao.
//!
//! O que ainda NAO se fecha, dito: no Windows nao ha esta conferencia (a
//! `std` nao expoe dono nem ACL), e o mesmo uid correndo contra si mesmo nao e
//! fronteira de nada.
//!
//! O `mode` da entrada NAO e aplicado: o arquivo nasce com o `umask` de quem
//! extrai (ver o topo da crate: atributo e informacao, nao ordem), ou `0600`
//! quando quem chama pede [`Destino::arquivo_privado`] -- o conteudo que veio
//! cifrado nao fica legivel para o grupo so porque saiu do 7z.

use std::collections::BTreeSet;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::erro::Erro;
use crate::leitor::conferir_nome;

/// `FILE_ATTRIBUTE_UNIX_EXTENSION` do 7-Zip: os 16 bits altos sao o `st_mode`.
const ATRIBUTO_UNIX: u32 = 0x8000;
const MODO_TIPO: u32 = 0o170_000;
const MODO_LINK: u32 = 0o120_000;

/// Se os atributos de uma entrada 7z carregam o modo Unix de LINK SIMBOLICO.
/// Informacao para a lista e para o aviso -- nunca ordem: o [`Destino`] grava
/// arquivo comum de qualquer jeito.
pub fn marcado_como_link(atributos: Option<u32>) -> bool {
    match atributos {
        Some(a) if a & ATRIBUTO_UNIX != 0 => (a >> 16) & MODO_TIPO == MODO_LINK,
        _ => false,
    }
}

/// O caminho vai nas mensagens com `{:?}`: um nome com `ESC [` vindo do
/// arquivo reescreveria o terminal de quem le o erro.
fn disco(e: std::io::Error, onde: &Path) -> Erro {
    Erro::Disco(format!("{onde:?}: {e}"))
}

fn inseguro(texto: String) -> Erro {
    Erro::DestinoInseguro(texto)
}

/// Quem extrai, no Unix: o uid e o gid efetivos.
#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct Quem {
    uid: u32,
    gid: u32,
}

#[cfg(unix)]
mod dono {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    /// Cria e apaga uma pasta-sonda na raiz: o dono dela e o uid efetivo.
    pub(super) fn sondar(raiz: &Path) -> Result<Quem, Erro> {
        let marca = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let sonda = raiz.join(format!(".phxzip-sonda-{}-{marca}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&sonda)
            .map_err(|e| disco(e, raiz))?;
        let m = fs::symlink_metadata(&sonda);
        let _ = fs::remove_dir(&sonda);
        let m = m.map_err(|e| disco(e, &sonda))?;
        Ok(Quem {
            uid: m.uid(),
            gid: m.gid(),
        })
    }

    /// A regra da corrida (ver o topo). `ancestral`: acima da raiz, onde o
    /// bit pegajoso basta.
    pub(super) fn conferir(
        m: &fs::Metadata,
        quem: Quem,
        onde: &Path,
        ancestral: bool,
    ) -> Result<(), Erro> {
        if m.uid() != quem.uid && m.uid() != 0 {
            return Err(inseguro(format!(
                "{onde:?} e de outro usuario (uid {}): ele poderia trocar uma pasta por \
                 um link no meio da extracao",
                m.uid()
            )));
        }
        let modo = m.mode();
        let outros = modo & 0o002 != 0;
        let grupo_alheio = modo & 0o020 != 0 && m.gid() != quem.gid;
        let pegajoso = modo & 0o1000 != 0;
        if (outros || grupo_alheio) && !(ancestral && pegajoso) {
            return Err(inseguro(format!(
                "{onde:?} aceita escrita de outros (modo {:o}): outro usuario poderia \
                 trocar uma pasta por um link no meio da extracao",
                modo & 0o7777
            )));
        }
        Ok(())
    }

    pub(super) fn criar_pasta(caminho: &Path) -> std::io::Result<()> {
        fs::DirBuilder::new().mode(0o755).create(caminho)
    }

    pub(super) fn abrir_novo(caminho: &Path, privado: bool) -> std::io::Result<fs::File> {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(if privado { 0o600 } else { 0o666 })
            .open(caminho)
    }
}

#[cfg(not(unix))]
mod dono {
    use super::*;

    pub(super) fn criar_pasta(caminho: &Path) -> std::io::Result<()> {
        fs::create_dir(caminho)
    }

    pub(super) fn abrir_novo(caminho: &Path, _privado: bool) -> std::io::Result<fs::File> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(caminho)
    }
}

/// Uma raiz de extracao.
pub struct Destino {
    raiz: PathBuf,
    sobrescrever: bool,
    #[cfg(unix)]
    quem: Quem,
}

impl Destino {
    /// A raiz tem de existir e ser pasta. Ela mesma pode ser link: foi quem
    /// chama que a escolheu, e o que se protege e o que vem DO ARQUIVO -- por
    /// isso ela e resolvida UMA vez aqui, e o resto anda sobre o caminho
    /// resolvido. No Unix, a raiz e cada ancestral passam pela regra da
    /// corrida (ver o topo).
    pub fn novo(raiz: &Path) -> Result<Destino, Erro> {
        let raiz = fs::canonicalize(raiz).map_err(|e| disco(e, raiz))?;
        let m = fs::metadata(&raiz).map_err(|e| disco(e, &raiz))?;
        if !m.is_dir() {
            return Err(inseguro(format!("{raiz:?} nao e pasta")));
        }
        #[cfg(unix)]
        let quem = {
            let quem = dono::sondar(&raiz)?;
            dono::conferir(&m, quem, &raiz, false)?;
            for acima in raiz.ancestors().skip(1) {
                let ma = fs::symlink_metadata(acima).map_err(|e| disco(e, acima))?;
                dono::conferir(&ma, quem, acima, true)?;
            }
            quem
        };
        Ok(Destino {
            raiz,
            sobrescrever: false,
            #[cfg(unix)]
            quem,
        })
    }

    /// Arquivo que ja existe no lugar e apagado antes de gravar -- o NOME, que
    /// num link e o link, nunca o alvo. Pasta no lugar continua recusando.
    pub fn sobrescrevendo(mut self) -> Destino {
        self.sobrescrever = true;
        self
    }

    /// Os componentes do nome canonico, cada um conferido tambem pelo `Path`
    /// da plataforma.
    fn componentes(nome: &str) -> Result<Vec<String>, Erro> {
        let conferido = conferir_nome(nome)?;
        let mut v = Vec::new();
        for c in conferido.split('/') {
            let mut partes = Path::new(c).components();
            match (partes.next(), partes.next()) {
                (Some(Component::Normal(x)), None) if x == c => v.push(c.to_string()),
                _ => return Err(Erro::NomePerigoso(String::from(nome))),
            }
        }
        Ok(v)
    }

    /// Confere um lote inteiro ANTES de gravar: cada nome (as duas
    /// conferencias acima) e a colisao entre arquivo e pasta -- `a` arquivo e
    /// `a/b`, sem diferenca de caixa, porque o disco do Windows e do macOS nao
    /// faz. `entradas` e `(nome, e_pasta)`.
    pub fn conferir_lote<'a, I>(entradas: I) -> Result<(), Erro>
    where
        I: IntoIterator<Item = (&'a str, bool)>,
    {
        let mut planos = Vec::new();
        let mut arquivos = BTreeSet::new();
        for (nome, pasta) in entradas {
            let comps = Destino::componentes(nome)?;
            if !pasta {
                arquivos.insert(comps.join("/").to_lowercase());
            }
            planos.push(comps);
        }
        for comps in &planos {
            for fim in 1..comps.len() {
                let acima = comps[..fim].join("/");
                if arquivos.contains(&acima.to_lowercase()) {
                    return Err(inseguro(format!(
                        "{acima:?} e arquivo e pasta ao mesmo tempo no lote; nada foi gravado"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Anda (e cria) as pastas, recusando o que nao e pasta de verdade -- e,
    /// no Unix, a pasta que ja existia onde outro usuario escreve.
    fn pastas(&self, comps: &[String]) -> Result<PathBuf, Erro> {
        let mut atual = self.raiz.clone();
        for c in comps {
            atual.push(c);
            match fs::symlink_metadata(&atual) {
                Ok(m) if m.file_type().is_dir() => {
                    #[cfg(unix)]
                    dono::conferir(&m, self.quem, &atual, false)?;
                }
                Ok(m) => {
                    let o_que = if m.file_type().is_symlink() {
                        "um link"
                    } else {
                        "um arquivo"
                    };
                    return Err(inseguro(format!(
                        "{atual:?} ja existe e e {o_que}, nao pasta"
                    )));
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    dono::criar_pasta(&atual).map_err(|e| disco(e, &atual))?;
                }
                Err(e) => return Err(disco(e, &atual)),
            }
        }
        Ok(atual)
    }

    /// Cria a pasta (e as de cima). Pasta que ja existe como pasta e aceita.
    pub fn pasta(&self, nome: &str) -> Result<(), Erro> {
        let comps = Destino::componentes(nome)?;
        self.pastas(&comps).map(|_| ())
    }

    /// Com [`Destino::sobrescrevendo`], tira do caminho o que estiver no nome
    /// final -- o arquivo, ou o LINK ele mesmo (`remove_file` nao segue).
    fn liberar(&self, caminho: &Path) -> Result<(), Erro> {
        if !self.sobrescrever {
            return Ok(());
        }
        match fs::symlink_metadata(caminho) {
            Ok(m) if m.file_type().is_dir() => {
                Err(inseguro(format!("{caminho:?} ja existe como pasta")))
            }
            Ok(_) => fs::remove_file(caminho).map_err(|e| disco(e, caminho)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(disco(e, caminho)),
        }
    }

    /// Grava um arquivo novo. `modificado` em segundos Unix.
    pub fn arquivo(
        &self,
        nome: &str,
        conteudo: &[u8],
        modificado: Option<i64>,
    ) -> Result<(), Erro> {
        self.gravar(nome, conteudo, modificado, false)
    }

    /// Como [`Destino::arquivo`], nascendo `0600` no Unix: para o conteudo que
    /// veio cifrado, que nao deve ficar legivel ao grupo so por ter saido.
    pub fn arquivo_privado(
        &self,
        nome: &str,
        conteudo: &[u8],
        modificado: Option<i64>,
    ) -> Result<(), Erro> {
        self.gravar(nome, conteudo, modificado, true)
    }

    fn gravar(
        &self,
        nome: &str,
        conteudo: &[u8],
        modificado: Option<i64>,
        privado: bool,
    ) -> Result<(), Erro> {
        let comps = Destino::componentes(nome)?;
        let Some((ultimo, acima)) = comps.split_last() else {
            return Err(Erro::NomePerigoso(String::from(nome)));
        };
        let mut caminho = self.pastas(acima)?;
        caminho.push(ultimo);
        self.liberar(&caminho)?;
        // `create_new` mesmo depois de liberar: se um link aparecer no nome
        // entre o apagar e o abrir, a abertura falha em vez de segui-lo.
        let mut f = dono::abrir_novo(&caminho, privado).map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                inseguro(format!(
                    "{caminho:?} ja existe (arquivo ou link); nada se sobrescreve sem pedir"
                ))
            } else {
                disco(e, &caminho)
            }
        })?;
        f.write_all(conteudo).map_err(|e| disco(e, &caminho))?;
        if let Some(s) = modificado {
            let quando = if s >= 0 {
                UNIX_EPOCH.checked_add(Duration::from_secs(s as u64))
            } else {
                UNIX_EPOCH.checked_sub(Duration::from_secs(s.unsigned_abs()))
            };
            // Data que o sistema nao representa fica com a de agora: e
            // informacao, e recusar o arquivo por ela perderia o dado.
            if let Some(t) = quando {
                let _ = f.set_modified(t);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn bit_de_link_so_vale_com_a_extensao_unix() {
        assert!(marcado_como_link(Some(ATRIBUTO_UNIX | (0o120_777 << 16))));
        assert!(!marcado_como_link(Some(0o120_777 << 16)));
        assert!(!marcado_como_link(Some(ATRIBUTO_UNIX | (0o100_644 << 16))));
        assert!(!marcado_como_link(None));
    }

    #[test]
    fn componente_que_sai_da_raiz_recusa() {
        for ruim in ["../x", "a/../x", "..", "/x", ""] {
            assert!(Destino::componentes(ruim).is_err(), "{ruim:?} passou");
        }
        assert_eq!(
            Destino::componentes("a/b c/./d.txt").unwrap(),
            ["a", "b c", "d.txt"]
        );
    }

    #[test]
    fn colisao_de_arquivo_e_pasta_recusa_o_lote() {
        assert!(Destino::conferir_lote([("a", false), ("a/b", false)]).is_err());
        assert!(Destino::conferir_lote([("A", false), ("a/b", false)]).is_err());
        // O irmao: pasta declarada com filhos e o caso normal, nao colisao.
        assert!(Destino::conferir_lote([("a", true), ("a/b", false), ("c", false)]).is_ok());
    }
}
