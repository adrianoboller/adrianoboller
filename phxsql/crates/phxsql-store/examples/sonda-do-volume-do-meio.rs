//! Sonda: numa tabela com CINCO volumes, o fecho da janela leva ao disco o
//! volume do MEIO -- aquele que nao e o 1 nem o da fronteira?
//!
//! A pergunta nasceu lendo `RegFile::sincronizar`, que abre explicitamente so'
//! dois volumes:
//!
//! ```text
//! self.volumes.abrir_para_sincronizar(1)?;
//! let (fronteira, _) = self.localizar(self.slot_count.max(1));
//! if fronteira != 1 { self.volumes.abrir_para_sincronizar(fronteira)?; }
//! ```
//!
//! Ler isso sugere que o volume 3 de cinco fica de fora. Mas ler nao mede, e
//! esta casa ja pagou por confundir «chama a funcao que faz X» com «faz X»:
//! quem decide a lista final e' `Volumes::sincronizar_listas`, que UNE os
//! abertos com o registro de escritas pendentes DO PROCESSO.
//!
//! ```bash
//! strace -f -y -e trace=fsync target/release/examples/sonda-do-volume-do-meio
//! ```
//!
//! A cerca (`/tmp/phx-cerca-<n>`) separa as fases no `strace`: contar `fsync`
//! sem cerca mistura a semeadura com o fecho.
//!
//! A prova e' EXTERNA de proposito: quem conta e' o `strace`, e nao um contador
//! do proprio motor. Contador interno de sincronizacao ja enganou esta casa uma
//! vez -- `Volumes::sincronizacoes()` sobe ANTES do laco, entao mede a intencao
//! e nao o fato.
//!
//! # O que a corrida de 05/09/2026 mediu, ANTES do conserto
//!
//! ```text
//! fase 3, fecho depois de sujar o volume 3:            .reg -> 001, 003, 005
//! fase 5, fecho depois de sujar o 2 por OUTRA grafia:  .reg -> 001, 005
//! ```
//!
//! A primeira linha responde a pergunta do titulo: o volume do meio ENTRA, e
//! entra pelo registro de pendentes -- os volumes 2 e 4, limpos, ficam de fora,
//! que e' a lista certa. A segunda linha era o achado, e estava na fase 5: o
//! volume 2, sujo pela grafia relativa, nao ia ao disco.
//!
//! # E o que ela mede DEPOIS, no mesmo dia
//!
//! ```text
//! fase 3:  .reg -> 001, 003, 005   (nao mudou)
//! fase 5:  .reg -> 001, 002, 005   (o volume sujo entrou)
//! ```
//!
//! A chave do registro de pendentes passou a sair de `volume::familia`, que
//! resolve o diretorio para caminho absoluto lexico antes de montar o nome --
//! entao a grafia relativa e a absoluta caem na MESMA familia. A guarda que
//! trava isso sem `strace` e' `tests/grafia-do-diretorio-nao-divide-a-familia.rs`;
//! esta sonda continua sendo a prova de fora, contra o sistema operacional.

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn cerca(n: u32) {
    let _ = std::fs::File::open(format!("/tmp/phx-cerca-{n}"));
}

/// Quantos bytes cada volume do `.reg` tem, na ordem do sufixo.
fn volumes(dir: &std::path::Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let e = e.ok()?;
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".reg") {
                Some((n, e.metadata().ok()?.len()))
            } else {
                None
            }
        })
        .collect();
    v.sort();
    v
}

fn main() {
    let dir = std::env::temp_dir().join(format!("phx-meio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // 20 linhas por volume: 100 linhas dao CINCO volumes, e o do meio e' o 3.
    let esq = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
    .com_paginacao(Paginacao::nova(20, 999).unwrap())
    .unwrap();
    let linha = |i: i64| vec![Value::Int(i), Value::Str(format!("nome {i}"))];

    // fase 1: nasce com cinco volumes e vai ao disco (estado limpo).
    {
        let mut t = Table::criar(&dir, esq).unwrap();
        for i in 1..=100 {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    println!("volumes depois da semeadura: {:?}", volumes(&dir));

    cerca(1);
    // fase 2: ALTERA uma linha do volume do MEIO e morre sem sincronizar.
    // Alterar (e nao inserir) e' o ponto: inserir sempre cai na fronteira, e
    // uma sonda que so' insere nunca faz a pergunta que este arquivo faz.
    {
        let mut t = Table::abrir(&dir, "clientes").unwrap();
        // rowid 50 = volume 3, slot 10.
        t.atualizar(50, &linha(500_050)).unwrap();
    }

    cerca(2);
    // fase 3: o fecho da janela -- reabre e sincroniza, como
    // `descarregar_sujas_com` do servidor. O que interessa esta ENTRE a cerca 2
    // e a cerca 3.
    {
        let mut t = Table::abrir(&dir, "clientes").unwrap();
        t.sincronizar().unwrap();
    }
    cerca(3);

    // A SEGUNDA pergunta, e ela e' da premissa escrita no comentario do
    // proprio registro: "duas grafias diferentes do mesmo diretorio dariam duas
    // familias e a marca se perderia -- e ai a degradacao e a mesma de acima,
    // para o comportamento antigo, nunca para menos que ele".
    //
    // O comportamento antigo e' `abrir_para_sincronizar(1)` mais a fronteira, e
    // a fase 3 acabou de provar que ele NAO alcanca o volume do meio. Entao a
    // degradacao nao e' benigna: ela perde exatamente o volume que o registro
    // salva. As fases 4 e 5 medem isso.
    //
    // A grafia "torta" e' RELATIVA de proposito, e nao `/tmp/./x`: medido nesta
    // mesma sonda, o `.` redundante NAO divide a familia, porque `PathBuf`
    // compara por `components()` e o componente `CurDir` some ali. Uma sonda
    // com `/tmp/./x` mediria "a marca atravessa" e teria razao pelo motivo
    // errado -- as duas grafias sao a MESMA chave.
    std::env::set_current_dir(dir.parent().unwrap()).unwrap();
    let torto = std::path::PathBuf::from(dir.file_name().unwrap());

    cerca(4);
    // fase 4: escreve no volume 2 pela grafia TORTA (familia B) e morre.
    //
    // Volume DOIS, e nao o 3: o 3 ja foi sujo e sincronizado nas fases 2 e 3,
    // entao um `fsync` nele na fase 5 nao distingue "a marca atravessou a
    // grafia" de "sobrou coisa da fase anterior". O 2 nunca foi tocado depois
    // da semeadura, e por isso ele responde.
    {
        let mut t = Table::abrir(&torto, "clientes").unwrap();
        // rowid 30 = volume 2, slot 10.
        t.atualizar(30, &linha(500_030)).unwrap();
    }

    cerca(5);
    // fase 5: o fecho pela grafia DIREITA (familia A) -- que nao ve a marca.
    {
        let mut t = Table::abrir(&dir, "clientes").unwrap();
        t.sincronizar().unwrap();
    }
    cerca(6);

    // O diretorio FICA: quem le o `strace` precisa casar o descritor com o
    // arquivo, e `-y` so' resolve o nome enquanto ele existe.
    println!("diretorio da sonda: {}", dir.display());
    println!(
        "conte os fsync de .reg entre as cercas 2-3 (espera 001,003,005) e \
         entre as cercas 5-6 (o achado)"
    );
}
