# Atualizar a prosa do medidor
# 29/08 03:30

import io
p='crates/phxsql-store/examples/indice-adiado.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    println!(
        "\\n  Piso: {varrer:.2}s de varredura -- uma so, para os dois -- mais\\n  \\
         {ordenar:.2}s de ordenacao por indice, contra os {custo_de_hoje:.2}s que o\\n  \\
         `reindexar` de hoje cobra pelos dois. E AI que mora o ganho de adiar,\\n  \\
         e nao no adiar em si, que sozinho vale 1,02x."
    );'''
novo = '''    println!(
        "\\n  Piso: {varrer:.2}s de varredura -- uma so, para os dois -- mais\\n  \\
         {ordenar:.2}s de ordenacao por indice, contra os {custo_de_hoje:.2}s que o\\n  \\
         `reindexar` cobra pelos dois. Foi para ca que a construcao em lote\\n  \\
         trouxe o `reindexar`: ele insere as chaves ORDENADAS, enchendo folha\\n  \\
         por folha, e nao mais uma descida na arvore por chave."
    );'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2 = '''//! O tempo de reconstrucao e medido de verdade, com `reindexar()`, e entra na
//! conta. Adiar nao apaga trabalho -- move de lugar e faz em lote.'''
novo2 = '''//! O tempo de reconstrucao e medido de verdade, com `reindexar()`, e entra na
//! conta. Adiar nao apaga trabalho -- move de lugar e faz em lote.
//!
//! # O que mudou desde a primeira corrida
//!
//! Este medidor ja disse que adiar valia **1,02x**, e a conclusao estava certa
//! para o `reindexar` daquele dia: ele inseria chave a chave, uma descida na
//! arvore por chave -- exatamente o trabalho que se queria evitar. Com a
//! construcao em lote (`NdxFile::construir_em_lote`) o `reindexar` passou a
//! encher folha por folha a partir das chaves ordenadas, e o mesmo medidor
//! passou a dizer **3,28x** para o teto e **1,59x** para o caminho que nao abre
//! mao da unicidade. Nao foi o adiamento que mudou; foi o preco do fim.'''
assert s.count(velho2)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho2,novo2))
print('ok')
