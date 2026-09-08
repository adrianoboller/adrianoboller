# Bancada do PITR — restaurar a um INSTANTE

```bash
cargo build --release
python3 bancada/pitr/provar.py
```

Sai `resultados.json`, com a data em que foi medido dentro do próprio arquivo
— e não pelo `mtime`, porque juntar corridas de dias diferentes sem dizer
quando publica um retrato que nunca existiu.

## O que ela prova

A história é uma só: linha 1, cópia, linha 2, **alteração da 1**, linha 3. Com
`ate` entre a alteração e a inclusão da terceira, o restaurado tem a 1
**alterada** e a 2, e **não** tem a 3.

Vão junto as quatro recusas pelo mesmo soquete — `ate` antes da cópia, `ate` e
`ate_ms` no mesmo pedido, fuso escrito à mão, e o modo por cima —, mais a
conferência de que **nenhuma delas deixou database para trás**: recusa que já
criou o banco não é recusa, é estrago com mensagem.

## Por que ela existe, se já há teste de unidade

Porque a linha do PITR no `docs/COMPARATIVO.md` era uma **sonda de código**, e
sonda de código responde «o campo existe» — que não é a mesma pergunta que «a
linha 3 ficou de fora». *Célula que virou TEM por edição do medidor sem prova
de efeito é o erro que a § 1 daquele documento já pagou cinco vezes.*

E porque a receita escrita em `docs/RESTAURACAO.md` § 7.10 precisa **rodar**,
e não só estar escrita: roteiro que resolveu algo não morre com a sessão, e
receita errada no documento vira sonda quebrada na bancada.

## O controle positivo, e por que ele é a metade que importa

O **mesmo** backup restaurado **sem** `ate` volta com uma linha só, na mesma
corrida. Sem ele, uma restauração que devolvesse duas linhas por acaso
passaria — e uma bancada que só sabe dizer «deu certo» não sabe dizer nada.

Prova real nos dois sentidos, medida em 08/09/2026: com o filtro de carimbo
desligado no servidor (`if false && e.carimbo > ate_ms`), a bancada acusa
**3 falhas** e mostra a linha 3 dentro do restaurado. Verde: **22 conferências,
zero falhas.**

## As duas armadilhas, e as duas já pagas

1. **O relógio precisa andar** entre as escritas. Três inserções seguidas caem
   no mesmo milissegundo, e aí não existe instante entre a alteração e a
   última linha. A bancada espera **e confere** que andou antes de cortar — se
   não andou, falha dizendo isso, em vez de medir outra coisa.
2. **O corte sai do próprio diário**, medido. Um `ate` digitado à mão seria um
   número que ninguém mediu.

## Onde o binário está

O `target` costuma ser **compartilhado** entre árvores de trabalho nesta
máquina. Quem manda é o `CARGO_TARGET_DIR`, o mesmo que o `cargo build`
obedece:

```bash
CARGO_TARGET_DIR=/caminho/do/target python3 bancada/pitr/provar.py
```

Sem ele, a bancada procura `target/` ao lado do fonte — e recusa dizendo o
comando, em vez de medir um binário que não existe. *Medidor com binário velho
mede o passado.*

## E o servidor sobe com a imagem ligada

`"replicacao": {"imagem_da_linha": true}` entra no `config.json` **antes** de
o servidor subir: a imagem vale para o que for gravado daí em diante, e não
para o diário que já está no disco. Sem ela o PITR recusa nomeando o
interruptor — e essa recusa tem teste próprio, no `servidor.rs`.

O servidor morre pelo **PID** que o script guardou. Nunca `pkill`: o processo
pode ser de outro agente.
