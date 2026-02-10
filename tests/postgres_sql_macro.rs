use uorm::Param;
use uorm::Result;
use uorm::driver_manager::U;
use uorm::sql;
#[cfg(feature = "postgres")]
use uorm::udbc::postgres::pool::PostgresDriver;

#[sql("pg_user")]
struct PgDao;

#[derive(Param)]
struct Stats {
    total: i32,
    completed_count: i32,
    failed_count: i32,
}

impl PgDao {
    #[sql("create_table")]
    async fn create_table() -> Result<u64> {
        exec!()
    }

    #[sql("clear_table")]
    async fn clear_table() -> Result<u64> {
        exec!()
    }

    #[sql("insert_chunk")]
    async fn insert_chunk(state: i32) -> Result<u64> {
        exec!()
    }

    #[sql("state_stats")]
    async fn state_stats() -> Result<Stats> {
        exec!()
    }
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn test_postgres_sql_macro() -> Result<()> {
    let url = std::env::var("DATABASE_URL").unwrap_or_default();
    if url.is_empty() {
        return Ok(());
    }

    println!("url: {}", url);

    uorm::mapper_loader::clear();
    U.assets("tests/resources/**/*.xml")?;

    let driver = PostgresDriver::new(url).build()?;
    U.register(driver)?;

    PgDao::create_table().await?;
    PgDao::clear_table().await?;
    PgDao::insert_chunk(3).await?;
    PgDao::insert_chunk(3).await?;
    PgDao::insert_chunk(4).await?;
    PgDao::insert_chunk(1).await?;
    let stats = PgDao::state_stats().await?;

    assert_eq!(stats.total, 4);
    assert_eq!(stats.completed_count, 2);
    assert_eq!(stats.failed_count, 1);
    Ok(())
}
