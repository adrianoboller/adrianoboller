# Atualizar as pendencias e a pagina
# 29/08 07:36

import io,re
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()
linhas=s.split('\n')

novos={
 6: ('☑️','**Servidor MCP** | `phxsqld --mcp` fala JSON-RPC por stdio, com `ExecutorLocal` chamando o `despachar` — o portão continua sendo um — e `stdio` no lugar do IP no log de acessos. O `tools/list` **lê o catálogo de operações** em vez de uma segunda lista escrita à mão. Teste roda o binário de verdade; senha via `PHXSQL_SENHA`, nunca em argumento'),
 83:('☑️','**Comandos SQL reconhecem `matriz.estoque` e `filial.estoque`** | op `sql` ligada ao servidor pelo portão que já existe (`executar_derivado`), com o teste `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` provando nos dois sentidos. Ligar achou o que os 44 testes da crate não podiam achar: `WHERE id = 2` chegava como texto e era recusado — o motor alargou (coluna inteira aceita inteiro em texto, que é o que ODBC vai mandar), o tradutor não apertou'),
}
for i,l in enumerate(linhas):
    m=re.match(r'^\| (☑️|◐|☐) \| (\d+) \|', l)
    if m and int(m.group(2)) in novos:
        e,t=novos[int(m.group(2))]
        linhas[i]=f'| {e} | {m.group(2)} | {t} |'
    if m and int(m.group(2))==127:
        linhas[i]=('| ◐ | 127 | **Diagrama ER e editor de modelo** | o diagrama está feito (`ui/diagrama-er.js`, sete defeitos achados no navegador) e a meia-verdade caiu: **`criar_tabela` agora declara chave estrangeira pelo protocolo**, com `duplicar_tabela` preservando e um teste que trava que *declarar não é aplicar* — o motor não impõe a FK, e o teste falha no dia em que isso mudar. Faltam o editor visual e a tela «Nova tabela» oferecer FK |')

# pedido novo: o console
i129=[i for i,l in enumerate(linhas) if l.startswith('| ☑️ | 129 |')][0]
linhas.insert(i129+1, '| ☑️ | 130 | **`phxsqlcmd`: interface terminal com todos os comandos, `/help` e `/help comando`** | crate `phxsql-cmd`, autenticando pelo mesmo desafio-resposta da réplica. O `/help` **vem do servidor** (op `catalogo`: 79 operações descritas por dados, com um teste que deriva a lista do próprio `despachar` — operação nova não nasce sem descrição, e ajuda escrita à mão não existe para envelhecer). 9 testes por soquete; o soquete achou o que a unidade não achava (o partidor comia as aspas do JSON). Sem histórico/setas nesta rodada, dito no `--help` |')

s='\n'.join(linhas)
feitos=sum(1 for l in linhas if l.startswith('| ☑️ |')); parc=sum(1 for l in linhas if l.startswith('| ◐ |')); plan=sum(1 for l in linhas if l.startswith('| ☐ |'))
s=re.sub(r'\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.',
         f'**{feitos} feitos · {parc} parciais · {plan} planejados**, de {feitos+parc+plan} pedidos.', s)
io.open(p,'w',encoding='utf-8').write(s)
print(f'{feitos} feitos, {parc} parciais, {plan} planejados, de {feitos+parc+plan}')
