# Fix the summary line and rerun
# 29/08 00:55

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/indice-adiado.rs")
linhas = p.read_text().split("\n")
# linhas 245..248 (1-based) -> indices 244..247
novo = '''    // A varredura e UMA para os dois indices; a ordenacao e por indice. Um
    // lote de verdade ainda gravaria as paginas, mas em SEQUENCIA: sao poucos
    // milhares, a ~2,3 us de CRC cada.
    println!(
        "\\n  Piso: {varrer:.2}s de varredura -- uma so, para os dois -- mais\\n  \\
         {ordenar:.2}s de ordenacao por indice, contra os {custo_de_hoje:.2}s que o\\n  \\
         `reindexar` de hoje cobra pelos dois. E AI que mora o ganho de adiar,\\n  \\
         e nao no adiar em si, que sozinho vale 1,02x."
    );'''
linhas[244:248] = novo.split("\n")
p.write_text("\n".join(linhas))
print("ok")
