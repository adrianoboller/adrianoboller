//! Ciclo do phxSERVERMAIL exercitando o BANCO DE VERDADE -- nao uma maquete.
//!
//! Enquanto o `--example correio-e2e` (em phxsql-core) prova o MODELO do correio
//! em memoria (dominios, tres camadas de cripto, confianca, moderacao), este
//! exemplo grava e le do disco real: `Table` do `phxsql-store`, com `.reg`,
//! `.ndx` e as chaves estrangeiras CONFERIDAS na gravacao. E o que a bateria de
//! `bancada/servermail/` pede para os passos 2, 3, 4 e 7.
//!
//! Rodar:
//!   cargo run --release --example servermail-ciclo -p phxsql-store -- <base>
//!
//! Sem argumento cria uma base temporaria e a remove no fim. Com argumento a
//! base fica no lugar pedido, para o script poder listar os arquivos do formato.
//!
//! O QUE PROVA, e sempre nos DOIS sentidos (recusa quando tem de recusar, passa
//! quando tem de passar):
//!   PASSO 2  id do usuario e um UUID v7 REAL, guardado no `.reg`: versao()==7,
//!            monotonico entre dois seguidos, e volta identico do disco.
//!   PASSO 3  empresa (pai) <- funcionario (filho) por chave conferida, com
//!            indice dos DOIS lados: apagar a empresa com funcionario RECUSA
//!            (nunca se mata o pai que tem filho); apagar depois dos filhos passa.
//!   PASSO 4  coligacao (relacao de confianca) entre duas empresas, por tabela
//!            com chave conferida para os dois lados: nasce, e lida, e a empresa
//!            coligada nao pode ser apagada enquanto a coligacao existir.
//!   PASSO 7  portao WX: o cadastro so LIBERA depois de o v7 do usuario ter sido
//!            armazenado. NAO se conecta na WX (177.69.238.17) -- e producao do
//!            dono; o armazenamento remoto e simulado por uma marca local.

use std::path::{Path, PathBuf};

use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::uuid::Uuid;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Placar da prova: cada checagem com o seu rotulo, e o processo sai 1 se
/// qualquer uma falhar -- o mesmo molde do correio-e2e.
struct Placar {
    itens: Vec<(bool, String)>,
}
impl Placar {
    fn novo() -> Self {
        Placar { itens: vec![] }
    }
    fn ok(&mut self, cond: bool, nome: &str) {
        println!("   {}  {nome}", if cond { " ok  " } else { "FALHA" });
        self.itens.push((cond, nome.to_string()));
    }
    fn fechar(&self) {
        let falhas = self.itens.iter().filter(|(c, _)| !c).count();
        println!(
            "\n{} checagens, {} ok, {} falha(s).",
            self.itens.len(),
            self.itens.len() - falhas,
            falhas
        );
        println!(
            "{}",
            if falhas == 0 {
                "PROVA VERDE"
            } else {
                "PROVA VERMELHA"
            }
        );
        if falhas != 0 {
            std::process::exit(1);
        }
    }
}

/// Esquema de `empresas`: o PAI. `id` e UUID v7; `porId` e a chave primaria e o
/// indice que a mae usa para responder "existe este pai?" ao gravar a filha.
fn esquema_empresas() -> Schema {
    Schema::new(
        "empresas",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("nome", ColumnType::Str(80)).obrigatoria(),
            Column::new("dominio", ColumnType::Str(120)),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
    )
    .expect("esquema empresas")
}

/// Esquema de `funcionarios`: o FILHO. `id` e UUID v7 (o id do usuario); a chave
/// `fk_empresa` aponta para `empresas.id` e NASCE conferida. Ela exige indice
/// dos dois lados: `porId` na mae (acima) e `porEmpresa` aqui, senao achar quem
/// aponta para uma empresa ao apaga-la seria uma varredura escondida.
fn esquema_funcionarios() -> Schema {
    let base = Schema::new(
        "funcionarios",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("empresa_id", ColumnType::Uuid).obrigatoria(),
            Column::new("nome", ColumnType::Str(80)).obrigatoria(),
            Column::new("cargo", ColumnType::Str(60)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
            IndexDef::new("porEmpresa", vec![IndexColumn::asc(1)]),
        ],
    )
    .expect("esquema funcionarios");
    base.com_chaves_estrangeiras(vec![ForeignKey::new(
        "fk_empresa",
        vec![1],
        "empresas",
        vec!["id".into()],
    )])
    .expect("fk funcionarios -> empresas")
}

/// Esquema de `coligacoes`: a relacao de confianca entre DUAS empresas. Duas
/// chaves conferidas, uma para cada lado, cada uma com o seu indice na filha.
fn esquema_coligacoes() -> Schema {
    let base = Schema::new(
        "coligacoes",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("empresa_a", ColumnType::Uuid).obrigatoria(),
            Column::new("empresa_b", ColumnType::Uuid).obrigatoria(),
            Column::new("estado", ColumnType::Str(20)).obrigatoria(),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
            IndexDef::new("porA", vec![IndexColumn::asc(1)]),
            IndexDef::new("porB", vec![IndexColumn::asc(2)]),
        ],
    )
    .expect("esquema coligacoes");
    base.com_chaves_estrangeiras(vec![
        ForeignKey::new("fk_a", vec![1], "empresas", vec!["id".into()]),
        ForeignKey::new("fk_b", vec![2], "empresas", vec!["id".into()]),
    ])
    .expect("fks coligacoes -> empresas")
}

/// Esquema de `cadastros`: o portao da WX. `wx_armazenado` vira true so DEPOIS
/// de o v7 do usuario ter sido gravado nos servidores da WX; `liberado` so pode
/// virar true depois disso. As duas marcas ficam gravadas no `.reg`.
fn esquema_cadastros() -> Schema {
    Schema::new(
        "cadastros",
        vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("nome", ColumnType::Str(80)).obrigatoria(),
            Column::new("wx_armazenado", ColumnType::Bool).obrigatoria(),
            Column::new("liberado", ColumnType::Bool).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria()],
    )
    .expect("esquema cadastros")
}

/// Le a coluna 0 (id Uuid) de uma linha ja lida.
fn id_da_linha(linha: &[Value]) -> Uuid {
    match &linha[0] {
        Value::Uuid(u) => *u,
        outro => panic!("coluna 0 nao e Uuid: {outro:?}"),
    }
}

/// Lista os arquivos do formato de uma tabela, com o tamanho -- a prova de que
/// a base nasceu no disco.
fn listar_arquivos(banco: &Path, tabela: &str) {
    let mut nomes: Vec<_> = std::fs::read_dir(banco)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(&format!("{tabela}.")) || n.starts_with(&format!("{tabela}_")))
        .collect();
    nomes.sort();
    for n in nomes {
        let tam = std::fs::metadata(banco.join(&n))
            .map(|m| m.len())
            .unwrap_or(0);
        println!("      {n:28} {tam:>8} bytes");
    }
}

fn main() {
    let arg = std::env::args().nth(1);
    let (banco, remover_no_fim) = match arg {
        // A base do server mail: <base>/correio/, como um servidor organiza
        // (cada banco e uma pasta). Aqui o banco chama-se "correio".
        Some(base) => (PathBuf::from(base).join("correio"), false),
        None => {
            let d = std::env::temp_dir()
                .join(format!("phx-servermail-{}", std::process::id()))
                .join("correio");
            (d, true)
        }
    };
    let _ = std::fs::remove_dir_all(&banco);
    std::fs::create_dir_all(&banco).expect("criar pasta do banco");

    println!("===== ciclo do phxSERVERMAIL, exercitando o banco de verdade =====");
    println!("banco (base do server mail): {}\n", banco.display());

    let mut p = Placar::novo();

    // ------------------------------------------------------------------
    // Preparo: cria empresas e funcionarios (a base nasce no disco).
    // ------------------------------------------------------------------
    println!("PASSO 1/base) tabelas do server mail nascem no disco:");
    let mut empresas = Table::criar(&banco, esquema_empresas()).expect("criar empresas");
    let mut funcs = Table::criar(&banco, esquema_funcionarios()).expect("criar funcionarios");

    // A empresa mae, com id v7.
    let id_wx = Uuid::v7();
    empresas
        .inserir(&[
            Value::Uuid(id_wx),
            Value::Str("WX Solucoes".into()),
            Value::Str("empresa.phxsql.com.br".into()),
        ])
        .expect("inserir empresa");
    empresas.sincronizar().expect("sincronizar empresas");
    println!("   arquivos de empresas:");
    listar_arquivos(&banco, "empresas");
    p.ok(
        banco.join("empresas.reg").exists() && banco.join("empresas.ndx").exists(),
        "base nasce: empresas.reg e empresas.ndx criados no disco",
    );

    // ------------------------------------------------------------------
    // PASSO 2 -- UUID v7 do usuario, guardado no .reg, lido de volta.
    // ------------------------------------------------------------------
    println!("\nPASSO 2) id do usuario e UUID v7 REAL, guardado no .reg:");
    // Dois ids seguidos ANTES de gravar: monotonicos por construcao.
    let id1 = Uuid::v7();
    let id2 = Uuid::v7();
    p.ok(id1.versao() == 7 && id2.versao() == 7, "v7: versao() == 7");
    p.ok(id2 > id1, "v7: dois seguidos sao crescentes (monotonico)");

    let r1 = funcs
        .inserir(&[
            Value::Uuid(id1),
            Value::Uuid(id_wx),
            Value::Str("Adriano Boller".into()),
            Value::Str("Diretor".into()),
        ])
        .expect("inserir funcionario 1");
    let r2 = funcs
        .inserir(&[
            Value::Uuid(id2),
            Value::Uuid(id_wx),
            Value::Str("Joana Prado".into()),
            Value::Str("Gerente".into()),
        ])
        .expect("inserir funcionario 2");
    funcs.sincronizar().expect("sincronizar funcionarios");
    println!("   arquivos de funcionarios (onde o id fica: coluna 0, 16 bytes no slot do .reg):");
    listar_arquivos(&banco, "funcionarios");

    // Le de volta do DISCO e confere byte a byte.
    let lido1 = id_da_linha(&funcs.ler(r1).expect("ler f1").expect("f1 existe"));
    let lido2 = id_da_linha(&funcs.ler(r2).expect("ler f2").expect("f2 existe"));
    p.ok(lido1 == id1 && lido2 == id2, "id volta IDENTICO do disco");
    p.ok(
        lido1.versao() == 7 && lido2.versao() == 7,
        "id lido do disco continua v7",
    );
    p.ok(
        lido2 > lido1 && lido1.instante_ms().is_some(),
        "id lido do disco: crescente e com relogio legivel (v7)",
    );
    // A busca pelo indice unico acha exatamente a linha do id.
    let achou = funcs
        .buscar("porId", &[Value::Uuid(id1)])
        .expect("buscar porId");
    p.ok(
        achou == vec![r1],
        "busca pelo .ndx (porId) acha o rowid do id v7",
    );

    // ------------------------------------------------------------------
    // PASSO 3 -- empresa (pai) <- funcionario (filho): RESTRICT nos dois sentidos.
    // ------------------------------------------------------------------
    println!("\nPASSO 3) integridade: nunca se mata o pai que tem filho:");
    // Inserir funcionario apontando para empresa INEXISTENTE tem de recusar
    // (a chave nasce conferida).
    let id_fantasma = Uuid::v7();
    let ins_orfa = funcs.inserir(&[
        Value::Uuid(Uuid::v7()),
        Value::Uuid(id_fantasma),
        Value::Str("Orfao".into()),
        Value::Str("-".into()),
    ]);
    p.ok(
        ins_orfa.is_err(),
        "funcionario para empresa inexistente e RECUSADO (chave conferida)",
    );

    // Achar o rowid da empresa mae.
    let rid_empresa = empresas.buscar("porId", &[Value::Uuid(id_wx)]).unwrap()[0];
    // Apagar a empresa com funcionario vivo tem de RECUSAR (de vez e suave).
    let del_dura = empresas.excluir_de_vez(rid_empresa, "teste: apagar mae com filho");
    p.ok(
        del_dura.is_err(),
        "excluir_de_vez da empresa com funcionario e RECUSADO (RESTRICT)",
    );
    let del_suave = empresas.excluir_suave(rid_empresa, "teste: apagar suave mae com filho");
    p.ok(
        del_suave.is_err(),
        "excluir_suave da empresa com funcionario e RECUSADO (orfa que ninguem ve e pior)",
    );

    // Remover os filhos e ENTAO apagar a mae passa. O `sincronizar` da filha e
    // OBRIGATORIO aqui: `conferir_filhas` abre a filha num handle NOVO e enxerga
    // so o que ja foi para o disco -- e o mesmo read-your-own-writes que a mae e
    // a filha na mesma transacao tambem esbarram. Sem o flush, a mae ainda ve os
    // filhos que este handle acabou de remover e recusa a exclusao.
    funcs
        .excluir_de_vez(r1, "teste: sai o filho antes da mae")
        .expect("excluir f1");
    funcs
        .excluir_de_vez(r2, "teste: sai o filho antes da mae")
        .expect("excluir f2");
    funcs
        .sincronizar()
        .expect("flush dos filhos antes de apagar a mae");
    let del_ok = empresas.excluir_de_vez(rid_empresa, "teste: sem filhos, pode sair");
    p.ok(
        matches!(del_ok, Ok(true)),
        "sem filhos, a empresa pode ser apagada",
    );
    // Recria a empresa para os passos seguintes.
    empresas
        .inserir(&[
            Value::Uuid(id_wx),
            Value::Str("WX Solucoes".into()),
            Value::Str("empresa.phxsql.com.br".into()),
        ])
        .expect("recriar empresa");

    // ------------------------------------------------------------------
    // PASSO 4 -- coligacao: relacao de confianca entre duas empresas.
    // ------------------------------------------------------------------
    println!("\nPASSO 4) coligacao (confianca) entre empresas, com integridade:");
    // Uma segunda empresa (outra base logica, mesma pasta do server mail).
    let id_outra = Uuid::v7();
    empresas
        .inserir(&[
            Value::Uuid(id_outra),
            Value::Str("Empresa Coligada".into()),
            Value::Str("time.phxmail.com.br".into()),
        ])
        .expect("inserir 2a empresa");
    empresas.sincronizar().unwrap();

    let mut colig = Table::criar(&banco, esquema_coligacoes()).expect("criar coligacoes");
    let id_colig = Uuid::v7();
    let rid_colig = colig
        .inserir(&[
            Value::Uuid(id_colig),
            Value::Uuid(id_wx),
            Value::Uuid(id_outra),
            Value::Str("aceita".into()),
        ])
        .expect("inserir coligacao");
    colig.sincronizar().unwrap();
    println!("   arquivos de coligacoes:");
    listar_arquivos(&banco, "coligacoes");

    // Nasce e e lida com as duas empresas certas.
    let lc = colig.ler(rid_colig).unwrap().unwrap();
    let a_ok = matches!(&lc[1], Value::Uuid(u) if *u == id_wx);
    let b_ok = matches!(&lc[2], Value::Uuid(u) if *u == id_outra);
    let estado_ok = matches!(&lc[3], Value::Str(s) if s == "aceita");
    p.ok(
        a_ok && b_ok && estado_ok,
        "coligacao nasce e e lida (A, B, estado)",
    );

    // Coligacao para empresa inexistente e recusada.
    let colig_orfa = colig.inserir(&[
        Value::Uuid(Uuid::v7()),
        Value::Uuid(id_wx),
        Value::Uuid(Uuid::v7()),
        Value::Str("aceita".into()),
    ]);
    p.ok(
        colig_orfa.is_err(),
        "coligacao para empresa inexistente e RECUSADA (chave conferida)",
    );

    // Empresa dentro de uma coligacao viva nao pode ser apagada.
    let rid_outra = empresas.buscar("porId", &[Value::Uuid(id_outra)]).unwrap()[0];
    let del_coligada = empresas.excluir_de_vez(rid_outra, "teste: apagar empresa coligada");
    p.ok(
        del_coligada.is_err(),
        "empresa numa coligacao viva NAO pode ser apagada (RESTRICT do outro lado)",
    );
    // Desfeita a coligacao, a empresa pode sair. Mesmo motivo do PASSO 3: a mae
    // so enxerga a coligacao desfeita depois do flush do handle da filha.
    colig
        .excluir_de_vez(rid_colig, "teste: desfaz a coligacao")
        .expect("excluir coligacao");
    colig
        .sincronizar()
        .expect("flush da coligacao antes de apagar a empresa");
    let del_coligada2 = empresas.excluir_de_vez(rid_outra, "teste: sem coligacao, pode sair");
    p.ok(
        matches!(del_coligada2, Ok(true)),
        "desfeita a coligacao, a empresa pode ser apagada",
    );

    // ------------------------------------------------------------------
    // PASSO 7 -- portao WX: so libera o cadastro depois de armazenar o v7.
    // NAO se conecta na WX (177.69.238.17). O armazenamento remoto e SIMULADO.
    // ------------------------------------------------------------------
    println!("\nPASSO 7) portao WX: libera o cadastro so depois de armazenar o v7:");
    let mut cadastros = Table::criar(&banco, esquema_cadastros()).expect("criar cadastros");
    let id_usuario = Uuid::v7(); // o UUID v7 do usuario, o mesmo que iria para a WX
    let rid_cad = cadastros
        .inserir(&[
            Value::Uuid(id_usuario),
            Value::Str("Novo Usuario".into()),
            Value::Bool(false), // wx_armazenado: ainda nao
            Value::Bool(false), // liberado: ainda nao
        ])
        .expect("inserir cadastro pendente");
    cadastros.sincronizar().unwrap();

    // O portao: liberar exige wx_armazenado == true. Aqui esta o gate testavel.
    let liberar = |cad: &mut Table, rid: u64| -> Result<(), String> {
        let linha = cad
            .ler(rid)
            .map_err(|e| e.to_string())?
            .ok_or("cadastro sumiu")?;
        let wx = matches!(linha[2], Value::Bool(true));
        if !wx {
            return Err("RECUSADO: o v7 do usuario ainda nao foi armazenado na WX".into());
        }
        cad.atualizar(
            rid,
            &[
                linha[0].clone(),
                linha[1].clone(),
                Value::Bool(true),
                Value::Bool(true),
            ],
        )
        .map_err(|e| e.to_string())
    };

    // Antes de armazenar na WX: liberar RECUSA.
    p.ok(
        liberar(&mut cadastros, rid_cad).is_err(),
        "sem o v7 armazenado, o cadastro NAO libera (portao fechado)",
    );

    // Simula o armazenamento do v7 na WX -- SEM conectar em 177.69.238.17.
    // (Em producao, aqui iria a chamada remota; ela e proposital e honestamente
    //  NAO executada para nao tocar a producao do dono.)
    println!("   [simulado] v7 {id_usuario} armazenado na WX -- NAO se conectou em 177.69.238.17");
    let l = cadastros.ler(rid_cad).unwrap().unwrap();
    cadastros
        .atualizar(
            rid_cad,
            &[
                l[0].clone(),
                l[1].clone(),
                Value::Bool(true),
                Value::Bool(false),
            ],
        )
        .expect("marcar wx_armazenado");

    // Agora liberar PASSA.
    p.ok(
        liberar(&mut cadastros, rid_cad).is_ok(),
        "com o v7 armazenado, o cadastro LIBERA (portao abre)",
    );
    cadastros.sincronizar().unwrap();
    let final_ = cadastros.ler(rid_cad).unwrap().unwrap();
    p.ok(
        matches!(final_[2], Value::Bool(true)) && matches!(final_[3], Value::Bool(true)),
        "no disco: wx_armazenado=true e liberado=true, e o id e o mesmo v7",
    );
    p.ok(
        id_da_linha(&final_) == id_usuario,
        "o id gravado no cadastro e o v7 do usuario (o mesmo que iria para a WX)",
    );

    // ------------------------------------------------------------------
    // Integridade final do banco inteiro.
    // ------------------------------------------------------------------
    println!("\nverificacao de integridade das tabelas:");
    for (nome, t) in [
        ("empresas", &mut empresas),
        ("funcionarios", &mut funcs),
        ("coligacoes", &mut colig),
        ("cadastros", &mut cadastros),
    ] {
        let rel = t.verificar().expect("verificar");
        println!("   {nome}: {rel:?}");
    }

    println!("\n===== RESULTADO =====");
    p.fechar();

    if remover_no_fim {
        if let Some(pai) = banco.parent() {
            let _ = std::fs::remove_dir_all(pai);
        }
    }
}
