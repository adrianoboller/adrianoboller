# Update the pending list
# 28/08 23:09

import pathlib, re
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
antigo = """| ☑️ | 67 | **Botão e menu Tabelas**"""
novo = """| ☑️ | 116 | **Profiler na barra de ferramentas** | vê o que chega pela porta **antes de virar dado** — o ponto de captura é uma linha antes do despacho, então o pedido que trava aparece como «em curso». Filtra por banco, usuário, operação e só-escrita; grava num `.txt` no caminho escolhido. A senha é redigida **analisando** o pedido, nunca recortando o texto; pedido que não é JSON vira o tamanho em bytes. Observa as duas portas e não observa a si mesmo |
| ☑️ | 117 | **Cores da ação nos botões** | verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta. Contorno e não fundo cheio — fundo laranja com texto escuro em cima já tinha ficado ilegível uma vez. No diálogo de excluir o botão troca de cor junto com o texto |
| ☑️ | 118 | **Rodar em Docker** | imagem `scratch`, 4,7 MB, sem shell nem gerenciador de pacotes — só possível por não haver dependência externa. Exige o alvo **musl**: medi, o padrão linka `libc.so.6` e o carregador dinâmico, e `FROM scratch` não subiria. O binário musl roda; o `docker build` **não foi executado** (sem daemon na máquina) |
| ☑️ | 119 | **Várias instâncias em portas diferentes** | já era assim: cada `phxsqld` lê o `config.json` do diretório em que foi iniciado. Provado com quatro de uma vez em `bancada/replicacao/` e com três em contêineres |
| ☑️ | 120 | **Chave composta livre e única** | as duas já existiam no formato; faltava teste que as separasse. A única recusa **antes de gravar**, e a recusa não consome slot |
| ☑️ | 121 | **Analisar o PDF do HFSQL(R) contra o projeto** | `docs/HFSQL.md`, item por item. O que falta, em ordem de valor: direito no nível da **tabela**, índice de texto completo, índice parcial, ordenação linguística, e a **janela de conflito de escrita** |
| ☑️ | 122 | **Analisar o DBeaver: o que dá para reaproveitar** | `docs/DBEAVER.md`. Código: não vale — Apache 2.0 permite, mas seria trazer o Eclipse inteiro. Ferramenta: vale muito, e os três caminhos exigem a **mesma** camada SQL |
| ☐ | 123 | **Janela de conflito de escrita** | a melhor ideia do PDF do HFSQL(R): o segundo a salvar vê «valor anterior / o outro escreveu / você escreve» e escolhe. Hoje a segunda gravação vence em silêncio — e a peça já está no formato: o `.reg` guarda uma **versão por registro**. É o item mais barato com o maior ganho de correção |
| ☐ | 124 | **Direito no nível da tabela** | hoje a permissão para na base: quem lê a base lê todas as tabelas. O portão já existe e é um ponto só |
| ☐ | 125 | **Marcar coluna como dado pessoal (LGPD/GDPR)** | uma marca por coluna e uma tela que audita onde elas estão. O cadastro de campos já tem `caption`, `descricao` e `mascara` — é mais um campo |
| ☐ | 126 | **Cluster: endereço único, eleição e promoção automática** | `docs/CLUSTER.md`. A peça difícil já está pronta — a réplica que alcança sozinha e para quando diverge. Falta o que fica em volta |
| ☐ | 127 | **Diagrama ER e editor de modelo** | as chaves estrangeiras já estão declaradas e já vêm no `esquema`; falta o desenho e a edição visual. É SVG, que é do que o dossiê inteiro é feito |
| ☑️ | 67 | **Botão e menu Tabelas**"""
assert antigo in s
s = s.replace(antigo, novo)
linhas = [l for l in s.splitlines() if re.match(r"^\| (☑️|◐|☐) \| \d+ \|", l)]
from collections import Counter
c = Counter(l.split("|")[1].strip() for l in linhas)
s = re.sub(r"\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.",
           f"**{c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados**, de {len(linhas)} pedidos.", s)
p.write_text(s)
print(f"{c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados, de {len(linhas)}")
