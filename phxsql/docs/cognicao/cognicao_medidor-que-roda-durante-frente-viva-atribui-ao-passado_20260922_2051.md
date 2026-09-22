# Medidor que roda durante frente viva atribui ao passado o que acabou de nascer

**Descoberto:** 22/09/2026, 20:51 UTC.
**Papel:** A (orquestrador), integrando o inventario do papel G.

## 1. O que aconteceu

O papel G entregou o inventario das saidas de rede e, junto, o estado das
catracas. Ele reportou duas vermelhas e escreveu, com todas as letras:

> As duas vermelhas sao ANTERIORES a mudanca em revisao.

Para a `TETO_ROTULOS_E_CRASE` (951 contra teto 950) ele foi alem e datou: o
teto 950 entrou em `ba0f2ca` (17/09), depois dele **dois** commits tocaram o
`ui/index.html` -- `6319396` e `91784cb` --, e «o +1 nasceu num dos dois e
ninguem remediu».

Eu aceitei, e **contei isso ao dono como correcao de uma coisa que eu tinha
dito errado**. Minutos antes eu havia atribuido o +1 a frente das telas.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o papel G tinha medido e eu tinha inferido -- logo ele estava
certo e eu errado. Era o contrario.

Medido por bisseção, em vez de por historia:

| arvore | `a_catraca_dos_textos_fora_da_fabrica` |
|---|---|
| com `ui/index.html`, `config.rs` e `idiomas.rs` da frente E guardados no stash (= HEAD) | **passa** |
| com os tres de volta na arvore | **951 contra 950, FAILED** |

O +1 **e** da frente viva. A minha primeira leitura estava certa, e eu a
troquei por uma inferencia melhor escrita.

## 3. A causa, e ela e uma lei desta casa pelo avesso

O papel G rodou `cargo build --release --examples -p phxsql-server` -- **3m16s**
-- e so entao o `docs/qa/medir.py`. Durante esses 3m16s a frente E estava
escrevendo o `ui/index.html`. O binario que ele mediu **ja carregava o edit em
voo**.

Esta casa ja tinha a lei: *«Medidor com binario velho mede o passado»* -- uma
rodada inteira de ganhos (16,4 -> 7,5 us) ficou invisivel porque o
`cargo build --release` nao recompila os examples.

O que aconteceu agora e o **inverso**, e e por isso que a lei antiga nao
protegia: o binario estava novo **demais**. Ele nasceu no meio de uma escrita
que ainda nao era commit, mediu um estado que nao existia em lugar nenhum da
historia, e depois a **atribuicao** foi feita por `git log` -- pelo unico lugar
onde aquele estado nao podia estar.

**O alcance da lei e maior do que ela dizia:** nao e sobre o binario estar
velho. E sobre **medicao tirada enquanto ha frente viva nao poder ser atribuida
a historia**, em nenhum dos dois sentidos. O numero pode estar certo; a frase
«isto e anterior» e que nao se deduz de `git log` -- deduz-se do stash.

## 4. O que fica

- **Quem mede durante uma rodada com frentes paralelas diz o que mediu, e
  nao de onde veio.** A atribuicao pede bisseção: guardar o que a frente mexeu
  e remedir. Sao dois comandos, e substituem um paragrafo de arqueologia.
- **O contrato do subagente passa a dizer isso** quando a rodada tem frente
  viva: «diga o numero; para dizer DESDE QUANDO, guarde os arquivos das outras
  frentes e meca de novo».
- E o de sempre, agora com o segundo caso: **diagnostico plausivel nao e
  diagnostico medido**, e o errado sobrevive melhor quando vem com data,
  commit curto e nome de arquivo -- que e exatamente a forma de uma medicao.

## 5. O que NAO mudou

A outra vermelha do mesmo relatorio, `TETO_DEBUG_COM_SEGREDO` (0 contra 2),
**era mesmo anterior** -- os dois campos vivem em `bidirecional.rs` e
`conferidor_texto_cru.rs`, que nenhuma frente tocou. O papel G acertou nessa,
e acertou tambem o veredito: os dois sao falsos positivos do lexico
(`chave_da_posicao` e chave de MAPA, `Cru.chave` e chave de MENSAGEM), e o
caminho licito e a entrada de `ISENTOS` com o motivo -- **nunca subir o teto**.

Ou seja: o relatorio nao estava errado. Estava **certo no numero e errado na
atribuicao**, e isso e pior de achar do que um numero errado, porque a parte
certa avaliza a parte que ninguem conferiu.
