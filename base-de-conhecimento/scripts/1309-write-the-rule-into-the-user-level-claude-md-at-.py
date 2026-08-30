# Write the rule into the user-level CLAUDE.md at its real path
# 30/08 16:42

regra = '''## A base de conhecimento é entregável

**Todo projeto mantém um documento de tecnologias, e ele é obrigatório.** Não é
o `README` (que diz como usar) nem o manual (que diz o que faz): é o inventário
do que se usou **para fazer o produto e para fazer o trabalho** — as duas
metades, porque a segunda é a que se reaproveita e é a que ninguém escreve.

O que ele carrega:

- **Linguagens e volume, contados** — não «usamos Rust», mas quantas linhas e
  onde.
- **Dependências, e o que a escolha comprou ou custou**, em números medidos.
- **O que foi escrito à mão, e as normas conferidas** — com RFC e vetor.
- **As ferramentas do trabalho**: como se orquestrou, como se mediu, como se
  provou, como se compilou para outra arquitetura.
- **O que foi avaliado e RECUSADO, com o número.** É a seção que mais poupa
  tempo depois: recusa medida impede a mesma proposta de voltar.

E o corolário que vale como regra: **script, comando e roteiro que resolveram
algo não podem morrer com a sessão.** Um transcrito de 99 MB não é base de
conhecimento — é matéria-prima. A base sai dele por **extrator**, para que se
refaça na sessão seguinte em vez de envelhecer: base montada à mão é base que
ninguém consegue atualizar.

**Quando escrever:** ao fim de cada rodada, junto do resto da documentação.
Documento de tecnologia adiado é documento que se escreve de memória — e
memória é exatamente o que ele existe para substituir.

'''
p='/root/.claude/CLAUDE.md'
import os
s=open(p,encoding='utf-8').read() if os.path.exists(p) else None
assert s, p+" nao existe"
alvo='## A regra que atravessa os dez'
assert s.count(alvo)==1
open(p,'w',encoding='utf-8').write(s.replace(alvo, regra + alvo))
print("regra escrita em", p)
