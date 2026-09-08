# O backup carrega o próprio marcador — o começo do PITR não sai do relógio

*Descoberto em 08/09/2026, às 16h40, na frente F-PITR.*

## 1. O que aconteceu

O contrato do PITR (`docs/propostas/comparativo-19.md`) diz: «restaura a cópia e
reaplica o diário vivo de cada tabela, do instante da cópia até `ate`». A
pergunta que decide o desenho inteiro é uma só — **de onde começa a
reaplicação?** — e a resposta óbvia é «do primeiro evento cujo carimbo passou
do instante da cópia».

Ela está errada, e o erro não aparece lendo: aparece medindo o que o backup
leva dentro e o que o carimbo garante.

## 2. O que eu concluí primeiro, e estava errado

**Três coisas, e duas delas vieram escritas na própria encomenda da frente.**

- **«O backup talvez não saiba quando foi tirado; se não souber, o instante
  entra no formato.»** Sabia — `backup.json` tem `quando` desde a v1, com
  precisão de milissegundo. O que faltava não era o **dado**, era o **tipo**:
  ele é texto de tela (`2026-08-29 03:00:04,132`), e ler um número de uma
  cadeia formatada para gente amarra o motor ao jeito de escrever.
- **«O diário pode ter sido rodado (pedido 228/G4).»** Não pode. O
  `rodizio.rs` é dos logs de **texto** — `perfil.txt`, `diretivas.log`,
  `acessos.log`. O `.log` de tabela nunca gira: o único lugar do motor que
  apaga um é o `excluir_tabela`, que leva a tabela junto. A recusa que eu ia
  escrever para «diário rodado» não tinha caso; o caso real é outro, e mais
  estreito: **tabela apagada e recriada**.
- **«A conta de tamanho e a comparação do evento são dois guardas.»** São um
  guarda e uma mensagem. Ver a § 3.

As duas primeiras não são desatenção de quem escreveu a encomenda: são
**premissas plausíveis**, do tipo que sobrevive melhor quando ninguém as mede
porque o conserto que elas motivam funciona por outro motivo.

## 3. O que a medição disse

**(a) O `.log` viaja dentro do backup, e o diário da cópia é PREFIXO do vivo.**
Cópia tirada com um evento: `c.log` com um evento. Depois de mais uma inserção,
o `.log` vivo tem dois, e o primeiro é byte a byte o mesmo:

```
copia = 1 evento  | Evento { carimbo: 1788884516707, Inclusao, rowid: 1, versao: 1, tam_imagem: 20 }
vivo  = 2 eventos | o mesmo evento acima, e mais um
```

Ou seja: **o número de eventos do diário da cópia é a posição em que o mundo
estava na hora da cópia.** O marcador vem dentro do backup, de graça, e sem
relógio nenhum.

**(b) O carimbo NÃO é monotônico.** Medido num diário de três eventos, com o
do meio gravado pelo caminho do bidirecional:

```
carimbos: [1788884516705, 1000000000000, 1788884516705]
origens:  [0,             7,             0]
```

Vinte e cinco anos atrás, no meio da lista. Duas causas, as duas no código: o
`Table::forcar_proximo_evento` carimba com o instante em que a escrita
**nasceu** no outro servidor, porque é ele que decide o conflito lá; e o
`agora_ms` é relógio de parede (`SystemTime::now`), que anda para trás num
acerto de NTP.

**(c) Evento gravado antes de ligar a imagem tem `tam_imagem = 0`**, e o
`aplicar_evento` já recusa **nomeando o interruptor**:

```
tam_imagem por evento: [(0, 0), (20, 20)]
RECUSOU: evento de inclusao no rowid 1 veio sem imagem: o source gravou o
         diario com `imagem_da_linha` desligada
RECUSOU: replica divergiu em c: o source diz rowid 2 e aqui saiu 1
```

A segunda linha é a que mais ensina: **pular um evento de inclusão faz a
inclusão seguinte parar na guarda de rowid.** A guarda que a réplica já tinha
é a que segura o PITR quando o filtro de carimbo pula um evento no meio.

**(d) A conta de tamanho não pega nada que a comparação deixaria passar.**
Sabotada (`if false && vivos < posicao`), os dois testes de continuidade
continuam vermelhos — diário mais curto que a posição devolve lista vazia, e
lista vazia já cai na recusa da comparação. Ela ficou, mas pela **mensagem**:
«tem 1 evento e a cópia tinha 2» diz a um operador o que houve, e «o evento 1
não é o mesmo» não diz.

## 4. A regra

**Quando uma cópia tem de continuar de onde parou, procure o marcador DENTRO
da cópia antes de reconstruí-lo pelo relógio.** E: **carimbo de tempo não
ordena nada neste motor — ele filtra; quem ordena é a posição.**

## 5. Como está guardado hoje

- O desenho inteiro está no cabeçalho de `Servidor::reaplicar_diario_ate`
  (`crates/phxsql-server/src/servidor.rs`), que é onde o papel C cobra, e em
  `docs/RESTAURACAO.md` § 7.
- O `quando_ms` do manifesto está em `docs/FORMATO.md` § 10, com o motivo do
  tipo escrito ao lado; os dois campos saem do mesmo número dentro do gerador,
  para não divergirem.
- A não-monotonia está escrita em `docs/RESTAURACAO.md` § 7.3 **com o número**,
  e não como «pode acontecer».
- A dependência nova do `aplicar_evento` está avisada em `docs/REPLICACAO.md`:
  ele tem dois donos agora, e os testes que o travam moram em dois lugares.
- **O buraco que fica:** a `bancada/comparativo/` continua com a sonda de
  código devolvendo `NAO` para o PITR. Ela só vira `TEM` pelo medidor, com
  sonda **viva** — a sequência exata de pedidos do protocolo está em
  `docs/RESTAURACAO.md` § 7.10, escrita para ser transformada em sonda pela
  F-BANCADA, com o controle positivo na mesma corrida. Enquanto ela não rodar,
  esta linha do comparativo continua dizendo que não temos.
