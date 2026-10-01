fn main() {
    // Style « fluent » des widgets standard : suit le thème clair/sombre de Windows.
    // Textes `@tr` en anglais, traductions (`lang/<langue>/LC_MESSAGES/bw-app.po`)
    // embarquées dans le binaire, sans contexte par composant.
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("compilation de l'interface Slint");
    println!("cargo:rerun-if-changed=lang");
}
