# Medir a copia da pagina
# 29/08 04:12

import io
p='crates/phxsql-store/examples/onde-doi.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    let com_crc = m.lidas + m.gravadas;
    println!(
        "\\n  So a leitura do arquivo e a gravacao passam pelo CRC -- {com_crc:.2} paginas\\n  \\
         por linha, ou {:.1} us de CRC, de {dois:.1} us medidos ({:.0}%). O acerto de\\n  \\
         cache custa a copia da pagina, e nao o CRC dela: e dai que veio o ganho.",
        com_crc * por_pagina,
        com_crc * por_pagina / dois * 100.0
    );'''
novo = '''    let com_crc = m.lidas + m.gravadas;
    println!(
        "\\n  So a leitura do arquivo e a gravacao passam pelo CRC -- {com_crc:.2} paginas\\n  \\
         por linha, ou {:.1} us de CRC, de {dois:.1} us medidos ({:.0}%).",
        com_crc * por_pagina,
        com_crc * por_pagina / dois * 100.0
    );
    // O acerto de cache nao paga CRC -- mas paga a COPIA, porque `ler_pagina`
    // devolve `Vec<u8>`. E o numero de acertos cresce com a altura da arvore.
    let copiadas = m.servidas + m.lidas + m.gravadas;
    println!(
        "  E toda pagina que entra ou sai do cache e COPIADA: {copiadas:.2} por\\n  \\
         linha, ou {:.1} us, {:.0}% -- e este cresce com a tabela, porque a\\n  \\
         arvore fica mais alta e a descida toca mais paginas.",
        copiadas * copia_pagina,
        copiadas * copia_pagina / dois * 100.0
    );'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
