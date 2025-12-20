use uorm::mapper_assets;
use uorm::mapper_loader;

// Ensure this runs at startup
mapper_assets!["tests/resources/**/*.xml"];

#[test]
fn test_macro_assets() {
    // Note: the path is relative to CARGO_MANIFEST_DIR which is the crate root

    // Check if the mapper is already loaded via the top-level macro call
    let mapper = mapper_loader::find_mapper("test_ns.selectUser", "mysql");
    let sql = mapper
        .as_ref()
        .expect("Assets were not loaded automatically. The ctor-based registration failed.");
    assert!(
        sql.content
            .as_ref()
            .unwrap()
            .contains("SELECT * FROM users")
    );
}
