# Cognição: a prova real de uma defesa contra o homem-no-meio precisa de dois defeitos

**Descoberta:** 07/09/2026 20:50 UTC, fechando o gap §10 da cifra do fio — a
amarração da credencial ao canal (channel binding).

## 1. O que aconteceu

A amarração ao canal amarra a prova do login à transcrição do túnel: o cliente
faz a prova sobre a transcrição *dele*, o servidor a confere sobre a *dele*, e
as duas só coincidem se não há ninguém no meio que tenha terminado o túnel.
Escrevi o teste de servidor
(`login_amarrado_ao_canal_confere_contra_a_transcricao_da_sessao`) com quatro
casos: (1) cliente honesto entra, (2) homem-no-meio não entra, (3) amarrar sem
túnel é recusa nomeada, (4) sem amarrar, o login é o de sempre.

Escrevi no comentário do teste que **repor o defeito derruba o caso (2)**, o do
homem-no-meio — porque é o caso que a leitura do código não pegaria, e por isso
o que dá valor ao teste.

## 2. O que eu concluí primeiro, e estava errado

Que «o» defeito da amarração é um só, e que ele cai no caso do atacante. Repus
o defeito mais óbvio — o `op_login` ignorar a transcrição (`canal_ref = None`) —
esperando ver o caso (2) cair.

## 3. O que a medição disse

Caiu o caso **(1)**, não o (2). Medido, `servidor.rs:23314`, o assert do
cliente honesto: com o servidor conferindo `None` e o cliente tendo feito a
prova com `Some(transcrição)`, as mensagens divergem e o **cliente honesto para
de entrar** — o teste cai por ali, antes de chegar ao caso do atacante.

O caso (2), o do homem-no-meio, só cai com um defeito **diferente**: o servidor
**ler a transcrição do pedido** em vez da sessão. Aí o cliente honesto continua
entrando (o pedido dele traz a transcrição certa) e é o atacante que passa a
furar — porque ele manda a transcrição que quiser. São dois defeitos, e cada um
mora num caso.

A guarda do catálogo (`amarra-ao-canal-ignorada`) repõe o primeiro, e o
executor devolveu **PROVADA, 1/1 caíram** em 37,5 s. O comentário do teste foi
corrigido para dizer a verdade medida: o `None` derruba o (1); ler do pedido
derrubaria o (2).

## 4. A regra

**Numa defesa contra o homem-no-meio, o caso do atacante e o caso do cliente
honesto caem com defeitos DIFERENTES — a prova real tem de nomear qual defeito
cai em qual caso, senão o comentário aponta para o caso errado e ninguém vê que
o outro nunca foi exercitado.** É a mesma família do «a guarda que dispara antes
esconde a que se queria provar»: aqui o defeito fácil de repor esconde que o
difícil — o que a leitura não pega — não foi medido.

E o corolário de desenho, que caiu de graça: **a transcrição do túnel é
propriedade da CONEXÃO, não do pedido** — mora na `Sessao` ao lado do `ip`,
posta uma vez quando o aperto fecha. Deixar o cliente mandá-la no pedido é
exatamente o defeito (2): devolve ao atacante o que a amarração tira dele.

## 5. Como está guardado hoje

- `desafio.rs`: as cinco funções ganharam `canal: Option<&[u8]>`; `None` é byte
  a byte a mensagem de sempre — a regra pétrea «pedida, não imposta» no cálculo,
  travada por `sem_canal_a_prova_e_identica_a_de_sempre`.
- `servidor.rs`: `Sessao.transcricao_do_fio`, posta no `atender` quando o aperto
  fecha; `op_login` amarra quando o cliente pede `amarrar_canal`, e recusa com
  `erro.amarra_sem_tunel` quem pede sem túnel.
- `replica.rs`: a réplica amarra sozinha quando fala por dentro do túnel — o
  primeiro cliente de produção da amarração.
- A guarda `amarra-ao-canal-ignorada` (PROVADA) e as duas provas reais de
  cripto (`prova_amarrada_a_um_canal_nao_serve_em_outro`,
  `sem_canal_a_prova_e_identica_a_de_sempre`).
- **O buraco que fica, escrito:** a amarração é *pedida*, e um atacante ativo
  que terminou o túnel do cliente pode cortar o campo `amarrar_canal` antes de
  reencaminhar — a mesma aritmética do rebaixamento do `exigir`. Por isso ela
  **reforça o pino, não o substitui**, e o próximo degrau (o servidor *exigir* a
  amarração quando há túnel) está anotado na §10 do `docs/CIFRA-DO-FIO.md` como
  decisão de implantação, não como coisa feita.
