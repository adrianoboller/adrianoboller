# Corrigir o 4.8, registrar o 4.9 e a licao
# 29/08 06:54

import io
p='docs/DESEMPENHO.md'
s=io.open(p,encoding='utf-8').read()

velho='''### Só que a bancada mal se mexeu: 265,2 → 261,8 s

O motivo não é escala, e não é disco: a corrida de 10 milhões é **95% CPU** e
escreve **2,4 GiB contra 32,0 GiB do MySQL(R)** — treze vezes menos.

É o **esquema**. Mesmo tamanho, mesmo código, um processo só:

| esquema | µs/linha |
|---|---:|
| 3 colunas — `Int8`, `Str(40)`, `Str(20)` | **7,50** |
| 5 colunas da bancada — as três acima mais `Decimal(15,2)` e `Date` | **16,61** |

**2,2× de diferença por causa de duas colunas.** O `onde-doi` mede um esquema
mais simples do que o da bancada, e por isso viu um ganho que a bancada não vê.

Isso não invalida o write-back — ele é real e está medido —, mas **realoca a
fila**: o custo dominante agora é a **codificação da linha** (`montar_payload`,
`codificar_chave`), e não a árvore. As duas colunas suspeitas são o `Decimal`,
que é `i128`, e o `Date`.

**Não está medido** qual das duas custa, nem quanto disso é encode contra
tamanho de slot. É a próxima medição, e ela vem antes de qualquer conserto — a
regra que este documento já aplicou seis vezes.

> E fica a lição sobre o próprio medidor: o `onde-doi` e a bancada usam esquemas
> diferentes, e essa diferença esteve escondida em todos os números desta
> sessão. **Medidor que não mede a mesma coisa que a bancada mede outra coisa.**'''

novo='''### O falso culpado que ficou registrado aqui por algumas horas

A primeira versão desta seção dizia: «a bancada mal se mexeu (265,2 → 261,8 s)
porque o **esquema** dela custa 2,2× — o `Decimal` e o `Date` levam a inserção
de 7,50 para 16,61 µs». Tinha tabela, tinha medição, e estava **errada**.

A prova que a derrubou, em três passos:

1. `--example abrir-contra-criar`: tabela recém-criada e tabela reaberta
   inserem igual (7,48 contra 7,46 µs) — não era o caminho de abertura;
2. o mesmo esquema de 5 colunas medido por um exemplo novo dava **8,0 µs**,
   e pelo `carga` dava 16,9 — **na mesma máquina quieta**;
3. `ls -l` no binário: `target/release/examples/carga` era das 01:56,
   **anterior ao write-back**. `cargo build --release` **não recompila os
   examples**, e a bancada chama o binário direto.

Recompilado: **7,92 µs/linha, 126.280 linhas/s** — o esquema da bancada custa
~0,4 µs a mais que o simples (5%), não 2,2×. O «custo da codificação da linha»
que esta seção mandava investigar era o custo de um binário velho.

**É o sétimo diagnóstico plausível que este documento derruba, e este era
nosso duas vezes**: a medição estava certa, o medidor é que media o passado.

> **Medidor com binário velho mede o passado.** `cargo build --release` não
> recompila os examples; antes de qualquer medição, `cargo build --release
> --examples -p phxsql-store`. A bancada de 261,8 s rodou com um `carga`
> anterior ao write-back — o número oficial está sendo refeito.

### 4.9 O `sincronizar` no caminho da operação: 8 µs/linha, medido

A leitura do Cassandra (`docs/CASSANDRA.md`) apontou: lá o cliente **nunca**
executa `fsync` — nem no modo `batch`, quem sincroniza é uma thread própria
(`AbstractCommitLogService.java:154`). Aqui, a `Durabilidade::PorLote` fecha a
janela **dentro** da 200ª operação.

O critério foi combinado antes de medir: abaixo de 0,46 µs/linha (2% da
bancada), o item morre. `--example custo-do-fsync`, esquema da bancada,
trechos intercalados:

| tabela com | a cada 200 | uma vez só | delta |
|---:|---:|---:|---:|
| 1.000.000 | 16,13 µs | 7,99 µs | **8,14 µs** |
| 3.000.000 | 16,92 µs | 8,05 µs | **8,87 µs** |

**O item vive, e é grande**: o `sincronizar` a cada 200 **dobra** o custo por
linha. O `fsync` em si é ~0,8 ms ÷ 200 = 4 µs; o resto é o write-back sendo
neutralizado — sincronizar a cada 200 descarrega as páginas sujas antes de a
folha encher, e o CRC volta a ser pago por poucas chaves em vez de por
centenas.

O que fazer com isso é **decisão de garantia, não de código**: tirar o `fsync`
do caminho da operação é exatamente o modo `periodic` do Cassandra — o OK deixa
de significar «durável» e passa a significar «recebido». O `BULKINSERT` já dá
isso a quem pede, para carga. Estender ao caminho comum muda o contrato de todo
cliente, e guarda nova entra pedida, não imposta.'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('DESEMPENHO ok')

# A licao no CLAUDE.md, junto das outras de medicao
p='/home/user/adrianoboller/CLAUDE.md'
s=io.open(p,encoding='utf-8').read()
velho='''**Número digitado à mão envelhece calado.**'''
novo='''**Medidor com binário velho mede o passado.** `cargo build --release` não
recompila os *examples*, e a bancada chama `target/release/examples/carga`
direto: uma rodada inteira de ganhos (16,4 → 7,5 µs) ficou invisível na bancada
porque o binário dela era de antes — e a conclusão «o esquema custa 2,2×»
nasceu, com tabela e tudo, dessa diferença. Antes de medir:
`cargo build --release --examples -p phxsql-store`.

**Número digitado à mão envelhece calado.**'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('CLAUDE ok')
