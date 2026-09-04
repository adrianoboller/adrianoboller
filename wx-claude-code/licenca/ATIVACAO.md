# Ativação por serial, cliente a cliente

Versão legível desta instrução; a versão para imprimir e mandar ao cliente é
`docs/ativacao-do-serial.pdf`, gerada de `docs/ativacao-do-serial.html`.

## Passo 0 — uma vez só: o seu par de chaves

```bash
python3 skills/conversao-wx/scripts/licenca.py chaves gerar --saida ~/.wx-claude-code/chaves
cp ~/.wx-claude-code/chaves/chave-publica.json licenca/chave-publica.json
```

A pública entra no plugin e vai para todo cliente. A privada fica só com você,
em `0600`, e **nunca** entra no repositório: quem a tiver emite serial no seu
lugar. A chave que vem no pacote é de demonstração — troque antes do primeiro
cliente.

## Passo 1 — o cliente manda a impressão da máquina

```bash
python3 skills/conversao-wx/scripts/licenca.py maquina
```

Sai um código curto, como `7c76428b7ed7fd9e`. Não tem nada de sigiloso.

## Passo 2 — você emite o serial

```bash
python3 skills/conversao-wx/scripts/licenca.py gerar \
  --cliente "Cliente Exemplo Ltda" \
  --validade 2027-12-31 \
  --maquina 7c76428b7ed7fd9e \
  --email contato@cliente.com.br \
  --chave-privada ~/.wx-claude-code/chaves/chave-privada.json
```

Sai um serial `WX2.…` assinado em RSA-2048/SHA-256. **Guarde-o**: hoje ele não
fica registrado em lugar nenhum.

## Passo 3 — o cliente instala e confere

```bash
python3 skills/conversao-wx/scripts/licenca.py instalar "WX2.eyJjbGll…"
python3 skills/conversao-wx/scripts/licenca.py verificar
```

Grava em `~/.wx-claude-code/licenca` (ou onde `WX_LICENCA` apontar). A partir
daí os hooks liberam os scripts, e o `CLAUDE.md` gerado no projeto sai com a
marca d'água dizendo para quem a licença foi emitida.

## Quando o serial é recusado

| o que aconteceu | por quê | o que fazer |
| --- | --- | --- |
| serial alterado | assinatura não confere | peça de novo; e-mail cortando linha é a causa comum |
| serial vencido | passou da validade | emita outro; não há renovação do antigo |
| serial de outra máquina | a impressão não bate | emita um novo para a máquina certa |
| licença ausente | nada instalado | rode `instalar` |

## O que vai dentro do serial

O serial é assinado, não cifrado: quem o tiver lê `id`, `cliente`, `email`,
`validade`, `maquina` e `emitido_em`. Nada sigiloso entra aí — e é por isso que
você reconstrói o cadastro a partir dos seriais guardados, sem a chave privada.

## Como gerenciar as ativações

Hoje **não há gestão**: o `gerar` imprime o serial e não registra nada. Até que
isso mude, o procedimento é manual:

- **Guarde cada serial emitido**, com cliente e data. É o seu cadastro.
- **Sempre passe `--maquina`.** Sem ela o serial funciona em qualquer computador.
- **Use validade curta**, do tamanho do contrato: é o único controle que existe.
- **Renove com antecedência** — o cliente descobre o vencimento quando o plugin para.

Duas coisas que não existem, e é melhor saber antes de prometer: **revogação**
(um serial vale até a validade, ponto) e **proteção real** (a licença é
dissuasão; não impede quem tem o pacote de ler os arquivos, e a validade é
conferida contra o relógio da máquina do cliente). O que está pendente está em
`licenca/LEIA-ME.md`.
