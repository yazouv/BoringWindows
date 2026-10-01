fn main() {
    // Style « fluent » des widgets standard : suit le thème clair/sombre de Windows.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("compilation de l'interface Slint");
}
