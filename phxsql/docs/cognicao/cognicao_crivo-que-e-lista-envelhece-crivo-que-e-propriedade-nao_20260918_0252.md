# Crivo que é lista envelhece; crivo que é propriedade, não

**18/09/2026, 02:52** — hora da descoberta, integrando a rodada dos pareceres.

## 1. O que aconteceu

O `comunicacao.sh` imprimiu «· nada compilando nem rodando agora» com a
**corrente dos geradores viva**, escrevendo o dossiê. Medido no momento:

```
 4786  02:20  python3 docs/dossie/portao-dos-geradores.py
 4790  02:20  python3 .../docs/tecnologias/extrair.py
./bancada/esta-medindo.sh  →  saída 1   («não há medição»)
o crivo por `comm` (cargo/rustc/node/phxsqld) → 0 processos
```

É a **sexta** vez que o crivo curto mente nesta base, e a primeira em que ele
mente sobre a corrente dos geradores. O crivo 2 do `esta-medindo.sh` era:

```sh
case "$linha" in
*bancada/*.py*) motivo='bancada em python' ;;
esac
```

E os geradores moram em `docs/`. Nunca houve como ele os ver.

Isto não é cosmético: o comentário do próprio `comunicacao.sh` chama essa frase
de **a pior mentira que este relatório pode contar, «porque é a que faz o
próximo agente rodar por cima da medição»**. Trocando «medição» por «escrita do
dossiê», a frase vale inteira — o zelador ou um build por cima de uma página
sendo gravada é a mesma colisão.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o conserto era **acrescentar `docs/` à lista**. Escrevi a linha
mentalmente antes de parar: `*bancada/*.py*|*docs/*.py*`.

Estava errado, e o erro tem nome nesta casa: é a **lista usada como
inventário**. Uma lista de duas pastas protege menos que uma de uma só no dia
em que a pasta três aparecer — e aparece, porque `bancada/`, `docs/` e
`testes-web/` não são o fim de nada. A pétrea do portão único já diz o mesmo de
outro jeito: *não espalhe o portão por quarenta operações, porque a que alguém
esquecer vira a porta dos fundos*.

O segundo erro, menor e que só a medição pegou: pensei em casar o **texto** do
`cmdline` contra a raiz absoluta do repositório. Não casa — quem roda
`python3 docs/dossie/gerador.py` da raiz escreve um caminho **relativo**, e um
crivo que nunca casa é pior que crivo nenhum, porque parece consertado.

## 3. O que a medição disse

O crivo certo não é uma lista de pastas: é **a propriedade «este script é do
nosso repositório»**, resolvida contra o `/proc/<pid>/cwd` do próprio processo.
Script de ninguém mora fora do repositório; script nosso rodando é exatamente o
que se quer saber. A lista desaparece, e com ela a pasta N+1.

Prova real nos **três** sentidos, na mesma corrida:

| sentido | o que rodava | saída |
|---|---|---|
| **VERMELHO** (crivo antigo) | portão + `extrair.py` vivos | 1 — a mentira |
| **VERDE** (crivo novo) | os mesmos dois | 0, nomeados — e pegou a forma **relativa** e a **absoluta** |
| **CONTROLE** | nada nosso | 1 |
| **CONTROLE 2** | `python3` de verdade rodando `.py` **fora** do repositório, com `cwd` **dentro** dele | 1 |

O controle 2 é o que importa, e ele quase não existiu: na primeira tentativa eu
lancei o processo com `(cd / && python3 ...&)`, e o `&&` impede o `exec` — o
processo continuou sendo `bash`, e o controle **não testou nada**. Só vi porque
fui olhar o `ps`: o `exe` era `bash`, não `python3`. **Controle positivo que não
exercita o caminho dá zero por cegueira**, e zero por cegueira é pior que zero
ausente.

E o rótulo foi junto: o cabeçalho dizia «⏳ BANCADA MEDINDO» e o motivo logo
abaixo passaria a dizer «frente em python do repositório». Duas frases sobre o
mesmo fato, uma contradizendo a outra — o defeito que esse arquivo já pagou
quando o cabeçalho dizia «3 processos» e a lista vinha vazia. Virou «⏳ FRENTE
VIVA», que é o que se mede.

## 4. A regra

> **Quando um crivo precisar de uma pasta nova, não acrescente a pasta:
> pergunte que PROPRIEDADE distingue o que deve casar.** Lista é o lugar onde o
> item N+1 se esquece; propriedade não tem item N+1.

## 5. Como está guardado hoje

- `bancada/esta-medindo.sh`: o crivo 2 é a propriedade, com a sexta ocorrência
  numerada no comentário e o motivo da resolução pelo `cwd` escrito ali.
- `comunicacao.sh`: o cabeçalho diz FRENTE e não BANCADA, com o motivo.

**Onde o buraco ficou, e ele é grande:** este conserto faz o *aviso* falar a
verdade, e **não** cria portão nenhum. A varredura que ele provocou achou coisa
pior — a catraca `alcancam-fsync` está **reprovada há sete commits** (25 contra
teto 22, medida commit a commit com a régua de hoje: subiu em `20d2c59`, 17/09
06:06), porque **nenhum dos quatro portões desta casa roda as catracas de
`bancada/`**. Quem as roda é este aviso, de 15 em 15 minutos. **Aviso que
ninguém é obrigado a atender não é catraca, é notificação** — está no pedido
362, com as duas frentes que ele pede e com a pétrea que ele não deixa violar:
o teto não sobe.
