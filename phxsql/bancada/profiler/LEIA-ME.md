# A bancada do Profiler

Cinco sondas que provam o Profiler **por soquete**, contra um servidor de
verdade — porque teste de unidade não prova arquivo no disco nem queda de
conexão, e já houve teste aqui que passava por engano.

Todas sobem o `phxsqld` numa porta da faixa **6250–6299**, num diretório
próprio, e **matam só o PID que subiram** — nunca `pkill`.

```bash
cargo build --release -p phxsql-server --bin phxsqld
python3 bancada/profiler/sonda.py
```

| Arquivo | O que prova |
|---|---|
| `comum.py` | sobe o servidor, fala o protocolo, derruba pelo PID |
| `sonda.py` | **a redação**: 20 pedidos torcidos com uma sentinela no lugar da senha; procura a sentinela no anel *e* no `.txt`, e conta as linhas forjadas |
| `sonda-log.py` | **o arquivo**, contra o sistema operacional: disco cheio (tmpfs de 64 KB), sistema de arquivos só-leitura, caminho que é diretório, reinício, e quanto ele cresce |
| `sonda-permissao.py` | **quem entra**: leitor, leitor com `administrar` no curinga, administrador e token de serviço |
| `custo.py` | **quanto custa desligado**: três binários com o portão trocado (`Relaxed` / `false` / `true`) e a mesma carga nos três |
| `custo-ligado.py` | **quanto custa ligado**, separando o tamanho do anel do resto |

## As duas armadilhas desta bancada

**O binário tem de ser o de agora.** `cargo build --release` não recompila o
que já está lá se nada mudou, mas trocar de branch ou esquecer o `--bin`
deixa um binário velho medindo o passado. O `custo.py` compila cada variante
ele mesmo e **copia** o resultado para `bancada/profiler/bin/`, para não haver
dúvida sobre qual binário produziu qual número.

**A máquina tem de estar quieta.** Estas sondas medem diferenças de 1% a 30%
num servidor local; um vizinho ocupando os mesmos núcleos muda o número mais
que a mudança medida. As medições anotadas em `docs/DESEMPENHO.md` dizem em
que condição foram tiradas, e o `custo.py` intercala as variantes (ida e
volta) para o desvio lento se cancelar em vez de cair todo sobre a última.

## O que a sonda devolve

`sonda.py` sai com código 1 se achar a sentinela onde ela não podia estar, ou
uma linha no `.txt` que não começa com data nem com `===` — que é como uma
linha forjada aparece.

Dois casos aparecem **de propósito** e não contam como vazamento: o campo
`obs` em que um humano escreveu `"senha":"..."` dentro do texto (isso é dado,
e tapar seria mentir sobre o dado), e a senha escrita dentro do texto de um
`SELECT`, que nenhuma redação por nome de campo alcança.
