use uorm::mapper_loader;

#[test]
fn test_multi_db_selection() {
    // Load the specific file
    mapper_loader::load("tests/resources/mapper/multi_db.xml").expect("Failed to load mapper");

    // Test Default
    // Pass a db_type that doesn't match any specific one, should fallback to the one without databaseType
    let stmt = mapper_loader::find_statement("multi_db.get_date", "sqlite")
        .expect("Should find default mapper");
    assert_eq!(stmt.r#type, mapper_loader::StatementType::Select);
    assert!(stmt.content.as_ref().unwrap().trim().contains("default"));

    // Test MySQL
    let stmt =
        mapper_loader::find_statement("multi_db.get_date", "mysql").expect("Should find mysql mapper");
    assert_eq!(stmt.r#type, mapper_loader::StatementType::Select);
    assert!(stmt.content.as_ref().unwrap().trim().contains("mysql"));

    // Test Postgres
    let stmt = mapper_loader::find_statement("multi_db.get_date", "postgres")
        .expect("Should find postgres mapper");
    assert_eq!(stmt.r#type, mapper_loader::StatementType::Select);
    assert!(stmt.content.as_ref().unwrap().trim().contains("postgres"));
}
