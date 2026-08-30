# Close pending item 107
# 28/08 20:02

import pathlib, re
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()

antigo = """| ☐ | 107 | **Salto para uma página específica** | o cursor sabe ir e voltar; ir direto para «a página 500» exigiria contar a tabela, que é o que foi removido. Quem precisa de ponto certo usa `rownum` com a bissecção |"""
novo = """| ☑️ | 107 | **Salto para uma página específica** | `pular` deixou de andar: quando a posição de uma linha **é** o `rownum` dela, o começo da página sai de uma bissecção. Medido em 200.000 linhas pelo protocolo: 6 ms contra 131 ms no fim da tabela, e **plano** com a profundidade. Caixa «ir para a página» na grade — 116 ms no navegador, com o desenho. Contar voltou a ser barato: `visiveis = registros − marcadas`, os dois do cabeçalho |"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("substituido 107")

# achar o maior numero para acrescentar os novos
nums = [int(m) for m in re.findall(r"^\| [^|]+ \| (\d+) \|", s, re.M)]
print("maior:", max(nums))
