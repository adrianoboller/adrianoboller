# HFSQL — Diretivas do banco e do servidor

## 1. Visão geral

O HFSQL não possui um único comando equivalente ao `ALTER SYSTEM ... SET` do PostgreSQL. A configuração é dividida entre:

- servidor: `HSetServer()` ou `<Connection>.SetServer()`;
- conexão: propriedades da variável `Connection`;
- banco e tabelas: funções `HSet...`;
- tarefas, usuários, replicações e triggers: funções específicas.

> Atenção: várias funções `HSet...` alteram apenas o contexto HFSQL da aplicação atual. Elas não representam necessariamente uma configuração global e persistente do servidor.

## 2. Conexão administrativa

```wlanguage
CnxHFSQL is Connection
CnxHFSQL.Server   = "192.168.1.10:4900"
CnxHFSQL.Database = "ERP"
CnxHFSQL.User     = "admin"
CnxHFSQL.Password = "senha"
CnxHFSQL.Provider = hAccessHFClientServer
CnxHFSQL.Access   = hOReadWrite

IF NOT HOpenConnection(CnxHFSQL) THEN
	Error(HErrorInfo())
	RETURN
END
```

Para usar `HSetServer`, o usuário precisa possuir o direito administrativo `hRightsManageServer`.

## 3. Sintaxe básica do servidor

### Consultar uma diretiva

```wlanguage
sValue is string = HSetServer(CnxHFSQL, hActiveDirectory)
```

### Alterar uma diretiva

```wlanguage
HSetServer(CnxHFSQL, hActiveDirectory, 1)
```

Sintaxe orientada a objeto:

```wlanguage
CnxHFSQL.SetServer(hActiveDirectory, 1)
```

## 4. Diretivas Booleanas do servidor

| Recurso | Constante | Ativar | Desativar | Aplicação |
|---|---|---:|---:|---|
| Autenticação Active Directory | `hActiveDirectory` | `1` | `0` | Imediata |
| Estatísticas automáticas dos índices | `hAutoStatisticalCalc` | `1` | `0` | Imediata |
| Pesquisa automática de chaves | `hFindKey` | `1` | `0` | Imediata |
| Balanceamento dinâmico de carga | `hlbActive` | `True` | `False` | Imediata |
| Tabelas de sistema maiores que 2 GB | `hMode2GB` | `1` | `0` | Após reiniciar |
| Telemetria PC SOFT | `hTelemetryEnable` | `1` | `0` | Imediata |
| Histórico de reindexação | `hConserveHistoryReindexing` | `1` | `0` | Conforme a versão |

### Comandos

```wlanguage
// Active Directory
HSetServer(CnxHFSQL, hActiveDirectory, 1) // Ativa
HSetServer(CnxHFSQL, hActiveDirectory, 0) // Desativa

// Cálculo automático de estatísticas
HSetServer(CnxHFSQL, hAutoStatisticalCalc, 1)
HSetServer(CnxHFSQL, hAutoStatisticalCalc, 0)

// Pesquisa automática de chaves
HSetServer(CnxHFSQL, hFindKey, 1)
HSetServer(CnxHFSQL, hFindKey, 0)

// Balanceamento de carga
HSetServer(CnxHFSQL, hlbActive, True)
HSetServer(CnxHFSQL, hlbActive, False)

// Tabelas de sistema acima de 2 GB
HSetServer(CnxHFSQL, hMode2GB, 1)
HSetServer(CnxHFSQL, hMode2GB, 0)

// Telemetria
HSetServer(CnxHFSQL, hTelemetryEnable, 1)
HSetServer(CnxHFSQL, hTelemetryEnable, 0)

// Histórico de reindexação
HSetServer(CnxHFSQL, hConserveHistoryReindexing, 1)
HSetServer(CnxHFSQL, hConserveHistoryReindexing, 0)
```

## 5. Diretivas que equivalem a ligado/desligado

### Estatísticas de atividade

```wlanguage
// Ativa e grava os contadores a cada 60 segundos
HSetServer(CnxHFSQL, hActivityStatisticsPeriod, 60)

// Desativa a coleta
HSetServer(CnxHFSQL, hActivityStatisticsPeriod, 0)
```

### Auditoria das chamadas ao servidor

```wlanguage
// Registra as chamadas WLanguage
HSetServer(CnxHFSQL, hLogLevel, "WL")

// Registra chamadas e parâmetros
HSetServer(CnxHFSQL, hLogLevel, "WL,PARAM")

// Desativa a auditoria
HSetServer(CnxHFSQL, hLogLevel, "")
```

### Limite de conexões

```wlanguage
// Limite de 100 aplicações
HSetServer(CnxHFSQL, hMaxNumberConnection, 100)

// Zero significa sem limite
HSetServer(CnxHFSQL, hMaxNumberConnection, 0)
```

### Cache de disco do Windows

```wlanguage
HSetServer(CnxHFSQL, hWindowsDiskCacheSize, -1)   // Automático
HSetServer(CnxHFSQL, hWindowsDiskCacheSize, 0)    // Ilimitado
HSetServer(CnxHFSQL, hWindowsDiskCacheSize, 4096) // Limite personalizado
```

## 6. Outras diretivas do servidor

| Constante | Função |
|---|---|
| `hActivityStatisticsPath` | Diretório dos arquivos de estatísticas |
| `hActivityStatisticsPeriod` | Intervalo de gravação das estatísticas |
| `hBackupPath` | Diretório dos backups |
| `hCacheNbUnusedFiles` | Máximo de tabelas não utilizadas mantidas abertas |
| `hDaemonUser` | Usuário que executa o serviço no Linux |
| `hDatabasePath` | Diretório raiz dos bancos |
| `hDebuggingPort` | Porta para depuração de procedures e triggers |
| `hJNLBackupPath` | Diretório de backup dos journals |
| `hJNLPath` | Diretório principal dos journals |
| `hkaInterval` | Intervalo do keep-alive |
| `hkaTimeout` | Timeout do keep-alive |
| `hlbClientCalls` | Peso das chamadas no balanceamento |
| `hlbDisk` | Peso dos bytes de disco |
| `hlbDiskAccess` | Peso dos acessos ao disco |
| `hlbMaxTimeout` | Espera máxima do balanceamento |
| `hlbReceived` | Peso dos bytes recebidos |
| `hlbSent` | Peso dos bytes enviados |
| `hLogLevel` | Nível de auditoria |
| `hLogPath` | Diretório dos logs de auditoria |
| `hMaxActivityStatisticsSize` | Tamanho máximo do arquivo estatístico |
| `hMaxLogSize` | Tamanho máximo do arquivo de auditoria |
| `hMaxNumberConnection` | Máximo de conexões por aplicação |
| `hMaxSizeHistoryPlanning` | Máximo de execuções no histórico das tarefas |
| `hNdxCacheSize` | Cache dos índices em MB |
| `hServerLanguage` | Idioma do servidor: `FR`, `US` ou `ES` |
| `hServerPort` | Porta TCP do servidor |
| `hTempDirectory` | Diretório de arquivos temporários |
| `hWindowsDiskCacheSize` | Limite do cache de disco do Windows |

Exemplo:

```wlanguage
HSetServer(CnxHFSQL, hNdxCacheSize, 1024)
HSetServer(CnxHFSQL, hBackupPath, "D:\HFSQL\Backups")
HSetServer(CnxHFSQL, hTempDirectory, "D:\HFSQL\Temp")
HSetServer(CnxHFSQL, hkaInterval, 300)
HSetServer(CnxHFSQL, hkaTimeout, 60)
```

As opções avançadas, como porta, diretórios raiz, usuário do daemon, `hMode2GB` e cache do Windows, podem exigir reinicialização do servidor.

## 7. Diretivas da conexão

```wlanguage
CnxHFSQL.Compression     = True
CnxHFSQL.ActiveDirectory = False
CnxHFSQL.Encryption      = hEncryptionAES256
```

| Propriedade | Tipo | Observação |
|---|---|---|
| `Compression` | Boolean | Compacta os dados transmitidos |
| `ActiveDirectory` | Boolean | Usa autenticação Active Directory |
| `Encryption` | Constante | Define a criptografia da comunicação |
| `Access` | Constante | Leitura ou leitura/escrita |
| `CursorOptions` | Constante | Cursor no cliente ou servidor |

Normalmente essas propriedades devem ser definidas antes de `HOpenConnection()`.

## 8. Diretivas do banco e das tabelas

| Recurso | Ativar | Desativar |
|---|---|---|
| Transações | `HSetTransaction(Tabela, True)` | `HSetTransaction(Tabela, False)` |
| Journal da tabela | `HSetLog(Tabela, True)` | `HSetLog(Tabela, False)` |
| Integridade referencial | `HSetIntegrity(Ligacao, True)` | `HSetIntegrity(Ligacao, False)` |
| Controle de duplicidade | `HSetDuplicates(Tabela.Chave, True)` | `HSetDuplicates(Tabela.Chave, False)` |
| Triggers da aplicação | `HSetTrigger(Tabela, True)` | `HSetTrigger(Tabela, False)` |
| Arquivo `.REP` | `HSetREP(True)` | `HSetREP(False)` |
| Replicação universal | `HSetReplication(True)` | `HSetReplication(False)` |
| Acesso remoto temporário | `HSetRemoteAccess(True)` | `HSetRemoteAccess(False)` |

Exemplo:

```wlanguage
HSetTransaction(Pedido, True)
HSetLog(Pedido, True)
HSetDuplicates(Cliente.CNPJ, True)

// Utilize o nome da ligação existente na análise
HSetIntegrity("Pedido_Cliente", True)
```

Desligar transações, journal, integridade ou duplicidade em produção pode provocar perda de proteção, inconsistências ou dificultar a recuperação.

## 9. Tarefas programadas

```wlanguage
// Ativa
HManageTask(CnxHFSQL, TaskIdentifier, True)

// Desativa
HManageTask(CnxHFSQL, TaskIdentifier, False)
```

A tarefa permanece cadastrada, mas deixa de executar automaticamente quando estiver desativada.

## 10. Triggers do servidor

```wlanguage
HActivateServerTrigger(CnxHFSQL, "TRG_AtualizaEstoque")
HDeactivateServerTrigger(CnxHFSQL, "TRG_AtualizaEstoque")
```

Para triggers armazenados no servidor, prefira as funções específicas em vez de `HSetTrigger`.

## 11. Bloqueio de um banco completo

```wlanguage
// Bloqueia novos acessos
HNoDatabaseAccess(CnxHFSQL, "ERP")

// Libera novamente
HEndNoDatabaseAccess(CnxHFSQL, "ERP")
```

## 12. Modelo de procedimento para alterar uma opção

```wlanguage
PROCEDURE SetHFSQLServerFlag(Cnx is Connection, Option is int, Enabled is boolean)

nValue is int
IF Enabled THEN
	nValue = 1
ELSE
	nValue = 0
END

sPreviousValue is string = HSetServer(Cnx, Option, nValue)

IF HError() <> 0 THEN
	Error(HErrorInfo())
	RESULT False
END

Trace("Valor anterior: " + sPreviousValue)
RESULT True
```

Uso:

```wlanguage
SetHFSQLServerFlag(CnxHFSQL, hTelemetryEnable, False)
SetHFSQLServerFlag(CnxHFSQL, hAutoStatisticalCalc, True)
SetHFSQLServerFlag(CnxHFSQL, hFindKey, True)
```

Para `hlbActive`, pode-se passar `True` ou `False` diretamente.

## 13. Proposta equivalente para o PHXSQL

Uma sintaxe centralizada seria mais simples de administrar:

```sql
ALTER SERVER SET active_directory = TRUE;
ALTER SERVER SET telemetry = FALSE;
ALTER SERVER SET load_balancing = TRUE;
ALTER SERVER SET automatic_statistics = TRUE;

ALTER DATABASE erp SET transactions = TRUE;
ALTER DATABASE erp SET journal = TRUE;
ALTER DATABASE erp SET referential_integrity = TRUE;
ALTER DATABASE erp SET triggers = TRUE;

ALTER TABLE clientes SET duplicate_check = TRUE;
ALTER CONNECTION SET compression = TRUE;

SHOW SERVER SETTINGS;
SHOW DATABASE erp SETTINGS;
```

O PHXSQL deveria registrar toda alteração administrativa:

```text
data_hora
servidor
banco
recurso
valor_anterior
valor_novo
usuario
ip_origem
motivo
```

## 14. Referências oficiais

- [HSetServer / Connection.SetServer](https://doc.windev.com/en-US/?1000022664=)
- [HFConf.ini](https://help.windev.com/en-US/?3044345=)
- [Funções HFSQL](https://help.windev.com/en-US/?3044156=)
- [Variável Connection](https://help.windev.com/en-US/?1514073=)
- [HSetTransaction](https://help.windev.com/en-US/?3044066=)
- [HSetLog](https://help.windev.com/en-US/?3044052=)
- [HSetDuplicates](https://help.windev.com/en-US/?3044057=)
- [HSetIntegrity](https://help.windev.com/en-US/?3044058=)
- [HManageTask](https://help.windev.com/en-US/?1000017113=)

