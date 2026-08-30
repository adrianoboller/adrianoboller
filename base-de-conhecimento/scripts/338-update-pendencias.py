# Update PENDENCIAS
# 28/08 11:49

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()

# ------------------------------------------------------------ linhas novas
v = '''| ☑️ | 67 | **Botão e menu Tabelas**'''
n = '''| ☑️ | 68 | **Copiar e colar tabela** de um lugar para outro | `copiar_tabela` atravessa databases e schemas; a permissão de criar é conferida **no destino** |
| ☑️ | 69 | **Configurações gerais do servidor, do banco e dos usuários**, cada uma com sua tela | três telas, três alcances. **Leem, não gravam** — gravar o `config.json` pela web daria a uma sessão roubada o poder de abrir o firewall e criar supervisor |
| ☑️ | 70 | **SysTables e SysColumns** | o catálogo em forma de dado, e o dicionário de dados com id, caption, descrição, máscara e papel na chave |
| ☑️ | 71 | **Gerir database**: conexões, triggers, procedures, arquivos bloqueados, modo exclusivo, transações, backup/restaure e jobs | 15 itens numa tela; 11 funcionam, 4 apagados dizendo o que falta e de que dependem |
| ☑️ | 72 | **Diretivas de acesso ao banco e diretivas de acesso** | os seis portões na ordem em que fecham, e quem alcança o banco resolvido pelas três regras |
| ☑️ | 73 | **Editor de menu** para trocar o nome exibido | 81 rótulos; fica no navegador de quem mexeu, não no servidor |
| ☑️ | 74 | **Configurações e diretivas das tabelas** | a geometria decidida na criação, os índices e chaves, e o que a tabela herda do servidor |
| ☑️ | 75 | **Cadastro de campos** com id automático, nome, caption, descrição, tipo, tamanho, máscara e chave primária/estrangeira/composta | **mudança de formato**: esquema `PSCH` v3. O `id` é UUID v7 e nunca muda; o papel na chave é derivado dos índices |
| ☑️ | 76 | **Tabela particionada** com grade de gestão: por faixa de quantidade, mensal, bimestral, semestral ou anual | **mudança de formato**: o volume corta pelo calendário, e cada volume grava a própria fronteira no cabeçalho |
| ☑️ | 67 | **Botão e menu Tabelas**'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''**59 feitos · 2 parciais · 6 planejados**, de 67 pedidos.'''
n = '''**68 feitos · 2 parciais · 6 planejados**, de 76 pedidos.'''
assert s.count(v) == 1
s = s.replace(v, n)

# --------------------------------------------- o planejado que mudou de forma
v = '''| 1 | **Jobs de execução** | é o mais barato dos três, e ficou para depois do painel |'''
n = '''| 1 | **Jobs de execução** | é o mais barato dos três; tem tela apagada em *Gerir banco* dizendo o que falta |'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''| 2 | **Triggers** | onde disparar já existe'''
n = '''| 2 | **Triggers** | tem tela apagada em *Gerir banco*; onde disparar já existe'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''| 13 | TLS | — |'''
n = '''| 13 | Modo exclusivo | tem tela apagada em *Gerir banco* | reservar uma tabela por um período. Hoje a trava única já serializa as escritas, mas não há como RESERVAR — depende da trava por tabela, que é o mesmo trabalho da concorrência fina |
| 14 | Restaurar backup | o *Backup e restauração* mostra o item apagado | copiar de volta é mais do que copiar: é decidir o que fazer com o que está lá. Sobrescrever um database em uso, com a trava tomada, precisa de um desenho — parar, restaurar ao lado e trocar, ou restaurar com outro nome |
| 15 | Editar `config.json` e usuários pela web | as telas leem e dizem qual campo mexer | gravar credencial e política por HTTP precisa de desenho próprio: quem pode, o que fica no log, e a senha nunca em claro em ponto nenhum do caminho |
| 16 | TLS | — |'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
