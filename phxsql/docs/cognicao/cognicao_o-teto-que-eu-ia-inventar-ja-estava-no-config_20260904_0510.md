# O teto que eu ia inventar já estava no `config.json`, imposto e provado

**Descoberto em 04/09/2026, 05:10**, quando o dono disse *«o timeout tem um
parâmetro no `config.json`, o tempo máximo obedece essa config»* no meio de uma
decisão de arquitetura.

## 1. O que aconteceu

A pergunta 1 da pesquisa de MVCC punha duas saídas na mesa: versão velha em RAM
(zero formato, mas **teto de memória** e leitor longo recusado) ou em disco
(`.reg` v6, migração de tudo, mas sem teto).

Eu apresentei a opção A com a objeção dela — *«leitor longo estoura, e a saída é
recusá-lo»* — como se o teto fosse um número **novo** a inventar. Não era.

Medido depois da frase do dono:

| campo | limita | padrão | imposto? |
|---|---|---|---|
| `timeout_s` | a espera por um pedido numa conexão ociosa | 30 s | sim, mas **não** limita transação que continua conversando |
| **`transacao_prazo_min`** | **a transação INTEIRA** | **5 min** | **sim** — `transacao.rs:620` filtra por `expira_ms`, e a bancada exercita com prazo de 1 min |

O comentário do campo já trazia o raciocínio inteiro, escrito antes de a
pesquisa existir: *«uma transacao segura tabelas contra a escrita de todo mundo,
e ninguem digita por dez minutos com uma transacao aberta. Zero nao desliga:
cairia no padrao, porque transacao sem prazo nenhum e exatamente a que trava a
tabela para sempre.»*

## 2. O que eu concluí primeiro, e estava errado

Duas coisas, e a segunda é a que ensina.

**A primeira, menor:** aceitei o `timeout_s` como o parâmetro certo porque foi
o que o dono nomeou. Ele **não** é: é `set_read_timeout` no soquete, e uma
transação que continua mandando pedidos nunca esbarra nele. O parâmetro certo
era outro, e melhor. *A frase do dono estava certa no raciocínio e errada no
nome — e eu quase registrei o nome errado numa decisão de formato.*

**A segunda, que é o aprendizado:** eu tratei «teto de memória» como um custo
**a criar**, e por isso a opção A parecia mais cara do que é. Nunca perguntei
**qual teto já existe**. E não era um teto obscuro: são **três** prazos de
transação no `config.rs`, com um comentário de doze linhas explicando por que
são três e não um.

O documento de pesquisa tem 1.400 linhas, leu oito motores no fonte, e não
olhou o `config.json` do próprio projeto.

## 3. O que a medição disse

A cadeia de versões da Sombra **não pode crescer sem fim**: a transação que a
segura morre em **5 minutos** por um prazo que já existe, já é imposto e já tem
prova. A recusa estilo `ORA-01555` deixa de ser a primeira defesa e vira a
**segunda rede** — só dispara se a memória estourar antes do prazo.

E isso não é desenho novo: é o mesmo par de redes do `carga_prazo_min`, escrito
no mesmo arquivo — *«a primeira é a queda da conexão, que desfaz na hora; esta
pega o soquete pendurado vivo com o cliente morto do outro lado.»*

Com a objeção removida, a opção A deixou de ser «a barata com um risco» e
passou a ser a resposta.

## 4. A regra

**Antes de propor um limite novo, pergunte qual limite já existe.** Uma
proposta que inventa teto onde já há teto não custa só o código a mais: ela
**superestima o preço da opção que estava certa**, e pode empurrar a decisão
para o lado errado.

E o corolário sobre a fonte: *pesquisa que lê oito motores lá fora e não lê o
`config.json` daqui mediu o mundo e não mediu a casa.* O inventário do que já
existe é parte da pesquisa, não um passo anterior a ela.

## 5. Como está guardado hoje

- A condição está na **§8.0.1** do `docs/PESQUISA-MVCC-E-FORMATO.md`, com a
  tabela dos dois campos, o que cada um limita de fato, e a prova de que o
  segundo é imposto (`transacao.rs:620`).
- A decisão registrada diz **«o teto não é número novo»**, para que ninguém
  volte a escrever um.
- **O prazo não sobe.** Está escrito ali que o campo é curto de propósito, e
  que trocar risco de RAM por risco de tabela travada precisa de número.
- **O buraco que fica:** este erro não tem guarda. Não há conferidor que
  pergunte «esta proposta inventa um limite que já existe?», e eu não sei
  escrever um que não seja casamento de frase — que é o que esta casa proíbe.
  Fica como lei escrita e hábito, não como catraca.
