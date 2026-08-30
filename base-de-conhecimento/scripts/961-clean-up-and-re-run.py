# Clean up and re-run
# 29/08 00:54

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/indice-adiado.rs")
s = p.read_text()

# tira o resto morto da primeira versao
alvo = '''    let teto = medir("teto", vec![], &ls, false);
    let r_ambos = medir("r-ambos", vec![unico(), nao_unico()], &[], true);
    let adiar_tudo = Medida {
        inserir: teto.inserir,
        // O `reindexar` de uma tabela vazia nao mede nada; o custo real e o de
        // reconstruir sobre as N linhas, medido logo abaixo.
        reindexar: r_ambos.reindexar,
    };
    let _ = adiar_tudo;

    // Reconstrucao medida sobre as N linhas de verdade: insere e reindexa.
    let dois = medir("adiar-dois", vec![unico(), nao_unico()], &ls, true);'''
novo = '''    // So o `.reg`, sem indice nenhum: e o que a carga custaria com o `.ndx`
    // parado.
    let teto = medir("teto", vec![], &ls, false);

    // A reconstrucao e medida sobre as N linhas de verdade, e nao sobre uma
    // tabela vazia -- e ela que entra na conta do adiamento.
    let dois = medir("adiar-dois", vec![unico(), nao_unico()], &ls, true);'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''    println!(
        "\\n  Piso de ~{:.2}s por indice, contra os {custo_de_hoje:.2}s que o\\n  \\
         `reindexar` de hoje custa para os dois. E ai que mora o ganho de\\n  \\
         adiar -- e nao no adiar em si.",
        varrer + ordenar
    );''','''    // A varredura e UMA para os dois indices; a ordenacao e por indice. Um
    // lote de verdade ainda gravaria as paginas, em sequencia: sao poucas
    // milhares, a ~2,3 us de CRC cada.
    println!(
        "\\n  Piso: {:.2}s de varredura (uma so, para os dois) + {:.2}s de\\n  \\
         ordenacao por indice, contra os {custo_de_hoje:.2}s que o `reindexar`\\n  \\
         de hoje cobra pelos dois. E AI que mora o ganho de adiar -- e nao no\\n  \\
         adiar em si, que sozinho vale 1,02x.",
        varrer, ordenar
    );''', 1)
p.write_text(s)
print("ok")
