# Add server config module
# 27/08 18:40

# Centraliza a formatacao de instante em ms no core, para o .log e o log de acessos usarem.
p='crates/phxsql-core/src/datahora.rs'
s=open(p).read()
s=s.replace('''#[cfg(test)]
mod tests {''','''/// Formata um instante em milissegundos desde a epoca como
/// `AAAA-MM-DD HH:MM:SS,mmm`.
pub fn instante_iso(milissegundos: i64) -> String {
    let dias = milissegundos.div_euclid(86_400_000) as i32;
    let resto = milissegundos.rem_euclid(86_400_000);
    let (h, m, s, ms) = (
        resto / 3_600_000,
        (resto / 60_000) % 60,
        (resto / 1_000) % 60,
        resto % 1_000,
    );
    format!("{} {h:02}:{m:02}:{s:02},{ms:03}", data_iso(dias))
}

#[cfg(test)]
mod tests {''')
s=s.replace('''    #[test]
    fn horas() {''','''    #[test]
    fn instante_completo() {
        // 2024-10-04 13:45:30,250
        let ms = 20_000i64 * 86_400_000 + 13 * 3_600_000 + 45 * 60_000 + 30 * 1_000 + 250;
        assert_eq!(instante_iso(ms), "2024-10-04 13:45:30,250");
        assert_eq!(instante_iso(0), "1970-01-01 00:00:00,000");
    }

    #[test]
    fn horas() {''')
open(p,'w').write(s)

p='crates/phxsql-store/src/log.rs'
s=open(p).read()
s=s.replace('''    /// Data e hora do evento em ISO (`AAAA-MM-DD HH:MM:SS,mmm`).
    pub fn instante_iso(&self) -> String {
        let dias = self.carimbo.div_euclid(86_400_000) as i32;
        let resto = self.carimbo.rem_euclid(86_400_000);
        let (h, m, s, ms) = (
            resto / 3_600_000,
            (resto / 60_000) % 60,
            (resto / 1_000) % 60,
            resto % 1_000,
        );
        format!(
            "{} {h:02}:{m:02}:{s:02},{ms:03}",
            phxsql_core::datahora::data_iso(dias)
        )
    }''','''    /// Data e hora do evento em ISO (`AAAA-MM-DD HH:MM:SS,mmm`).
    pub fn instante_iso(&self) -> String {
        phxsql_core::datahora::instante_iso(self.carimbo)
    }''')
open(p,'w').write(s)
