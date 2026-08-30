# Rewrite the attachment test
# 28/08 20:12

import pathlib
p = pathlib.Path("crates/phxsql-store/tests/replicacao.rs")
s = p.read_text()
i = s.index("/// Os anexos são o caso")
j = s.index("/// Réplica que divergiu")
novo = '''/// Os anexos são o caso em que copiar o ponteiro daria bloco errado: os
/// offsets do `.bin` do source não valem no `.bin` da réplica. A imagem leva o
/// CONTEÚDO, e este teste prova que o conteúdo está dentro dela.
#[test]
fn os_anexos_atravessam_com_conteudo_e_nao_com_ponteiro() {
    let ds = DirTemp::novo("rep-anexo-s");
    let dr = DirTemp::novo("rep-anexo-r");
    let (mut s, mut r) = par(&ds, &dr);

    // Anexos antes, para a linha que interessa cair longe do começo do `.bin`
    // e do `.memo` do source — e o ponteiro dela não ser um número pequeno.
    for i in 1..=6 {
        s.inserir(&linha(i)).unwrap();
    }
    let foto = vec![0xABu8; 5000];
    let ficha = "memo grande ".repeat(500);
    s.inserir(&vec![
        Value::Int(7),
        Value::Str("Com anexo".into()),
        Value::Decimal(0),
        Value::Memo(ficha.clone()),
        Value::Bin(foto.clone()),
    ])
    .unwrap();

    // A imagem do último evento: o conteúdo tem de estar DENTRO dela.
    let eventos = s.diario_com_imagem(6, 0).unwrap();
    assert_eq!(eventos.len(), 1);
    let (_, imagem) = &eventos[0];
    let (payload, externos) = Table::abrir_imagem(imagem).unwrap();
    assert!(
        imagem.len() > payload.len() + foto.len() + ficha.len(),
        "a imagem tem {} bytes: cabe o ponteiro, não cabe o anexo",
        imagem.len()
    );
    assert_eq!(externos.len(), 2, "as duas colunas externas");
    let memo = externos.iter().find(|(c, _)| *c == 3).unwrap();
    let bin = externos.iter().find(|(c, _)| *c == 4).unwrap();
    assert_eq!(memo.1, ficha.as_bytes(), "o memo não é o conteúdo");
    assert_eq!(bin.1, foto, "o binário não é o conteúdo");

    // E do outro lado a linha volta inteira, com blocos gravados aqui.
    replicar(&mut s, &mut r, 0);
    let l = r.ler(7).unwrap().unwrap();
    assert_eq!(l[3], Value::Memo(ficha));
    assert_eq!(l[4], Value::Bin(foto));
    r.verificar().unwrap();
}

'''
s = s[:i] + novo + s[j:]
p.write_text(s)
