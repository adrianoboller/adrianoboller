# Segurança do WX Claude Code — o que protege, o que não

Documento da área, no lugar da conversa: o que foi **medido**, o que foi
**decidido**, e o que continua aberto. Atualize aqui quando mexer em licença,
hooks ou guardas.

## A verdade de partida

Um plugin do Claude Code é **texto**. Quem instala lê os agentes, os scripts e
a chave pública; o verificador roda na máquina de quem pode querer burlá-lo.
Nenhuma engenharia local resolve isso — nem ofuscação, nem binário compilado.

Por isso a palavra certa para a camada de hoje é **dissuasão**, não proteção.
Ela vale para o cliente honesto: licença vencida, máquina trocada, pasta passada
adiante por descuido.

## O que quebra, medido no próprio repositório

| ataque | esforço | por que funciona |
| --- | --- | --- |
| apagar duas linhas do `hooks.json` | segundos | o hook é a única trava |
| `WX_LICENCA=/dev/null`, ou editar o `licenca.py` | um minuto | o verificador é do lado do atacante |
| trocar `chave-publica.json` pela própria | minutos | passa a emitir os próprios seriais |
| copiar a pasta para outra máquina | imediato | o serial só prende com `--maquina` na emissão |

Nada disso é defeito a consertar: é a consequência de o produto ser texto na
máquina do cliente.

## O que a criptografia garante de fato

- **Ninguém forja um serial seu.** RSA-2048; o plugin só tem a pública, e por
  isso não emite nem estende validade. Provado por teste: um byte alterado
  invalida.
- **Você sabe a quem vendeu** — o livro de emissões do `ferramentas/wx-serial/`.
- **A senha nunca aparece.** Nem em arquivo, nem em log, nem em resposta; há
  teste que falha se a ficha de usuário vazar o hash.

## O único furo que NÃO é dissuasão

Distribuir com a **chave pública de demonstração** que vem no repositório:
quem lê o repositório tem o par e emite serial válido para o seu plugin.

Isso passou despercebido até a 3.41.0. Agora três lugares avisam, e nenhum
recusa — guarda nova entra pedida, não imposta:

- `licenca.py verificar` imprime o aviso em `stderr`;
- `instalar.sh` e `instalar.ps1` avisam no passo 5;
- `validate_plugin_bundle.py` devolve `avisos_de_distribuicao`, **fora** de
  `warnings` de propósito: em `--strict` um warning derruba `valid`, e uma
  pendência de distribuição não pode travar a bateria de quem só desenvolve.
  O teste guarda exatamente esse comportamento velho.

A correção é de quem distribui, e é um comando:

```bash
python3 ferramentas/wx-serial/emitir.py chaves --saida ~/.wx-serial/chaves
cp ~/.wx-serial/chaves/chave-publica.json licenca/chave-publica.json
```

## A proteção real, quando valer a pena

Servir o que tem valor de um servidor: o corpus do Help (12.035 páginas), os
agentes e as referências, com o serial conferido a cada chamada. Aí apagar o
hook não adianta — não há o que consultar — e serial vazado se revoga na hora.
É o item 20 de `docs/PENDENCIAS.md`, e o `licenca.py` já foi escrito para ser o
cliente dela.

**Recomendação registrada:** a 300 €, o custo de burlar já se aproxima do preço,
e travar mais não paga. O que precisa estar certo é a chave privada nunca sair
da sua máquina — e é para isso que o emissor mora em pasta separada, com o livro
em `~/.wx-serial/`.
