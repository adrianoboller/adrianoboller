# Update the pending list
# 28/08 17:07

p='docs/PENDENCIAS.md'
s=open(p).read()
a='''| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
b='''| ☑️ | 93 | **Exportar as tabelas para xlsx, json, xml, html, csv, docx e txt** | os sete, escritos aqui. XLSX e DOCX são ZIP de XML, e o projeto já escrevia ZIP com DEFLATE — planilha com cabeçalho pintado, zebra, painel congelado e autofiltro; data como número com formato, não como texto. Conferido com leitores independentes |
| ☑️ | 94 | **O dossiê estava esquecendo o `.bkp`** | e no pior lugar: a seção do **fluxo de gravação**. O espelho não aparecia no desenho, e parecia uma cópia feita depois — ele é escrito no mesmo instante. Corrigido no dossiê, no `FORMATO.md`, no `MANUAL.txt` e no `README` |
| ◐ | 95 | **Integrar o MULTILINK no DbLink** | **bloqueado como está**: o pacote traz só binários (`.rlib`), sem fonte, compilados com rustc 1.98 contra o 1.94 daqui — provado, não suposto. E um `.rlib` é dependência externa, que a regra do projeto proíbe. O caminho que funciona está descrito em `docs/MULTILINK.md`: falar com ele por **protocolo**, e não por link |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
assert a in s; s=s.replace(a,b,1)
a='''**82 feitos · 4 parciais · 6 planejados**, de 92 pedidos.'''
b='''**84 feitos · 5 parciais · 6 planejados**, de 95 pedidos.'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
