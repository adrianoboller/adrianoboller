//! A troca do separador de volume, `_` -> `#`, no disco que ja existe (pedido
//! 508).
//!
//! # Por que renomear, e nao ler os dois formatos
//!
//! Enquanto o `_NNN` antigo fosse reconhecido, `vendas_2024.reg` continuaria
//! ambiguo -- volume 2024 de `vendas`, ou a tabela `vendas_2024`? --, e a
//! ambiguidade e o defeito que a troca existe para matar. Entao o que o
//! binario anterior gravou se renomeia UMA vez, por diretorio, e o catalogo
//! passa a entender um formato so ([`phxsql_core::paginacao::separar_volume`]).
//!
//! # A marca, e por que ela e por DIRETORIO
//!
//! `_formato-volumes.json`, no diretorio do database e no de cada schema. O
//! catalogo lista as tabelas de um diretorio pelo nome dos `.reg`, entao ele
//! precisa saber QUAL analisador usar antes de listar -- e a resposta e do
//! diretorio, nao da tabela. Diretorio que nasce neste binario nasce marcado
//! ([`marcar_novo`]); diretorio sem marca e do binario anterior e passa por
//! [`migrar_database`] antes de ser lido.
//!
//! # A queda no meio
//!
//! Cada `rename` e atomico, e a marca so se grava depois de todos eles e do
//! `fsync` do diretorio. A queda no meio deixa um diretorio misturado e SEM
//! marca, e a reabertura re-roda: o que ja tem o nome novo pula (o plano o le
//! pelo analisador novo), o que ainda tem o velho se renomeia. Os dois nomes
//! para o mesmo volume ao mesmo tempo nao acontecem por este caminho, e por
//! isso param como corrompido em vez de escolher um.
//!
//! # A ambiguidade antiga, desfeita pelo cabecalho
//!
//! Antes do pedido 368, `vendas_2024` era nome aceito. Num diretorio velho,
//! `vendas_2024.reg` pode ser a tabela ou o volume 2024 de uma `vendas`
//! paginada, e o NOME nao responde. O cabecalho responde: todo volume carrega o
//! proprio numero (bytes 12..16) e o esquema com a paginacao, e o volume 2024
//! de `vendas` e o arquivo cuja paginacao, escrita no separador antigo, da
//! exatamente `_2024` para o numero que ele declara. Uma tabela `vendas_2024`
//! sem paginacao nunca passa nessa conta, e uma paginada nem tem arquivo sem
//! sufixo. O `.reg` cujo cabecalho nao se le fica com a leitura do binario
//! anterior (digitos sao volume; letra de balde e volume quando o `_A` esta do
//! lado): e o que aquele arquivo significava ate aqui, e recusar travaria o
//! database inteiro -- e a restauracao de um backup antigo -- por um volume
//! que o binario anterior lia do mesmo jeito.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::{
    separar_volume, separar_volume_com, SEPARADOR_DE_VOLUME, SEPARADOR_LEGADO,
};
use phxsql_core::EXT_REG;

use crate::catalogo::Database;

/// O nome da marca. Prefixo `_` e extensao `.json`, como o `_database.json`:
/// nao e `.reg`, entao a listagem de tabelas nao a ve, e nao colide com nome
/// de tabela nenhum.
pub const MARCA_FORMATO_VOLUMES: &str = "_formato-volumes.json";

/// O conteudo da marca. So a PRESENCA decide; o texto e para quem abre o
/// diretorio com um `ls` e quer saber o que ela e.
const TEXTO_DA_MARCA: &str = "{\n  \"separador_de_volume\": \"#\",\n  \"pedido\": 508\n}\n";

/// Os diretorios de database que ja se sabem migrados NESTE processo.
///
/// # Por que existe
///
/// A pergunta «este database precisa migrar?» custa um `stat` da marca por
/// diretorio mais a listagem dos schemas, e a abertura de database acontece a
/// cada operacao. Depois da primeira resposta «nao», a resposta nao muda
/// enquanto o processo vive -- nada neste binario cria diretorio sem marca --,
/// e guardar o «nao» e o que deixa o laco quente sem syscall nenhuma por
/// isto.
///
/// # O que ela NAO cobre, dito
///
/// Um diretorio velho COPIADO por fora para dentro da base, com o processo
/// no ar, sob o nome de um database que ja respondeu «nao» nesta vida do
/// processo. A subida seguinte o migra. Restaurar pelo motor nao cai nisso: o
/// palco migra antes de entrar na base.
fn ja_migrados() -> &'static Mutex<HashSet<PathBuf>> {
    static MIGRADOS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    MIGRADOS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn lembrar(caminho: &Path) {
    ja_migrados()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(caminho.to_path_buf());
}

fn lembrado(caminho: &Path) -> bool {
    ja_migrados()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(caminho)
}

/// Este diretorio ja esta no formato do pedido 508?
pub fn marcado(diretorio: &Path) -> bool {
    diretorio.join(MARCA_FORMATO_VOLUMES).is_file()
}

/// O diretorio tem algum `.reg` -- alguma tabela cujo nome um analisador
/// precise ler?
///
/// Diretorio sem `.reg` nao tem o que migrar nem o que ler errado, e NAO
/// recebe marca: o catalogo chama de schema todo subdiretorio, e escrever a
/// marca dentro de um que so esta ali por outro motivo (um `clientes.pag`
/// que virou diretorio, no teste da falha do `.pag`) e mexer no que nao e
/// nosso. Se uma tabela nascer nele depois, nasce com o separador novo, e a
/// migracao da proxima subida acha o plano vazio e so entao marca.
fn tem_reg(diretorio: &Path) -> bool {
    let fim = format!(".{EXT_REG}");
    std::fs::read_dir(diretorio).is_ok_and(|entradas| {
        entradas.filter_map(|e| e.ok()).any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.ends_with(fim.as_str()))
        })
    })
}

/// O diretorio pede migracao: sem marca, e com tabela dentro.
fn sem_marca_com_tabela(diretorio: &Path) -> bool {
    !marcado(diretorio) && tem_reg(diretorio)
}

/// O diretorio do database e os dos schemas: os lugares onde mora tabela.
/// Os schemas saem da MESMA listagem que o catalogo usa para responder
/// «que schemas ha aqui», e nao de uma segunda.
fn diretorios_do_database(caminho: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = vec![caminho.to_path_buf()];
    dirs.extend(
        crate::catalogo::subdiretorios(caminho)?
            .into_iter()
            .map(|s| caminho.join(s)),
    );
    Ok(dirs)
}

/// O database precisa migrar antes de ser lido?
///
/// So LE: e a pergunta da ficha compartilhada, que atende leitores em
/// paralelo e nao pode renomear nada -- quem ouve «sim» desce para a ficha
/// exclusiva e migra por la.
pub fn precisa_migrar(caminho: &Path) -> bool {
    if lembrado(caminho) {
        return false;
    }
    if !caminho.is_dir() {
        return false;
    }
    match diretorios_do_database(caminho) {
        Ok(dirs) => {
            let precisa = dirs.iter().any(|d| sem_marca_com_tabela(d));
            if !precisa {
                lembrar(caminho);
            }
            precisa
        }
        // Nao deu para listar: quem responde e a ficha exclusiva, que tenta
        // de novo e diz o erro.
        Err(_) => true,
    }
}

/// Migra o database inteiro -- o diretorio dele e o de cada schema -- e
/// devolve quantos arquivos renomeou. Idempotente: diretorio marcado pula.
///
/// Quem chama tem a ficha exclusiva, ou nao ha ficha nenhuma porque ninguem
/// mais enxerga o diretorio (a subida do servidor, o palco da restauracao).
///
/// Pergunta ao DISCO, e nao a memoria de [`ja_migrados`]: quem chama e a
/// subida e a restauracao, que rodam uma vez, e o palco da restauracao e um
/// caminho que se reusa -- lembrar dele faria a proxima restauracao de um
/// backup antigo pular a migracao.
pub fn migrar_database(caminho: &Path) -> Result<usize> {
    if !caminho.is_dir() {
        return Ok(0);
    }
    let mut feitos = 0;
    for dir in diretorios_do_database(caminho)? {
        if sem_marca_com_tabela(&dir) {
            feitos += migrar_diretorio(&dir)?;
        }
    }
    Ok(feitos)
}

/// Migra todo database de uma raiz de dados -- os mesmos diretorios que o
/// catalogo lista como database. E o que a subida do servidor e a abertura
/// da [`crate::catalogo::Instancia`] chamam, antes de qualquer operacao: dali
/// em diante nenhuma secao da trava tem o que migrar.
///
/// Devolve `(renomeados, falhas)`: um database cuja migracao recusa (o mesmo
/// volume nos dois nomes, um `rename` ou `fsync` que o disco negou) nao impede
/// os outros de subir -- ele continua recusando na abertura, com o motivo,
/// ate alguem o consertar.
pub fn migrar_base(base: &Path) -> (usize, Vec<String>) {
    let mut feitos = 0;
    let mut falhas = Vec::new();
    let Ok(dbs) = crate::catalogo::subdiretorios(base) else {
        return (0, falhas);
    };
    for db in dbs {
        match migrar_database(&base.join(&db)) {
            Ok(n) => feitos += n,
            Err(e) => falhas.push(format!("{db}: {e}")),
        }
    }
    (feitos, falhas)
}

/// A pergunta da ABERTURA de database, que roda dentro da trava: o database
/// esta no formato do pedido 508?
///
/// # Por que ela nao migra
///
/// Migrar custa `fsync` -- o do diretorio depois dos `rename`s e o da marca
/// --, e a abertura de database esta em toda secao critica do servidor. A
/// migracao mora na abertura da RAIZ ([`migrar_base`]), que roda uma vez,
/// antes da primeira secao. Aqui sobra o caso que ela nao alcanca:
///
/// * o diretorio SEM nada a renomear (nasceu por fora deste motor, ou perdeu
///   a marca numa queda logo depois de nascer): recebe a marca, sem `fsync` --
///   perde-la de novo so faz a proxima abertura perguntar outra vez, e migrar
///   um diretorio que ja esta no formato novo nao renomeia nada;
/// * o diretorio com volume no nome velho, copiado para dentro da base com o
///   processo no ar: RECUSA, dizendo o que fazer. Renomear ali seria migrar
///   sem o `fsync` que torna a migracao segura contra queda.
pub fn exigir_migrado(caminho: &Path) -> Result<()> {
    if !precisa_migrar(caminho) {
        return Ok(());
    }
    for dir in diretorios_do_database(caminho)? {
        if !sem_marca_com_tabela(&dir) {
            continue;
        }
        let plano = planejar(&dir)?;
        if let Some((de, para)) = plano.first() {
            return Err(PhxError::Esquema(format!(
                "{} esta no formato de nome anterior ao pedido 508 ({} arquivo(s) \
                 de volume com `_`, como {} -> {}): a migracao roda na abertura \
                 da raiz de dados, antes da primeira operacao. Reinicie o \
                 servidor (ou reabra a base) para migrar",
                dir.display(),
                plano.len(),
                de.file_name().unwrap_or_default().to_string_lossy(),
                para.file_name().unwrap_or_default().to_string_lossy(),
            )));
        }
        marcar_novo(&dir)?;
    }
    lembrar(caminho);
    Ok(())
}

/// Grava a marca num diretorio que acabou de nascer neste binario -- nao ha o
/// que migrar nele, e sem a marca a abertura seguinte o varreria a toa.
///
/// Sem `fsync`, de proposito: a marca de um diretorio SEM volume velho so
/// economiza uma varredura. Perdida numa queda, a abertura seguinte refaz a
/// pergunta, o plano sai vazio, e a marca volta -- nada se renomeia.
pub(crate) fn marcar_novo(diretorio: &Path) -> Result<()> {
    if marcado(diretorio) {
        return Ok(());
    }
    crate::util::escrever_do_banco(&diretorio.join(MARCA_FORMATO_VOLUMES), TEXTO_DA_MARCA)?;
    Ok(())
}

/// Temporario, `fsync` dele, `rename` e `fsync` do diretorio -- pelo motor
/// de troca duravel que a casa ja tem, e nao por um segundo.
fn gravar_marca(diretorio: &Path) -> Result<()> {
    let marca = diretorio.join(MARCA_FORMATO_VOLUMES);
    let temporario = diretorio.join(format!("{MARCA_FORMATO_VOLUMES}.novo"));
    let arquivo = crate::util::recriar_do_banco(&temporario, false)?;
    {
        use std::io::Write;
        (&arquivo).write_all(TEXTO_DA_MARCA.as_bytes())?;
    }
    crate::sincronia::sync_all(&arquivo, &temporario)?;
    drop(arquivo);
    crate::sincronia::trocar_duravel(&temporario, &marca)
}

/// Tira a extensao de tabela (e o `.novo` da troca do `acrescentar_coluna`,
/// quando houver): `x_001.reg.novo` -> `("x_001", ".reg.novo")`.
///
/// A lista de extensoes e a do catalogo -- a mesma que o `excluir_tabela`
/// apaga --, e nao uma terceira copia.
fn cortar_extensao(nome: &str) -> Option<(&str, &str)> {
    for ext in Database::extensoes_de_uma_tabela() {
        for rabo in ["", ".novo"] {
            let fim = format!(".{ext}{rabo}");
            if let Some(base) = nome.strip_suffix(fim.as_str()) {
                if !base.is_empty() {
                    return Some((base, &nome[base.len()..]));
                }
            }
        }
    }
    None
}

/// As tabelas que um diretorio velho contem, lidas pelo nome dos `.reg` e,
/// onde o nome e ambiguo, pelo cabecalho. Ver o topo do modulo.
fn tabelas_do_diretorio(dir: &Path, nomes: &[String]) -> Result<BTreeSet<String>> {
    let mut tabelas = BTreeSet::new();
    for nome in nomes {
        let Some(base) = nome.strip_suffix(&format!(".{EXT_REG}")) else {
            continue;
        };
        // Ja no formato novo: e a reabertura depois de uma queda no meio.
        if let Some((t, _)) = separar_volume(base) {
            tabelas.insert(t.to_string());
            continue;
        }
        let Some((t, sufixo)) = separar_volume_com(base, SEPARADOR_LEGADO) else {
            tabelas.insert(base.to_string());
            continue;
        };
        let e_volume = match crate::reg::volume_e_paginacao_declarados(&dir.join(nome)) {
            Some((volume, paginacao)) => {
                paginacao.ligada()
                    && paginacao.sufixo_legado(volume) == format!("{SEPARADOR_LEGADO}{sufixo}")
            }
            // Cabecalho que nao se le: vale a leitura do binario ANTERIOR,
            // que e o que o arquivo significava ate aqui -- digitos sao
            // volume; letra de balde e volume quando o balde 1 esta do lado.
            None => {
                sufixo.bytes().all(|b| b.is_ascii_digit())
                    || nomes.contains(&format!(
                        "{t}{SEPARADOR_LEGADO}{}.{EXT_REG}",
                        phxsql_core::paginacao::BALDES[0]
                    ))
            }
        };
        tabelas.insert(if e_volume { t } else { base }.to_string());
    }
    Ok(tabelas)
}

/// O plano de um diretorio: cada `(de, para)` que falta renomear.
fn planejar(dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut nomes: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    nomes.sort();
    let tabelas = tabelas_do_diretorio(dir, &nomes)?;
    let presentes: BTreeSet<&str> = nomes.iter().map(String::as_str).collect();
    let mut plano = BTreeMap::new();
    for nome in &nomes {
        let Some((base, rabo)) = cortar_extensao(nome) else {
            continue;
        };
        // O arquivo da propria tabela (`vendas_2024.ndx`) fica: so volume de
        // OUTRA tabela muda de nome.
        if tabelas.contains(base) {
            continue;
        }
        let Some((t, sufixo)) = separar_volume_com(base, SEPARADOR_LEGADO) else {
            continue;
        };
        if !tabelas.contains(t) {
            continue;
        }
        let novo = format!("{t}{SEPARADOR_DE_VOLUME}{sufixo}{rabo}");
        if presentes.contains(novo.as_str()) {
            return Err(PhxError::Corrompido(format!(
                "{}: existem {nome} e {novo}, o mesmo volume nos dois formatos \
                 de nome -- a migracao do separador (pedido 508) nao escolhe \
                 entre os dois. Nada foi renomeado neste diretorio",
                dir.display()
            )));
        }
        plano.insert(dir.join(nome), dir.join(novo));
    }
    Ok(plano.into_iter().collect())
}

/// Migra UM diretorio e grava a marca dele. Devolve quantos renomeou.
fn migrar_diretorio(dir: &Path) -> Result<usize> {
    let plano = planejar(dir)?;
    for (de, para) in &plano {
        // O ponto da prova de queda: o processo para aqui ENTRE dois
        // `rename`s, antes da marca. Em `release` nao existe.
        crate::ndx::panico_de_teste::passar(
            crate::ndx::panico_de_teste::Ponto::EntreRenomesDoSeparador,
        );
        std::fs::rename(de, para)?;
    }
    if let Some((_, para)) = plano.first() {
        // Os nomes novos no disco ANTES da marca: marca duravel com rename
        // ainda no cache diria «migrado» de um diretorio que a queda desfez.
        crate::sincronia::sincronizar_os_diretorios(para, para, true)?;
    }
    regravar_os_pag(dir)?;
    gravar_marca(dir)?;
    Ok(plano.len())
}

/// O `.pag` guarda o NOME de cada arquivo de balde (`"arquivo":
/// "clientes#A.reg"`), e o velho diz `clientes_A.reg`. Ele so se regrava no
/// `sincronizar`, e a tabela que ninguem escrever depois da migracao ficaria
/// com um descritor mentindo sobre o arquivo -- entao ele se regera aqui, do
/// `.reg`, pelo mesmo gerador. Todo `.pag` do diretorio, e nao so o de quem
/// teve arquivo renomeado nesta passada: a queda entre o ultimo `rename` e
/// esta regravacao deixaria o plano da reabertura vazio e o `.pag` velho.
///
/// O `.pag` nao e fonte de verdade: se o `.reg` nao abre, o descritor velho
/// sai em vez de ficar mentindo -- apagar o `.pag` nao quebra a tabela.
fn regravar_os_pag(dir: &Path) -> Result<()> {
    let fim = format!(".{}", crate::pag::EXT_PAG);
    let tabelas: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter_map(|n| n.strip_suffix(fim.as_str()).map(str::to_string))
        .collect();
    for t in tabelas {
        let regravado = crate::reg::RegFile::abrir(dir, &t).and_then(|reg| {
            crate::pag::escrever(dir, &t, reg.esquema(), reg.baldes(), &reg.volumes())
        });
        if regravado.is_err() {
            let _ = std::fs::remove_file(dir.join(format!("{t}{fim}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_extensao_sai_com_o_rabo_da_troca() {
        assert_eq!(cortar_extensao("x_001.reg"), Some(("x_001", ".reg")));
        assert_eq!(
            cortar_extensao("x_001.reg.novo"),
            Some(("x_001", ".reg.novo"))
        );
        assert_eq!(cortar_extensao("x_A.lgpd"), Some(("x_A", ".lgpd")));
        assert_eq!(cortar_extensao("_database.json"), None);
        assert_eq!(cortar_extensao(".reg"), None);
    }
}
