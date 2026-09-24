# O padrão é o lado de quem esquece — e o botão que existe não fecha o furo se o padrão está aberto

**Descoberta:** 24/09/2026, 03:50, pedido 471 (PhxZip abrindo arquivo hostil).

## 1. O que aconteceu

O parecer SEC achou o PhxZip derivando 2^24 rodadas de SHA-256 já no `abrir`
de um arquivo enviado por qualquer um. A conferência EXISTIA: o
`NumCyclesPower` do 7zAES era comparado com `Limites::ciclos` antes de derivar.
O furo era o padrão, `Limites::default()` com `ciclos: 24`, que é o máximo que
o 7-Zip lê. E é o `default()` que a web, o PhxZipCmd e o `.phz` recebem quando
ninguém escolhe.

## 2. O que eu concluí primeiro, e estava errado

Na etapa 1 escolhi o padrão pela **interoperabilidade**: «o leitor aceita o que
o 7-Zip aceita». Pareceu a régua certa, e ainda pareceu a regra da casa: *guarda
nova entra pedida, não imposta*. Só que essa pétrea protege o **cliente antigo**
de parar de funcionar de um dia para o outro. Uma crate nascida no mesmo dia
não tem cliente antigo nenhum. Ali o padrão só decide para que lado cai quem
esquece de escolher, e eu o deixei cair no lado caro.

## 3. O que a medição disse

- Com o padrão antigo (24), o teste adverso levou **48,6 s** em debug para
  responder, e a resposta nem era a recusa: era `SenhaErrada`, depois da
  derivação inteira.
- Com o padrão 19 (o que o 7-Zip GRAVA, `7zAes.cpp:236`), a recusa
  `CICLOS_DEMAIS` saiu em **28 µs**. Todo fixture do 7-Zip continua abrindo pelo
  padrão.
- Abrir com a folga do 7-Zip virou ato escrito: `Limites::confiavel()`.
- Na mesma rodada, as contagens do cabeçalho: um arquivo de **171 bytes**
  alocava **29.259.867 bytes** de pico (146× o cabeçalho descomprimido). Com o
  teto das contagens, **275.168 bytes**.
- Dois alcances medidos na mesma rodada:
  - **Dano que depende do sistema operacional não cabe no catálogo.** O
    provador roda nativo, no Linux. Lá o vermelho do nome de dispositivo
    (`nul.txt`) só mostra a recusa faltando. O dano apareceu sob o `wine`: os 7
    bytes foram para o dispositivo nulo, com «Invalid handle», e o `a.txt.`
    sobrescreveu o `a.txt`. Guarda com esse dano no catálogo não prova nada.
  - **Defesa nova pode cegar guarda velha.** A recusa de «ponto no fim» também
    pega `..`, porque `..` termina em ponto. Se ficasse assim, o vermelho da
    guarda do zip-slip (tirar a conferência de `..`) não cairia mais: a guarda
    velha viraria «não pegou» sem nenhum teste acusar. O que achou isso foi o
    `trecho-morto`. Conserto: uma recusa por motivo.

## 4. A regra

Numa API nova que abre dado de fora, o padrão é o limite de entrada NÃO
confiável, e a folga é o construtor com nome. «Guarda nova entra pedida» vale
para quem já depende do comportamento velho, e só para ele.

## 5. Como está guardado hoje

- `crates/phxzip/src/leitor.rs`: o `Default` de `Limites` com o motivo de cada
  número, e o `Limites::confiavel`.
- Guardas `phxzip-ciclos-do-arquivo`, `phxzip-derivacoes-por-abertura`,
  `phxzip-contagem-sem-teto`, `phxzip-cabecalho-plano-sem-teto` e
  `phxzip-nome-repetido-na-leitura`, todas provadas.
- **Buraco:** o dano do nome de dispositivo e do ponto final só se mede sob o
  `wine`, à mão. O teste `nome_que_o_windows_desvia_nao_perde_dado_no_destino`
  existe e cai lá, mas nenhuma guarda do catálogo o repõe.
