fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(slint_widgets::library_paths())
        .with_style("fluent".into())
        .with_debug_info(true);
    slint_build::compile_with_config("ui/tests.slint", config).unwrap();
}
