# Bancada de comparação

A medição do PhxSql contra outros motores. Tudo aqui é para ser **refeito**:
número de desempenho que ninguém consegue reproduzir é número em que não se
deve acreditar.

| Arquivo | O que é |
|---|---|
| `medir.py` | a bancada: cerca cada fase com os contadores do `/proc` |
| `graficos.py` | gera a página de comparação a partir do `resultados.json` |
| `resultados.json` | a última medição, crua |
| `carga-10-milhoes.log` | o log da corrida de 10.000.000 |

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

## O que se mede

Tempo de parede, CPU (`utime+stime`), pico de memória residente (`VmHWM`) e
bytes lidos e escritos (`read_bytes`/`write_bytes`). Do lado do MySQL(R) o
trabalho acontece no `mysqld`, então os contadores dele entram **por
diferença** em volta de cada fase.

## As três regras que fazem a comparação valer

1. **Mesmos dados.** O gerador é previsível, sem sorteio: os dois motores
   recebem exatamente as mesmas linhas.
2. **Mesmo esquema.** Chave primária em `id` e índice secundário em `cidade`,
   dos dois lados.
3. **Mesma forma de pergunta.** Uma instrução por operação nos dois. A
   primeira versão desta bancada mandava ao MySQL(R) um único
   `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — o número saía
   41× a favor do MySQL(R) pela *forma da pergunta*, não pelo motor.

## O que NÃO é comparado

**Durabilidade.** Os dois carregam em massa, com uma sincronização por lote.
Uma bancada com `commit` por linha daria outros números — e é a que importa
para quem grava pedido a pedido.

**Transações.** O MySQL(R) tem; o PhxSql não. Parte do custo de escrita dele é
o log de *redo*, que compra algo que o PhxSql ainda não oferece.
