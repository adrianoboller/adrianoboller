# A guarda existia, e travava UMA struct — não a lei

**Descoberto em 17/09/2026, 00:10 UTC.** Frente do `Debug` derivado que vaza
segredo (papel B com F), na rodada da pétrea «senha nunca em texto puro».

## 1. O que aconteceu

Uma frente irmã, catalogando a pétrea «senha nunca em texto puro», mediu o raio
de cada defeito que repunha e achou de lado um vazamento vivo:
`crates/phxsql-server/src/dblink/mod.rs:115` declara

```rust
#[derive(Debug, Clone)]
pub struct Definicao { …, senha: String, …, token: String, … }
```

e os comentários dos **dois** campos declaram o problema resolvido — *«ela nunca
sai em JSON nem em log»* e *«ele nunca sai em JSON, em log nem na tela»*. Os
dois estavam certos sobre o `para_json` e errados sobre o `Debug`.

A frente aberta para consertar varreu `crates/` inteiro e achou **9 estruturas,
14 campos**: `Definicao`, `Config`, `Origem`, `Cluster`, `Email`, `Rest`,
`Usuario`, `Comando`, `Receita`. Um `{:?}` no `Config` despeja **oito** segredos
de uma vez.

**E o achado que dá nome a este arquivo:** a guarda já existia. O catálogo tinha
`debug-da-cifra-mostra-a-senha` desde a frente G-CRIPTO, com o `porque`
escrevendo a lei inteira e até o raio medido — «ZERO dos 1.103 testes do `--lib`
caem». Esta casa **já tinha diagnosticado e travado este defeito exato**. Em uma
struct. As outras nove continuaram derivando porque a guarda travou a
**estrutura**, não a **lei**.

## 2. O que eu concluí primeiro, e estava errado

Duas coisas, e as duas na mesma direção — a de subestimar.

**Primeiro: escrevi no briefing da frente que «o achado desta frente é um».**
Pus a pergunta certa logo em seguida («a pergunta do papel F é quantos são»),
mas a premissa que dei era de caso isolado. Eram nove, e a diferença não é de
grau: um caso isolado se conserta com um `impl`; nove com o mesmo formato são um
**padrão sem régua**, e o conserto certo é outro.

**Segundo, e pior: escrevi que o campo era `token_remoto` e colei uma saída de
programa que o mostrava assim.** O campo Rust é `token` (linha 143);
`token_remoto` é o nome no **JSON/protocolo**, e há um bloco de documentação no
próprio campo explicando por que os dois nomes diferem. O `derive(Debug)`
imprime o nome do **campo**, então a saída que colei não podia ter saído de
corrida nenhuma. Eu disse «conferido no fonte por mim», e o que eu de fato
conferi foi a **existência** do defeito — o `derive` e os dois comentários —,
não aquele texto, que veio do relatório de outra frente e que eu repassei como
se fosse medida minha.

É a lei de sempre vista de dentro: **briefing de orquestrador também é número
citado.** Evidência repassada sem ser reproduzida é evidência de ouvir dizer, e
aqui ela sobreviveu porque o achado era verdadeiro — o conserto funcionou, e o
texto errado teria passado junto.

## 3. O que a medição disse

O crivo que achou as nove tem **três** partes, e as duas primeiras sozinhas
erram feio: `grep senha` devolve **40** campos, dos quais **26 não são defeito**.

1. o **nome** casa o léxico (`senha|token|chave|segredo|credencial|privada|…`);
2. o **tipo** carrega valor (`String`, `Vec<u8>`, `[u8;N]`, `Option<String>`) —
   é o que mata `senha_env` (nome de variável de ambiente) e `usa_senha: bool`;
3. o valor **é mesmo segredo** — e **essa só se decide lendo o campo**. Nenhum
   casador de texto a faz: `chave_do_fio` é a chave **pública** do pino, e
   `Botao.chave` é um seletor de CSS. Cinco falsos positivos ficaram declarados
   em vez de escondidos, porque esconder trocaria vazamento por diagnóstico
   cego.

A prova nos dois sentidos: com o `derive(Debug)` de volta (`git diff` de **241
inserções e 0 remoções**, ou seja o código original byte a byte com os testes
novos por cima), as **6 provas novas falharam**, cada uma nomeando o segredo que
vazou. Com o conserto, as 6 passam — `clippy` com **zero** avisos e **2.400**
testes verdes.

E uma consequência que só aparece no encontro das frentes: os nove `impl Debug`
novos criaram **três** linhas `.field("senha", &"(oculta)")` idênticas em
`config.rs`, e isso deixou **ambíguo** o `trecho` da guarda velha. O provador
passou a recusar a entrada antes de tentar, e `TETO_TRECHO_AMBIGUO` — que é 0 —
acusou. O conserto foi alongar o trecho em uma linha. **Consertar o alcance de
uma guarda quebrou a guarda que existia**, e quem disse isso foi a catraca, não
a leitura.

## 4. A regra

**Guarda que repõe defeito trava a ESTRUTURA onde o defeito foi reposto, nunca a
lei que o motivou. Quando o mesmo defeito couber em mais de um lugar, o que
protege a lei é uma RÉGUA que conte os lugares — e a guarda passa a ser o
exemplo dela, não a proteção.**

## 5. Como está guardado hoje — e onde o buraco ficou

**Guardado:** os 9 `impl std::fmt::Debug` à mão, no molde do `config.rs`, com
uma diferença que vale por si — cada um **desestrutura a struct sem `..`**, de
modo que campo novo **para de compilar** ali em vez de entrar calado na saída.
É «quando algo depende de uma lista, a lista sai do código» aplicado a Rust, e
transforma o conserto numa catraca de compilador. Mais a §16 do
`docs/SEGURANCA.md`, as 6 provas, e a entrada `debug-da-ligacao-mostra-a-senha`
no catálogo — cujo defeito reposto **não** devolve o `derive` (isso não
compilaria, e guarda que não compila não prova nada): desfaz o conserto por
dentro, em duas trocas.

**O buraco, e ele está aberto:** com essa entrada são **2** structs com guarda
de catraca, de **9**. As outras sete têm prova — e prova não é catraca: ela pega
o defeito *naquelas* structs, e não impede a **décima** struct de nascer
derivando `Debug` com um segredo dentro. O caminho certo não são mais sete
entradas de catálogo; é o **conferidor** que varre `derive(Debug)` contra o
crivo de três partes, com os cinco falsos positivos declarados numa lista
visível, nascendo no número medido do dia e só descendo. Enquanto ele não
existe, este arquivo é o registro de que a lei continua sem régua — e o próximo
`derive(Debug)` com segredo volta pelo mesmo caminho, que é exatamente o que
acabou de acontecer.
