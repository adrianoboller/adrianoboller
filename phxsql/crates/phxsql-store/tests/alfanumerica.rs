//! Partição alfanumérica: `Clientes_A.reg` até `Clientes_Outros.reg`.
//!
//! O que estes testes protegem, em ordem de importância:
//!
//! 1. a linha vai para o ARQUIVO da letra dela, e o nome do arquivo é a letra;
//! 2. o endereço continua saindo de uma conta — ler não mudou em nada;
//! 3. a ordem de digitação global não se perdeu: ela está no `rownum`;
//! 4. a varredura salta os vazios entre baldes em vez de andar por eles —
//!    para a frente E para trás, que é o irmão que ficou de fora primeiro;
//! 5. alterar a coluna de referência é RECUSADO, porque o balde é o endereço.

#[allow(dead_code, reason = "o modulo comum serve a varios testes")]
mod comum;

use comum::DirTemp;

use phxsql_core::paginacao::{balde_de, Paginacao, BALDES, BALDE_OUTROS};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Salto, Table, Visao};

const NOME: usize = 1;

/// Cem por letra é pouco de propósito: assim o teto por letra é alcançável
/// dentro de um teste.
fn esquema(por_letra: u64) -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
    .com_paginacao(Paginacao::por_letra(por_letra, 1).unwrap())
    .unwrap()
}

fn linha(id: i64, nome: &str) -> Vec<Value> {
    vec![Value::Int(id), Value::Str(nome.into())]
}

fn rownum(t: &mut Table, rowid: u64) -> u64 {
    let i = t.esquema().coluna_rownum().unwrap();
    let l = t.ler(rowid).unwrap().unwrap();
    match &l[i] {
        Value::UInt(n) => *n,
        outro => panic!("{outro:?}"),
    }
}

#[test]
fn o_balde_sai_da_primeira_letra() {
    assert_eq!(balde_de("Adriano"), 1);
    assert_eq!(balde_de("boller"), 2);
    assert_eq!(balde_de("Zeus"), 26);
    assert_eq!(balde_de("0800"), 27);
    assert_eq!(balde_de("9 de julho"), 36);
    // Acento cai na letra sem acento: «Ávila» e «Avila» são a mesma pessoa
    // digitada por duas pessoas, e não podem parar em arquivos diferentes.
    assert_eq!(balde_de("Ávila"), 1);
    assert_eq!(balde_de("Ângela"), 1);
    assert_eq!(balde_de("Éder"), 5);
    assert_eq!(balde_de("Óscar"), 15);
    assert_eq!(balde_de("Çelik"), 3);
    // O que não é letra nem algarismo, e o vazio, vão para Outros — que é um
    // lugar visível, e não escondido no meio do balde A.
    assert_eq!(balde_de(""), BALDE_OUTROS);
    assert_eq!(balde_de("   "), BALDE_OUTROS);
    assert_eq!(balde_de("#hashtag"), BALDE_OUTROS);
    assert_eq!(balde_de("日本"), BALDE_OUTROS);
    assert_eq!(BALDES[BALDE_OUTROS as usize - 1], "Outros");
}

#[test]
fn cada_letra_ganha_o_arquivo_dela() {
    let dir = DirTemp::novo("arquivos");
    let mut t = Table::criar(&dir.0, esquema(1000)).unwrap();
    for (i, nome) in ["Adriano", "Bruno", "Silva", "Souza", "0800", "#etc"]
        .iter()
        .enumerate()
    {
        t.inserir(&linha(i as i64 + 1, nome)).unwrap();
    }
    t.sincronizar().unwrap();

    let existe = |s: &str| dir.0.join(format!("clientes_{s}.reg")).exists();
    assert!(existe("A"), "faltou o clientes_A.reg");
    assert!(existe("B"));
    assert!(existe("S"));
    assert!(existe("0"));
    assert!(existe("Outros"));
    // O balde que nunca recebeu linha NÃO ganha arquivo: uma tabela de
    // clientes sem nenhum nome com Q não precisa de um _Q.reg vazio.
    assert!(!existe("Q"), "criou arquivo para um balde vazio");

    // Duas Silvas moram no mesmo arquivo.
    let b = t.baldes();
    assert_eq!(b[18], 2, "as duas de S deviam estar no mesmo balde");
    assert_eq!(b[0], 1);
    assert_eq!(b[36], 1, "o # devia estar em Outros");
}

/// O que faz o desenho funcionar: o rowid CONTINUA sendo endereço por conta.
#[test]
fn o_rowid_diz_o_arquivo_por_uma_conta() {
    let dir = DirTemp::novo("conta");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
    let a = t.inserir(&linha(1, "Adriano")).unwrap();
    let s = t.inserir(&linha(2, "Silva")).unwrap();
    let o = t.inserir(&linha(3, "#etc")).unwrap();

    // balde A = 1 -> rowid 1; S = 19 -> (19-1)*100+1 = 1801;
    // Outros = 37 -> (37-1)*100+1 = 3601.
    assert_eq!(a, 1);
    assert_eq!(s, 1801);
    assert_eq!(o, 3601);

    // E ler por esse rowid devolve a linha certa, sem índice nenhum.
    assert_eq!(t.ler(s).unwrap().unwrap()[NOME], Value::Str("Silva".into()));
    assert_eq!(t.ler(o).unwrap().unwrap()[NOME], Value::Str("#etc".into()));
}

/// A regra da casa: a ordem de digitação não se perde — ela muda de campo.
#[test]
fn a_ordem_de_digitacao_esta_no_rownum() {
    let dir = DirTemp::novo("ordem");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
    let ordem = ["Silva", "Adriano", "Zeus", "Bruno", "Ávila"];
    let mut rowids = Vec::new();
    for (i, n) in ordem.iter().enumerate() {
        rowids.push(t.inserir(&linha(i as i64 + 1, n)).unwrap());
    }

    // O rowid NÃO cresce com a chegada: o Silva foi o primeiro e tem o maior.
    assert!(rowids[0] > rowids[1], "o rowid ainda segue a digitação");

    // O rownum cresce com a chegada, um a um, na ordem em que foram digitados.
    for (i, r) in rowids.iter().enumerate() {
        assert_eq!(
            rownum(&mut t, *r),
            i as u64 + 1,
            "{} fora de ordem",
            ordem[i]
        );
    }

    // E a varredura sai em ordem ALFABÉTICA de balde, que é a do arquivo.
    let nomes: Vec<String> = t
        .varrer()
        .unwrap()
        .into_iter()
        .map(|(_, l)| match &l[NOME] {
            Value::Str(s) => s.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(nomes, ["Adriano", "Ávila", "Bruno", "Silva", "Zeus"]);
}

/// A varredura não pode andar de um em um pelo vazio entre baldes: com teto
/// de um milhão por letra, isso seria 36 milhões de leituras para achar a
/// segunda linha.
#[test]
fn a_varredura_salta_os_vazios_entre_baldes() {
    let dir = DirTemp::novo("salto");
    // Teto altíssimo: entre o balde A e o Z há quase 37 milhões de slots que
    // nunca existiram. Se a varredura andasse por eles, este teste não
    // terminaria.
    let mut t = Table::criar(&dir.0, esquema(1_000_000)).unwrap();
    t.inserir(&linha(1, "Adriano")).unwrap();
    t.inserir(&linha(2, "Zeus")).unwrap();
    t.inserir(&linha(3, "#etc")).unwrap();

    let v = t.varrer().unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].0, 1);
    assert_eq!(v[1].0, 25_000_001);
    assert_eq!(v[2].0, 36_000_001);

    // E a paginação por cursor atravessa os mesmos vazios.
    let p = t.pagina_depois_de(0, 2, Visao::Ativas).unwrap();
    assert_eq!(p, vec![1, 25_000_001]);
    let q = t.pagina_depois_de(25_000_001, 2, Visao::Ativas).unwrap();
    assert_eq!(q, vec![36_000_001]);
}

/// O IRMÃO da varredura para a frente, e ele tinha ficado.
///
/// `pagina_depois_de` sabe saltar o vazio entre baldes desde o primeiro dia,
/// porque anda pelo `proximo_ativo`. A página ANTERIOR andava de um em um com
/// o `ler` cru — e na alfanumérica o slot além do `usados` do balde não
/// devolve «vazio»: devolve ERRO, porque ele não existe.
///
/// O efeito não era lentidão, era recusa: pela porta de dados o `varrer` monta
/// o campo `ha_antes` com esta função, então **toda página que começasse no
/// primeiro slot de um balde** voltava `NAO_ENCONTRADO` em vez de linhas —
/// e a página 1 também, quando o balde `_A` estava vazio. Achado exercitando
/// o `varrer` por soquete (`bancada/utilizacao-padrao/`), não lendo o código:
/// os 33 testes daqui provavam a ida, e nenhum provava a volta.
#[test]
fn a_pagina_anterior_atravessa_o_vazio_entre_baldes() {
    let dir = DirTemp::novo("anterior");
    // Teto altíssimo de propósito: entre um balde e o outro há quase um milhão
    // de slots que nunca existiram. Andar por eles não terminaria; ler o
    // primeiro deles reprova.
    let mut t = Table::criar(&dir.0, esquema(1_000_000)).unwrap();
    t.inserir(&linha(1, "Adriano")).unwrap();
    t.inserir(&linha(2, "Mendes")).unwrap();
    t.inserir(&linha(3, "Zeus")).unwrap();
    let (adriano, mendes, zeus) = (1, 12_000_001, 25_000_001);

    // Uma antes de Zeus é a Mendes, treze baldes atrás.
    assert_eq!(
        t.pagina_antes_de(zeus, 1, Visao::Ativas).unwrap(),
        vec![mendes]
    );
    // E a página inteira sai na ordem dos baldes, como a de ir.
    assert_eq!(
        t.pagina_antes_de(zeus, 10, Visao::Ativas).unwrap(),
        vec![adriano, mendes]
    );
    // Perguntar o que vem antes do primeiro não é erro: é uma lista vazia.
    assert!(t
        .pagina_antes_de(adriano, 1, Visao::Ativas)
        .unwrap()
        .is_empty());

    // O balde _A vazio é o caso que quebrava a PÁGINA 1: o primeiro rowid da
    // tabela deixa de ser 1, e `ha_antes` passa a perguntar por um slot que
    // não existe.
    let dir2 = DirTemp::novo("anterior-sem-a");
    let mut u = Table::criar(&dir2.0, esquema(1_000_000)).unwrap();
    u.inserir(&linha(1, "Bruno")).unwrap();
    assert!(u
        .pagina_antes_de(1_000_001, 1, Visao::Ativas)
        .unwrap()
        .is_empty());
}

/// Mudar a coluna de referência mudaria o arquivo em que a linha mora, e com
/// ele o rowid — que é a identidade dela e está em todo índice.
#[test]
fn alterar_a_coluna_de_referencia_e_recusado() {
    let dir = DirTemp::novo("mudar");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
    let r = t.inserir(&linha(1, "Silva")).unwrap();

    let e = t.atualizar(r, &linha(1, "Andrade")).unwrap_err();
    let texto = format!("{e}");
    assert!(texto.contains("balde"), "{texto}");
    assert!(texto.contains("Exclua e insira"), "{texto}");

    // A linha não mudou.
    assert_eq!(t.ler(r).unwrap().unwrap()[NOME], Value::Str("Silva".into()));

    // Mas alterar DENTRO do mesmo balde passa.
    t.atualizar(r, &linha(1, "Silveira")).unwrap();
    assert_eq!(
        t.ler(r).unwrap().unwrap()[NOME],
        Value::Str("Silveira".into())
    );
}

/// O teto passa a ser POR LETRA, e o erro diz qual encheu — «tabela cheia»
/// com 3% de ocupação seria uma mensagem que não ajuda ninguém.
#[test]
fn o_balde_cheio_diz_qual_letra() {
    let dir = DirTemp::novo("cheio");
    let mut t = Table::criar(&dir.0, esquema(3)).unwrap();
    for i in 1..=3 {
        t.inserir(&linha(i, &format!("Silva {i}"))).unwrap();
    }
    let e = t.inserir(&linha(4, "Souza")).unwrap_err();
    let texto = format!("{e}");
    assert!(texto.contains("balde S"), "{texto}");

    // E as outras letras continuam aceitando.
    t.inserir(&linha(5, "Adriano")).unwrap();
}

#[test]
fn atravessa_o_fechamento_e_continua_de_onde_parou() {
    let dir = DirTemp::novo("reabre");
    {
        let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
        for (i, n) in ["Silva", "Souza", "Adriano"].iter().enumerate() {
            t.inserir(&linha(i as i64 + 1, n)).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let mut t = Table::abrir(&dir.0, "clientes").unwrap();
    assert_eq!(
        t.baldes()[18],
        2,
        "o contador do balde S nao voltou do disco"
    );
    assert_eq!(t.baldes()[0], 1);
    assert_eq!(t.rownum_atual(), 4);

    // A próxima Silva entra no slot 3 do balde S, e não por cima de ninguém.
    let novo = t.inserir(&linha(4, "Santos")).unwrap();
    assert_eq!(novo, 18 * 100 + 3);
    assert_eq!(rownum(&mut t, novo), 4);
    assert_eq!(t.varrer().unwrap().len(), 4);
    t.verificar().unwrap();
}

/// O `.pag` descreve o que existe no disco, e é JSON que outro programa lê.
#[test]
fn o_pag_descreve_a_particao() {
    use phxsql_core::json::Json;
    let dir = DirTemp::novo("pag");
    let mut t = Table::criar(&dir.0, esquema(500)).unwrap();
    for (i, n) in ["Silva", "Souza", "Adriano", "#etc"].iter().enumerate() {
        t.inserir(&linha(i as i64 + 1, n)).unwrap();
    }
    t.sincronizar().unwrap();

    let texto = std::fs::read_to_string(dir.0.join("clientes.pag")).unwrap();
    let j = Json::analisar(&texto).expect("o .pag nao e JSON valido");
    assert_eq!(j.texto_ou("modo", ""), "letra");
    assert_eq!(j.texto_ou("coluna_referencia", ""), "nome");
    assert_eq!(j.inteiro_ou("registros", -1), 4);

    let baldes = j.campo("baldes").and_then(Json::lista).unwrap();
    assert_eq!(baldes.len(), 37);
    assert_eq!(baldes[18].texto_ou("letra", ""), "S");
    assert_eq!(baldes[18].inteiro_ou("registros", -1), 2);
    assert_eq!(baldes[18].texto_ou("arquivo", ""), "clientes_S.reg");
    assert_eq!(baldes[18].inteiro_ou("primeiro_rowid", -1), 18 * 500 + 1);

    // Apagar o descritor não quebra a tabela: ele é gerado, não é verdade.
    std::fs::remove_file(dir.0.join("clientes.pag")).unwrap();
    let mut de_novo = Table::abrir(&dir.0, "clientes").unwrap();
    assert_eq!(de_novo.varrer().unwrap().len(), 4);
}

/// Exclusão física continua passando pela lixeira, e o slot do balde não é
/// reaproveitado — a mesma regra de sempre, agora por balde.
#[test]
fn excluir_no_balde_nao_reaproveita_o_slot() {
    let dir = DirTemp::novo("excluir");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
    let a = t.inserir(&linha(1, "Silva")).unwrap();
    t.inserir(&linha(2, "Souza")).unwrap();

    t.excluir_de_vez(a, "duplicidade").unwrap();
    assert_eq!(t.lixeira(0, 0, true).unwrap().len(), 1);

    let nova = t.inserir(&linha(3, "Santos")).unwrap();
    assert_eq!(nova, 18 * 100 + 3, "o slot do Silva foi reaproveitado");
    assert_eq!(t.varrer().unwrap().len(), 2);
}

/// Só o `.reg` se parte por letra. Um `clientes_B.log` se leria como «o
/// diário do balde B», e o diário é da tabela inteira — ele rola por tamanho.
#[test]
fn so_o_reg_leva_a_letra_no_nome() {
    let dir = DirTemp::novo("sufixos");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
    t.inserir(&linha(1, "Silva")).unwrap();
    t.sincronizar().unwrap();

    let tem = |n: &str| dir.0.join(n).exists();
    // O .reg do balde S, com a letra.
    assert!(tem("clientes_S.reg"), "faltou o clientes_S.reg");
    // E os externos com o sufixo numérico, não com letra.
    for ext in ["bin", "memo", "log", "trash", "reason"] {
        assert!(
            tem(&format!("clientes_001.{ext}")),
            "o .{ext} nao saiu com sufixo numerico"
        );
        assert!(
            !tem(&format!("clientes_A.{ext}")),
            "o .{ext} saiu com sufixo de LETRA"
        );
    }
}

/// Os 37 arquivos são UMA tabela na listagem, e não 37. Antes desta correção
/// a árvore da tela mostrava `clientes`, `clientes_A`, `clientes_B`… como
/// tabelas diferentes.
#[test]
fn os_baldes_aparecem_como_uma_tabela_so() {
    use phxsql_store::catalogo::Instancia;
    let dir = DirTemp::novo("listagem");
    let base = dir.0.join("base");
    std::fs::create_dir_all(&base).unwrap();
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();

    let mut t = Table::criar(db.caminho(), esquema(100)).unwrap();
    for (i, n) in ["Silva", "Adriano", "Zeus", "#etc"].iter().enumerate() {
        t.inserir(&linha(i as i64 + 1, n)).unwrap();
    }
    t.sincronizar().unwrap();
    drop(t);

    let tabelas = db.tabelas(None).unwrap();
    assert_eq!(
        tabelas,
        vec!["clientes"],
        "os baldes viraram tabelas: {tabelas:?}"
    );
}

/// E uma tabela que por acaso se chama `dados_X` continua sendo ela mesma:
/// só conta como balde quando o volume 1 (`_A`) está ali do lado.
#[test]
fn tabela_com_nome_de_balde_nao_e_confundida() {
    use phxsql_store::catalogo::Instancia;
    let dir = DirTemp::novo("homonimo");
    let base = dir.0.join("base");
    std::fs::create_dir_all(&base).unwrap();
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();

    // Sem partição: uma tabela comum chamada `dados_X`.
    let e = Schema::new(
        "dados_X",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![],
    )
    .unwrap();
    let mut t = Table::criar(db.caminho(), e).unwrap();
    t.inserir(&[Value::Int(1)]).unwrap();
    t.sincronizar().unwrap();
    drop(t);

    let tabelas = db.tabelas(None).unwrap();
    assert_eq!(tabelas, vec!["dados_X"], "a tabela virou balde de `dados`");
}

/// A bissecção pelo `rownum` NÃO vale aqui, e o motor tem de saber disso.
///
/// A armadilha: `rowid = (balde-1) × registros_por_arquivo + slot`. A Silva
/// digitada primeiro mora no `_S`, com rowid alto; a Alves digitada depois
/// mora no `_A`, com rowid 1. O `rownum` 1 está num rowid MAIOR que o do
/// `rownum` 2 — a sequência não está ordenada, e bissetar devolveria a linha
/// errada em silêncio.
#[test]
fn achar_pelo_numero_de_ordem_continua_certo_com_baldes() {
    let dir = DirTemp::novo("ordem-balde");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();

    // Digitados de trás para frente no alfabeto, de propósito.
    let nomes = ["Zeca", "Silva", "Mendes", "Alves", "Bruno", "Otto", "Ana"];
    for (i, n) in nomes.iter().enumerate() {
        t.inserir(&linha(i as i64 + 1, n)).unwrap();
    }

    // O rowid NÃO cresce com o rownum: é isso que quebra a bissecção.
    let por_ordem: Vec<u64> = (1..=7)
        .map(|n| t.rowid_do_rownum(n).unwrap().unwrap())
        .collect();
    assert!(
        por_ordem.windows(2).any(|p| p[0] > p[1]),
        "os rowids saíram crescentes: o teste deixou de provar o que existe para provar"
    );

    // E mesmo assim cada número de ordem acha a linha dele.
    for (i, esperado) in nomes.iter().enumerate() {
        let r = t.rowid_do_rownum(i as u64 + 1).unwrap().unwrap();
        assert_eq!(rownum(&mut t, r), i as u64 + 1);
        match &t.ler(r).unwrap().unwrap()[NOME] {
            Value::Str(s) => assert_eq!(s, esperado),
            outro => panic!("{outro:?}"),
        }
    }

    // Um alvo que caiu num buraco devolve o PRÓXIMO número, e não o próximo
    // do arquivo: aqui a ordem que manda é a de digitação.
    let terceiro = t.rowid_do_rownum(3).unwrap().unwrap();
    t.excluir_de_vez(terceiro, "").unwrap();
    let r = t.rowid_do_rownum(3).unwrap().unwrap();
    assert_eq!(rownum(&mut t, r), 4);

    // Além do fim, nada.
    assert!(t.rowid_do_rownum(99).unwrap().is_none());
}

/// Aqui a posição na lista nunca é o `rownum`: a leitura sai balde a balde e o
/// `rownum` guarda a digitação. As duas ordens são diferentes de propósito.
#[test]
fn a_particao_por_letra_nao_bisseta_a_posicao() {
    let dir = DirTemp::novo("posicao-balde");
    let mut t = Table::criar(&dir.0, esquema(100)).unwrap();
    for (i, n) in ["Zeca", "Alves", "Mendes"].iter().enumerate() {
        t.inserir(&linha(i as i64 + 1, n)).unwrap();
    }
    assert!(!t.posicao_e_rownum(Visao::Ativas));
    assert!(!t.posicao_e_rownum(Visao::Todas));

    // E a página por posição continua certa — pelo caminho que anda.
    let (p, como) = t.pagina_por_posicao(1, 2, Visao::Ativas).unwrap();
    assert_eq!(como, Salto::Passo);
    assert_eq!(p, t.pagina(1, 2, Visao::Ativas).unwrap());
    // Ordem de LEITURA: Alves, Mendes, Zeca. Pular uma começa em Mendes.
    match &t.ler(p[0]).unwrap().unwrap()[NOME] {
        Value::Str(s) => assert_eq!(s, "Mendes"),
        outro => panic!("{outro:?}"),
    }
}
