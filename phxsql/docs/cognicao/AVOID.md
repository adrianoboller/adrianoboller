# AVOID -- o que ja falhou aqui, e por que

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** INFRUTIFERO` -- 1 hoje, de 323 cognicoes no total.

## Dono de arquivo é sinal FORTE, não um palpite como data ou conteúdo

- Causa: a hipótese «dono diferente do processo e do parceiro = terceiro» tomou o sinal que não se FORJA pelo sinal que IDENTIFICA o intruso. O root também é outro dono, e é o administrador: na instalação do MANUAL (§7.4), `sudo 7z x` para trocar um token vazado deixa o `.json` do root ao lado do `.phz` do serviço, e o serviço subia do `.phz` VELHO com o token revogado valendo — a revisão SEC provou pelo sistema operacional, e o teste `crates/phxsql-server/tests/config-phz.rs::o_json_do_root_ao_lado_do_phz_do_servico_recusa_o_arranque` cai com esta regra reposta («ainda rodava depois de 20 s: subiu como servidor», o servidor como uid 65534). O sticky bit, que é o que torna um nome alheio «plantado», nem era conferido. E a régua citada media outra coisa: MySQL e MariaDB ignoram por MODO (gravável por todos); medido, o `mysqld` 8.0.46 LÊ um `my.cnf` de outro dono com 0644.
- Prevencao: antes de usar um metadado como prova de intruso, liste QUEM MAIS produz o mesmo sinal legitimamente (o root, o dono da pasta, o próprio serviço) e exija a condição do sistema operacional que torna o sinal exclusivo do intruso (aqui: sticky bit E pasta gravável por outros, com o root e o dono da pasta fora da conta de terceiro); e ao citar outro motor na régua, cite o CRITÉRIO dele medido pelo binário, não só o comportamento.
- Arquivo: [cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md](cognicao_dono-de-arquivo-e-sinal-forte-nao-palpite_20260924_1024.md)
