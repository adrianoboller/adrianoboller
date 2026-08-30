# Record the round's lessons in CLAUDE.md
# 29/08 00:06

import pathlib
p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()
alvo = '''**Número digitado à mão envelhece calado.**'''
novo = '''**Guarda nova entra pedida, não imposta.** A janela de conflito de escrita
podia recusar toda gravação sem versão — e aí todo cliente escrito antes dela
pararia de gravar de um dia para o outro, recebendo um erro que não sabe tratar.
Quem manda `"versao"` ganha a garantia; quem não manda continua como antes; a
interface web manda sempre, porque é onde existe gente e existe a janela de
minutos entre abrir a ficha e clicar em salvar. **Proteção que quebra todo
cliente antigo não é proteção, é estrago** — e o teste que trava isso é o do
comportamento *velho*, não o do novo.

**Merge de conflito marca quem MEXEU, não quem perguntou por último.** Deixar
«o meu» marcado em todas as colunas desfaria em silêncio o trabalho do outro
nas colunas que eu nem toquei — o mesmo estrago de antes, com mais cliques. O
padrão certo é por coluna: a que eu digitei fica comigo, a que só o outro mudou
fica com ele. Dois que editaram campos diferentes saem com os dois trabalhos e
sem escolher nada.

**O CSS global morde todo componente novo da tela.** `input{width:100%}` e
`label{text-transform:uppercase}` são certos para um formulário e errados
dentro de uma tabela: o rádio virou uma bolinha do tamanho da célula, e
«Blumenau» apareceu como «BLUMENAU» — que é uma **mentira sobre o dado**, porque
quem olha não sabe se está gravado assim. Nenhum dos dois aparece lendo o
código. Componente novo se abre no navegador e se olha, e é a mesma lição do
vídeo por outro caminho.

**Número digitado à mão envelhece calado.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
