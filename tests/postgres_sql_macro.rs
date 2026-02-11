use uorm::Param;
use uorm::Result;
use uorm::driver_manager::U;
use uorm::sql;
use uorm::transaction;
#[cfg(feature = "postgres")]
use uorm::udbc::postgres::pool::PostgresDriver;
use std::time::Instant;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

#[sql("pg_user")]
struct PgDao;

#[derive(Param)]
struct Stats {
    total: i32,
    completed_count: i32,
    failed_count: i32,
}

#[derive(Param)]
struct ChunkInsert {
    state: i32,
    tiny_i8: i8,
    small_i16: i16,
    big_i64: i64,
    u8_val: u8,
    u16_val: u16,
    u32_val: u32,
    u64_val: u64,
    nullable_int: Option<i32>,
    f32_val: f32,
    f64_val: f64,
    flag: bool,
    letter: char,
    name: String,
    varchar_val: String,
    score: f64,
    enabled: bool,
    dec_val: String,
    payload: String,
    event_date: String,
    event_time: String,
    event_ts: String,
    json_val: String,
    jsonb_val: String,
    embedding: String,
}

#[derive(Param)]
struct BatchInsert {
    items: Vec<ChunkInsert>,
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
    async fn insert_chunk(chunk: ChunkInsert) -> Result<u64> {
        exec!()
    }

    #[sql("insert_chunk_batch")]
    async fn insert_chunk_batch(batch: BatchInsert) -> Result<u64> {
        exec!()
    }

    #[sql("state_stats")]
    async fn state_stats() -> Result<Stats> {
        exec!()
    }

    #[sql("select_nullable_varchar")]
    async fn select_nullable_varchar(val: Option<i32>) -> Result<Option<String>> {
        exec!()
    }

    #[sql("count_nullable_int_null")]
    async fn count_nullable_int_null() -> Result<i64> {
        exec!()
    }
}

#[transaction]
async fn insert_batches_with_tx(batches: Vec<BatchInsert>) -> Result<u64> {
    let mut total = 0;
    for batch in batches {
        total += PgDao::insert_chunk_batch(batch).await?;
    }
    Ok(total)
}

fn build_chunk(seed: i32) -> ChunkInsert {
    let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let time = NaiveTime::from_hms_opt(12, 30, 45).unwrap();
    let payload = [seed as u8, (seed + 1) as u8]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let json_val = format!(r#"{{"key": "json_{}", "value": {}}}"#, seed, seed);
    let jsonb_val = format!(r#"{{"key": "jsonb_{}", "value": {}}}"#, seed, seed);
    let embedding = format!(
        "[{:.3},{:.3},{:.3}]",
        seed as f64 + 0.1,
        seed as f64 + 0.2,
        seed as f64 + 0.3
    );
    ChunkInsert {
        state: seed,
        tiny_i8: (seed % 120) as i8,
        small_i16: (seed * 10) as i16,
        big_i64: seed as i64 * 1000,
        u8_val: (seed as u8).wrapping_add(10),
        u16_val: (seed as u16).wrapping_add(100),
        u32_val: seed as u32 * 1000,
        u64_val: seed as u64 * 10000,
        nullable_int: if seed % 2 == 0 { Some(seed) } else { None },
        f32_val: seed as f32 + 0.25,
        f64_val: seed as f64 + 0.75,
        flag: seed % 2 == 0,
        letter: char::from_u32(65 + (seed as u32 % 26)).unwrap(),
        name: format!("name_{}", seed),
        varchar_val: format!("varchar_{}", seed),
        score: seed as f64 + 0.5,
        enabled: seed % 2 == 0,
        dec_val: format!("{:.2}", (seed as f64 * 1.23)),
        payload,
        event_date: date.to_string(),
        event_time: time.to_string(),
        event_ts: NaiveDateTime::new(date, time).to_string(),
        json_val,
        jsonb_val,
        embedding,
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
    PgDao::insert_chunk(build_chunk(3)).await?;
    PgDao::insert_chunk(build_chunk(3)).await?;
    PgDao::insert_chunk(build_chunk(4)).await?;
    PgDao::insert_chunk(build_chunk(1)).await?;
    let stats = PgDao::state_stats().await?;
    let none_varchar = PgDao::select_nullable_varchar(None).await?;
    let some_varchar = PgDao::select_nullable_varchar(Some(123)).await?;
    let null_count = PgDao::count_nullable_int_null().await?;

    assert_eq!(stats.total, 4);
    assert_eq!(stats.completed_count, 2);
    assert_eq!(stats.failed_count, 1);
    assert_eq!(none_varchar, None);
    assert_eq!(some_varchar, Some("123".to_string()));
    assert!(null_count >= 1);
    Ok(())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn test_postgres_batch_insert_with_tx_perf() -> Result<()> {
    let url = std::env::var("DATABASE_URL").unwrap_or_default();
    if url.is_empty() {
        return Ok(());
    }

    uorm::mapper_loader::clear();
    U.assets("tests/resources/**/*.xml")?;

    let driver = PostgresDriver::new(url).build()?;
    U.register(driver)?;

    PgDao::create_table().await?;
    PgDao::clear_table().await?;

    let total = 2000000usize;
    let batch_size = 200usize;
    let mut batches = Vec::new();
    let mut current = Vec::with_capacity(batch_size);

    for i in 0..total {
        current.push(build_chunk((i % 5) as i32));
        if current.len() == batch_size {
            batches.push(BatchInsert { items: current });
            current = Vec::with_capacity(batch_size);
        }
    }
    if !current.is_empty() {
        batches.push(BatchInsert { items: current });
    }

    let start = Instant::now();
    let affected = insert_batches_with_tx(batches).await?;
    let elapsed = start.elapsed();

    println!(
        "postgres batch insert with tx: total={}, affected={}, elapsed_ms={}",
        total,
        affected,
        elapsed.as_millis()
    );

    let stats = PgDao::state_stats().await?;
    assert_eq!(stats.total as usize, total);
    Ok(())
}
