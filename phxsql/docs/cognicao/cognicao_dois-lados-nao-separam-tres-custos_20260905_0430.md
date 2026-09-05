# Dois lados não separam três custos — e o terceiro lado é o que os separa

*Descoberto em 05/09/2026, 04:30, montando a bancada de utilização padrão.*

## 1. O que aconteceu

O pedido era «20.000 registros em tabela complexa **com e sem binários e
memos**». Dois lados, uma razão. A lei da casa já estava lida antes de eu
escrever a primeira linha: *bancada compara trabalho igual, não só pergunta
igual* — e a `bancada/LEIA-ME.md` traz os dois casos em que ela foi quebrada,
apontando para lados opostos, sem que nada no número denunciasse.

Montei os dois lados obedecendo à lei como eu a entendia: as mesmas linhas, o
mesmo esquema a menos das duas colunas, os mesmos cinco índices, o mesmo
servidor, uma instrução por operação dos dois lados. E mesmo assim a razão
entre eles **não** mede o que o nome diz.

Porque uma linha com blob difere da outra em **três** lugares ao mesmo tempo:

1. o **fio** — o mesmo binário viaja em hexadecimal, com o dobro do tamanho;
2. o **slot** do `.reg` — duas colunas a mais, com o ponteiro de cada bloco;
3. os **arquivos externos** — `.bin` e `.memo`, que o outro lado nem abre.

«Com blob custa N×» é a soma das três, e depois não há como separar.

## 2. O que eu concluí primeiro, e estava errado

Concluí que **a maior parte do custo seria o arquivo externo**. É o que o nome
do eixo sugere («com e sem binários e memos»), é onde está a estrutura de dados
nova, e é onde eu procuraria se quisesse otimizar. Escrevi a bancada com dois
lados e ia publicar a razão.

Estava errado em ordem de grandeza. Medido com um terceiro lado que carrega o
mesmo peso no fio e **não abre arquivo externo nenhum**, o `.bin`/`.memo` é a
menor das três parcelas — e no disco ela é **negativa**: guardar 256 bytes crus
no `.bin` gasta menos que guardar 512 caracteres de hexadecimal numa coluna de
largura fixa.

O erro sobreviveria bem, e é isso que o torna caro: o número de dois lados
estaria **certo** («com blob custa mais»), a conclusão estaria errada, e
ninguém teria como perceber, porque a conclusão não aparece no número.

## 3. O que a medição disse

Três lados, as mesmas 20.000 linhas, o mesmo esquema a menos das duas colunas
do fim. O lado `largo` declara essas duas colunas com os **mesmos nomes e os
mesmos valores**, como `Str(n)` — o pedido no fio sai byte a byte idêntico ao
do lado `com`, e isso é conferido: os dois medem **1384,1 bytes por linha** no
fio, contra 209,0 do lado sem blob.

| a diferença | fio | disco |
|---|---:|---:|
| `sem` → `largo` (o peso do pedido e do slot) | +1175,1 B/linha | +1160,0 B/linha |
| `largo` → `com` (só o `.bin` e o `.memo`) | 0 B/linha | **−233,4 B/linha** |

O dado da linha são 856 bytes. No `.bin`/`.memo` ele custa 926,6 bytes de disco
— 8,2% de sobra, que são cabeçalho de bloco, CRC e o ponteiro no slot. Em
coluna de largura fixa custa 1160,0, e a conta fecha **exatamente** com as
larguras declaradas (640 + 520): o `.reg` cresce o que a coluna pediu, cheia ou
vazia.

E o `fsync` não distingue os dois: em `por_operacao`, um lote e uma linha
custam **8 `fsync`** nos três lados — o lado que não tem `Bin` nem `Memo`
nenhum paga o `fsync` do `.bin` e do `.memo` igual aos outros, porque o fecho
da janela é um comboio dos oito arquivos da tabela. *O custo do blob aparece em
bytes, não em chamadas ao disco.*

E o **tempo não separa os três lados**: na segunda carga de cada um, o motor
gasta praticamente o mesmo para as mesmas 20.000 linhas — a diferença entre o
lado sem blob nenhum e o lado com os dois cabe dentro do ruído entre duas
cargas do mesmo lado. Quem separa são os bytes. *Os números de tempo não são
repetidos aqui de propósito: eles saem do gerador, em
`bancada/utilizacao-padrao/LEIA-ME.md` e em `docs/DESEMPENHO.md` §18.1, e uma
cópia nesta página envelheceria na próxima corrida. Os de byte estão acima
porque são determinísticos — as mesmas linhas dão os mesmos arquivos.*

Duas coisas apareceram no caminho, e valem mais que o eixo original:

**A chave estrangeira conferida triplica a gravação** da mesma tabela sem ela —
medianas de três cargas por braço, com o índice `porCategoria` presente nos
dois lados, de modo que o que está medido é a conferência e não o índice. É o
maior custo da tabela complexa, e é o preço da regra primordial cobrado na
entrada. O número está em `docs/DESEMPENHO.md` §18.2, gerado.

**E um efeito que ficou sem causa**, escrito assim de propósito: a primeira
carga de um lado com peso grande custa mais da metade a mais que a segunda
carga do mesmo lado, e a diferença aparece dentro do motor. Dois controles
mataram as duas explicações óbvias — inverter a ordem das seis cargas
(`PHX_ORDEM_INVERTIDA=1`) não muda de quem é a lentidão, e três cargas
idênticas seguidas (o braço da chave conferida) custam todas o mesmo. Nenhum
dos dois explicou o efeito, e **nenhuma conclusão foi tirada dele**.

O portão do tempo passou a ser consultado **por fase**, e não pela corrida
inteira: a corrida leva um minuto e meio, e numa máquina com vizinho ativo isso
jogava fora também o tempo das fases que rodaram na janela limpa. Cada fase
carrega o próprio veredito, e o gerador publica só as que couberam inteiras no
silêncio.

## 4. A regra

**Quando um lado difere do outro em mais de um eixo, um terceiro lado que
segure todos os eixos menos um é o que transforma uma razão em três parcelas.**
A pergunta que o monta é sempre a mesma: *o que eu consigo manter idêntico?* —
aqui, os bytes no fio, e a prova de que ficaram idênticos é a própria medição
(1384,1 dos dois lados).

E o corolário sobre o alcance da lei que já existia: *comparar trabalho igual*
não basta quando o trabalho difere em três coisas de uma vez. A lei impede a
comparação **injusta**; ela não impede a comparação **indivisível**. Trabalho
desigual bem-intencionado continua sendo um número que não se pode interpretar.

## 5. Como está guardado hoje

- `bancada/utilizacao-padrao/medir.py`, com os três lados e o comentário que
  explica por que são três.
- `bancada/utilizacao-padrao/LEIA-ME.md` e `docs/DESEMPENHO.md` §18 — os dois
  gerados por `bancada/utilizacao-padrao/gera-leia-me.py`, e nenhum número
  digitado. O gerador lê a lista de colunas e de índices do próprio `medir.py`,
  porque *quando um gerador depende de uma lista, a lista tem de sair do
  código*.
- A regra do tempo mora no gerador: se o portão acusou antes **ou** depois da
  corrida, ele escreve «não medido» e o motivo.

**Onde o buraco ficou:** o lado `largo` iguala o fio, mas **não** iguala o
slot — ele cresce 1160 bytes por linha, e o lado `com` cresce 32. Separar
essas duas parcelas pediria um quarto lado (blob no fio, jogado fora sem
gravar), que não existe no protocolo. Enquanto não existir, a linha
`sem → largo` da tabela é a soma de duas coisas, e está rotulada assim.
