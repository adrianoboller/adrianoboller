# Separate test modules and rebuild
# 29/08 17:22

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
# Os dois modulos de teste viraram um so cabecalho colado. Cada um volta a ser
# um modulo, com a funcao auxiliar que estava truncada de volta no seu dono.
velho = '''#[cfg(test)]
mod testes_firewall_e_mensagens {
    use super::*;

    fn dir_temp(rotulo: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-fw-{}-{rotulo}", std::process::id()));
mod testes_papel {
    use super::*;
'''
novo = '''#[cfg(test)]
mod testes_firewall_e_mensagens {
    use super::*;

    fn dir_temp(rotulo: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-fw-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

#[cfg(test)]
mod testes_papel {
    use super::*;
'''
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("modulos de teste separados")
