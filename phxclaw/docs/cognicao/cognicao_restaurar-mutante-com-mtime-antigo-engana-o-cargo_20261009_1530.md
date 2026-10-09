# Restaurar o arquivo mutante preservando o mtime engana o cargo

Data da descoberta: 09/10/2026 (integrador, reconferência do semáforo do `max_paralelo`).
Estado: **FRUTÍFERO** — evidência: a mesma reconferência mediu o binário mutante (11/1, rc 101)
logo depois do restauro com `cp -p`, e o verde (12/12, rc 0) só depois de `touch` e recompilar,
com o sha256 do arquivo idêntico nas duas corridas.

## 1. O que se queria
Provar o teste nos dois sentidos: repor o defeito, ver o RED, restaurar e ver o verde.

## 2. O que se descobriu
O cargo decide recompilar pelo mtime do fonte. `cp -p` devolve o conteúdo certo com o mtime de
ANTES da mutação, mais velho que o artefato compilado do mutante: o cargo não recompila e o
«verde» depois do restauro ainda é o binário com o defeito.

## 3. O que eu concluí primeiro, e estava errado
Que o sha256 conferido depois do restauro bastava para dar o verde por bom. O sha prova o fonte,
não o binário que rodou.

## 4. Como se aplica
Restaurar mutante com `cp` sem `-p` (ou `touch` depois), e só aceitar o verde de uma corrida que
recompilou o crate (a linha «Compiling phxclaw-agent» na saída). Vale para toda prova real desta
casa que repõe defeito em fonte Rust.

## 5. Número
1 verde falso medido (11/1, rc 101 com o fonte já restaurado); 0 depois do `touch`.
