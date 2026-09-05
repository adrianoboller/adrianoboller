//! Guarda: duas GRAFIAS do mesmo diretorio nao podem dividir a familia de
//! escritas pendentes -- porque, dividida, o fecho da janela deixa dado para
//! tras.
//!
//! # O defeito, e a sonda que o mediu
//!
//! O registro de `ESCRITAS_PENDENTES` (`src/volume.rs`) e' o que faz o fecho
//! da janela de durabilidade alcancar o volume que uma instancia JA MORTA
//! sujou. A chave dele e' a familia de arquivos, e ate' 05/09/2026 a chave era
//! o caminho **cru**: quem abrisse a tabela por `dados/loja` e quem a fechasse
//! por `/srv/dados/loja` entrava em duas familias diferentes, e a marca de
//! quem escreveu nao chegava a quem sincroniza.
//!
//! O comentario do proprio registro afirmava que isso era benigno -- «a
//! degradacao e a mesma de acima, para o comportamento antigo, nunca para
//! menos que ele». A sonda `--example sonda-do-volume-do-meio` mediu, com
//! `strace`, que nao e':
//!
//! ```text
//! fase 3, familia inteira, volume 3 sujo:   .reg -> 001, 003, 005
//! fase 5, familia partida, volume 2 sujo:   .reg -> 001, 005
//! ```
//!
//! O comportamento antigo e' `abrir_para_sincronizar(1)` mais a fronteira de
//! escrita, e ele **nao alcanca o volume do meio**. Familia partida nao custa
//! velocidade: custa o dado, calada, e so' numa queda de energia.
//!
//! # Por que ela mede `sincronizados()`, e nao `sincronizacoes()`
//!
//! Porque `sincronizacoes()` e `selo()` sobem ANTES do laco -- medem que
//! `sincronizar()` foi PEDIDO, e passariam com o defeito de pe. Esta guarda
//! conta o ARQUIVO que passou pelo `sync_all`, que e' o efeito.
//!
//! **Prova real (05/09/2026):** com a `volume::familia` devolvendo o caminho
//! cru (`diretorio.join(arquivo)`), o primeiro teste falha com
//! `sincronizados = 0` onde espera 1 -- o volume sujo fica no disco por
//! sincronizar.
//!
//! # Por que este arquivo troca o diretorio de trabalho
//!
//! Grafia relativa so' existe contra um diretorio de trabalho. `set_current_dir`
//! e' do PROCESSO, e um teste de integracao e' um binario proprio -- nenhum
//! outro teste da suite compartilha este processo. Os dois testes daqui
//! apontam para o MESMO destino (`std::env::temp_dir()`), entao rodar em
//! paralelo nao os faz brigar.

mod comum;

use phxsql_core::paginacao::Paginacao;
use phxsql_store::table::Table;
use phxsql_store::Volumes;

/// Vai para o pai do temporario e devolve as DUAS grafias do mesmo diretorio.
fn duas_grafias(d: &comum::DirTemp) -> (std::path::PathBuf, std::path::PathBuf) {
    let pai = d.parent().expect("o temporario tem pai");
    std::env::set_current_dir(pai).expect("entrar no pai do temporario");
    let nome = d.file_name().expect("o temporario tem nome");
    // A absoluta sai do `current_dir` de VERDADE, e nao do `temp_dir`: onde
    // `/tmp` for um symlink, os dois textos diferem e a guarda mediria a
    // diferenca errada.
    let absoluta = std::env::current_dir().expect("cwd legivel").join(nome);
    (absoluta, std::path::PathBuf::from(nome))
}

/// O caso da sonda: escreve por uma grafia, fecha a janela pela outra.
#[test]
fn o_fecho_por_outra_grafia_alcanca_o_volume_sujo() {
    let d = comum::DirTemp::novo("grafia-familia");
    let (absoluta, relativa) = duas_grafias(&d);
    let p = Paginacao::nova(10, 99).unwrap();

    // Nasce com cinco volumes e vai ao disco: estado limpo, e as marcas que o
    // `criar` deixou saem da frente antes de a do `escrever` ser cobrada --
    // sem isto o teste passaria por engano.
    {
        let mut a = Volumes::novo(&absoluta, "t", "reg", p);
        for v in 1..=5 {
            a.criar(v).unwrap();
        }
        a.sincronizar().unwrap();
    }

    // Suja SO o volume do meio, pela grafia RELATIVA, e morre sem sincronizar.
    // O volume 3 e' o ponto: o 1 e o 5 o comportamento antigo ja alcanca
    // sozinho (cabecalho e fronteira), entao um teste sobre eles passaria com
    // o defeito de pe.
    {
        let mut b = Volumes::novo(&relativa, "t", "reg", p);
        b.escrever(3, 0, b"linha alterada no meio").unwrap();
    }

    // O fecho da janela, pela grafia ABSOLUTA -- e' o `descarregar_sujas_com`
    // do servidor, que reabre a tabela so' para sincronizar.
    let mut c = Volumes::novo(&absoluta, "t", "reg", p);
    assert_eq!(
        c.sincronizados(),
        0,
        "a instancia do fecho nao tocou nada ainda"
    );
    c.sincronizar().unwrap();
    assert_eq!(
        c.sincronizados(),
        1,
        "o fecho tinha de mandar ao disco o volume 3, sujo pela outra grafia \
         do mesmo diretorio -- nem menos (o dado se perde numa queda de \
         energia) nem mais (fsync em volume limpo custa 52 us medidos)"
    );
}

/// A metade de VELOCIDADE: a tabela resolve o diretorio uma vez, e nao sete.
///
/// A correcao mora na `volume::familia`, que se vira com qualquer grafia. Esta
/// guarda cobra a outra decisao -- `Table` resolve o caminho ao abrir, entao
/// os sete conjuntos de volumes de uma tabela caem no ramo barato da `familia`
/// (`is_absolute`) em vez de pagarem sete `getcwd` de 395 ns.
#[test]
fn a_tabela_resolve_o_diretorio_uma_vez_ao_abrir() {
    let d = comum::DirTemp::novo("grafia-resolve");
    let (_absoluta, relativa) = duas_grafias(&d);

    let esquema = phxsql_core::schema::Schema::new(
        "clientes",
        vec![
            phxsql_core::schema::Column::new("id", phxsql_core::types::ColumnType::Int8)
                .obrigatoria(),
        ],
        vec![phxsql_core::schema::IndexDef::new(
            "porId",
            vec![phxsql_core::schema::IndexColumn::asc(0)],
        )
        .unico()],
    )
    .unwrap();

    let t = Table::criar(&relativa, esquema).unwrap();
    assert!(
        t.diretorio().is_absolute(),
        "criada por grafia relativa, a tabela devia guardar o caminho \
         resolvido; guardou {:?}",
        t.diretorio()
    );
    drop(t);

    let t = Table::abrir(&relativa, "clientes").unwrap();
    assert!(
        t.diretorio().is_absolute(),
        "aberta por grafia relativa, a tabela devia guardar o caminho \
         resolvido; guardou {:?}",
        t.diretorio()
    );
}
