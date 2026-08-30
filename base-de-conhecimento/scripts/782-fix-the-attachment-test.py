# Fix the attachment test
# 28/08 20:11

import pathlib
p = pathlib.Path("crates/phxsql-store/tests/replicacao.rs")
s = p.read_text()
i = s.index("/// Os anexos são o caso")
j = s.index("/// Réplica que divergiu")
novo = '''/// Os anexos são o caso em que copiar o ponteiro daria bloco errado. A imagem
/// leva o CONTEÚDO, e a réplica grava no `.bin`/`.memo` dela — com ponteiros
/// que são dela.
#[test]
fn os_anexos_atravessam_com_conteudo_e_nao_com_ponteiro() {
    let ds = DirTemp::novo("rep-anexo-s");
    let dr = DirTemp::novo("rep-anexo-r");
    let (mut s, mut r) = par(&ds, &dr);

    // O source já tem anexos gravados: a linha que interessa vai cair LONGE do
    // começo do `.bin` e do `.memo` dele.
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

    // A réplica recebe SÓ o último evento, num `.bin` e num `.memo` vazios:
    // os ponteiros dela têm de sair diferentes dos do source.
    let eventos = s.diario_com_imagem(6, 0).unwrap();
    assert_eq!(eventos.len(), 1);
    let (e, imagem_source) = &eventos[0];
    // O rowid tem de bater, e aqui bate por acaso: a réplica está vazia e este
    // é o primeiro registro dela. É o que permite aplicar só este evento.
    r.aplicar_evento(e.operacao, e.rowid, imagem_source).unwrap();

    let l = r.ler(1).unwrap().unwrap();
    assert_eq!(l[3], Value::Memo(ficha), "o memo não atravessou inteiro");
    assert_eq!(l[4], Value::Bin(foto), "o binário não atravessou inteiro");

    // E a prova de que o ponteiro NÃO foi copiado: os payloads crus diferem,
    // porque cada lado guarda o offset do bloco na máquina dele.
    let imagem_replica = {
        let p = r.imagem_da_linha_do_rowid(1).unwrap();
        p
    };
    let (payload_s, _) = Table::abrir_imagem(imagem_source).unwrap();
    let (payload_r, _) = Table::abrir_imagem(&imagem_replica).unwrap();
    assert_eq!(payload_s.len(), payload_r.len());
    assert_ne!(
        payload_s, payload_r,
        "os payloads saíram iguais: o ponteiro do source teria sido copiado"
    );

    // A réplica confere o CRC dos próprios blocos.
    r.verificar().unwrap();
}

'''
s = s[:i] + novo + s[j:]
p.write_text(s)
