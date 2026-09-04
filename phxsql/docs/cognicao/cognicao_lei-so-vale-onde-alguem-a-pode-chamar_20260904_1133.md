# A lei estava escrita, e cobria o script — não cobria a pergunta

*04/09/2026, 11:33 — hora da descoberta, não do commit.*

## 1. O que aconteceu

O batimento de 15 minutos disparou e eu fui rodar o `comunicacao.sh`. Antes,
para não repetir o erro do dia (rodar aviso e zelador dentro de uma janela de
medição reprovou **três** baterias em 04/09), improvisei no prompt:

```
pgrep -af "escolher-o-desenho|o-comboio|quanto-a-trava|…|carga"
```

Ele achou **dois** processos. Nenhuma bancada estava de pé: os dois eram o meu
próprio shell, porque a linha de comando dele carregava o padrão inteiro —
inclusive a palavra `carga`.

A lei que impede exatamente isso **já estava escrita**, e no arquivo que eu
tinha acabado de chamar. `comunicacao.sh`, no comentário acima do bloco de
processos:

> «E a TERCEIRA vez que esta armadilha aparece nesta base: já pegou um
> `pgrep -f cacar2` e um `pgrep -f video-demonstracao` […] E o crivo é o **NOME
> DO EXECUTÁVEL**, nunca a linha de comando.»

Foi a quarta.

## 2. O que eu concluí primeiro, e estava errado

**Concluí que era descuido meu** — que eu tinha escrito um `pgrep -f` sem
pensar, e que a lição era «lembrar da lei». Isso não explica nada: a lei estava
a três linhas de distância, e a frente que a escreveu também não tinha
lembrado dela na segunda vez.

O errado é a forma da lei, não a memória de quem a lê. Uma lei escrita **dentro
de um script** protege aquele script e mais nada. A pergunta «há bancada
medindo agora?» não morava em script nenhum: era improvisada no prompt a cada
vez, e improviso não herda comentário.

E há uma segunda coisa que eu teria errado se tivesse aplicado a lei ao pé da
letra: **«o crivo é o nome do executável» não funciona aqui.** O executável de
uma bancada é `python3`, que não distingue bancada de coisa nenhuma. A
identidade dela mora no `argv[1]`. A lei precisava de um caso que ela não tinha.

## 3. O que a medição disse

Com a máquina **limpa**, nenhuma medição em curso:

| crivo | quantos «achou» |
|---|---|
| `pgrep -f "bancada/concorrencia"` | **1** — o shell que perguntou |
| `pgrep -f "bancada/"` (de dentro da bateria) | **1** — a própria bateria |
| `bancada/esta-medindo.sh` | **0**, e sai 1 |

E com uma bancada de pé (um `sleep` com `exec -a` vestindo a linha de comando
de uma bateria de concorrência), o portão acha **1** e diz qual crivo a pegou.

Prova real nos dois sentidos, com o defeito reposto no próprio portão (a
exclusão por linhagem removida): **2 das 5 conferências caem**, e a listagem
mostra o portão achando o `cp bancada/esta-medindo.sh /tmp/…` que eu tinha
acabado de digitar. Com o código certo, as cinco passam.

## 4. A regra

**Lei escrita dentro de um script só vale para aquele script. Pergunta que se
repete vira script, ou se improvisa errado do mesmo jeito toda vez.**

E o corolário do crivo: **exclua o observador por LINHAGEM, nunca por texto.**
Nenhum ancestral do processo que pergunta conta; descendente conta. Foi o que
fez o `esta-medindo.sh` não se achar mesmo sendo chamado por um shell que
carrega o nome dele na linha de comando.

## 5. Como está guardado hoje

- `bancada/esta-medindo.sh` — o portão. Dois crivos (nome do executável onde
  ele diz algo: `cargo`, `rustc`; caminho do script onde o executável é só o
  interpretador), exclusão por linhagem lida em `/proc/<pid>/stat`.
- `comunicacao.sh` o consulta e imprime `⏳ BANCADA MEDINDO`. As duas metades
  se medem **antes** de qualquer uma imprimir — a primeira versão desta emenda
  imprimia «nada compilando nem rodando agora» e «BANCADA MEDINDO» no mesmo
  relatório, que é o mesmo defeito que aquele arquivo já pagou uma vez.
- `zelador.sh` **recusa** e sai 3, em vez de avisar: quem o chama de hora em
  hora não está lendo a saída. `--mesmo-assim` existe para o disco acabando de
  verdade, que é mais caro que uma bateria perdida.
- `bancada/bateria/prova-bateria.py`, **item 0b** — antes de qualquer servidor
  subir, porque é estático. E ele carrega o **contra-exemplo dentro de si**:
  mede o crivo por texto na mesma corrida e exige que ele *se ache*. No dia em
  que o `pgrep -f` parar de se achar, esta conferência cai — e cair aí é aviso
  de que a régua mudou, não de que o portão quebrou.

**Onde o buraco ficou:** o portão não entra no `bancada/guardas/catalogo.py`,
que é o catálogo que repõe defeito e roda os testes que devem cair. O executor
de lá roda `cargo test`, e este defeito mora num `.sh` provado por um item de
bateria em Python. Não forcei a entrada; a prova real vive dentro do item 0b,
que é o único lugar onde ela roda sozinha.
