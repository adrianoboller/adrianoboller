# Fix the aria-labels and re-render
# 29/08 02:03

import pathlib
p = pathlib.Path("$SC/caminho-da-insercao.html")
s = p.read_text()
s = s.replace("O custo está concentrado no ponto ndx: 10,9 dos 17,0 microssegundos.",
              "O custo está concentrado no ponto ndx: 11,1 dos 15,9 microssegundos.")
s = s.replace("Barra empilhada dos 17,0 microssegundos de uma inserção: o ponto reg mais o ponto log ocupam 5,4 microssegundos ou 31,8 por cento, o primeiro índice 5,4 microssegundos ou 31,8 por cento, a conferência da chave única 0,7 microssegundos ou 4 por cento, e o segundo índice 5,5 microssegundos ou 32,4 por cento. Somados, os índices e a conferência são 64,2 por cento do total.",
              "Barra empilhada dos 15,9 microssegundos de uma inserção: o ponto reg mais o ponto log ocupam 4,8 microssegundos ou 30,3 por cento, o primeiro índice 5,4 microssegundos ou 33,9 por cento, a conferência da chave única 0,3 microssegundos ou 1,9 por cento, e o segundo índice 5,4 microssegundos ou 34 por cento. Somados, os índices e a conferência são 69,8 por cento do total.")
s = s.replace("somando 4,8 microssegundos ou 28 por cento do custo de uma inserção.",
              "somando 4,8 microssegundos ou 30 por cento do custo de uma inserção.")
p.write_text(s)
