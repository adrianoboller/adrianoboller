# O truque dos colchetes protege o padrão, e não o resto da linha de comando

- **Quando:** 2026-09-18, 00:10
- **Onde:** os vigias de fundo da rodada do pedido 343 — quatro herdados da
  compactação e um escrito por mim, já sabendo da lei
- **Custo:** quatro vigias parados por ~11 h sem nunca acordar, e um quinto que
  eu escrevi **citando a lei no próprio comentário** e que se achou do mesmo
  jeito, por outra porta

## O que aconteceu

A lei desta casa já existe desde 02/09/2026, em
`cognicao_quem-pergunta-quem-esta-rodando-se-acha_20260902_1946.md`:
**identidade de processo se pergunta pelo nome do executável, nunca pela linha
de comando.** Eu a conhecia. Ainda assim:

Quatro vigias vieram da janela anterior esperando geradores terminarem. Dois
perguntavam `pgrep -af "prova-dos-portoes"` — sem colchete nenhum, e a própria
linha de comando deles carrega esse texto. Os outros dois usavam
`"extrair[.]py|comparativo-no-dossie|pagina-do-status-do-projeto|status-html"`:
colchete no **ponto**, para escapar a expressão regular, e **nada** no
`status-html`, que casava a si mesmo.

Escrevi o meu com o colchete no lugar certo, e com o comentário explicando por
quê:

```sh
# O truque dos colchetes: o cmdline DESTE processo contem "[s]tatus-html",
# que a regex "[s]tatus-html" NAO casa. Sem ele, o vigia se acha e nunca sai.
while pgrep -f "[s]tatus-html|[s]tatus/pagina-do-status" >/dev/null 2>&1; do sleep 10; done
echo "=== status-html terminou; cauda do log ==="
```

O padrão estava protegido. A linha **seguinte** do mesmo `bash -c` diz
`status-html terminou` — texto cru, sem colchete —, e a regex `[s]tatus-html`
casa isso perfeitamente. O vigia esperava a si mesmo.

## O que eu concluí primeiro, e estava errado

Que o colchete resolvia o problema, porque o colchete é *a* receita conhecida
para ele. Errado duas vezes:

1. O colchete protege **um trecho** da linha de comando, não a linha. Qualquer
   outra menção ao mesmo nome — um `echo`, uma mensagem de erro, um caminho de
   log — rearma a armadilha. O escopo da proteção é o token, e eu li como se
   fosse o processo.
2. Antes disso, concluí que a lei de 02/09 já cobria o caso e bastava aplicá-la.
   Mas a receita dela — «pergunte pelo nome do executável» — **não alcança este
   alvo**: o nome do executável de `portao-dos-geradores.py` é `python3`, e o de
   `status-html.sh` é `bash`. Perguntar pelo executável aqui não distingue nada.
   A lei vale; a receita dela tem um buraco exatamente onde o alvo é um
   **script** e não um programa.

## O que a medição disse

- **5 vigias**, 5 se achando. Nenhum por falta da lei; quatro por ignorá-la e
  um por aplicar a receita errada dela.
- O `portao2.log` do terceiro vigia estava datado de **17/09 12:33** enquanto o
  relógio marcava **23:35** — ~**11 h** esperando um processo que já tinha
  morrido.
- `pgrep -af "[s]tatus-html"` devolveu **2 PIDs**, e nenhum era gerador: 13728
  era o próprio vigia e 13990 era o shell da conferência. `pgrep -af
  "bash.*status-html\.sh$"` devolveu **vazio** — não havia gerador nenhum
  rodando.
- O `portao-r2.log` **não existia**: a segunda volta do portão nunca começou.
  Rodado à mão, o portão fechou em **4 giros** (VELHO em 4, depois 3, depois 3,
  depois VERDE).

## A regra

**Não pergunte «terminou?» pela linha de comando: espere pelo PID, ou não
espere.** Quando o alvo é um script e o executável é `python3` ou `bash`, a
receita do nome do executável não distingue nada, e o colchete protege só o
token em que você o pôs — a mesma linha que menciona o nome de novo, em
qualquer lugar, volta a se achar. Guarde o PID ao disparar, ou deixe o processo
terminar em primeiro plano e leia a saída.

## Como está guardado hoje

**Não está, e o buraco fica nomeado.** Não há guarda nem catraca que reprove um
vigia que se acha: o padrão é escrito na hora, num `bash -c` efêmero que não
vive no repositório, então não há arquivo para um conferidor varrer. A lei de
02/09 cobre o `comunicacao.sh`, que **está** no repositório e **está** correto
— ela nunca alcançou os vigias de sessão.

O que existe hoje é a tarefa #84 (*«o portão está medindo?» — a quarta vez do
pgrep que se acha*), fechada, e esta é a **quinta**. Cinco ocorrências do mesmo
padrão em dezesseis dias, em quatro lugares diferentes, é sinal de que o
conserto certo não é lembrar melhor: é **não ter o que lembrar**. Enquanto
alguém não escrever o ajudante que dispara e devolve o PID, este arquivo é a
única coisa entre a lei e a sexta vez.
