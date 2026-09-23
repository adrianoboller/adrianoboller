//! O CONGELAMENTO de uma tabela: enquanto ela esta sendo REESCRITA inteira,
//! ninguem mais a abre para gravar.
//!
//! # O defeito que ele existe para impedir
//!
//! Reescrever o `.reg` de uma tabela (acrescentar coluna, migrar para o
//! `PSCH` v10, alargar o bloco de esquema) acontece em duas fases: a FASE A
//! escreve um `*.novo` COMPLETO ao lado de cada volume e o sincroniza; a
//! FASE B troca os arquivos por `rename`. O que torna a operacao retomavel
//! contra queda e exatamente isso -- o arquivo vivo continua inteiro e velho
//! enquanto a FASE A corre.
//!
//! So que o `*.novo` e um **retrato**. Uma linha gravada no volume velho
//! durante a FASE A **nao esta nele**, e a FASE B renomeia por cima:
//! *escrita confirmada, perdida, sem bilhete*. Hoje o unico impedimento para
//! isso e a trava global de dados ficar presa a operacao inteira -- 2,76 s a
//! 2 milhoes de slots, ~13,8 s a 10 milhoes, medidos pelo papel C em
//! 23/09/2026. Nao e servidor lento: e servidor parado.
//!
//! Tirar a trava da FASE A sem por nada no lugar **compila, passa na suite e
//! perde dado**. O que entra no lugar e este registro.
//!
//! # Por que aqui, no armazem, e nao no servidor
//!
//! O parecer do papel C apontou `Servidor::abrir_travada_com` como o ponto
//! unico -- e ele e o ponto unico do pedido do CLIENTE, sobre o campo
//! `tabela`. So que ele **nao e o ponto unico do disco**: a cascata do
//! `ao_alterar` abre a tabela irma por `Table::abrir` (`table.rs`), a
//! conferencia de integridade abre a mae por `Table::abrir`
//! (`integridade.rs`), e a replicacao, o `dblink` e o fecho da janela de
//! escrita abrem por `Database::abrir_qualificada`. Nenhum desses tres passa
//! pelo portao do servidor, e os tres GRAVAM.
//!
//! O ponto por onde todos passam, sem excecao, e [`Table::abrir_com`] com
//! `escrever = true` -- e e la que este registro e consultado. E a mesma lei
//! do portao de permissao: *o portao e UM so, e o campo que ele le e o furo*.
//! Espalhar a conferencia pelas quarenta operacoes deixaria a que alguem
//! esquecesse como a porta dos fundos, e ninguem a acharia por leitura.
//!
//! # O que este portao RECUSA a mais do que o necessario, medido
//!
//! `escrever = true` quer dizer «abrir esta tabela pode escrever», e nao «esta
//! operacao vai gravar dado». Nesta casa a maioria das leituras pede a ficha
//! exclusiva: `op_ler` (`servidor.rs`) abre por `abrir_travada` porque pode
//! gravar a trilha de acesso, e so o `op_varrer` tenta a ficha COMPARTILHADA
//! primeiro. Entao, com a tabela congelada, um `ler` dela e recusado junto
//! com o `inserir`.
//!
//! **Isso NAO tira capacidade de ninguem**, e o numero diz por que: hoje,
//! durante a reescrita, a trava global esta presa e esse mesmo `ler` espera
//! 2,76 s a 2 milhoes de slots e ~13,8 s a 10 milhoes -- ele nunca foi
//! atendido dentro da janela. O que muda e a FORMA: espera longa vira recusa
//! imediata com nome, codigo e `repetir: true`, que e o que um cliente
//! consegue tratar. Espera de 13,8 s com o tempo de leitura do soquete em 5 s
//! vira conexao morta sem mensagem nenhuma.
//!
//! Separar intencao de LEITURA de intencao de ESCRITA na abertura e frente
//! propria: exigiria o `op_ler` deixar de pedir a ficha exclusiva, ou um
//! parametro de intencao atravessando as ~30 aberturas de tabela da arvore --
//! e e' essa segunda forma que vira a porta dos fundos que esta lei proibe.
//!
//! # Por que um registro do PROCESSO, e nao uma marca em disco
//!
//! Porque o que ele descreve e uma operacao **em voo neste processo**, como a
//! propria trava global. Uma marca em disco sobreviveria a uma queda e
//! deixaria a tabela congelada para sempre, sem ninguem para descongela-la --
//! e depois da queda nao ha o que proteger: o volume vivo esta inteiro e
//! velho, e os `*.novo` orfaos ja sao lixo reconhecido pelo `abrir`.
//!
//! # O portao vem ANTES do trabalho
//!
//! [`conferir`] le um `AtomicUsize` antes de montar chave, antes de tocar no
//! mutex e antes de alocar. Com nada congelado -- que e sempre, menos durante
//! uma reescrita -- ele custa um `load(Relaxed)` e volta. E a licao do
//! Profiler: *quando entrar um observador novo, procure o que ele faz antes
//! de olhar o proprio interruptor*.

use phxsql_core::error::{PhxError, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Caminho da tabela (`diretorio/nome`, ja resolvido) -> o que explicar a
/// quem esbarrar.
///
/// `BTreeMap` e nao `HashMap` pelo mesmo motivo do `ESCRITAS_PENDENTES` do
/// `volume.rs`: ele e construtivel em `const`, e um `static` sem
/// inicializacao preguicosa nao precisa de `OnceLock` nenhum.
static CONGELADAS: Mutex<BTreeMap<PathBuf, String>> = Mutex::new(BTreeMap::new());

/// Quantas tabelas estao congeladas AGORA. E o interruptor, e e por isso que
/// ele e atomico e nao `CONGELADAS.lock().len()`: o caminho comum nao paga
/// mutex nenhum.
static QUANTAS: AtomicUsize = AtomicUsize::new(0);

/// A posse do congelamento. Descongela no `Drop`.
///
/// Devolver um valor com `Drop` e nao um par `congelar`/`descongelar` e
/// decisao: um `?` no meio da FASE A sairia da funcao sem passar pelo
/// descongelar escrito a mao, e a tabela ficaria travada ate o servidor
/// reiniciar. Com o `Drop`, o caminho de erro solta sozinho.
#[derive(Debug)]
pub struct Congelada {
    chave: PathBuf,
}

impl Congelada {
    /// O caminho congelado. So para teste e para mensagem.
    pub fn chave(&self) -> &Path {
        &self.chave
    }
}

impl Drop for Congelada {
    fn drop(&mut self) {
        if let Ok(mut c) = CONGELADAS.lock() {
            if c.remove(&self.chave).is_some() {
                QUANTAS.fetch_sub(1, Ordering::SeqCst);
            }
        }
        // Trava envenenada: a entrada FICA, e a tabela continua recusando
        // escrita. E o lado seguro -- uma tabela que recusa aparece; uma que
        // aceita no meio de uma reescrita perde dado em silencio.
    }
}

/// Congela `diretorio/nome`, com o motivo que vai na recusa de quem esbarrar.
///
/// Recusa se ja estiver congelada: duas reescritas simultaneas da mesma
/// tabela sao exatamente o que isto impede, e a segunda tem de ouvir isso em
/// vez de comecar.
pub fn congelar(diretorio: &Path, nome: &str, motivo: impl Into<String>) -> Result<Congelada> {
    let chave = diretorio.join(nome);
    let mut c = CONGELADAS
        .lock()
        .map_err(|_| PhxError::Corrompido("o registro de congelamento esta envenenado".into()))?;
    if let Some(ja) = c.get(&chave) {
        return Err(PhxError::EmMigracao(format!(
            "a tabela {nome} ja esta sendo reescrita: {ja}"
        )));
    }
    c.insert(chave.clone(), motivo.into());
    QUANTAS.fetch_add(1, Ordering::SeqCst);
    Ok(Congelada { chave })
}

/// Esta tabela pode ser aberta para GRAVAR?
///
/// O `Ok(())` e o caminho de sempre, e ele custa um `load` -- ver o cabecalho
/// do modulo.
pub fn conferir(diretorio: &Path, nome: &str) -> Result<()> {
    // O portao ANTES do trabalho: sem nada congelado nao se monta chave, nao
    // se aloca `PathBuf` e nao se toca no mutex.
    if QUANTAS.load(Ordering::SeqCst) == 0 {
        return Ok(());
    }
    let chave = diretorio.join(nome);
    let c = CONGELADAS
        .lock()
        .map_err(|_| PhxError::Corrompido("o registro de congelamento esta envenenado".into()))?;
    match c.get(&chave) {
        None => Ok(()),
        Some(motivo) => Err(PhxError::EmMigracao(format!(
            "a tabela {nome} esta sendo REESCRITA neste instante ({motivo}) e \
             nao atende ate terminar; as outras tabelas atendem normalmente. \
             Tente de novo em alguns instantes -- a reescrita termina sozinha \
             e nada se perde"
        ))),
    }
}

/// Quantas estao congeladas. So para medir e para teste.
pub fn quantas() -> usize {
    QUANTAS.load(Ordering::SeqCst)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O comportamento VELHO, que e o teste que mais importa numa guarda
    /// nova: servidor sem reescrita nenhuma em curso nao sente diferenca.
    #[test]
    fn sem_nada_congelado_nada_muda() {
        let d = std::path::Path::new("/tmp/phxsql-congelamento-vazio");
        assert!(conferir(d, "clientes").is_ok());
        assert!(conferir(d, "pedidos").is_ok());
    }

    #[test]
    fn congelada_recusa_e_nomeia_a_tabela() {
        let d = std::path::Path::new("/tmp/phxsql-congelamento-um");
        let posse = congelar(d, "clientes", "migracao para o PSCH v10").unwrap();
        let e = conferir(d, "clientes").unwrap_err();
        assert!(
            matches!(e, PhxError::EmMigracao(_)),
            "a recusa saiu como {e:?} em vez de EmMigracao"
        );
        let texto = e.to_string();
        assert!(
            texto.contains("clientes"),
            "a recusa nao nomeia a tabela: {texto}"
        );
        assert!(
            texto.contains("PSCH v10"),
            "a recusa nao traz o motivo de quem congelou: {texto}"
        );
        // A VIZINHA nao sente: congelar uma nao congela a base.
        assert!(conferir(d, "pedidos").is_ok());
        drop(posse);
        assert!(conferir(d, "clientes").is_ok(), "o Drop nao descongelou");
    }

    #[test]
    fn duas_reescritas_da_mesma_tabela_nao_comecam_juntas() {
        let d = std::path::Path::new("/tmp/phxsql-congelamento-duas");
        let _posse = congelar(d, "notas", "acrescentar coluna").unwrap();
        let e = congelar(d, "notas", "migracao").unwrap_err();
        assert!(matches!(e, PhxError::EmMigracao(_)), "{e:?}");
    }

    /// O interruptor VOLTA, senao o portao barato deixa de ser barato para
    /// sempre depois da primeira reescrita do processo.
    ///
    /// A conta e relativa e nao absoluta de proposito: o registro e do
    /// PROCESSO, e a suite roda em paralelo -- um zero cravado aqui mediria
    /// o teste vizinho.
    #[test]
    fn o_contador_volta_ao_que_era() {
        let d = std::path::Path::new("/tmp/phxsql-congelamento-contador");
        let antes = quantas();
        {
            let _a = congelar(d, "a", "x").unwrap();
            let _b = congelar(d, "b", "x").unwrap();
            assert!(
                quantas() >= antes + 2,
                "duas congeladas e o contador nao subiu dois"
            );
        }
        assert!(conferir(d, "a").is_ok(), "o Drop de `a` nao descongelou");
        assert!(conferir(d, "b").is_ok(), "o Drop de `b` nao descongelou");
    }
}
