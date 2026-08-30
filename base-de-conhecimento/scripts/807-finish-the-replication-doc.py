# Finish the replication doc
# 28/08 20:34

import pathlib
p = pathlib.Path("docs/REPLICACAO.md")
s = p.read_text()

antigo = """## 6. Protocolo

Três operações novas, no mesmo JSON Lines da porta 5000:

```json
{"token":"...","op":"posicao","database":"Z"}
{"ok":true,"resultado":{"cadastroClientes":1234,"pedidos":87}}

{"token":"...","op":"replicar","database":"Z","tabela":"cadastroClientes",
 "desde":1234,"max":500}
{"ok":true,"resultado":{"eventos":[...],"ate":1734,"fim":false}}

{"token":"...","op":"aplicar","database":"Z","tabela":"cadastroClientes",
 "eventos":[...]}
```

A réplica roda um laço: pergunta a posição, puxa em lotes, aplica, repete.
Quando o Source responde `"fim":true`, ela espera e pergunta de novo — ou
mantém a conexão aberta e o Source segura a resposta até ter novidade
(long-poll), que é o mais parecido com o binlog dump do MySQL(R)."""
novo = """## 6. Protocolo

Três operações, no mesmo JSON Lines da porta 5000:

```json
{"token":"...","op":"posicao","database":"Z","com_esquema":true}
{"ok":true,"resultado":{
   "papel":"source","imagem_da_linha":true,
   "tabelas":{"cadastroClientes":{"eventos":1234,"registros":1200,
                                  "esquema":"50534348..."}}}}

{"token":"...","op":"replicar","database":"Z","tabela":"cadastroClientes",
 "desde":1234,"max":500}
{"ok":true,"resultado":{"eventos":[...],"desde":1234,"ate":1734,
                        "total":1734,"fim":true}}

{"token":"...","op":"aplicar","database":"Z","tabela":"cadastroClientes",
 "eventos":[...]}
{"ok":true,"resultado":{"recebidos":500,"aplicados":500,"posicao":1734,
                        "erro":null}}
```

A imagem viaja em **hexadecimal**, porque o transporte é JSON e JSON não tem
bytes. Dobra o tamanho; a alternativa seria acrescentar um formato binário ao
protocolo, e isso é uma decisão maior do que esta.

O `com_esquema` traz o **bloco de esquema cru**, o mesmo que mora dentro do
`.reg`. É assim que a réplica cria uma tabela que ainda não existe nela: a
partir dos mesmos bytes, e não de uma remontagem coluna a coluna a partir de
JSON — que é onde um tipo ou uma escala se perderiam sem ninguém notar.

**Três permissões diferentes, de propósito.** `posicao` e `replicar` exigem
`replicar`, que é uma permissão própria: o fluxo é o diário com a linha inteira
dentro, e dá para concedê-lo a uma réplica sem conceder mais nada. `aplicar`
exige `administrar`, porque grava com o rowid escolhido e o payload cru, por
fora das conferências normais.

**`aplicar` não está na lista de operações de escrita**, e a ausência é
deliberada: uma réplica roda em `somente_leitura` justamente para a aplicação
não escrever nela, e a única escrita que ela deve aceitar é a que vem do source.

A réplica roda um laço: pergunta a posição, puxa em lotes de 500, aplica,
dorme `reconectar_em` segundos, repete. Uma **thread por origem**, para uma
origem lenta ou caída não segurar as outras. Erro não mata a thread — escreve e
espera; um source que caiu volta e a réplica retoma do número em que parou.

O laço mora dentro do próprio `phxsqld`: basta `papel: replica` e uma origem no
`config.json`. As operações continuam existindo para quem quiser dirigir a
replicação de fora.

### A senha não viaja

A réplica se autentica pelo mesmo desafio-resposta do resto do protocolo: pede
um nonce, calcula o HMAC com a chave derivada e manda a **prova**. No
`config.json` da réplica mora o `senha_hash` — o mesmo texto que já mora no
cadastro de usuários —, e dele sai a chave derivada. Não há senha em claro em
lugar nenhum.

```json
"origens": [
  {"nome":"curitiba","host":"10.1.1.102","porta":5000,"token":"...",
   "usuario":"replicador","senha_hash":"pbkdf2-sha256$210000$...",
   "databases":["Z"],"reconectar_em":10}
]
```"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """## 9. Ordem de implementação

1. `.log` versão 2 com imagem da linha, atrás do interruptor no `config.json`
2. Ops `posicao` e `replicar` no Source
3. Laço da réplica: puxar, aplicar, conferir o rowid, repetir
4. Long-poll no Source, para a réplica não ficar perguntando à toa
5. Reconexão com espera crescente e retomada pela posição
6. Multi-source (o `config.json` já modela; falta uma thread por origem)
7. TLS no transporte — hoje o JSON vai em claro e depende do IPSec"""
novo = """## 9. O que está feito, e o que falta

| | |
|---|---|
| ☑️ | `.log` versão 2 com imagem da linha, atrás do interruptor |
| ☑️ | Ops `posicao`, `replicar` e `aplicar` |
| ☑️ | Laço da réplica dentro do `phxsqld`: puxar, aplicar, conferir o rowid |
| ☑️ | Criar na réplica a tabela que ainda não existe, do esquema cru do source |
| ☑️ | Reconexão e retomada pela posição — medido: 1,0 s para 4.000 eventos |
| ☑️ | Multi-source: uma thread por origem |
| ☑️ | **Cascata** — Master → Slave01 → Slave03. O segundo salto custou 1.827 ms contra 1.679 do primeiro |
| ☐ | Long-poll no Source, para a réplica não perguntar à toa |
| ☐ | Espera crescente na reconexão (hoje é intervalo fixo) |
| ☐ | TLS no transporte — hoje o JSON vai em claro e depende do IPSec |

### A posição é o diário da própria réplica

A réplica não guarda um arquivo com «apliquei até aqui». Ela **conta os eventos
do `.log` dela** — e é isso que faz a retomada funcionar sem estado extra:
matar a réplica no meio de um lote não perde nem repete, porque o número que
ela usa é o que os arquivos dela dizem, não o que ela lembrava.

Para isso valer, cada evento aplicado tem de gerar **exatamente um** evento
local. É por isso que uma exclusão que não acha o que excluir é tratada como
divergência e para: se passasse batido, o evento não geraria evento, a posição
não andaria, e a replicação giraria em falso puxando o mesmo para sempre.

### Cascata

Uma réplica pode ser origem de outra, e para isso ela precisa de
`imagem_da_linha` ligada **nela também** — senão o diário dela grava que a
linha mudou sem gravar a linha, e o segundo salto não tem o que aplicar. O erro
é explícito e diz o que ligar."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """## 10. O que isto NÃO vai ser

- **Não é replicação síncrona.** É assíncrona, como o padrão do MySQL(R): a
  réplica fica atrás do Source por algum tempo.
- **Não resolve conflito de escrita nos dois lados.** É um caminho só,
  Source → Réplica. Multi-master é outro problema.
- **Não substitui backup.** Réplica repete o `DELETE` errado que você fez no
  Source, e repete rápido."""
novo = """## 10. O que isto NÃO é

- **Não é replicação síncrona.** É assíncrona, como o padrão do MySQL(R): a
  réplica fica atrás do Source por algum tempo. Medido: 1,3 s a 2,1 s com o
  laço em 2 s.
- **A réplica aplica mais devagar do que o master escreve** — 4.273 eventos/s
  contra 18.773 linhas/s, com as três réplicas competindo pela mesma máquina.
  Sob carga sustentada elas ficam para trás. A razão está no caminho: aplicar
  decodifica a imagem para `Value` e **reencoda** o payload, em vez de gravar
  os bytes que vieram. Gravar o payload direto, remendando só os ponteiros dos
  anexos, é o próximo ganho grande — e é o que a seção 3 descreve.
- **Não resolve conflito de escrita nos dois lados.** É um caminho só,
  Source → Réplica. Multi-master é outro problema.
- **Não substitui backup.** Réplica repete o `DELETE` errado que você fez no
  Source, e repete rápido.
- **Não há transação**, então não há ordem global entre tabelas a preservar —
  e é por isso que a posição é por tabela. Quando as transações entrarem, entra
  junto um número de sequência do database inteiro.

---

## 11. Como refazer a medição

```bash
cargo build --release
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```

`montar.py --cascata` põe o Slave03 puxando do Slave01. Detalhes e a última
corrida em `bancada/replicacao/LEIA-ME.md`."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
