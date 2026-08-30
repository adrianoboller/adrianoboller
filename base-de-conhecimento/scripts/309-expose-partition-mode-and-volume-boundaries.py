# Expose partition mode and volume boundaries
# 28/08 11:22

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''                        ("digitos", Json::de_u64(pag.digitos as u64)),'''
n = '''                        ("digitos", Json::de_u64(pag.digitos as u64)),
                        (
                            "modo",
                            Json::texto_de(match pag.modo.periodo() {
                                None => "quantidade".to_string(),
                                Some(p) => p.nome().to_string(),
                            }),
                        ),
                        (
                            "coluna",
                            match pag.modo.coluna() {
                                None => Json::Nulo,
                                Some(i) => Json::texto_de(&e.colunas()[i].nome),
                            },
                        ),'''
assert s.count(v) == 1
s = s.replace(v, n)

# --------------------------------------------------- as fronteiras de volume
v = '''            ("chaves_estrangeiras", Json::Lista(fks)),'''
n = '''            ("chaves_estrangeiras", Json::Lista(fks)),
            // Na particao por periodo o volume nao sai de conta: quem sabe
            // onde cada faixa comeca e a tabela de fronteiras, lida dos
            // cabecalhos. Sem isto a tela teria de adivinhar.
            (
                "volumes",
                Json::Lista(
                    t.fronteiras()
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            Json::objeto(vec![
                                ("volume", Json::de_u64(i as u64 + 1)),
                                ("primeiro_rowid", Json::de_u64(f.primeiro_rowid)),
                                (
                                    "periodo",
                                    match pag.modo.periodo() {
                                        None => Json::Nulo,
                                        Some(p) => Json::texto_de(p.rotulo(f.chave_periodo)),
                                    },
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        let pag = e.paginacao();'''
n = '''        let pag = e.paginacao();
        let _ = &pag;'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
