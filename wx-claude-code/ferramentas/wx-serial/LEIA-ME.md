# wx-serial — o emissor de serial, separado do plugin

A ferramenta de **quem vende**. Ela não vai para o cliente.

```bash
python3 emitir.py chaves --saida ./segredo        # uma vez, no começo
python3 emitir.py novo --cliente "Softhouse X" --validade 2027-12-31 \
        --email contato@x.com --chave-privada ./segredo/chave-privada.json
python3 emitir.py livro                            # a quem você vendeu
python3 emitir.py reenviar <id>                    # o cliente perdeu o e-mail
python3 emitir.py empacotar --saida wx-serial.zip  # levar num pendrive
```

## Por que separado

O `licenca.py` viaja **dentro do plugin**, na máquina do cliente. Quem emite
precisa da chave **privada**, e chave privada não pode morar na mesma pasta do
produto entregue — basta um `zip -r` distraído. Aqui a pasta é outra, o
`.gitignore` cobre chaves e livro, e a ferramenta se empacota sozinha.

A matemática vem do próprio `licenca.py` por import, nunca de uma cópia: duas
implementações da mesma assinatura divergem em silêncio.

## O livro de emissões

Append-only, um JSON por linha, `0600`, em `~/.wx-serial/` (mude com
`WX_SERIAL_DIR`). Ele mora **fora** do repositório porque a primeira versão o
criou ao lado do script — o acidente exato que a ferramenta existe para evitar.

## O que ela não faz, e a diferença importa

- **Não revoga.** Revogação exige um servidor que o cliente consulte, e não há.
  `marcar-revogado` anota no **seu** livro; o serial continua valendo na máquina
  do cliente até a validade, e o comando diz isso na cara.
- **Não conta instalações.** O serial não telefona para casa.
- **Não imprime a chave privada**, nem em erro.

## Duas recusas provadas

- **Validade no passado** não vira serial: emitir vencido faz o cliente
  descobrir o problema na hora de usar.
- **Serial que não passa no próprio verificador** não vai para o livro nem para
  o cliente. Foi assim que apareceu o defeito real: a conferência usava a chave
  pública do plugin, não a par da privada usada — todo serial recém-emitido saía
  «assinatura-invalida», e o emissor recusaria o próprio trabalho correto.

E o pacote do `empacotar` leva `registro.py` junto porque, sem ele, o zip não
roda: descompactar num diretório vazio é a única prova de que é autônomo.
