use uorm::mapper_assets;
use uorm::mapper_loader;

// Ensure this runs at startup
mapper_assets!["tests/resources/**/*.xml"];

#[test]
fn test_macro_assets() {
    // Note: the path is relative to CARGO_MANIFEST_DIR which is the crate root

    // Check if the mapper is already loaded via the top-level macro call
    let stmt = mapper_loader::find_statement("test_ns.selectUser", "mysql");
    let sql = stmt
        .as_ref()
        .expect("Assets were not loaded automatically. The ctor-based registration failed.");
    assert_eq!(sql.r#type, mapper_loader::StatementType::Select);
    assert!(
        sql.content
            .as_ref()
            .unwrap()
            .contains("SELECT * FROM users")
    );
}
