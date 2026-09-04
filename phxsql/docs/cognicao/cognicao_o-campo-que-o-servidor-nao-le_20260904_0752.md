# O campo que o servidor não lê — e por que quatro medidores não se
# contradiziam

**04/09/2026, 07:52.** Descoberto vinte e sete minutos depois de eu publicar a
conclusão errada.

## 1. O que aconteceu

Às 07:25 publiquei a §13 do `docs/CONCORRENCIA.md`: *«o teto do `RwLock` fica
em ~2,0× com o `varrer` em limite 1, 10, 50 e 200 — uma faixa de 200× no número
de linhas — porque o formato da leitura não muda o custo dela»*. Escrevi o
pedido 181 e abri o 182 em cima disso.

O medidor mandava `{"op": "varrer", ..., "limite": N}`. O `op_varrer` pega o
tamanho da página em `Servidor::limite(p)`:

```rust
fn limite(&self, p: &Json) -> u64 {
    let teto = self.max_linhas();
    let pedido = p.inteiro_ou("max", teto as i64).max(0) as u64;
    ...
}
```

**O campo é `max`.** `limite` não existe nesse pedido; ele era ignorado em
silêncio e toda leitura caía no teto de configuração, que vale **1.000**. As
quatro «variações» custaram o mesmo porque **eram a mesma leitura**.

Provado com o servidor de pé, tabela de 1.500 linhas:

| pedido | linhas devolvidas |
|---|---:|
| `"limite": 1` | **1000** |
| `"limite": 50` | **1000** |
| `"limite": 200` | **1000** |
| `"max": 1` | 1 |
| `"max": 50` | 50 |
| `"max": 200` | 200 |

## 2. O que eu concluí primeiro, e estava errado

**Que eu tinha achado um custo fixo misterioso.** Como 1 linha e 200 linhas
custavam os mesmos 5 ms, escrevi que existiam «~4,8 ms perdidos dentro do
caminho de leitura» e abri o pedido 182 para caçá-los. Cheguei a estreitar o
alvo com uma tabela bonita: `ping` custa 97 µs, `inserir` custa 231 µs pagando
trava global e `open`, logo os 4,8 ms não eram nada disso.

O raciocínio estava certo **dado o dado**. O dado é que estava errado, e um
raciocínio impecável sobre dado errado dá uma conclusão errada com ar de
rigorosa — que é pior do que um palpite, porque ninguém a questiona.

**E há um segundo erro, mais fino.** Quando refiz a medição com `max`, a
conclusão da §13 **se confirmou**: o teto não depende do formato da leitura.
Fiquei tentado a tratar a retratação como formalidade. Não é: a §13 dizia que
o teto não varia *porque o custo da leitura não varia*, e o custo varia **28×**.
Ela acertou o resultado e errou o mundo inteiro em volta dele. **Conclusão
certa tirada de medição inválida não é acerto** — e teria «aposentado» a
ressalva da §3.1 com um argumento que qualquer um derruba, levando a conclusão
certa junto na queda.

## 3. O que a medição disse

Refeita com `max`, duas baterias limpas (07:59 e 08:05):

| `max` | ler 1 cliente | razão ler/gravar | teto do `RwLock` |
|---:|---:|---:|---:|
| 1 | 4.968 op/s | **0,9×** | 1,80× |
| 10 | 4.118 op/s | 1,1× | 2,05× |
| 50 | 2.078 op/s | 2,1× | 1,86× |
| 200 | 819 op/s | 5,3× | 2,15× |
| 1.000 | 192 op/s | **22,5×** | 2,06× |

A razão percorre **28×** e o teto fica entre **1,79× e 2,15×** nas dez
medições, sem tendência. E o achado que só aparece com o campo certo: em
`max: 1` a leitura custa **menos** que a escrita (201 µs contra 231 µs) e o
teto ainda é 1,80×. **O que serializa não é só o tempo sob a trava; é o pedido
dela.**

**Como o defeito apareceu, e este é o número que mais ensina:** não foi pela
bancada. Ela era coerente consigo mesma e passou no `quieta.Vigia` **duas
vezes**. Apareceu porque o medidor **em processo** (`--example
onde-doi-na-leitura`) discordou dela: lá dentro 200 linhas custam **11,6×** uma
linha (513 µs contra 44 µs), e pela rede custavam o mesmo. Um dos dois estava
mentindo, e a discordância disse que havia o que procurar.

**E por que ninguém tinha achado antes:** os **quatro** medidores de
concorrência mandavam `"limite"` — `a-trava-serializa.py`,
`quanto-a-trava-fica-presa.py`, `escolher-o-desenho.py` e o
`o-comboio-do-fecho.py`; três são de 03/09. A `bancada/bateria/prova-bateria.py`
sempre mandou `"max"`, e por isso está certa. Como as quatro da concorrência
mandavam **o mesmo campo errado**, nenhuma discordava de nenhuma, e o rótulo
«`varrer` de 50 linhas» atravessou as §3.1, §7.1, §11 e §12 sem que ninguém
tivesse motivo para conferi-lo.

## 4. A regra

**Instrumento que só concorda com irmão do mesmo molde não confere nada.** A
concordância entre quatro medidores que compartilham a origem é uma cópia, não
uma confirmação — o erro comum some. **Quem confere é o instrumento de outra
camada**, e vale a pena tê-lo mesmo quando ele parece redundante: foi o medidor
em processo que pegou o de rede.

E a segunda, do erro nº 2: **conclusão certa tirada de medição inválida não é
acerto — retrate do mesmo jeito.** É a lei da casa por outra porta: *o errado
sobrevive melhor quando o conserto funcionou por outro motivo.*

## 5. Como está guardado hoje

* **Os cinco medidores mandam `max`.** O `escolher-o-desenho.py` ganhou
  `LINHAS_LIDAS`, com padrão **1.000 de propósito**: é o que as baterias
  publicadas mediram, e trocar a régua no mesmo commit em que se conserta o
  instrumento perderia a comparação com tudo o que já saiu. *Consertar o
  instrumento e mudar a régua de uma vez é perder a série.*
* **A §13 fica no documento, retratada**, com o texto original citado. Apagar
  deixaria a página certa e a lição perdida.
* **As quatro seções com o rótulo errado levam nota de correção** (§3.1, §7.1,
  §11, §12): as razões continuam valendo, porque cada bateria comparou curvas
  medidas com a *mesma* leitura; o que estava errado é **qual carga** elas
  descrevem.
* **As corridas invalidadas ficam versionadas** ao lado das boas, e as boas
  levam `CERTO` no nome. Corrida invalidada apagada é série perdida.
* **A guarda existe, e fechou na mesma hora:** `quieta.confira_a_pagina`.
  Antes de qualquer número, a bancada pede 1, 7 e 50 linhas e confere que
  vieram 1, 7 e 50. Ligada no `o-perfil-da-carga.py` e no
  `escolher-o-desenho.py`, que são as duas cujo número depende do tamanho da
  página. **Prova real nos dois sentidos:** com o `"limite"` reposto ela
  reprova e sai **1**, imprimindo o pedido culpado
  (`{'op': 'varrer', ..., 'limite': 1}` devolveu 1000); com `"max"`, sai **0**.
* **E a guarda quase nasceu inútil, do mesmo jeito que o defeito.** A primeira
  versão montava `{"max": n}` por conta própria — e teria **passado** com a
  bancada mandando `limite`, porque conferia o *servidor* em vez de conferir a
  *bancada*. A segunda recebe o **construtor de pedido da própria bancada**, e
  o `pedido()` do `escolher-o-desenho.py` ganhou o tamanho da página como
  parâmetro só para que a guarda passe por ele. *Guarda que não percorre o
  caminho do defeito é decoração* — e essa foi a terceira vez, na mesma hora,
  que a resposta certa exigiu perguntar «isto exercita o caminho que quebrou?».
* **Esse buraco também fechou, na mesma hora.** As outras duas bancadas
  (`a-trava-serializa.py` e `quanto-a-trava-fica-presa.py`) ganharam um
  `pedido_de_leitura()` com o tamanho da página como parâmetro — existir como
  função é o que permite a guarda percorrer o mesmo caminho — e chamam
  `confira_a_pagina`. As **quatro** bancadas conferem agora.
* **Onde o buraco ficou de verdade:** as séries publicadas (§7.1, §11, §12)
  descrevem uma carga de **1.000 linhas**. A série para a carga de **50** não
  existe. Uma corrida curta de fumaça sugere que a trava presa lendo cai de
  ~3.150 µs para a ordem de 500 µs — **e eu não publico esse número**, porque a
  corrida foi curta e o vigia não conferiu a máquina. Fica nomeado na §8 e na
  §14.3: refazer as três baterias com o instrumento consertado.
