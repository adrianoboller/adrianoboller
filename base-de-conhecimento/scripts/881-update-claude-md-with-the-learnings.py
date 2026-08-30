# Update CLAUDE.md with the learnings
# 28/08 23:08

import pathlib
p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()

antigo = """**Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do
protocolo. Há teste que falha se a ficha de usuário vazar o hash."""
novo = """**Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do
protocolo. Há teste que falha se a ficha de usuário vazar o hash.

E o corolário, que o Profiler obrigou a escrever: **funcionalidade que mostra
texto cru redige ANALISANDO, nunca recortando.** Recortar depende de o pedido
estar escrito de um jeito; analisar e reserializar não. O que não se analisa
não vira texto — vira o tamanho em bytes. Se a estrutura não se lê, não há como
tapar o campo dentro dela."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """**Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar o
formato é barato; depois vira migração. Foi assim com o volume no ponteiro."""
novo = """**Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar o
formato é barato; depois vira migração. Foi assim com o volume no ponteiro.

**Receita de fora se mede contra o nosso gargalo antes de virar plano.** Chegou
uma arquitetura completa para acelerar escrita — WAL sequencial, group commit,
MemTable, LSM. É uma boa receita **para o gargalo que ela descreve**, que é o
`fsync` do InnoDB. Medi o nosso antes de aceitar: **83,5% do tempo de uma
inserção está no `.ndx`**, e o arquivo de dados já é *append-only* e custa
16,5%. Das dez propostas, cinco já existiam aqui, duas miravam um problema que
não temos, uma quebraria a ordem de digitação, e duas eram reais. Está em
`phxsql/docs/DESEMPENHO.md`, com o medidor (`--example onde-doi`).

**Interface só se prova exercitando.** Gravar um vídeo de demonstração achou
**três defeitos em cinco minutos** que ler o código não acharia — e o pior deles
quebrava *todo salvar e todo incluir* pela tela desde que o `rownum` entrou. O
padrão dos três é o mesmo: **coluna de sistema nova quebra quem filtra pela
primeira**. Quando entrar uma peça no fim de uma lista, procure quem usa
`find(...)` onde devia usar `filter(...)`.

**Número digitado à mão envelhece calado.** O selo da capa do dossiê passou
**quatro lançamentos** dizendo 0.11.0 — e o script que existe justamente para
impedir isso não cobria aquele pedaço. Todo número visível ou sai de um gerador,
ou está errado e ninguém percebeu ainda."""
assert antigo in s
s = s.replace(antigo, novo)

# Estilo: a licao das cores
antigo = """## Estilo"""
novo = """## Cores da ação, na interface

Convenção decidida e aplicada: **verde inclui, amarelo altera, rosa marca (o
excluir que volta), vermelho exclui de vez, azul consulta.**

Sempre **contorno, nunca fundo cheio** — a lição já estava num comentário do
CSS antes de virar regra: fundo laranja com texto escuro em cima ficava
ilegível, e foi assim que o botão de excluir apareceu. O preenchimento só
acontece no `hover`, quando há intenção. No tema claro as cinco escurecem, pelo
mesmo motivo do vermelhão da marca: verde e rosa claros não passam de 4,5:1
sobre papel.

## Estilo"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
