use uorm::Param;
use uorm::Result;
use uorm::driver_manager::U;
use uorm::sql;
#[cfg(feature = "postgres")]
use uorm::Vector;
#[cfg(feature = "postgres")]
use uorm::udbc::postgres::pool::PostgresDriver;

#[sql("pg_user")]
struct PgVectorDao;

#[cfg(feature = "postgres")]
#[derive(Param)]
struct VectorInsert {
    doc_id: i64,
    embedding: Vector,
}

#[cfg(feature = "postgres")]
#[derive(Param)]
struct VectorQuery {
    embedding: Vector,
}

#[cfg(feature = "postgres")]
impl PgVectorDao {
    #[sql("create_vector_table")]
    async fn create_vector_table() -> Result<u64> {
        exec!()
    }

    #[sql("clear_vector_table")]
    async fn clear_vector_table() -> Result<u64> {
        exec!()
    }

    #[sql("insert_vector")]
    async fn insert_vector(row: VectorInsert) -> Result<u64> {
        exec!()
    }

    #[sql("select_vector_nearest")]
    async fn select_vector_nearest(query: VectorQuery) -> Result<i64> {
        exec!()
    }
}

#[cfg(feature = "postgres")]
fn build_vector(seed: f32, dim: usize) -> Vector {
    let mut values = Vec::with_capacity(dim);
    for i in 0..dim {
        values.push(seed + (i as f32) * 0.001);
    }
    Vector::from(values)
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn test_postgres_vector_insert_query() -> Result<()> {
    let url = std::env::var("DATABASE_URL").unwrap_or_default();
    if url.is_empty() {
        return Ok(());
    }

    uorm::mapper_loader::clear();
    U.assets("tests/resources/**/*.xml")?;

    let driver = PostgresDriver::new(url).build()?;
    U.register(driver)?;

    PgVectorDao::create_vector_table().await?;
    PgVectorDao::clear_vector_table().await?;

    let v1 = build_vector(0.01, 1024);
    let v2 = build_vector(10.01, 1024);

    PgVectorDao::insert_vector(VectorInsert {
        doc_id: 1,
        embedding: v1.clone(),
    })
    .await?;
    PgVectorDao::insert_vector(VectorInsert {
        doc_id: 2,
        embedding: v2.clone(),
    })
    .await?;

    let nearest1 = PgVectorDao::select_vector_nearest(VectorQuery { embedding: v1 }).await?;
    let nearest2 = PgVectorDao::select_vector_nearest(VectorQuery { embedding: v2 }).await?;

    assert_eq!(nearest1, 1);
    assert_eq!(nearest2, 2);
    Ok(())
}
