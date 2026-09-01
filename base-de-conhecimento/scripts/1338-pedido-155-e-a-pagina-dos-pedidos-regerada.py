# Pedido 155 e a pagina dos pedidos regerada
# 01/09 18:40

from pathlib import Path
p = Path("docs/PENDENCIAS.md")
s = p.read_text(encoding="utf-8")
marca = "\n<!-- pedidos:contagem:inicio -->"
assert s.count(marca) == 1

linha = (
    "| ☑️ | 155 | **Os três motores medidos no mesmo trabalho, a um milhão de "
    "linhas** | `bancada/comparacao/`, três rodadas com PhxSql, MySQL(R) e "
    "SQLite(R) **intercalados na mesma rodada** — somar as duas bancadas que já "
    "existiam daria três colunas e nenhuma comparação, porque medidas de dias "
    "diferentes carregam o ambiente junto. Medianas: inserir 1.000.000 em "
    "**9,93 s** contra 2,56 do SQLite(R) e 12,34 do MySQL(R); buscar 20.000 em "
    "**164 ms** contra 166 e 2,48 s; atualizar em **277 ms** contra 1,03 e 3,54 s; "
    "excluir em **1,05 s** contra 574 ms e 4,06 s. **O achado é o piso:** o "
    "MySQL(R) é o único que recebe o trabalho como texto por soquete, e 20.000 "
    "instruções que não fazem nada (`DO 1;`) custam **1,479 s** — **59,6% da "
    "barra de busca dele**. Sem medir isso teríamos publicado «15,16× mais "
    "rápido» quando entre motores são **6,12×**: mais da metade da vitória era "
    "do formato, não do motor. **E onde perdemos está dito:** a inserção para o "
    "SQLite(R) por 3,88×, a exclusão por 1,83×, e o disco — 253,6 MiB contra "
    "57,3 e 104,0, que é **4,42×** e **2,44×**, o preço do modelo de arquivos "
    "separados. A busca **empata** (as faixas se cruzam), e por isso o gráfico "
    "passou a só contornar vencedor quando elas *não* se cruzam. **A regra 1 "
    "estava sendo violada e nenhum tempo denunciava:** a `bancada/medir.py` "
    "grava `'2024-10-04'` em toda linha enquanto os outros dois gravam "
    "`20000 + (i % 400)` — dado diferente, do mesmo tamanho. Nasceu a fase "
    "`conferir` do `carga.rs`, que obriga os três ao mesmo estado (contagem, "
    "soma de `valor`, soma de `cadastro`) em três marcos, e os totais conferem "
    "contra a **forma fechada** calculada à parte (410.099.600.000 e "
    "20.199.500.000). Prova real nos dois sentidos: repor a data constante faz "
    "a bancada **recusar publicar**. Exercitar a página achou cinco defeitos que "
    "ler não acharia — rótulo por cima do bigode, uma rodada fora da curva "
    "esmagando o painel, vencedor declarado dentro do ruído, a nota do UPDATE "
    "dizendo «uma coluna» quando são todas, e a página sem dizer que as fases "
    "pontuais são 20.000 e não um milhão. `docs/DESEMPENHO.md` §13, "
    "`bancada/comparacao/LEIA-ME.md` |\n"
)
s = s.replace(marca, "\n" + linha + marca, 1)
p.write_text(s, encoding="utf-8")
print("PENDENCIAS: pedido 155")
