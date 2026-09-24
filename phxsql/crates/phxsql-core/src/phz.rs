//! `.phz`: os JSON de configuracao gravados como 7z, com senha no binario.
//!
//! Pedido 450, etapa 2. Decisao do dono, 24/09/2026: *«Os arquivos de
//! config.json serao zipados com senha gravada dentro do binario [...] para
//! evitar o acesso direto aos json de configuracao.»* Extensao `.phz`,
//! formato 7z (o PhxZip, `crates/phxzip`), nomes cifrados.
//!
//! # O que isto E, e o que NAO E
//!
//! E **barreira contra quem abre o arquivo** num editor ou num visualizador.
//! **Nao e cifra** contra quem tem o binario ou o repositorio: a senha esta
//! escrita abaixo, este repositorio e publico, `strings` no binario a entrega,
//! e ela e a mesma em toda instalacao. O integrador apresentou esse custo
//! ANTES e o dono confirmou «como pedido, senha no codigo». Nenhum documento
//! chama isto de cifra -- a mesma regra do `encryption_exigida` (pedido 366).
//! Segredo de verdade continua indo por variavel de ambiente (`_env`, pedido
//! 372), e nao por aqui.
//!
//! **Choque registrado com a petrea** «senha nunca em texto puro, nem em
//! arquivo»: a senha abaixo e a excecao que o dono decidiu, e esta linha
//! cita o pedido 450 para ninguem a copiar como precedente.
//!
//! # O caminho de quem ja tem `config.json` em claro
//!
//! Nada vira `.phz` sozinho. O `.json` em claro continua sendo lido como
//! sempre; a conversao e PEDIDA (`phxsqld --zipar-config`), confere a ida e
//! volta de cada arquivo antes de apagar o claro, e diz o que apagou. E quem
//! le escolhe pelo que existe no disco: `config.json` pedido e so
//! `config.phz` presente abre o `.phz`.

use std::io;
use std::path::{Path, PathBuf};

use phxzip::{Arquivo7z, Escritor, Limites, Opcoes};

/// A senha fixa dos `.phz` -- DECISAO DO DONO (pedido 450), com o custo
/// escrito no cabecalho deste modulo. Nao e segredo e nao protege segredo.
pub const SENHA_FIXA: &str = "PhxSql/config.phz/pedido-450";

/// A extensao.
pub const EXTENSAO: &str = "phz";

/// Teto de um `.phz` de configuracao: 64 MiB descompactados. O maior
/// `config.json` real tem dezenas de KiB; o teto existe para um arquivo
/// hostil nao pedir um GiB ao arranque.
const TETO: u64 = 64 << 20;

/// O caminho tem extensao `.phz`.
pub fn e_phz(caminho: &Path) -> bool {
    caminho
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSAO))
}

fn irmao_phz(caminho: &Path) -> PathBuf {
    caminho.with_extension(EXTENSAO)
}

fn irmao_json(caminho: &Path) -> PathBuf {
    caminho.with_extension("json")
}

/// O arquivo que de fato se le para o caminho pedido.
///
/// Pedido `.phz`: ele. Pedido `.json` que existe: ele -- **a nao ser que o
/// `.phz` irmao tambem exista**, e ai vale o `.phz`: ele so nasce pela
/// conversao pedida, entao e o estado que o operador escolheu por ultimo, e o
/// `.json` ao lado e sobra (quem le avisa, ver [`sobras`]). Pedido `.json`
/// ausente com `.phz` presente: o `.phz`.
pub fn resolver(caminho: &Path) -> PathBuf {
    if e_phz(caminho) {
        return caminho.to_path_buf();
    }
    let phz = irmao_phz(caminho);
    if phz.exists() {
        phz
    } else {
        caminho.to_path_buf()
    }
}

/// O `.json` em claro que sobrou ao lado de um `.phz` -- para o aviso de
/// arranque nomear. `None` quando nao ha sobra.
pub fn sobras(caminho: &Path) -> Option<PathBuf> {
    let json = if e_phz(caminho) {
        irmao_json(caminho)
    } else {
        caminho.to_path_buf()
    };
    (json.exists() && irmao_phz(&json).exists()).then_some(json)
}

/// Onde gravar: no arquivo que ja existe (o formato de cada um se mantem).
/// Se nenhum existe, o arquivo NOVO nasce `.phz` quando a pasta dele ja tem
/// um `.phz` -- senao a primeira ligacao criada pela tela reabriria em claro o
/// que o operador acabou de fechar.
///
/// A regra e pela PASTA, sem estado: a primeira versao guardava «o config e
/// .phz» numa variavel global do processo, e dois `Config` no mesmo processo
/// (os testes em paralelo, e amanha um servidor com dois) desligavam a
/// preferencia um do outro -- achado pelo teste da lista de bloqueio. Um
/// `.phz` so existe na pasta por conversao pedida, entao a pasta que tem um
/// e a pasta que o operador fechou.
pub fn caminho_para_gravar(caminho: &Path) -> PathBuf {
    let lido = resolver(caminho);
    if lido.exists() {
        return lido;
    }
    if !e_phz(caminho) && pasta_fechada(caminho) {
        irmao_phz(caminho)
    } else {
        caminho.to_path_buf()
    }
}

fn pasta_fechada(caminho: &Path) -> bool {
    let pasta = match caminho.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    std::fs::read_dir(pasta)
        .map(|it| it.flatten().any(|e| e_phz(&e.path())))
        .unwrap_or(false)
}

fn erro(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Le o texto do arquivo de configuracao, abrindo o `.phz` se for ele.
/// Ausencia continua sendo `NotFound` -- quem trata «nao existe» como
/// «vazio» (o `dblink.json` de quem nunca criou ligacao) segue igual.
pub fn ler_texto(caminho: &Path) -> io::Result<String> {
    let alvo = resolver(caminho);
    if !e_phz(&alvo) {
        return std::fs::read_to_string(&alvo);
    }
    let bytes = std::fs::read(&alvo)?;
    let a = Arquivo7z::abrir(
        &bytes,
        Some(SENHA_FIXA),
        Limites {
            max_pasta: TETO,
            max_entradas: 16,
            max_cabecalho: 1 << 20,
        },
    )
    .map_err(|e| erro(format!("{}: {e}", alvo.display())))?;
    let arquivos: Vec<usize> = a
        .entradas()
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.e_pasta)
        .map(|(i, _)| i)
        .collect();
    let [i] = arquivos[..] else {
        return Err(erro(format!(
            "{}: um .phz de configuracao guarda UM arquivo, e este guarda {}",
            alvo.display(),
            arquivos.len()
        )));
    };
    let d = a
        .extrair(i)
        .map_err(|e| erro(format!("{}: {e}", alvo.display())))?;
    String::from_utf8(d).map_err(|_| erro(format!("{}: o conteudo nao e UTF-8", alvo.display())))
}

/// Os bytes de um `.phz` com `texto` dentro, sob o nome `nome` (o do `.json`
/// que ele substitui -- quem abrir com o 7-Zip ve `config.json`).
pub fn empacotar(nome: &str, texto: &[u8]) -> io::Result<Vec<u8>> {
    let mut acaso = [0u8; 32];
    crate::cifra::sortear(&mut acaso);
    let mut e = Escritor::novo(Opcoes {
        nivel: 5,
        senha: Some(SENHA_FIXA.to_string()),
        cifrar_nomes: true,
        acaso,
        ..Opcoes::default()
    });
    e.arquivo(nome, texto.to_vec(), None, None)
        .map_err(|e| erro(e.to_string()))?;
    e.gravar().map_err(|e| erro(e.to_string()))
}

/// O nome do `.json` dentro do `.phz` que mora em `caminho`.
pub fn nome_interno(caminho: &Path) -> String {
    irmao_json(caminho)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.json".into())
}

#[cfg(test)]
mod testes {
    use super::*;

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir(nome: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("phz-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    #[test]
    fn ida_e_volta_e_o_nome_dentro_e_o_do_json() {
        let d = dir("ida");
        let phz = d.0.join("config.phz");
        let texto = "{\"token\":\"t\",\"porta\":5433}";
        std::fs::write(
            &phz,
            empacotar(&nome_interno(&phz), texto.as_bytes()).unwrap(),
        )
        .unwrap();
        // Pedido pelo nome de sempre, `config.json`, abre o `.phz`.
        assert_eq!(ler_texto(&d.0.join("config.json")).unwrap(), texto);
        let bytes = std::fs::read(&phz).unwrap();
        let a = Arquivo7z::abrir(&bytes, Some(SENHA_FIXA), Limites::default()).unwrap();
        assert_eq!(a.entradas()[0].nome, "config.json");
        assert!(a.cabecalho_cifrado(), "o nome do arquivo tem de ir cifrado");
        // E em claro nao se le: e barreira contra o editor, e ela funciona.
        assert!(!String::from_utf8_lossy(&bytes).contains("token"));
    }

    #[test]
    fn json_em_claro_continua_sendo_lido_e_ausencia_e_not_found() {
        let d = dir("claro");
        let j = d.0.join("dblink.json");
        assert_eq!(ler_texto(&j).unwrap_err().kind(), io::ErrorKind::NotFound);
        std::fs::write(&j, "{}").unwrap();
        assert_eq!(ler_texto(&j).unwrap(), "{}");
        assert!(sobras(&j).is_none());
    }

    #[test]
    fn com_os_dois_vale_o_phz_e_o_json_e_sobra_nomeada() {
        let d = dir("dois");
        let j = d.0.join("jobs.json");
        std::fs::write(&j, "{\"velho\":1}").unwrap();
        std::fs::write(
            d.0.join("jobs.phz"),
            empacotar("jobs.json", b"{\"novo\":1}").unwrap(),
        )
        .unwrap();
        assert_eq!(ler_texto(&j).unwrap(), "{\"novo\":1}");
        assert_eq!(sobras(&j), Some(j.clone()));
    }

    #[test]
    fn gravar_segue_o_formato_de_quem_existe_e_o_novo_segue_a_pasta() {
        let d = dir("gravar");
        let j = d.0.join("dblink.json");
        assert_eq!(
            caminho_para_gravar(&j),
            j,
            "pasta aberta: o novo nasce em claro"
        );
        std::fs::write(d.0.join("config.phz"), b"x").unwrap();
        assert_eq!(
            caminho_para_gravar(&j),
            d.0.join("dblink.phz"),
            "pasta fechada: nasce .phz"
        );
        std::fs::write(&j, "{}").unwrap();
        // Existe em claro: continua em claro ate a conversao pedida.
        assert_eq!(caminho_para_gravar(&j), j);
    }

    #[test]
    fn phz_com_dois_arquivos_ou_senha_alheia_e_recusado_com_o_nome() {
        let d = dir("torto");
        let p = d.0.join("config.phz");
        let mut e = Escritor::novo(Opcoes {
            senha: Some(SENHA_FIXA.into()),
            acaso: [7; 32],
            ..Opcoes::default()
        });
        e.arquivo("a.json", b"{}".to_vec(), None, None).unwrap();
        e.arquivo("b.json", b"{}".to_vec(), None, None).unwrap();
        std::fs::write(&p, e.gravar().unwrap()).unwrap();
        let m = ler_texto(&p).unwrap_err().to_string();
        assert!(m.contains("guarda 2"), "{m}");
        let mut e = Escritor::novo(Opcoes {
            senha: Some("outra".into()),
            acaso: [8; 32],
            ..Opcoes::default()
        });
        e.arquivo("config.json", b"{}".to_vec(), None, None)
            .unwrap();
        std::fs::write(&p, e.gravar().unwrap()).unwrap();
        assert!(ler_texto(&p).is_err());
    }
}
