# Bancada da partição alfanumérica

Mede a partição por primeira letra (`ModoParticao::PorLetra`, arquivos
`Tabela_A.reg` … `Tabela_Outros.reg`, descritor `.pag`) **contra um `phxsqld`
de pé**, e não lendo o código.

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
python3 bancada/alfanumerica/sonda.py
```

Variáveis: `PHX_SONDA_PORTA` (padrão 5479) e `PHX_SONDA_POR_LETRA` (padrão 200
linhas por letra, 5.200 no total).

## Por que ela existe

Pergunta do dono, 07/09/2026: *«tabela particionada por campo texto chave, ex
nome, pega a primeira letra e cria uma paginação de A a Z para ficar leve o
cadastro, e na hora de usar é transparente para select, insert, update,
softdelete e delete»*.

O desenho já existia (pedido 11). A pergunta não é «existe?», é **«responde?»**
— e essa só a sonda responde. Ler o código diz o que *deveria* acontecer.

## As três coisas que a primeira corrida ensinou, e que ficaram no script

1. **O instrumento antes do veredito.** A resposta do protocolo vem aninhada em
   `resultado`; lendo o nível de cima, o `varrer` «devolvia 0 linhas» com seis
   gravadas. Sonda que lê o campo errado publica um defeito que não existe.
2. **O controle da tabela SEM partição.** O `atualizar` parcial é recusado
   **também** na tabela normal (`coluna nome e obrigatoria e recebeu NULL`) —
   `atualizar` no PhxSql é linha inteira. Sem esse controle, «o update parcial
   quebra na particionada» teria virado defeito relatado, e seria falso.
3. **`xyzzy` como controle do campo desconhecido.** `balde` e `letra` são
   engolidos calados — mas `xyzzy` também. A tolerância é do protocolo inteiro,
   não um buraco da partição. Acusar a partição por isso seria medir um e culpar
   o outro.

## O que a corrida de 07/09/2026 mediu

| | |
|---|---|
| criar tabela `particao: "letra"` + `particao_coluna: "nome"` | ✅ |
| INSERT distribui por balde | ✅ `_A` (Alves, **Ávila**, Andrade), `_S`, `_9`, `_Outros` (`@estranho`) |
| acento dobra na letra sem acento | ✅ «Ávila» → `_A`, pela mesma tabela do `.fts` |
| `rowid = (balde−1)×1000 + slot` | ✅ 1, 2, 3 no `_A`; 18001 no `_S`; 36001 no `_Outros` |
| `varrer` mostra tudo como UMA tabela | ✅ 6 linhas, `rownum` guardando a ordem de digitação |
| UPDATE mantendo a letra | ✅ |
| UPDATE que mudaria a letra | ⛔ recusado **de propósito**, com a saída escrita na mensagem |
| softdelete e delete físico | ✅ |
| INSERT dentro de transação | ⛔ recusado **de propósito**, com o motivo escrito |
| ler UM balde, 5.200 linhas em 26 letras | ✅ `depois` + `max`, **200 examinadas** de 5.200 |
| custo da letra A × letra Z | 10,7 ms × **8,0 ms** — não cresce com a posição no alfabeto |

**O número que responde o «para ficar leve»**: ler a letra Z toca **200** linhas,
não 5.200, e custa o mesmo que ler a letra A. É a partição pagando o que promete.

**O que NÃO existe**: o paginador A–Z na tela. A canalização está pronta — o
`esquema` devolve `primeiro_rowid` e `registros` de cada balde, e o `varrer`
aceita `depois` + `max` —, mas nenhuma tela oferece as 26 letras para clicar.
