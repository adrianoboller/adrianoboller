# O Wine finge gravar a ACL, e a sintetiza na leitura

## O que aconteceu

O `acl.rs` do phxvpn restringe os arquivos com chave ao dono no Windows. O
primeiro teste, sob o Wine, leu a ACL de volta **idêntica à de antes**: três
entradas (SYSTEM, o usuário e Todos), sem a marca de protegida. E a chamada
tinha devolvido sucesso.

## O que eu concluí primeiro, e estava errado

1. **Que a ACL montada estava errada** (tamanho, revisão ou SID). Não estava:
   o rastro mostrou que o Wine nem chegou a mandá-la gravar.
2. **Que, trocada a chamada, a leitura mostraria o que gravei.** Também
   errado: o Wine recebeu exatamente o descritor certo e o traduziu para o
   modo Unix. Na leitura, **sintetiza** uma ACL a partir desse modo.
3. **Que a herança da pasta cobriria os arquivos novos.** No Wine não cobre:
   arquivo novo nasce do `umask`.

## O que a medição disse

Rastro `WINEDEBUG=+server`:

- **`SetNamedSecurityInfoW`**: só `get_security_object`, nenhum
  `set_security_object`, e retorno 0 (sucesso).
- **`SetFileSecurityW`**:
  - vai `set_security_object`, com `control=00001004`, uma entrada e o SID
    `S-1-5-21-0-0-0-1000`, e retorno 0;
  - a leitura seguinte devolve `S-1-5-18` e `S-1-5-21-0-0-0-1000`, sem a
    marca de protegida.
- **O essencial, que o Wine consegue provar:** antes, `S-1-1-0` (Todos) lia;
  depois, não lê mais.
- **Arquivo novo** numa pasta protegida, no Wine: volta a ter `S-1-1-0`.

## A regra

Sob o Wine, uma API que devolve sucesso não prova nada sobre o que foi feito.
Prove pelo efeito, lendo de volta. Quando a leitura for sintética, prove a
propriedade que sobrevive à síntese (aqui, "Todos não lê") e deixe a prova
estrita para o sistema de verdade — escrita, não esquecida.

## Como está guardado hoje

- `acl::sob_wine()` decide qual prova vale.
- O teste `arquivo_e_pasta_ficam_so_do_dono` é estrito no Windows e
  essencial no Wine, e diz qual rodou.
- A prova estrita está no `prova-windows.ps1`, passo 3b (`Get-Acl`: uma
  regra, a do usuário, protegida).
- **O buraco:** até alguém rodar esse roteiro num Windows real, a forma
  exata da ACL no Windows é afirmação, não medida.

## Estado

- **Estado:** INFRUTÍFERO
- **Evidência:** `commit:ed57c81`, `phxvpn/prova-windows.ps1` (o teste da ACL só roda no Windows)
- **Causa:** `SetNamedSecurityInfoW` sob o Wine lê a ACL e nunca a grava, devolvendo sucesso.
- **Prevenção:** usar `SetFileSecurityW` e provar pelo efeito, lendo de volta; a prova estrita fica no `prova-windows.ps1` (passo 3b), para Windows real.
- **Validado em:** 24/09/2026
