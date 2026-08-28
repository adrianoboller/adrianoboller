# Bancada de comparação

A medição do PhxSql contra outros motores. Tudo aqui é para ser **refeito**:
número de desempenho que ninguém consegue reproduzir é número em que não se
deve acreditar.

| Arquivo | O que é |
|---|---|
| `medir.py` | a bancada: cerca cada fase com os contadores do `/proc` |
| `graficos.py` | gera a página de comparação a partir do `resultados.json` |
| `resultados.json` | a última medição **completa**, crua — é dela que o dossiê se gera |
| `carga-10-milhoes.log` | o log da corrida de 10.000.000 |
| `resultados-3-milhoes.json` | a corrida de 3.000.000, guardada inteira |
| `carga-3-milhoes.log` | o log dela |

A carga do lado do PhxSql é `crates/phxsql-store/examples/carga.rs`, que roda
cada fase num processo separado — assim os contadores são daquela fase e de
mais nada.

## Como refazer

```bash
cargo build --release --example carga -p phxsql-store
service mysql start
python3 bancada/medir.py 10000000     # ~50 min: 45 do insert do PhxSql
python3 bancada/graficos.py
```

## O arquivo do repositório só muda no fim

Durante a corrida o progresso vai para `resultados.parcial.json`, que não é
versionado. Só quando a medição fecha inteira ele é promovido a
`resultados.json`, com `os.replace` — que troca de uma vez: ou o arquivo é o
antigo inteiro, ou o novo inteiro, nunca metade de cada.

Isso existe porque uma corrida de dez milhões leva vinte minutos, e durante
esse tempo o arquivo versionado ficava com meia medição dentro. Quem olhasse
via número, não via "faltam quatro fases".

## Duas escalas, e por que as duas ficam

O `resultados.json` guarda uma medição só, e o dossiê se gera dela. Mas
comparar escalas diferentes engana: tabela menor tem árvore mais rasa e cabe
melhor na cache, então parte de qualquer ganho vem do tamanho, não do motor.

Por isso a corrida de 3.000.000 ficou com nome próprio ao lado da de
10.000.000. A comparação limpa entre escalas diferentes é **o primeiro milhão**, que é o
mesmo trabalho nas duas: foi assim que se mediu o efeito do CRC slice-by-8,
5.089/s → 16.063/s.

Melhor ainda é comparar a **mesma escala em dois momentos**, e é para isso que
a corrida de 3.000.000 é refeita a cada rodada de desempenho. Ela isolou o
efeito da rodada da unicidade:

| fase | antes | depois | ganho |
|---|---:|---:|---:|
| inserir | 209,09 s | 159,40 s | 1,31× |
| buscar | 1,06 s | 0,55 s | 1,91× |
| varrer | 1,76 s | 0,66 s | 2,69× |
| atualizar | 0,66 s | 0,56 s | 1,18× |
| excluir | 1,87 s | 1,67 s | 1,12× |

E ensinou uma lição: o conserto que dava nome à rodada — a conferência de
unicidade — foi o **menor** dos cinco ganhos. Quem rendeu foi o `descer`
deixando de reler a folha, que tirou uma página inteira, com o CRC de 4 KB
junto, de *toda* busca do motor.

## O que se mede

Tempo de parede, CPU (`utime+stime`), pico de memória residente (`VmHWM`) e
bytes lidos e escritos (`read_bytes`/`write_bytes`). Do lado do MySQL(R) o
trabalho acontece no `mysqld`, então os contadores dele entram **por
diferença** em volta de cada fase.

## As quatro regras que fazem a comparação valer

1. **Mesmos dados.** O gerador é previsível, sem sorteio: os dois motores
   recebem exatamente as mesmas linhas.
2. **Mesmo esquema.** Chave primária em `id` e índice secundário em `cidade`,
   dos dois lados.
3. **Mesma forma de pergunta.** Uma instrução por operação nos dois. A
   primeira versão desta bancada mandava ao MySQL(R) um único
   `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — o número saía
   41× a favor do MySQL(R) pela *forma da pergunta*, não pelo motor.
4. **Mesma quantidade de trabalho.** Forma igual não basta: a varredura por
   faixa pedia ao MySQL(R) um `COUNT(*) + SUM(valor)` sobre 1.250.000 linhas
   e ao PhxSql a leitura de apenas 20.000 delas. Mesma pergunta, 1,6% do
   trabalho — e o resultado saía 5× a favor do PhxSql sem que o motor
   tivesse feito nada por isso. Corrigido: a fase `varrer` lê a faixa inteira
   e soma o valor, como o outro lado — e a prova de que agora está igual é a
   **soma**: os dois devolvem 1.250.000 linhas e 5.576.201.000,00, o mesmo
   total até o centavo, por dois códigos sem uma linha em comum. O resultado
   sobreviveu: a varredura continua a favor do PhxSql, por 3,3× em vez de 5×.

Estas duas regras vieram do mesmo lugar: **os dois erros favoreciam um lado
diferente, e nenhum dos dois era visível no número**. Bancada mal montada mente
com número, que é a mentira mais convincente que existe.

## O que NÃO é comparado

**Durabilidade.** Os dois carregam em massa, com uma sincronização por lote.
Uma bancada com `commit` por linha daria outros números — e é a que importa
para quem grava pedido a pedido.

**Transações.** O MySQL(R) tem; o PhxSql não. Parte do custo de escrita dele é
o log de *redo*, que compra algo que o PhxSql ainda não oferece.
