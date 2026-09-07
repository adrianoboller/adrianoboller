# A guarda que dispara antes esconde a que se queria provar

*Descoberto em 07/09/2026, 18:10, escrevendo a parte 5 da
`bancada/usuarios/provar.py` — pedido 221.*

## 1. O que aconteceu

O cadastro pelo protocolo ganhou duas recusas parecidas, e elas são
conferidas em ordem, sobre o cadastro que **sobrou**:

1. **sem administrador ativo** — a mudança deixaria o servidor sem ninguém que
   possa mexer no cadastro, nem para desfazer;
2. **a própria conta** — quem pediu tirou de si o poder de administrar, ou se
   apagou.

A bancada afirmava «não se apaga a própria conta» mandando a supervisora
`ana` — a **única** administradora do arquivo — se excluir. E recebia:

```
[FALHA] nao se apaga a propria conta -- [SP000025] acesso negado:
        usuario_excluir: isto deixaria o servidor sem nenhum supervisor ativo
```

A operação **foi** recusada. A recusa **estava certa**. E a afirmação da
bancada estava errada: ela dizia provar a guarda 2 e exercitava a guarda 1.

## 2. O que eu concluí primeiro, e estava errado

Que a bancada tinha achado um defeito de ordem — que a guarda da própria conta
deveria vir antes, porque é a mais específica.

Está errado, e o motivo é o que importa: **a ordem certa é a que dá a
explicação mais útil.** Quando as duas valem, «o servidor ficaria sem
administrador» diz o que está em jogo; «você perderia o poder» diz menos, e
manda a pessoa tentar de novo com outra conta — que também não vai funcionar.
Trocar a ordem melhoraria o teste e pioraria a mensagem.

O defeito não estava no motor. Estava no **cenário**: com um administrador só,
não existe caminho que exercite a guarda 2 sem passar pela 1.

## 3. O que a medição disse

Com o cenário certo — um segundo supervisor criado antes —, as duas passaram a
ser exercíveis, e cada uma pelo seu caminho:

```
[OK  ] o segundo administrador nasce (so supervisor cria supervisor)
[OK  ] nao se apaga a propria conta
[OK  ] nao se tira de si o poder de administrar
[OK  ] com DOIS administradores, desligar o outro vale
[OK  ] mas desligar o ULTIMO e recusado
```

A guarda 1 precisou de um caminho próprio: o **bruno** desliga a **ana**
(legítimo — sobra ele) e só então tenta desligar a si mesmo. Sem esse desvio,
ela nunca dispara com dois ativos e nunca é a primeira com um só.

Resultado da bancada inteira: **53 afirmações, 0 falhas**.

## 4. A regra

**Guarda provada pelo caminho errado não está provada — e ela passa verde.**
Quando duas recusas se sobrepõem, o teste de cada uma precisa de um cenário em
que **só ela** possa disparar; se não existe esse cenário, a afirmação está
descrevendo a outra guarda.

E o teste do teste: **leia a MENSAGEM da recusa, não só o veredito.** Aqui a
diferença entre certo e enganado eram duas frases diferentes atrás do mesmo
`ok: false` — e uma afirmação que só olhasse `not r["ok"]` teria passado, verde,
para sempre.

## 5. Como está guardado hoje

- `bancada/usuarios/provar.py`, parte 5 — as seis recusas casam o **pedaço da
  mensagem**, e não só o veredito; e o bloco do bruno existe só para dar à
  guarda do último administrador um caminho onde ela é a primeira.
- `crates/phxsql-server/src/servidor.rs`,
  `nao_se_tira_de_si_mesmo_o_poder_de_administrar` — o teste unitário cria um
  segundo supervisor pela mesma razão, e o comentário dele diz isso.
- `crates/phxsql-server/src/usuarios.rs`,
  `o_ultimo_administrador_nao_se_apaga` — exercita a guarda 1 pelo token de
  serviço (`quem = None`), que é o outro caminho em que a guarda 2 não existe.

**Onde o buraco ficou.** Nada impede que a próxima guarda de cadastro nasça
coberta por uma destas duas e ninguém perceba: não há conferidor que diga «esta
afirmação disparou a guarda que ela nomeia». O que existe é o hábito de casar o
texto da recusa — e é por isso que as mensagens desta família são todas
diferentes entre si.
