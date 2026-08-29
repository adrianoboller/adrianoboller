# Bancada de replicação — quatro servidores

Master + três espelhos, no ar de verdade, com a medição do que chega e em
quanto tempo. Como toda medição aqui, é para ser **refeita**.

```bash
cargo build --release
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```

`montar.py --cascata` põe o Slave03 puxando do Slave01 em vez do master, para
medir o segundo salto.

| Arquivo | O que é |
|---|---|
| `montar.py` | escreve os quatro `config.json` e sobe os quatro processos |
| `medir.py` | a bancada: atraso por tipo de escrita, vazão, queda e retomada |
| `resultados.json` | a última corrida completa |

## A topologia

```
Master 5800 ──┬──► Slave01 5801
              ├──► Slave02 5802
              └──► Slave03 5803
```

Quem procura é a réplica; o master não empurra nada. É o desenho do MySQL(R), e
existe por causa do firewall: o master abre **uma** porta de entrada e não
precisa alcançar ninguém de volta.

## O que ela compara — e por que não é «quantas linhas»

Duas tabelas com o mesmo número de linhas podem ter conteúdo diferente. A
bancada tira um **SHA-256 de cada linha inteira**, com `rowid` e `rownum`
juntos, lendo pelo cursor. Se um único campo atravessar errado, o retrato muda.

O `rowid` entrar na conta é o ponto: ele **não é transmitido**. O `.reg` nunca
reaproveita slot e o rowid é sempre `slot_count + 1`, então uma réplica que
aplicou todos os eventos na ordem chega ao mesmo número sozinha. Se não chegar,
divergiu — e a replicação para ali em vez de espalhar.

## A senha não fica em claro

O `montar.py` chama `phxsqld --senha` para gerar o `senha_hash`, e é dele que a
réplica deriva a chave do desafio-resposta. Não há senha em claro em nenhum
`config.json` que ele escreve.

## O que a última corrida mediu

| | |
|---|---|
| Master, com a imagem no diário | 34.048 linhas/s |
| Aplicação, por réplica (as três em paralelo) | **17.450 eventos/s** |
| Alcançar 100.000 eventos, as três | 5,7 s |
| Atraso de uma escrita até as três | 140 ms a 2,0 s |
| Réplica derrubada: voltar a atender | 323 ms |
| Réplica derrubada: alcançar 4.000 eventos | 0,3 s |
| Retrato das quatro, no fim | idênticos |

O atraso ainda é dominado pelo `reconectar_em` (2 s aqui) quando a escrita cai
logo depois de a réplica adormecer — é por isso que a mesma coluna traz 140 ms
e 2,0 s. Baixar o intervalo baixa o atraso e sobe o tráfego de perguntas em vão.

### O que estava escrito aqui, e estava errado

Esta seção dizia: «a réplica aplica mais devagar do que o master escreve — a
razão está no caminho: aplicar decodifica a imagem para `Value` e **reencoda**
o payload». Medido, a acusação não se sustenta: `aplicar_evento` custa
**16,15 µs** e uma inserção local pura custa **15,88 µs**
(`--example onde-doi-na-replica`). Decodificar e reencodar custam **0,27 µs**.

Os 4.273 eventos/s eram **229 µs por evento**, e o caminho de CPU inteiro dos
dois lados custa 20,5 µs. Os outros 208 estavam em dois lugares, nenhum deles
na réplica:

1. **O source varria o diário desde o começo a cada lote.** Servir «500 eventos
   a partir de P» caminhava pelos P anteriores lendo o cabeçalho de cada um —
   alcançar 100.000 em lotes de 500 custava **4,07 s só do lado de quem serve**
   (`--example custo-do-desde`). Com a marca de posição, **0,09 s: 45×**.
2. **O laço dormia depois de toda rodada, inclusive das produtivas.** O
   `reconectar_em` é o intervalo entre perguntas **em vão**; uma rodada que
   aplicou eventos volta na hora.

E um terceiro, menor: `bytes_para_hex` fazia um `format!` — e uma alocação de
`String` — **por byte** da imagem. Tabela de dígitos no lugar: 3,48 → 0,24 µs
por evento, **14,5×**.

**4.273 → 17.450 eventos/s por réplica: 4,08×**, e o alcance de 100.000 eventos
caiu de 18,7 s para 5,7 s. Em conjunto as três aplicam ~52.000 eventos/s, mais
do que o master escreve — o que era o pedido 111.

## O que a imagem custa no master

Mesma tabela, mesmas 100.000 linhas, só o interruptor mudando:

| `imagem_da_linha` | linhas/s | bytes por evento | `.log` |
|---|---:|---:|---:|
| desligada | 21.740 | 44 | 4,4 MB |
| ligada | 19.531 | 223 | 22,3 MB |

**10% mais devagar, e um diário 5,1× maior.** É o preço de a réplica receber a
linha e não só o aviso de que ela mudou. Quem só quer auditoria deixa desligado.
