# Fix the percentage base
# 28/08 14:29

p='crates/phxsql-server/src/sistema.rs'
s=open(p).read()

a='''#[derive(Debug, Clone)]
pub struct EspacoEmDisco {
    pub caminho: String,
    pub dispositivo: String,
    pub montagem: String,
    pub total_kb: u64,
    pub usado_kb: u64,
    pub livre_kb: u64,
}

impl EspacoEmDisco {
    /// Quanto do disco ja foi, de 0 a 100.
    pub fn usado_percentual(&self) -> f64 {
        if self.total_kb == 0 {
            return 0.0;
        }
        (self.usado_kb as f64 / self.total_kb as f64) * 100.0
    }

    pub fn livre_percentual(&self) -> f64 {
        100.0 - self.usado_percentual()
    }
'''
b='''#[derive(Debug, Clone)]
pub struct EspacoEmDisco {
    pub caminho: String,
    pub dispositivo: String,
    pub montagem: String,
    /// Tamanho do sistema de arquivos, como o `df` reporta.
    pub total_kb: u64,
    pub usado_kb: u64,
    pub livre_kb: u64,
}

impl EspacoEmDisco {
    /// Quanto deste sistema de arquivos ESTE processo alcanca.
    ///
    /// Nao e o `total_kb`, e a diferenca nao e detalhe. O `ext4` reserva 5%
    /// para o root, cota e contêiner reservam mais, e nesses casos
    /// `usado + livre` fica bem abaixo do tamanho do disco. A maquina onde
    /// isto foi medido:
    ///
    /// ```text
    /// Filesystem  1K-blocks      Used  Available  Use%
    /// /dev/vda    264212084  20986728   17861796   55%
    /// ```
    ///
    /// Dividir por `total_kb` daria 8% usados; o disco esta em 55%. O primeiro
    /// numero era o que este modulo mostrava, e um alerta de "menos de 10%
    /// livre" nunca dispararia -- o disco encheria calado. O `df` divide por
    /// `usado + livre`, e e essa a conta certa.
    pub fn utilizavel_kb(&self) -> u64 {
        self.usado_kb + self.livre_kb
    }

    /// Quanto do disco ja foi, de 0 a 100.
    pub fn usado_percentual(&self) -> f64 {
        let base = self.utilizavel_kb();
        if base == 0 {
            return 0.0;
        }
        (self.usado_kb as f64 / base as f64) * 100.0
    }

    pub fn livre_percentual(&self) -> f64 {
        100.0 - self.usado_percentual()
    }

    /// Quanto o `df` esconde: reserva do sistema de arquivos e cota.
    ///
    /// Mostrar isto e o que evita a pergunta "cade os outros 200 GB?" quando o
    /// total nao bate com a soma.
    pub fn reservado_kb(&self) -> u64 {
        self.total_kb.saturating_sub(self.utilizavel_kb())
    }
'''
assert a in s; s=s.replace(a,b,1)

a='''            ("total_kb", Json::de_u64(self.total_kb)),
            ("usado_kb", Json::de_u64(self.usado_kb)),
            ("livre_kb", Json::de_u64(self.livre_kb)),'''
b='''            ("total_kb", Json::de_u64(self.total_kb)),
            ("utilizavel_kb", Json::de_u64(self.utilizavel_kb())),
            ("reservado_kb", Json::de_u64(self.reservado_kb())),
            ("usado_kb", Json::de_u64(self.usado_kb)),
            ("livre_kb", Json::de_u64(self.livre_kb)),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
