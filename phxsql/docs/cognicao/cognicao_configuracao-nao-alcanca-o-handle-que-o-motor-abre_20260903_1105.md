# Configuração ligada num handle não alcança o handle que o motor abre por baixo

**03/09/2026, 11:05** — descoberto quando, já com a réplica deixando de julgar,
a sonda continuou recusando um evento — e a mensagem era outra.

## 1. O que aconteceu

Depois de a réplica parar de conferir chave estrangeira, as três ordens de
entrega ainda não fechavam. A recusa que sobrou:

```
pedidos Alteracao rowid 1 -> RECUSOU: evento de alteracao no rowid 1 veio sem
imagem: o source gravou o diario com `imagem_da_linha` desligada
```

Mas o source **tinha** a imagem ligada — a sonda abre `pedidos` com
`com_imagem_no_diario(true)`, e o evento de **inclusão** dela viajou com imagem
e foi aplicado.

O que não viajou foi o evento da **cascata**. A cascata do `ao_alterar` grava na
filha por um **handle próprio**, aberto pelo motor dentro de
`planejar_ao_alterar`:

```rust
let mut filha = Table::abrir(&self.diretorio, &irma)?;
```

Esse handle nasce com o padrão — `imagem_no_diario: false` — porque ninguém o
abriu, ninguém o configurou. O evento de alteração da filha ia para o diário
**sem a imagem da linha**, e a réplica não tinha como saber para que valor a
chave mudou. Recusava, nas **três** ordens.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a sonda estava errada — que ela tinha esquecido de ligar a imagem
na tabela filha, e que o defeito era do teste e não do motor. Cheguei a
procurar a linha para corrigir.

Ela **não** tinha esquecido: a linha estava lá, com `com_imagem_no_diario(true)`
na criação de `pedidos`. O que me enganou foi supor que **ligar num handle liga
na tabela**. Não liga: liga naquele handle. E a tabela é escrita por dois — o
handle de quem chama, e o que o motor abre por baixo para cascatear.

Esse engano é a versão sutil do defeito, e é por isso que ele sobreviveu: o
diário de `pedidos` tem dois eventos, a inclusão **com** imagem e a alteração
**sem**. Quem olhasse «a imagem está ligada nesta tabela?» veria que sim.

## 3. O que a medição disse

Com a sonda, antes e depois da herança:

| | eventos que o source tem | eventos que a réplica aplicou |
|---|---|---|
| antes | `pedidos: 2` | **1**, nas três ordens |
| depois | `pedidos: 2` | **2**, nas três ordens |

E a assertiva que trava o achado é sobre o **evento**, não sobre o resultado:
`o_evento_da_cascata_carrega_a_imagem_da_linha` olha `!imagem.is_empty()` no
evento que a cascata gerou. Um teste que só comparasse o valor final da filha
passaria com o defeito reposto em duas das três ordens.

## 4. A regra

**Interruptor ligado num handle vale só para aquele handle.** Quando o motor
abre uma tabela por baixo, ele tem de **herdar** o que o handle de quem chamou
tinha — senão a garantia vale só para o caminho que passou pela mão de quem a
ligou.

É a mesma família do KiB do rodapé (`a lista tem de sair do código`) por outro
caminho: lá a receita do número envelheceu; aqui a configuração não atravessou.
Nos dois casos o defeito é **invisível para quem olha o lugar onde ligou**.

## 5. Como está guardado hoje

* `planejar_ao_alterar` faz o par explícito, com o comentário dizendo o que se
  mediu:
  ```rust
  filha.ligar_imagem_no_diario(self.imagem_no_diario);
  filha.ligar_imagem_na_exclusao(self.imagem_na_exclusao);
  ```
* Guarda `cascata-sem-imagem-no-diario`, provada: com as duas linhas removidas
  caem dois testes.
* O `docs/INTEGRIDADE.md` §3 registra este como o **item 3** da réplica, e diz
  que ele não era decisão nenhuma — era defeito.
* **Onde o buraco ficou:** `Table` tem outros interruptores de handle
  (`evento_forcado`, `sobreposta`, `ver_so_o_disco`). Nenhum conferidor
  garante que um handle aberto pelo motor herde o que devia; a herança é feita à
  mão, aqui, num lugar só. O próximo interruptor que nascer entra pelo mesmo
  ponto ou não entra — e ninguém avisa.
