# `grep` pelo nome do arquivo não acha quem o escreve por variável

- **Quando:** 2026-09-02, 21:51
- **Onde:** `docs/dossie/numeros-do-projeto.py`, função `escrever_capacidades()`
- **Custo:** eu já tinha dito ao dono, como medido, que o `CAPABILITIES.json`
  era mantido à mão. É gerado.

## O que aconteceu

Procurando quem escreve o `CAPABILITIES.json` — o arquivo que este projeto
chama de *fonte única de verdade* —, rodei:

```sh
grep -rn "CAPABILITIES" --include="*.py" --include="*.rs" --include="*.sh" .
```

Vieram três linhas, todas de leitura. Concluí «nada escreve; é digitado à
mão», e **falei isso em voz alta** como se fosse medição.

A linha que grava é:

```python
alvo = RAIZ / "CAPABILITIES.json"
alvo.write_text(json.dumps(d, indent=2, ensure_ascii=False) + "\n", ...)
```

O `grep` achou a linha do `alvo =`, que tem o nome. **Não achou a do
`write_text`, que não tem** — o nome está numa variável, uma linha acima.

## O que eu concluí primeiro, e estava errado

Que «achei todas as menções do nome» era o mesmo que «achei todos os usos do
arquivo». Não é: nomear e usar são linhas diferentes, e a interessante costuma
ser a que não repete o nome.

E o segundo erro, embutido: eu tinha um sintoma que confirmava a conclusão
errada — o arquivo **estava** velho (commit `eaed55e`, 1.462 testes, contra
`01c87b8` e 1.494). «Está velho» encaixava tão bem em «é feito à mão» que eu
parei de procurar. Era outra causa: o gerador existe e **não rodava desde as
05:25**.

## O que a medição disse

Rodado `python3 docs/dossie/numeros-do-projeto.py`, o arquivo voltou a bater:
`0.18.0 @ 01c87b83`, 1.494 testes, 121 operações.

## A regra

**Para achar quem ESCREVE um arquivo, procure a operação, não o nome.**
`write_text`, `open(..., "w")`, `>` — e depois siga a variável de trás para a
frente. Procurar pelo nome acha quem o menciona, que é outra pergunta.

E a regra irmã, que é a que dói: **sintoma que confirma a hipótese não é
prova dela.** O arquivo estar desatualizado era compatível com «feito à mão» e
com «gerador parado», e eu escolhi a primeira porque já estava com ela na mão.

## Como está guardado hoje

O `docs/versao/conferir.py` lista o `CAPABILITIES.json` entre os **derivados**,
com o comando que o refaz — e com esta armadilha escrita ao lado, para o
próximo não repetir o `grep` pelo nome.
