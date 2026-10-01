# Um pareamento recusado apagava a identidade do nó

**Estado:** FRUTÍFERO

**Evidência:** `phxclaw-device-transport`, teste
`pareamento_recusado_nao_apaga_a_identidade_que_ja_vale`. Com o defeito reposto (gravar a
chave antes do envio), o teste reprova no `device.welcome`; com o conserto, passa (5/5,
01/10). O `phxclaw-device-node.exe` (x86_64-pc-windows-gnu), rodado no Wine 9.0 contra o
servidor Linux, pareia, recebe a recusa do token gasto e religa pela chave guardada, com a
cerca indo de 2 para 3. Isso vale com o cofre em arquivo e com o Credential Manager.

## O que aconteceu

Na primeira prova do binário Windows, a segunda execução foi com o mesmo token. O servidor
recusou, o que estava certo. A terceira, sem token, devia religar pela chave guardada e
recebeu «invalid signature». O nó gerava a chave e a gravava no chaveiro antes de enviar o
pedido. A recusa chegava depois, e a chave que o servidor conhecia já tinha sido
sobrescrita.

## O que eu concluí primeiro, e estava errado

Que a primeira execução não tinha pareado, porque não imprimiu «pareado». Ela tinha
pareado: a saída se perdeu no pipe do `grep` cortado pelo `timeout`, e o token gasto
provou isso. A segunda hipótese que levantei, uma diferença de assinatura do Windows,
morreu lendo o código: a gravação acontecia antes do envio em qualquer plataforma.

## O que a medição disse

O defeito é de ordem, não de criptografia. Não aparece no caminho feliz, nem nos testes que
pareiam uma vez só. Aparece quando alguém repete o token, por engano ou de propósito. Nesse
caso o atacante nem precisa de chave válida: basta reenviar um token gasto para derrubar o
nó legítimo.

## O que fica

A chave nasce em memória e só vai para o chaveiro depois do `device.enrolled`. O fluxo
saiu do `main.rs` do nó para `parear()` na biblioteca, e o nó e o teste chamam a mesma
função: o teste prova o caminho que o binário usa.
