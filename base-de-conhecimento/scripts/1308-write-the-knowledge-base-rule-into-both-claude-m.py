# Write the knowledge-base rule into both CLAUDE.md files
# 30/08 16:42

regra = '''
### A base de conhecimento é entregável, não sobra

**Todo projeto mantém um documento de tecnologias, e ele é obrigatório.** Não é
o `README` (que diz como usar) nem o manual (que diz o que faz): é o inventário
do que se usou **para fazer o produto e para fazer o trabalho** — as duas
metades, porque a segunda é a que se reaproveita e é a que ninguém escreve.

O que ele carrega:

- **Linguagens e volume, contados** — não «usamos Rust», mas quantas linhas e onde.
- **Dependências, e o que a escolha comprou ou custou.** Se há uma decisão de
  arquitetura por trás (aqui, zero dependências externas), o documento diz o
  que ela pagou em números medidos.
- **O que foi escrito à mão, e as normas conferidas** — com RFC e vetor.
- **As ferramentas do trabalho**: como se orquestrou, como se mediu, como se
  provou, como se compilou para outra arquitetura.
- **O que foi avaliado e RECUSADO, com o número.** Esta seção é a que mais
  poupa tempo depois: recusa medida impede a mesma proposta de voltar.

E o corolário que vale como regra: **script, comando e roteiro que resolveram
algo não podem morrer com a sessão.** Um transcrito de 99 MB não é base de
conhecimento — é matéria-prima. A base sai dele por **extrator**, para que se
refaça na sessão seguinte em vez de envelhecer: base montada à mão é base que
ninguém consegue atualizar.

**Quando escrever:** ao fim de cada rodada de trabalho, junto do restante da
documentação. Documento de tecnologia adiado é documento que se escreve de
memória — e memória é exatamente o que ele existe para substituir.

'''
# Projeto: entra na clausula petrea, ao lado dos dez papeis.
p='/home/user/adrianoboller/CLAUDE.md'
s=open(p,encoding='utf-8').read()
alvo='### J — Pesquisador'
assert s.count(alvo)==1
i=s.index('\n---\n', s.index(alvo))
open(p,'w',encoding='utf-8').write(s[:i] + '\n' + regra + s[i:])
print("regra escrita no CLAUDE.md do projeto")

# Usuario: vale para todos os projetos.
p2='/home/user/.claude/CLAUDE.md'
s2=open(p2,encoding='utf-8').read()
alvo2='## A regra que atravessa os dez'
assert s2.count(alvo2)==1
open(p2,'w',encoding='utf-8').write(s2.replace(alvo2, '## A base de conhecimento é entregável\n' + regra.replace('### A base de conhecimento é entregável, não sobra\n','') + '\n' + alvo2))
print("regra escrita no ~/.claude/CLAUDE.md (todos os projetos)")
