# `bancada/proibidos` — comando proibido: recusa, bloqueio e o aviso ao administrador

**Por que existe.** O pedido X do PDF das 26 perguntas é *«bateria de teste com
comandos definidos como proibidos para um banco x que devem ser negados pelo
servidor avisando o administrador por e-mail»*. A recusa e o bloqueio já
estavam medidos por teste unitário; o **e-mail nunca esteve** — e em 07/09/2026
esta bateria mostrou por quê: `violacao_grave` só escrevia no erro padrão. O
aviso entrou no mesmo dia, e a bateria é o que impede ele de sair sem ninguém
ver.

**O que mede.** Seis partes. A parte 0 é o **instrumento antes do veredito**:
um SMTP falso que não recebe nada não prova ausência de e-mail, prova que o
SMTP falso não presta — então ela dispara um aviso que o motor já sabia mandar
(job que falhou) e só depois disso a caixa vale como medida. Depois: o comando
proibido é recusado com `SP000025`, o IP entra na blacklist em disco, o
administrador recebe o e-mail com o corpo colado; a base proibida idem; a
conexão seguinte do IP bloqueado é barrada; o **controle positivo** — comando
permitido não gera e-mail nem bloqueio; e a **guarda pedida** — um terceiro
servidor com `alertas.email.avisar_seguranca` falso bloqueia igual e fica
calado.

**Como roda.** `python3 bancada/proibidos/provar.py`, com
`target/release/phxsqld` já compilado
(`flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server`). Sobe três
servidores (6500, 6501, 6502) e um SMTP falso (6510), todos em 127.0.0.1, com
tudo em `/tmp/phx-f5-<pid>`; derruba por PID no fim. São três servidores porque
cada um **se auto-bloqueia**: 127.0.0.1 não está na whitelist, então o comando
proibido barra o próprio medidor — que é exatamente o comportamento que se quer
provar. Os números vão para `resultados.json`.
