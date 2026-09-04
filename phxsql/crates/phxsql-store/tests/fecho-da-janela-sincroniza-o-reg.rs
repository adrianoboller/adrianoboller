//! Guarda: o fecho da janela de durabilidade tem de sincronizar o `.reg` de
//! verdade -- mesmo quando quem fecha e' um `Table` que acabou de ser
//! REABERTO, sem ter lido nem escrito nada antes. E' o caminho exato de
//! `descarregar_sujas_com`: quem gravou foi um `Table` que ja morreu, e quem
//! sincroniza e' um `Table` novo, so para fechar a janela.
//!
//! # O defeito, e por que nenhum teste de biblioteca o via
//!
//! `Volumes::sincronizar` (`src/volume.rs`) so chama `fsync` nos arquivos que
//! estao em `abertos` -- e um `Table` reaberto nunca tocou o volume do
//! `.reg`: o cabecalho vem de um `std::fs::File::open` direto, dentro de
//! `RegFile::abrir`, fora do cache de `Volumes`. Entao `abertos` esta vazio,
//! o laco de `fsync` roda zero vezes, e `sincronizar()` ainda devolve
//! `Ok(())` -- tendo sincronizado nada. Isto nunca apareceu porque a bateria
//! de durabilidade prova com `SIGKILL`, e pagina suja no cache do NUCLEO
//! sobrevive a processo morto -- so queda de ENERGIA mostraria, e e' isso que
//! o `fsync` ausente deixaria de proteger.
//!
//! # Por que nao um contador -- e' a parte dificil desta guarda
//!
//! `Volumes::sincronizacoes()` e `Volumes::selo()` (e o par que `Table`
//! expoe, `selos_de_sincronizacao()`) foram os dois candidatos obvios, e os
//! dois teriam servido SE a escrita e a sincronizacao acontecessem no MESMO
//! `Table` -- e' exatamente o que ja usam os testes de
//! `exclusao-na-janela.rs`, onde a mesma instancia grava e fecha a janela.
//!
//! Aqui nao: quem escreve e quem fecha a janela sao DUAS instancias de
//! `Table`. E os dois contadores incrementam na CHAMADA, nao no arquivo de
//! fato tocado -- `Volumes::sincronizar` faz `self.sincronizacoes += 1;
//! self.selo = SENHA.fetch_add(...)` ANTES de olhar se `abertos` tem algo.
//! Ou seja: eles medem que `sincronizar()` foi PEDIDO, nao que o disco foi
//! TOCADO -- e' exatamente a intencao, nao o fato, que esta guarda tem que
//! evitar medir. Um teste que so conferisse "`sincronizar` devolveu `Ok`", ou
//! que conferisse "o selo do `.reg` mudou", passaria com o defeito de pe, e
//! e' exatamente o teste que nao existia.
//!
//! Provar o FATO exigiria um contador novo -- por exemplo, "quantos `fsync`
//! passaram por um `File` de verdade, e nao por um `abertos` vazio" -- e isso
//! e' mudanca de API: e' da frente do CONSERTO, nao desta guarda. Por isso
//! esta guarda prova pelo unico caminho que sobrou sem mexer em fonte
//! nenhum: o syscall de verdade, com `strace`. E' a mesma lei que ja mandou
//! provar a queda de conexao do BULKINSERT contra o soquete, e nao com teste
//! unitario -- "o que depende do sistema operacional se prova contra o
//! sistema operacional".
//!
//! # A forma
//!
//! `sonda_reabre_e_sincroniza` (abaixo, `#[ignore]`) e' o CORPO tracado: um
//! `Table::abrir` seguido de `sincronizar()`, nada mais -- sem ler nem
//! escrever antes, que e' justamente o que deixa `abertos` do `.reg` vazio.
//! O teste de verdade semeia os dados por FORA (com outro `Table`, que
//! escreve e morre sem sincronizar -- o `gravar_de_verdade` que ainda nao
//! fechou a janela), e reexecuta o PROPRIO binario de teste sob `strace -f -y
//! -e trace=fsync`, filtrando so por `sonda_reabre_e_sincroniza`. Com `-y` o
//! `strace` decora cada `fsync(fd)` com o caminho resolvido
//! (`fsync(7</.../sonda_1.reg>)`), entao contar quantos `fsync` tocaram um
//! arquivo `.reg` e' so filtrar o log por `.reg>`.
//!
//! **Hoje isto reprova**: zero `fsync` toca o `.reg` nesse cenario. Passa
//! depois que o conserto abrir (ou de algum jeito tocar) o volume do `.reg`
//! antes ou durante o `sincronizar` do fecho de janela.

mod comum;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const VAR_DIR: &str = "PHX_SONDA_FECHO_REG_DIR";

fn esquema() -> Schema {
    Schema::new(
        "sonda",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("nome {i}"))]
}

/// O corpo tracado -- SO o fecho da janela, nada antes. Nao faz parte da
/// bateria normal: `#[ignore]` tira do laco padrao, e a saida cedo (sem a
/// variavel) e' so uma segunda rede para quem rodar com `--include-ignored`
/// sem semear o cenario primeiro.
#[test]
#[ignore = "corpo tracado da sonda -- so roda reexecutado por \
            fecho_da_janela_sincroniza_o_reg_de_verdade, com \
            PHX_SONDA_FECHO_REG_DIR setada"]
fn sonda_reabre_e_sincroniza() {
    let Ok(dir) = std::env::var(VAR_DIR) else {
        return;
    };
    let mut t = Table::abrir(&dir, "sonda").expect("abrir a tabela semeada");
    t.sincronizar().expect("sincronizar o fecho da janela");
}

/// A guarda. Falha HOJE porque o `.reg` nao sincroniza nada quando quem fecha
/// a janela e' um `Table` recem-reaberto -- ver a documentacao do modulo.
#[test]
fn fecho_da_janela_sincroniza_o_reg_de_verdade() {
    let d = comum::DirTemp::novo("fecho-reg");

    // Semeadura, por FORA da sonda tracada -- a sonda so pode conter o fecho
    // em si, senao o `strace` contaria os `fsync` da semeadura junto.
    //
    // fase 1: cria, escreve, sincroniza LIMPO.
    {
        let mut t = Table::criar(&*d, esquema()).unwrap();
        for i in 1..=20 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    // fase 2: escreve mais uma linha por OUTRO `Table` e larga sem
    // sincronizar -- o caminho exato de `gravar_de_verdade`: a janela ainda
    // nao fechou quando este `Table` morre.
    {
        let mut t = Table::abrir(&*d, "sonda").unwrap();
        t.inserir(&linha(999)).unwrap();
    }

    // fase 3, tracada: reabre um Table NOVO -- que nunca leu nem escreveu
    // nada -- e fecha a janela.
    let log = d.join("strace.log");
    let Some(saida) =
        comum::tracar_syscalls("sonda_reabre_e_sincroniza", "fsync", VAR_DIR, &d, &log)
    else {
        eprintln!(
            "strace nao esta instalado nesta maquina -- esta guarda so prova \
             contra o sistema operacional de verdade e nao tem substituto de \
             teste unitario. Instale o strace para rodar esta guarda."
        );
        return;
    };

    let fsyncs_no_reg = saida
        .lines()
        .filter(|l| l.contains("fsync(") && l.contains(".reg>"))
        .count();
    let total_fsyncs = saida.lines().filter(|l| l.contains("fsync(")).count();

    assert!(
        fsyncs_no_reg >= 1,
        "o fecho da janela num Table recem-reaberto NAO sincronizou o .reg -- \
         {total_fsyncs} fsync(s) no total, nenhum sobre um arquivo .reg. \
         `Volumes::sincronizar` so faz `fsync` nos volumes que estao em \
         `abertos`, e este `Table` nunca leu nem escreveu no `.reg` antes de \
         chamar `sincronizar()` -- o cabecalho foi lido por um \
         `std::fs::File` direto em `RegFile::abrir`, fora do cache de \
         `Volumes`. Uma queda de ENERGIA depois deste fecho perderia a linha \
         999 (e a lixeira/ndx/etc ja sincronizados apontariam para um `.reg` \
         desatualizado). Log completo do strace em {}",
        log.display()
    );
}
