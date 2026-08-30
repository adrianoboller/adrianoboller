# Add Nivel enum to the user model
# 27/08 21:09

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()

nivel = '''
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
    /// So le. E o padrao quando nao se diz nada: nega por omissao.
    #[default]
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
            "" | "leitor" | "consulta" | "leitura" => Nivel::Leitor,
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
            Nivel::Leitor => "leitor",
            Nivel::Operador => "operador",
            Nivel::Dono => "dono",
            Nivel::Admin => "admin",
        }
    }

    /// O que este nivel pode, numa base.
    pub fn permissoes(self) -> Permissoes {
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

'''
s=s.replace('/// As dez permissoes de uma base. Tudo comeca em `false`.', nivel + '/// As dez permissoes de uma base. Tudo comeca em `false`.')

# campo no Usuario
s=s.replace('''    /// Chave publica Ed25519, se este usuario tambem prova posse de chave.''',
'''    /// Nivel: o poder que este usuario tem nas bases onde nao ha regra
    /// explicita. `bases` continua mandando, quando existe.
    pub nivel: Nivel,
    /// Chave publica Ed25519, se este usuario tambem prova posse de chave.''')
open(p,'w').write(s)
