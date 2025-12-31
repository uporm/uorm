use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use uorm::Result;
use uorm::udbc::connection::Connection;
use uorm::udbc::driver::Driver;
use uorm::udbc::value::Value;
use uorm::executor::mapper::Mapper;
use uorm::mapper_loader;

// --- 1. 定义 Mock Driver (模拟数据库行为) ---
struct MockDriver;
#[async_trait]
impl Driver for MockDriver {
    fn name(&self) -> &str { "mock" }
    fn r#type(&self) -> &str { "mock" }
    fn placeholder(&self, _: usize, _: &str) -> String { "?".to_string() }
    async fn acquire(&self) -> Result<Box<dyn Connection>> {
        Ok(Box::new(MockConnection))
    }
    async fn close(&self) -> Result<()> { Ok(()) }
}

struct MockConnection;
#[async_trait]
impl Connection for MockConnection {
    async fn query(&mut self, sql: &str, _args: &[(String, Value)]) -> Result<Vec<HashMap<String, Value>>> {
        // 根据 SQL 模拟不同的返回值
        if sql.contains("count") {
            // 模拟返回 count(*) = 100
            let mut row = HashMap::new();
            row.insert("count".to_string(), Value::I64(100));
            return Ok(vec![row]);
        } else if sql.contains("max(name)") {
            // 模拟返回 max(name) = "Alice"
            let mut row = HashMap::new();
            row.insert("name".to_string(), Value::Str("Alice".to_string()));
            return Ok(vec![row]);
        } else if sql.contains("empty") {
            // 模拟返回空结果
            return Ok(vec![]);
        } else if sql.contains("null_val") {
            let mut row = HashMap::new();
            row.insert("val".to_string(), Value::Null);
            return Ok(vec![row]);
        }
        
        Ok(vec![])
    }
    async fn execute(&mut self, _sql: &str, _args: &[(String, Value)]) -> Result<u64> { Ok(0) }
    async fn last_insert_id(&mut self) -> Result<u64> { Ok(0) }
    async fn begin(&mut self) -> Result<()> { Ok(()) }
    async fn commit(&mut self) -> Result<()> { Ok(()) }
    async fn rollback(&mut self) -> Result<()> { Ok(()) }
}

// --- 2. 测试用例演示 ---
#[tokio::test]
async fn example_scalar_return() -> Result<()> {
    // 准备 XML Mapper
    let xml = r#"
    <mapper namespace="example">
        <!-- 返回单个整数 -->
        <select id="getCount">
            SELECT count(*) as count FROM users
        </select>
        
        <!-- 返回单个字符串 -->
        <select id="getMaxName">
            SELECT max(name) as name FROM users
        </select>

        <!-- 返回可能为空的值 -->
        <select id="getEmpty">
            SELECT id FROM users WHERE 1=2
        </select>

        <!-- 返回 Null 的 Scalar -->
        <select id="getNullScalar">
            SELECT null_val FROM users
        </select>
    </mapper>
    "#;
    
    // 加载 Mapper
    mapper_loader::load_assets(vec![("example.xml", xml)])?;

    // 初始化 Mapper
    let driver = Arc::new(MockDriver);
    let mapper = Mapper::new(driver);

    // --- 演示用法 1: 返回基本类型 (i64) ---
    // 对应 SQL: SELECT count(*) ...
    let count: i64 = mapper.execute("example.getCount", &()).await?;
    println!("Count: {}", count);
    assert_eq!(count, 100);

    // --- 演示用法 2: 返回 String ---
    // 对应 SQL: SELECT max(name) ...
    let name: String = mapper.execute("example.getMaxName", &()).await?;
    println!("Max Name: {}", name);
    assert_eq!(name, "Alice");

    // --- 演示用法 3: 返回 Option<T> (处理可能为空的情况) ---
    // 对应 SQL: SELECT id ... (无结果)
    let empty_res: Option<i64> = mapper.execute("example.getEmpty", &()).await?;
    println!("Empty Result: {:?}", empty_res);
    assert_eq!(empty_res, None);

    // --- 演示用法 4: 错误用法 (i32 接收 Null) ---
    // 对应 SQL: SELECT max(name) ... -> Null (假设没有数据或者 name 为 Null)
    // 这里我们要模拟一个返回 Null 的情况给 scalar (i32)
    // 我们的 MockConnection 对于 max_name 返回了 "Alice" (String)。
    // 让我们加一个返回 Null 的 case。
    let null_res_result: Result<i32> = mapper.execute("example.getNullScalar", &()).await;
    match null_res_result {
        Ok(_) => panic!("Should fail"),
        Err(e) => {
            let msg = e.to_string();
            // 验证不再是 "got List" 错误，而是明确的 "got Null" 错误
            assert!(msg.contains("Null"), "Should mention Null in error");
            assert!(!msg.contains("List"), "Should NOT mention List in error");
        }
    }

    // --- 演示用法 4: 结合宏 (伪代码说明) ---
    /*
    如果你定义了如下宏:
    #[sql("example.getCount")]
    pub async fn get_user_count() -> uorm::Result<i64> {
        exec!()
    }
    
    调用 get_user_count().await? 将直接返回 i64 类型的 100
    */

    Ok(())
}
