# O chaveiro do sistema era simulado: a chave do nó morria com o processo

**Estado:** FRUTÍFERO

**Evidência:** `869f42e`. O teste `chaveiro_simulado_nao_se_declara_seguro` falha com os
backends de plataforma tirados do `Cargo.toml` e passa com eles. Entre processos (30/09):
pareia, o processo morre, o novo processo entra sem token com cerca 3.

## O que aconteceu

O `OsKeyringProvider` dizia `is_release_safe() = true`. O crate `keyring` 3 foi incluído
sem nenhuma feature de plataforma, e nesse caso ele usa o armazenamento **simulado**, em
memória. Os testes unitários passavam, porque dentro de um processo o simulado funciona.

## O que eu concluí primeiro, e estava errado

Que a prova do servidor de dispositivos pelo fio (4 testes com TLS real) cobria o
pareamento. Ela rodava num processo só, com um chaveiro de teste. O defeito só aparece
quando **outro processo** tenta ler a chave: `KeyProvider(NotFound)`.

## O que a medição disse

Sem feature, `keyring::default` é o módulo `mock` (fonte do crate, `lib.rs`, blocos
«fallback to mock»). Com os backends ligados, este contêiner (sem D-Bus de sessão) passa a
**recusar** com o motivo, em vez de fingir.

## A regra

Provedor de segredo se prova **entre processos**, não dentro de um. E dependência que tem
«modo simulado por omissão» se declara com as features explícitas, com um teste que
reprova a omissão.

## Como está guardado hoje

`chaveiro_simulado_nao_se_declara_seguro` no `phxclaw-key-provider`; o nó recusa parear
com chaveiro não persistente e aponta o cofre em arquivo 0600 (`PHXCLAW_DEVICE_KEYSTORE`).
