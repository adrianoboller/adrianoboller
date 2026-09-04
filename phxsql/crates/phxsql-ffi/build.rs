// build.rs -- Script de compilacao que linka o wrapper JNI C com a cdylib Rust

use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let profile = env::var("PROFILE").unwrap_or_default();

    // Recado de build script vai por stderr: o STDOUT e o canal de
    // DIRETIVAS do cargo (`cargo:rustc-link-arg-cdylib=...` abaixo), e
    // misturar recado com protocolo e o mesmo erro do `erro.redireciona`.
    eprintln!("[build.rs] TARGET={target}, PROFILE={profile}");

    // Se compilando para Android ARM64, compila o wrapper JNI
    if target == "aarch64-linux-android" {
        eprintln!("[build.rs] Compilando wrapper JNI para Android ARM64");

        // Obtem o NDK e o path dos headers JNI
        let ndk_root = PathBuf::from("/opt/android-ndk-r27c");
        let jni_include =
            ndk_root.join("toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include");
        let jni_h =
            ndk_root.join("toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/include/jni.h");

        // Verifica se JNI headers existem
        if !jni_h.exists() {
            eprintln!("[build.rs] ERRO: jni.h nao encontrado em {:?}", jni_h);
            return;
        }

        eprintln!("[build.rs] JNI headers encontrados");

        // Path do wrapper JNI C
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let jni_c = PathBuf::from(&manifest_dir).join("jni/phxsql_jni.c");
        let include = PathBuf::from(&manifest_dir).join("include");

        eprintln!("[build.rs] Compilando: {}", jni_c.display());

        // Compila o wrapper C para um objeto
        let cc_path = "/opt/android-ndk-r27c/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android28-clang";
        let out_dir = env::var("OUT_DIR").unwrap();
        let obj_path = PathBuf::from(&out_dir).join("phxsql_jni.o");

        let output = std::process::Command::new(cc_path)
            .arg("-c")
            .arg("-fPIC")
            .arg(format!("-I{}", jni_include.display()))
            .arg(format!("-I{}", include.display()))
            .arg(&jni_c)
            .arg("-o")
            .arg(&obj_path)
            .output();

        match output {
            Ok(result) if result.status.success() => {
                eprintln!("[build.rs] JNI wrapper compilado: {}", obj_path.display());
                // Linked object fica em OUT_DIR/phxsql_jni.o
                // Passa objeto direto ao linker via -Wl
                println!("cargo:rustc-link-arg-cdylib={}", obj_path.display());
                println!("cargo:rerun-if-changed={}", jni_c.display());
            }
            Ok(result) => {
                eprintln!("[build.rs] ERRO ao compilar wrapper JNI:");
                eprintln!("{}", String::from_utf8_lossy(&result.stderr));
            }
            Err(e) => {
                eprintln!("[build.rs] ERRO ao executar clang: {}", e);
            }
        }
    }
}
