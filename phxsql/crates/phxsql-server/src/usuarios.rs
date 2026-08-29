//! Cadastro de usuarios e poder de cada um sobre cada base.
//!
//! Tudo mora no `config.json`, conforme pedido -- com uma diferenca: a senha
//! e guardada como HASH, nunca em texto puro. Ver [`phxsql_core::senha`].
//!
//! ```json
//! "root":  { "login": "root", "senha_hash": "pbkdf2-sha256$..." },
//! "usuarios": [
//!   {
//!     "id": 2,
//!     "nome": "Adriano Boller",
//!     "login": "adriano",
//!     "senha_hash": "pbkdf2-sha256$210000$...$...",
//!     "email": "adriano@empresa.com.br",
//!     "telefone": "+55 47 99999-0000",
//!     "supervisor": false,
//!     "ativo": true,
//!     "bases": {
//!       "*": { "ler": true, "verificar": true },
//!       "Z": { "ler": true, "inserir": true, "alterar": true, "diario": true }
//!     }
//!   }
//! ]
//! ```
//!
//! # Duas regras que valem a pena conhecer
//!
//! * **Nega por omissao.** Atividade que nao aparece na base e `false`. Base
//!   que nao aparece cai no `"*"`; sem `"*"`, o acesso e negado.
//! * **Supervisor pode tudo, em toda base.** O `root` e sempre supervisor.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::senha;

/// O que um usuario pode fazer numa base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atividade {
    /// Ler dados: `ler`, `varrer`, `buscar`, `esquema`, `tabelas`, `bancos`.
    Ler,
    Inserir,
    Alterar,
    Excluir,
    /// Criar database, schema ou tabela.
    Criar,
    /// Recriar o `.ndx`.
    Reindexar,
    /// Ver o diario da tabela.
    Diario,
    /// Conferir a integridade.
    Verificar,
    /// Ver o log de acessos, os IPs e a configuracao.
    Administrar,
    /// Pedir o fluxo de replicacao.
    Replicar,
}

impl Atividade {
    pub fn nome(self) -> &'static str {
        match self {
            Atividade::Ler => "ler",
            Atividade::Inserir => "inserir",
            Atividade::Alterar => "alterar",
            Atividade::Excluir => "excluir",
            Atividade::Criar => "criar",
            Atividade::Reindexar => "reindexar",
            Atividade::Diario => "diario",
            Atividade::Verificar => "verificar",
            Atividade::Administrar => "administrar",
            Atividade::Replicar => "replicar",
        }
    }

    pub const TODAS: [Atividade; 10] = [
        Atividade::Ler,
        Atividade::Inserir,
        Atividade::Alterar,
        Atividade::Excluir,
        Atividade::Criar,
        Atividade::Reindexar,
        Atividade::Diario,
        Atividade::Verificar,
        Atividade::Administrar,
        Atividade::Replicar,
    ];

    /// Qual atividade uma operacao do protocolo exige.
    ///
    /// `None` significa que a operacao nao exige poder nenhum alem de estar
    /// autenticado -- e o caso do `ping` e do `login`.
    pub fn da_operacao(op: &str) -> Option<Atividade> {
        Some(match op {
            // O catalogo entra aqui, junto com o `quem_sou`, e nao entre as
            // que pedem `ler`: ele descreve o PROTOCOLO, que e documentacao
            // publica, e nao dado. Quem exige poder para ver o catalogo tira a
            // ajuda de quem so insere -- e nao esconde nada, porque a lista de
            // operacoes ja esta no MANUAL. O que ele mostra e filtrado pelo
            // poder de quem perguntou, entao a resposta nunca promete mais do
            // que aquela sessao consegue chamar.
            "ping" | "login" | "desafio" | "quem_sou" | "sair" | "catalogo" => return None,
            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "buscar" => Atividade::Ler,
            // O catalogo e leitura: quem pode ler a tabela pode saber que ela
            // existe e que colunas tem.
            "sistabelas" | "systables" | "siscolunas" | "syscolumns" => Atividade::Ler,
            // O mapa de dado pessoal e catalogo, e nao dado: mostra QUE a
            // coluna guarda CPF, nunca o CPF. Pede `ler` pelo mesmo motivo
            // que `siscolunas` -- e, como varre a base inteira sem campo
            // "tabela", a propria operacao filtra tabela a tabela por dentro.
            "dados_pessoais" | "lgpd" => Atividade::Ler,
            // O pivot resume o que a varredura leria: quem pode ler a tabela
            // pode ver o total dela.
            "pivotar" | "pivot" => Atividade::Ler,
            // Junção e união leem duas ou mais tabelas da MESMA base, e o
            // poder de ler vale por base -- entao ler e o suficiente, e a
            // operacao confere de novo antes de abrir a segunda tabela.
            "juntar" | "join" | "unir" | "union" => Atividade::Ler,
            "sequencias" | "sequences" => Atividade::Ler,
            // A op `sql` so produz `varrer` e `buscar` hoje, e as duas pedem
            // `ler`. Este portao e o de FORA e nao dispensa o de dentro: o
            // pedido traduzido volta pelo mesmo `portoes_do_pedido`, com o
            // nome da tabela no campo que ele ja sabe olhar. Entao ele so pode
            // apertar, nunca afrouxar -- e apertar e o lado certo de errar.
            //
            // O preco esta escrito em docs/SQL.md: quem so tem direito por
            // TABELA, e nenhum na base, para aqui. Nao da para consertar sem o
            // portao ler o texto do SQL, e portao que interpreta linguagem e
            // portao que erra.
            "sql" => Atividade::Ler,
            // A soma de verificacao le a tabela inteira e devolve um numero:
            // quem pode ler a tabela pode saber se ela mudou.
            "checksum" | "soma_de_verificacao" => Atividade::Ler,
            // Exportar e ler a tabela inteira e levar embora. Nao e mais poder
            // do que `varrer` ja da -- e menos, porque nao altera nada.
            "exportar" | "export" => Atividade::Ler,
            // Mexer no contador pode fazer a proxima insercao repetir numero.
            "ajustar_sequencia" => Atividade::Administrar,
            // Consultar em memoria e ler: o dado e o mesmo, o caminho e outro.
            // Carregar tambem, porque carregar e varrer a tabela inteira.
            "memoria_carregar" | "memoria" | "SelectMemory" | "selectmemory"
            | "selecionar_memoria" => Atividade::Ler,
            // O painel conta so o que quem olha poderia abrir, entao pede
            // leitura -- e nao administrar. Um operador tem direito de ver o
            // tamanho do que ele mesmo opera.
            "painel" => Atividade::Ler,
            // Ja o monitor da MAQUINA pede administrar. Nome de placa de rede,
            // nome de disco e ponto de montagem descrevem a infraestrutura, e
            // nao o dado -- quem so le uma tabela nao ganha nada com isso e o
            // atacante ganha o mapa.
            "sistema" => Atividade::Administrar,
            // DbLink inteiro exige administrar, inclusive o que so LE do
            // outro banco. Uma ligacao guarda UMA credencial, e quem a usa
            // fala com o outro servidor como aquele usuario -- as permissoes
            // por base do PhxSql nao atravessam. Deixar um leitor navegar por
            // ela seria emprestar o poder de quem a criou.
            op if op.starts_with("dblink") => Atividade::Administrar,
            // Job inteiro exige administrar, inclusive so LER a lista, e pelo
            // mesmo motivo do DbLink: um job carrega o login sob o qual roda e
            // o pedido inteiro que executa. Ver a lista e ver que operacao roda
            // sobre que tabela de quem; poder salvar um e poder mandar o
            // servidor executar qualquer coisa com o poder de outro usuario.
            // Isso nao pode ser direito de leitor.
            "jobs" | "job_salvar" | "job_excluir" | "job_rodar" => Atividade::Administrar,
            // Parar e subir a porta de dados e o poder mais bruto que ha aqui:
            // um clique tira o servico do ar para todo mundo. Administrar, e
            // nada menos.
            "servico" | "servico_parar" | "servico_subir" => Atividade::Administrar,
            "inserir" => Atividade::Inserir,
            // Reservar a tabela para carga exige o poder de INSERIR nela, e
            // nao mais: quem pode gravar mil linhas pode pedir a tabela para
            // gravar mil linhas. Ja `cargas` -- a lista de quem reservou o que
            // -- mostra o movimento dos outros, e por isso pede administrar,
            // pela mesma razao que `sessoes` pede.
            "bulkinsert" => Atividade::Inserir,
            "cargas" => Atividade::Administrar,
            // Carga em lote e insercao, e nao mais que isso: quem pode gravar
            // uma linha pode gravar mil. O que muda e o custo, e para isso ha
            // o teto de linhas por carga.
            "inserir_lote" | "importar" | "carga" => Atividade::Inserir,
            // Conferir LE a carga que o proprio usuario colou e le o esquema
            // da tabela: nao grava nada, e por isso pede so `ler`. Barrar
            // aqui obrigaria a tentar gravar para descobrir se a carga serve.
            "importar_conferir" => Atividade::Ler,
            "atualizar" => Atividade::Alterar,
            "excluir" => Atividade::Excluir,
            // Restaurar e desfazer uma exclusao: exige o mesmo poder de
            // excluir, e nao mais. Quem pode tirar da lista pode devolver.
            "restaurar" => Atividade::Excluir,
            // O `.trash` e o `.reason` sao dos administradores, e a razao esta
            // no conteudo dos dois. O `.trash` guarda o dado que alguem mandou
            // apagar -- quem so tem `ler` perdeu o direito de ver aquela linha
            // no instante em que ela foi excluida, e a lixeira devolveria o
            // direito por outra porta. O `.reason` costuma ser ainda mais
            // revelador que o registro: "fraude", "pedido de remocao do
            // titular", "duplicidade com o contrato X".
            "lixeira" | "trash" | "motivos" | "reasons" => Atividade::Administrar,
            // Esvaziar a lixeira e a unica operacao do motor que apaga dado
            // sem rede nenhuma embaixo.
            "esvaziar_lixeira" => Atividade::Administrar,
            "criar_database" | "criar_schema" | "criar_tabela" | "duplicar_tabela"
            | "copiar_tabela" => Atividade::Criar,
            // Apagar uma tabela apaga os cinco arquivos de uma vez, e nao ha
            // desfazer. Nao basta poder excluir LINHA para poder excluir a
            // TABELA: isto exige administrar.
            "excluir_tabela" => Atividade::Administrar,
            "reindexar" => Atividade::Reindexar,
            "diario" => Atividade::Diario,
            // Aplicar GRAVA na tabela, e grava por fora das conferencias
            // normais: rowid escolhido do evento, payload cru vindo de fora.
            // Nao e insercao comum, e por isso pede o poder de administrar e
            // nao o de inserir.
            "aplicar" => Atividade::Administrar,
            "verificar" => Atividade::Verificar,
            // As estatisticas resumem o log de acessos, que ja exige
            // administrar: quem ve quanto cada usuario pediu ve o movimento
            // dos outros.
            "estatisticas" | "estatisticas_uso" => Atividade::Administrar,
            // Ver quem esta conectado e derrubar conexao sao poder de
            // administrador: a lista mostra o login e o IP dos outros, e
            // derrubar interrompe o trabalho alheio.
            "sessoes" | "processlist" | "encerrar_sessao" | "kill" => Atividade::Administrar,
            // `config_gravar` esta aqui declarado, e nao so caindo no `_`:
            // a operacao que reescreve o config.json e a ultima que deveria
            // depender do padrao para negar. A op ainda confere por dentro.
            "acessos" | "ips" | "config" | "config_gravar" | "usuarios" | "bloqueios"
            | "desbloquear" => Atividade::Administrar,
            // O profiler mostra o TEXTO dos pedidos de todo mundo, com os
            // dados que estao sendo gravados dentro. Quem pode ler uma tabela
            // nao ganha por isso o direito de ver o que os outros escrevem
            // nela -- nem de mandar o servidor escrever um arquivo no disco.
            "profiler" | "profiler_ligar" | "profiler_desligar" | "profiler_limpar" => {
                Atividade::Administrar
            }
            // O fluxo de replicacao e o diario com a linha inteira dentro:
            // permissao propria, para poder dar a uma replica sem dar mais
            // nada -- e para nao sair de graca junto com `ler`.
            "posicao" | "replicar" => Atividade::Replicar,
            // Operacao desconhecida exige o maior poder: nega por omissao.
            _ => Atividade::Administrar,
        })
    }
}

/// Nivel do usuario: um nome no lugar de dez booleanos.
///
/// Escrever dez permissoes por base, para cada usuario, e onde alguem erra --
/// esquece uma, deixa `administrar` ligado sem querer, copia a linha errada.
/// O nivel resolve o caso comum com uma palavra, e as permissoes por base
/// continuam la para o caso que o nivel nao cobre.
///
/// A ordem importa: cada nivel contem o anterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Nivel {
    /// Nada. E o padrao quando o `config.json` nao diz nivel nenhum.
    ///
    /// Existe porque a regra do projeto e negar por omissao, e sem este nivel
    /// o padrao viraria "le tudo" -- todo config que ja existe passaria a dar
    /// leitura em base que antes negava. Um teste antigo pegou exatamente
    /// isso, e este nivel e a correcao.
    #[default]
    Nenhum,
    /// So le.
    Leitor,
    /// Le e escreve, mas nao cria base nem mexe em indice.
    Operador,
    /// Tudo sobre os dados: cria, reindexa, replica.
    Dono,
    /// Tudo, mais o servidor: acessos, bloqueios, usuarios, backup.
    Admin,
}

impl Nivel {
    pub fn de_texto(s: &str) -> Result<Nivel> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "nenhum" | "nada" => Nivel::Nenhum,
            "leitor" | "consulta" | "leitura" => Nivel::Leitor,
            "operador" | "operacao" | "escrita" => Nivel::Operador,
            "dono" | "owner" | "proprietario" => Nivel::Dono,
            "admin" | "administrador" | "dba" => Nivel::Admin,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "nivel desconhecido: {outro:?} (use leitor, operador, dono ou admin)"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Nivel::Nenhum => "nenhum",
            Nivel::Leitor => "leitor",
            Nivel::Operador => "operador",
            Nivel::Dono => "dono",
            Nivel::Admin => "admin",
        }
    }

    /// O que este nivel pode, numa base.
    pub fn permissoes(self) -> Permissoes {
        if self == Nivel::Nenhum {
            return Permissoes::default();
        }
        let mut p = Permissoes {
            ler: true,
            diario: true,
            verificar: true,
            ..Permissoes::default()
        };
        if self >= Nivel::Operador {
            p.inserir = true;
            p.alterar = true;
            p.excluir = true;
        }
        if self >= Nivel::Dono {
            p.criar = true;
            p.reindexar = true;
            p.replicar = true;
        }
        if self >= Nivel::Admin {
            p.administrar = true;
        }
        p
    }
}

impl PartialOrd for Nivel {
    fn partial_cmp(&self, outro: &Nivel) -> Option<std::cmp::Ordering> {
        Some(self.cmp(outro))
    }
}

impl Ord for Nivel {
    fn cmp(&self, outro: &Nivel) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*outro as u8))
    }
}

/// As dez permissoes de uma base. Tudo comeca em `false`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Permissoes {
    pub ler: bool,
    pub inserir: bool,
    pub alterar: bool,
    pub excluir: bool,
    pub criar: bool,
    pub reindexar: bool,
    pub diario: bool,
    pub verificar: bool,
    pub administrar: bool,
    pub replicar: bool,
}

impl Permissoes {
    pub fn tudo() -> Permissoes {
        Permissoes {
            ler: true,
            inserir: true,
            alterar: true,
            excluir: true,
            criar: true,
            reindexar: true,
            diario: true,
            verificar: true,
            administrar: true,
            replicar: true,
        }
    }

    pub fn pode(&self, a: Atividade) -> bool {
        match a {
            Atividade::Ler => self.ler,
            Atividade::Inserir => self.inserir,
            Atividade::Alterar => self.alterar,
            Atividade::Excluir => self.excluir,
            Atividade::Criar => self.criar,
            Atividade::Reindexar => self.reindexar,
            Atividade::Diario => self.diario,
            Atividade::Verificar => self.verificar,
            Atividade::Administrar => self.administrar,
            Atividade::Replicar => self.replicar,
        }
    }

    fn de_json(j: &Json) -> Permissoes {
        Permissoes {
            ler: j.booleano_ou("ler", false),
            inserir: j.booleano_ou("inserir", false),
            alterar: j.booleano_ou("alterar", false),
            excluir: j.booleano_ou("excluir", false),
            criar: j.booleano_ou("criar", false),
            reindexar: j.booleano_ou("reindexar", false),
            diario: j.booleano_ou("diario", false),
            verificar: j.booleano_ou("verificar", false),
            administrar: j.booleano_ou("administrar", false),
            replicar: j.booleano_ou("replicar", false),
        }
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(
            Atividade::TODAS
                .iter()
                .map(|a| (a.nome(), Json::Bool(self.pode(*a))))
                .collect(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Usuario {
    /// Identificacao numerica, gravada no `.log` de cada tabela como autor da
    /// operacao. Se omitida no `config.json`, sai do CRC-32 do login.
    pub id: u32,
    pub nome: String,
    pub login: String,
    pub senha_hash: String,
    pub email: String,
    pub telefone: String,
    pub supervisor: bool,
    pub ativo: bool,
    /// Nivel: o poder que este usuario tem nas bases onde nao ha regra
    /// explicita. `bases` continua mandando, quando existe.
    pub nivel: Nivel,
    /// Chave publica Ed25519, se este usuario tambem prova posse de chave.
    ///
    /// A senha prova que ele SABE alguma coisa; a chave prova que ele TEM
    /// alguma coisa. Quem copiar o config.json leva so a publica, que nao
    /// assina nada -- e a diferenca em relacao ao hash da senha, que e
    /// exatamente o que o desafio-resposta usa para autenticar.
    pub chave_publica: Option<[u8; phxsql_core::ed25519::CHAVE_LEN]>,
    /// Poder por base. A chave `"*"` vale para as bases nao listadas.
    pub bases: Vec<(String, Permissoes)>,
    /// Poder por TABELA, dentro de cada base.
    ///
    /// Chave de fora: a base (ou `"*"`). Chave de dentro: a tabela (ou `"*"`).
    /// Vem de `"tabelas"` dentro do objeto da base, no `config.json`:
    ///
    /// ```json
    /// "bases": {
    ///   "Z": {
    ///     "ler": true, "inserir": true,
    ///     "tabelas": {
    ///       "folha":    { },
    ///       "clientes": { "ler": true, "inserir": true, "alterar": true }
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Por que e um campo separado, e nao um campo dentro de `bases`
    ///
    /// Se a regra da tabela morasse dentro do objeto da base, listar uma base
    /// so para escrever uma regra de tabela nela passaria a NEGAR tudo o mais
    /// naquela base -- porque a base listada ganha da regra `"*"`, e o objeto
    /// listado so por causa das tabelas teria as dez permissoes em `false`.
    /// Separado, a precedencia de base fica exatamente como era.
    pub tabelas: Vec<(String, Vec<(String, Permissoes)>)>,
}

impl Usuario {
    /// A senha confere?
    pub fn senha_confere(&self, oferecida: &str) -> bool {
        self.ativo && senha::conferir(oferecida, &self.senha_hash)
    }

    /// Permissoes efetivas numa base.
    /// O poder deste usuario nesta base.
    ///
    /// Ordem de precedencia, do mais especifico para o mais geral:
    /// supervisor, a regra da base, a regra `"*"`, e por fim o nivel. O
    /// especifico ganha do geral -- e o que permite dar `admin` a alguem e
    /// ainda assim tirar uma base especifica dele.
    pub fn permissoes(&self, database: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == database) {
            return *p;
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == "*") {
            return *p;
        }
        self.nivel.permissoes()
    }

    /// E administrador? Vale para operacao de servidor, que nao tem base.
    pub fn e_admin(&self) -> bool {
        self.supervisor || self.nivel >= Nivel::Admin
    }

    /// O poder deste usuario nesta TABELA desta base.
    ///
    /// # Por que existe
    ///
    /// Ate a 0.17.0 a permissao parava na base: quem lia a base lia todas as
    /// tabelas dela, e nao havia como dar `clientes` sem dar `folha`. A folha
    /// de pagamento e a tabela de clientes moram no mesmo banco porque o
    /// negocio e um so, e o direito de ler as duas nao e o mesmo direito.
    ///
    /// # A ordem de precedencia
    ///
    /// A mesma regra que ja valia entre base e `"*"`: **o especifico ganha do
    /// geral, e substitui**. Do mais especifico para o mais geral:
    ///
    /// 1. supervisor -- pode tudo, em toda tabela;
    /// 2. a regra desta tabela nesta base;
    /// 3. a regra `"*"` de tabela nesta base;
    /// 4. a regra desta tabela na base `"*"`;
    /// 5. a regra `"*"` de tabela na base `"*"`;
    /// 6. e so entao a regra da BASE (que por sua vez cai em `"*"` e no nivel).
    ///
    /// Substituir, e nao interceder, e o que permite os dois casos que a
    /// pratica pede: **tirar** uma tabela de quem le a base inteira, e **dar**
    /// uma tabela a quem nao le a base nenhuma.
    ///
    /// Tabela vazia -- operacao que nao fala de tabela, como `bancos` ou
    /// `criar_database` -- cai direto na regra da base.
    pub fn permissoes_em(&self, database: &str, tabela: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if !tabela.is_empty() {
            for base in [database, "*"] {
                if let Some((_, regras)) = self.tabelas.iter().find(|(b, _)| b == base) {
                    for alvo in [tabela, "*"] {
                        if let Some((_, p)) = regras.iter().find(|(t, _)| t == alvo) {
                            return *p;
                        }
                    }
                }
            }
        }
        self.permissoes(database)
    }

    /// Pode fazer a atividade nesta base?
    pub fn pode(&self, database: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes(database).pode(atividade)
    }

    /// Pode fazer a atividade nesta tabela desta base?
    ///
    /// Tabela vazia e o mesmo que perguntar so pela base.
    pub fn pode_em(&self, database: &str, tabela: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes_em(database, tabela).pode(atividade)
    }

    /// Ficha do usuario, sem a senha. Nunca devolve o hash.
    pub fn ficha(&self) -> Json {
        Json::objeto(vec![
            ("id", Json::de_u64(self.id as u64)),
            ("nome", Json::texto_de(&self.nome)),
            ("login", Json::texto_de(&self.login)),
            ("email", Json::texto_de(&self.email)),
            ("telefone", Json::texto_de(&self.telefone)),
            ("nivel", Json::texto_de(self.nivel.nome())),
            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),
            // Diz que HA chave, nunca qual e. A publica nao e segredo, mas
            // tambem nao ha motivo para espalhar quem usa o que.
            ("exige_chave", Json::Bool(self.chave_publica.is_some())),
            (
                "bases",
                Json::Objeto(
                    self.bases
                        .iter()
                        .map(|(b, p)| (b.clone(), p.para_json()))
                        .collect(),
                ),
            ),
            (
                "tabelas",
                Json::Objeto(
                    self.tabelas
                        .iter()
                        .map(|(b, regras)| {
                            (
                                b.clone(),
                                Json::Objeto(
                                    regras
                                        .iter()
                                        .map(|(t, p)| (t.clone(), p.para_json()))
                                        .collect(),
                                ),
                            )
                        })
                        .collect(),
                ),
            ),
        ])
    }

    fn de_json(j: &Json, avisos: &mut Vec<String>) -> Result<Usuario> {
        let login = j.texto_ou("login", "").trim().to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("usuario sem login".into()));
        }

        let hash = extrair_hash(j, &login, avisos)?;

        // supervisor e um admin de todas as bases -- e a forma antiga de
        // dizer a mesma coisa. Mantida, e agora ela ACERTA o nivel, para a
        // ficha nao dizer "leitor" de quem pode tudo.
        let supervisor = j.booleano_ou("supervisor", false);
        let nivel = if supervisor {
            Nivel::Admin
        } else {
            Nivel::de_texto(j.texto_ou("nivel", ""))?
        };

        let chave_publica = match j.campo("chave_publica").and_then(Json::texto) {
            None => None,
            Some(hex) if hex.trim().is_empty() => None,
            Some(hex) => Some(phxsql_core::ed25519::chave_de_hex(hex).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "chave_publica de {login} nao e uma chave Ed25519 (precisa de 64 hexadecimais)"
                ))
            })?),
        };

        let bases = match j.campo("bases") {
            Some(Json::Objeto(pares)) => pares
                .iter()
                .map(|(base, perm)| (base.clone(), Permissoes::de_json(perm)))
                .collect(),
            _ => Vec::new(),
        };

        // `"tabelas"` sai de dentro do objeto da base. Base sem `"tabelas"`
        // nao entra aqui: uma lista vazia e uma lista ausente dariam na mesma
        // no lookup, e a ausente nao ocupa lugar.
        let mut tabelas: Vec<(String, Vec<(String, Permissoes)>)> = Vec::new();
        if let Some(Json::Objeto(pares)) = j.campo("bases") {
            for (base, perm) in pares {
                if let Some(Json::Objeto(porta)) = perm.campo("tabelas") {
                    if porta.is_empty() {
                        continue;
                    }
                    tabelas.push((
                        base.clone(),
                        porta
                            .iter()
                            .map(|(t, p)| (t.clone(), Permissoes::de_json(p)))
                            .collect(),
                    ));
                }
            }
        }

        let id = j
            .campo("id")
            .and_then(Json::inteiro)
            .filter(|n| *n > 0 && *n <= u32::MAX as i64)
            .map(|n| n as u32)
            .unwrap_or_else(|| phxsql_core::crc::crc32(login.as_bytes()).max(1));

        Ok(Usuario {
            id,
            nome: j.texto_ou("nome", &login).to_string(),
            login,
            senha_hash: hash,
            email: j.texto_ou("email", "").to_string(),
            telefone: j.texto_ou("telefone", "").to_string(),
            supervisor,
            ativo: j.booleano_ou("ativo", true),
            nivel,
            chave_publica,
            bases,
            tabelas,
        })
    }
}

/// Aceita `senha_hash` (o certo) ou `senha` em texto puro (avisando alto).
fn extrair_hash(j: &Json, login: &str, avisos: &mut Vec<String>) -> Result<String> {
    if let Some(h) = j.campo("senha_hash").and_then(Json::texto) {
        if senha::e_hash(h) {
            return Ok(h.to_string());
        }
        return Err(PhxError::Esquema(format!(
            "senha_hash de {login} esta malformada; gere com: phxsqld --senha"
        )));
    }
    if let Some(clara) = j.campo("senha").and_then(Json::texto) {
        if clara.is_empty() {
            return Err(PhxError::Esquema(format!("usuario {login} sem senha")));
        }
        avisos.push(format!(
            "usuario {login} esta com a SENHA EM TEXTO PURO no config.json. \
             Troque por senha_hash: phxsqld --senha"
        ));
        return Ok(senha::cifrar(clara));
    }
    Err(PhxError::Esquema(format!(
        "usuario {login} sem senha_hash nem senha"
    )))
}

/// O cadastro inteiro: o root e os demais.
#[derive(Debug, Clone, Default)]
pub struct Cadastro {
    pub root: Option<Usuario>,
    pub usuarios: Vec<Usuario>,
    /// Problemas que nao impedem subir, mas que precisam aparecer no arranque.
    pub avisos: Vec<String>,
}

impl Cadastro {
    pub fn de_json(config: &Json) -> Result<Cadastro> {
        let mut avisos = Vec::new();

        let root = match config.campo("root") {
            None => None,
            Some(j) => {
                let mut u = Usuario::de_json(j, &mut avisos)?;
                if u.login.is_empty() {
                    u.login = "root".to_string();
                }
                // O root e supervisor por definicao, diga o que disser o arquivo.
                u.supervisor = true;
                u.nivel = Nivel::Admin;
                u.ativo = true;
                if u.id == 0 {
                    u.id = 1;
                }
                Some(u)
            }
        };

        let mut usuarios = Vec::new();
        if let Some(lista) = config.campo("usuarios").and_then(Json::lista) {
            for j in lista {
                let u = Usuario::de_json(j, &mut avisos)?;
                if usuarios.iter().any(|o: &Usuario| o.login == u.login) {
                    return Err(PhxError::Esquema(format!("login repetido: {}", u.login)));
                }
                if root.as_ref().is_some_and(|r| r.login == u.login) {
                    return Err(PhxError::Esquema(format!(
                        "o login {} colide com o root",
                        u.login
                    )));
                }
                usuarios.push(u);
            }
        }

        for u in &usuarios {
            if let Some(outro) = usuarios.iter().find(|o| o.id == u.id && o.login != u.login) {
                return Err(PhxError::Esquema(format!(
                    "id {} repetido entre {} e {}",
                    u.id, u.login, outro.login
                )));
            }
        }

        Ok(Cadastro {
            root,
            usuarios,
            avisos,
        })
    }

    /// Ha alguem cadastrado? Sem cadastro, o servidor cai no token de servico.
    /// Algum usuario exige chave? A pagina de entrada usa isto para decidir
    /// se mostra o campo -- e nao ha segredo nenhum na resposta.
    pub fn alguem_exige_chave(&self) -> bool {
        self.root
            .iter()
            .chain(self.usuarios.iter())
            .any(|u| u.chave_publica.is_some())
    }

    pub fn vazio(&self) -> bool {
        self.root.is_none() && self.usuarios.is_empty()
    }

    pub fn por_login(&self, login: &str) -> Option<&Usuario> {
        if let Some(r) = &self.root {
            if r.login == login {
                return Some(r);
            }
        }
        self.usuarios.iter().find(|u| u.login == login)
    }

    /// Confere login e senha. Devolve o usuario so quando os dois batem e a
    /// conta esta ativa.
    ///
    /// Quando o login nao existe, ainda assim gasta o tempo de um PBKDF2, para
    /// que "usuario inexistente" e "senha errada" nao se distingam pelo relogio.
    pub fn autenticar(&self, login: &str, oferecida: &str) -> Option<&Usuario> {
        match self.por_login(login) {
            Some(u) if u.senha_confere(oferecida) => Some(u),
            Some(_) => None,
            None => {
                let _ = senha::conferir(oferecida, &senha::cifrar_com("nao-existe", 1_000));
                None
            }
        }
    }

    /// Fichas de todos, sem senha.
    pub fn fichas(&self) -> Json {
        let mut lista: Vec<Json> = Vec::new();
        if let Some(r) = &self.root {
            lista.push(r.ficha());
        }
        lista.extend(self.usuarios.iter().map(Usuario::ficha));
        Json::Lista(lista)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cadastro(txt: &str) -> Cadastro {
        Cadastro::de_json(&Json::analisar(txt).unwrap()).unwrap()
    }

    fn hash_rapido(s: &str) -> String {
        senha::cifrar_com(s, 64)
    }

    #[test]
    fn le_o_cadastro_completo() {
        let txt = format!(
            r#"{{
              "root": {{"login":"root","senha_hash":"{}"}},
              "usuarios": [{{
                "id": 7,
                "nome": "Adriano Boller",
                "login": "adriano",
                "senha_hash": "{}",
                "email": "adriano@empresa.com.br",
                "telefone": "+55 47 99999-0000",
                "supervisor": false,
                "ativo": true,
                "bases": {{
                  "*": {{"ler": true}},
                  "Z": {{"ler": true, "inserir": true, "alterar": true, "diario": true}}
                }}
              }}]
            }}"#,
            hash_rapido("raiz"),
            hash_rapido("segredo")
        );
        let c = cadastro(&txt);
        let u = c.por_login("adriano").unwrap();
        assert_eq!(u.id, 7);
        assert_eq!(u.nome, "Adriano Boller");
        assert_eq!(u.email, "adriano@empresa.com.br");
        assert_eq!(u.telefone, "+55 47 99999-0000");
        assert!(!u.supervisor);
        assert!(u.ativo);
        assert!(c.avisos.is_empty());
    }

    #[test]
    fn poder_por_base_com_curinga() {
        let txt = format!(
            r#"{{"usuarios":[{{
                 "login":"joao","senha_hash":"{}",
                 "bases":{{
                   "*":{{"ler":true}},
                   "Z":{{"ler":true,"inserir":true,"alterar":true}},
                   "W":{{}}
                 }}}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        let u = c.por_login("joao").unwrap();

        // Base listada: vale o que esta la.
        assert!(u.pode("Z", Atividade::Ler));
        assert!(u.pode("Z", Atividade::Inserir));
        assert!(!u.pode("Z", Atividade::Excluir), "nega por omissao");

        // Base listada vazia: nega tudo, sem cair no curinga.
        assert!(!u.pode("W", Atividade::Ler));

        // Base nao listada: cai no curinga.
        assert!(u.pode("QualquerOutra", Atividade::Ler));
        assert!(!u.pode("QualquerOutra", Atividade::Inserir));
    }

    #[test]
    fn sem_curinga_e_sem_base_nega_tudo() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}","bases":{{"Z":{{"ler":true}}}}}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        let u = c.por_login("ana").unwrap();
        assert!(u.pode("Z", Atividade::Ler));
        for a in Atividade::TODAS {
            assert!(
                !u.pode("W", a),
                "base nao listada deveria negar {}",
                a.nome()
            );
        }
    }

    #[test]
    fn supervisor_pode_tudo_em_toda_base() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"chefe","senha_hash":"{}","supervisor":true}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        let u = c.por_login("chefe").unwrap();
        for a in Atividade::TODAS {
            assert!(
                u.pode("QualquerBase", a),
                "supervisor deveria poder {}",
                a.nome()
            );
        }
    }

    #[test]
    fn root_e_supervisor_mesmo_dizendo_o_contrario() {
        let txt = format!(
            r#"{{"root":{{"login":"root","senha_hash":"{}","supervisor":false,"ativo":false}}}}"#,
            hash_rapido("raiz")
        );
        let c = cadastro(&txt);
        let r = c.root.as_ref().unwrap();
        assert!(r.supervisor);
        assert!(r.ativo);
        assert!(r.pode("Z", Atividade::Administrar));
    }

    #[test]
    fn usuario_inativo_nao_entra_nem_faz_nada() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"afastado","senha_hash":"{}",
                 "ativo":false,"supervisor":true}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        assert!(c.autenticar("afastado", "x").is_none());
        let u = c.por_login("afastado").unwrap();
        assert!(!u.pode("Z", Atividade::Ler), "inativo nao pode nem ler");
    }

    #[test]
    fn autenticacao() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}"}}]}}"#,
            hash_rapido("Senha Certa")
        );
        let c = cadastro(&txt);
        assert!(c.autenticar("ana", "Senha Certa").is_some());
        assert!(c.autenticar("ana", "senha certa").is_none());
        assert!(c.autenticar("ana", "").is_none());
        assert!(c.autenticar("inexistente", "Senha Certa").is_none());
    }

    #[test]
    fn senha_em_texto_puro_funciona_mas_avisa() {
        let txt = r#"{"usuarios":[{"login":"legado","senha":"1234"}]}"#;
        let c = cadastro(txt);
        assert!(c.autenticar("legado", "1234").is_some());
        assert_eq!(c.avisos.len(), 1);
        assert!(
            c.avisos[0].contains("TEXTO PURO"),
            "aviso foi {:?}",
            c.avisos[0]
        );
        // E o que ficou em memoria ja e hash, nao a senha.
        assert!(senha::e_hash(&c.por_login("legado").unwrap().senha_hash));
    }

    #[test]
    fn a_ficha_nunca_devolve_a_senha() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}"}}]}}"#,
            hash_rapido("segredo")
        );
        let c = cadastro(&txt);
        let ficha = c.fichas().escrever();
        assert!(!ficha.contains("senha"), "a ficha vazou: {ficha}");
        assert!(!ficha.contains("pbkdf2"));
        assert!(ficha.contains("\"login\":\"ana\""));
    }

    #[test]
    fn cadastro_invalido_e_recusado() {
        let h = hash_rapido("x");
        for ruim in [
            r#"{"usuarios":[{"login":""}]}"#.to_string(),
            format!(
                r#"{{"usuarios":[{{"login":"a","senha_hash":"{h}"}},
                        {{"login":"a","senha_hash":"{h}"}}]}}"#
            ),
            format!(
                r#"{{"usuarios":[{{"id":5,"login":"a","senha_hash":"{h}"}},
                        {{"id":5,"login":"b","senha_hash":"{h}"}}]}}"#
            ),
            r#"{"usuarios":[{"login":"a","senha_hash":"nao-e-hash"}]}"#.to_string(),
            r#"{"usuarios":[{"login":"a"}]}"#.to_string(),
            format!(
                r#"{{"root":{{"login":"root","senha_hash":"{h}"}},
                        "usuarios":[{{"login":"root","senha_hash":"{h}"}}]}}"#
            ),
        ] {
            assert!(
                Cadastro::de_json(&Json::analisar(&ruim).unwrap()).is_err(),
                "deveria recusar: {ruim}"
            );
        }
    }

    #[test]
    fn id_sai_do_login_quando_omitido_e_e_estavel() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"adriano","senha_hash":"{}"}}]}}"#,
            hash_rapido("x")
        );
        let a = cadastro(&txt);
        let b = cadastro(&txt);
        let id = a.por_login("adriano").unwrap().id;
        assert_eq!(id, b.por_login("adriano").unwrap().id);
        assert!(id > 0, "o id vai para o .log e nao pode ser zero");
    }

    #[test]
    fn cada_operacao_exige_a_atividade_certa() {
        assert_eq!(Atividade::da_operacao("ping"), None);
        assert_eq!(Atividade::da_operacao("login"), None);
        assert_eq!(Atividade::da_operacao("buscar"), Some(Atividade::Ler));
        assert_eq!(Atividade::da_operacao("inserir"), Some(Atividade::Inserir));
        assert_eq!(
            Atividade::da_operacao("atualizar"),
            Some(Atividade::Alterar)
        );
        assert_eq!(Atividade::da_operacao("excluir"), Some(Atividade::Excluir));
        assert_eq!(Atividade::da_operacao("diario"), Some(Atividade::Diario));
        assert_eq!(Atividade::da_operacao("ips"), Some(Atividade::Administrar));
        assert_eq!(Atividade::da_operacao("desafio"), None);
        assert_eq!(
            Atividade::da_operacao("bloqueios"),
            Some(Atividade::Administrar)
        );
        assert_eq!(
            Atividade::da_operacao("desbloquear"),
            Some(Atividade::Administrar)
        );
        // Operacao desconhecida exige o maior poder, em vez de passar batido.
        assert_eq!(
            Atividade::da_operacao("op_que_nao_existe"),
            Some(Atividade::Administrar)
        );
    }
    #[test]
    fn a_memoria_pede_leitura_e_o_backup_pede_administrar() {
        // Consultar em memoria nao pode exigir mais poder do que ler do disco:
        // e o mesmo dado. Ja o backup e conta de administrador.
        for op in [
            "memoria_carregar",
            "memoria",
            "SelectMemory",
            "selecionar_memoria",
        ] {
            assert_eq!(Atividade::da_operacao(op), Some(Atividade::Ler), "{op}");
        }
        for op in ["backup", "conferir_backup", "memoria_liberar"] {
            assert_eq!(
                Atividade::da_operacao(op),
                Some(Atividade::Administrar),
                "{op}"
            );
        }
        assert_eq!(Atividade::da_operacao("sair"), None);
    }
    #[test]
    fn cada_nivel_contem_o_anterior() {
        let nenhum = Nivel::Nenhum.permissoes();
        let leitor = Nivel::Leitor.permissoes();
        let operador = Nivel::Operador.permissoes();
        let dono = Nivel::Dono.permissoes();
        let admin = Nivel::Admin.permissoes();

        for a in Atividade::TODAS {
            assert!(!nenhum.pode(a), "nenhum deu {}", a.nome());
            if leitor.pode(a) {
                assert!(operador.pode(a), "operador perdeu {}", a.nome());
            }
            if operador.pode(a) {
                assert!(dono.pode(a), "dono perdeu {}", a.nome());
            }
            if dono.pode(a) {
                assert!(admin.pode(a), "admin perdeu {}", a.nome());
            }
        }

        // E cada um acrescenta alguma coisa de verdade.
        assert!(!leitor.inserir && operador.inserir);
        assert!(!operador.criar && dono.criar);
        assert!(!dono.administrar && admin.administrar);
        // Leitor le, e so.
        assert!(leitor.ler && leitor.diario && leitor.verificar);
        assert!(!leitor.excluir && !leitor.reindexar && !leitor.replicar);
    }

    #[test]
    fn o_nivel_vale_onde_nao_ha_regra_de_base() {
        let txt = format!(
            r#"{{"usuarios":[{{
                 "login":"ana","senha_hash":"{}","nivel":"operador",
                 "bases":{{"Financeiro":{{}}}}
               }}]}}"#,
            hash_rapido("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let ana = c.por_login("ana").unwrap();
        assert_eq!(ana.nivel, Nivel::Operador);
        // Onde nao ha regra, vale o nivel.
        assert!(ana.pode("Comercial", Atividade::Inserir));
        assert!(ana.pode("Comercial", Atividade::Ler));
        assert!(!ana.pode("Comercial", Atividade::Criar));
        // Onde HA regra, a regra manda -- mesmo para tirar poder.
        assert!(!ana.pode("Financeiro", Atividade::Ler));
        assert!(!ana.pode("Financeiro", Atividade::Inserir));
    }

    #[test]
    fn sem_nivel_o_padrao_nega_tudo() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ze","senha_hash":"{}"}}]}}"#,
            hash_rapido("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let ze = c.por_login("ze").unwrap();
        assert_eq!(ze.nivel, Nivel::Nenhum);
        assert!(!ze.e_admin());
        // Nada. Config que nao diz nivel nao ganha poder nenhum de brinde --
        // e o que faz esta mudanca nao alterar nenhum config que ja existe.
        for a in Atividade::TODAS {
            assert!(!ze.pode("Qualquer", a), "sem nivel deu {}", a.nome());
        }
    }

    #[test]
    fn nivel_admin_e_supervisor_dizem_a_mesma_coisa() {
        let txt = format!(
            r#"{{"usuarios":[
                 {{"login":"a","senha_hash":"{h}","nivel":"admin"}},
                 {{"login":"b","senha_hash":"{h}","supervisor":true}}
               ]}}"#,
            h = hash_rapido("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let a = c.por_login("a").unwrap();
        let b = c.por_login("b").unwrap();
        assert!(a.e_admin() && b.e_admin());
        // A ficha do supervisor nao pode dizer "leitor" de quem pode tudo.
        assert_eq!(b.nivel, Nivel::Admin);
        for at in Atividade::TODAS {
            assert_eq!(
                a.pode("Comercial", at),
                b.pode("Comercial", at),
                "divergiram em {}",
                at.nome()
            );
        }
    }

    #[test]
    fn nivel_desconhecido_nao_sobe() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"x","senha_hash":"{}","nivel":"chefao"}}]}}"#,
            hash_rapido("x")
        );
        assert!(Cadastro::de_json(&Json::analisar(&txt).unwrap()).is_err());
    }

    #[test]
    fn os_apelidos_de_nivel_valem() {
        assert_eq!(Nivel::de_texto("ADMIN").unwrap(), Nivel::Admin);
        assert_eq!(Nivel::de_texto(" dba ").unwrap(), Nivel::Admin);
        assert_eq!(Nivel::de_texto("consulta").unwrap(), Nivel::Leitor);
        assert_eq!(Nivel::de_texto("").unwrap(), Nivel::Nenhum);
        assert_eq!(Nivel::de_texto("nenhum").unwrap(), Nivel::Nenhum);
        assert_eq!(Nivel::de_texto("leitor").unwrap(), Nivel::Leitor);
        assert_eq!(Nivel::de_texto("owner").unwrap(), Nivel::Dono);
    }
}
