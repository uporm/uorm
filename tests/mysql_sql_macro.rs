#[cfg(feature = "mysql")]
use uorm::Param;
#[cfg(feature = "mysql")]
use uorm::Result;
#[cfg(feature = "mysql")]
use uorm::driver_manager::U;
#[cfg(feature = "mysql")]
use uorm::sql;
#[cfg(feature = "mysql")]
use uorm::udbc::mysql::pool::MysqlDriver;

#[cfg(feature = "mysql")]
#[sql("user")]
struct ChunkDao;

#[cfg(feature = "mysql")]
#[derive(Param)]
struct StateStats {
    total: i32,
    completed_count: i32,
    failed_count: i32,
}

#[cfg(feature = "mysql")]
impl ChunkDao {
    #[sql("create_table")]
    async fn create_table() -> Result<u64> {
        exec!()
    }

    #[sql("insert_chunk")]
    async fn insert_chunk(state: i32) -> Result<u64> {
        exec!()
    }

    #[sql("state_stats")]
    async fn state_stats() -> Result<StateStats> {
        exec!()
    }
}

#[cfg(feature = "mysql")]
#[tokio::test]
async fn test_mysql_sql_macro() -> Result<()> {
    let url = std::env::var("DATABASE_URL").unwrap_or_default();
    if url.is_empty() {
        return Ok(());
    }

    uorm::mapper_loader::clear();
    U.assets("tests/resources/**/*.xml")?;

    let driver = MysqlDriver::new(url).build()?;
    U.register(driver)?;

    ChunkDao::create_table().await?;
    ChunkDao::insert_chunk(3).await?;
    ChunkDao::insert_chunk(3).await?;
    ChunkDao::insert_chunk(4).await?;
    ChunkDao::insert_chunk(1).await?;
    let stats = ChunkDao::state_stats().await?;

    assert_eq!(stats.total, 4);
    assert_eq!(stats.completed_count, 2);
    assert_eq!(stats.failed_count, 1);
    Ok(())
}
