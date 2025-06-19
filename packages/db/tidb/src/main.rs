use rand::Rng;
use sqlx::{Row, mysql::MySqlPoolOptions};
use std::env;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok(); // load .env (optional)
    let db_url = env::var("DATABASE_URL").expect("set DATABASE_URL=mysql://...");

    // 1) open an async pool (5 connections is fine for a demo)
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // 2) create the two tables (id is AUTO_INCREMENT so MySQL client returns last_insert_id)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agents (
            id   INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(100) NOT NULL,
            region VARCHAR(50)
        );"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_events (
            id        INT PRIMARY KEY AUTO_INCREMENT,
            agent_id  INT NOT NULL,
            event_type VARCHAR(40),
            FOREIGN KEY (agent_id) REFERENCES agents(id)
        );"#,
    )
    .execute(&pool)
    .await?;

    // 3) insert 10 agents
    let agents = [
        ("Oliver", "North"),
        ("Emma", "South"),
        ("Liam", "East"),
        ("Sophia", "West"),
        ("Noah", "North"),
        ("Isabella", "South"),
        ("Ethan", "East"),
        ("Mia", "West"),
        ("Lucas", "North"),
        ("Charlotte", "South"),
    ];
    // for (name, region) in agents {
    //     sqlx::query("INSERT INTO agents (name, region) VALUES (?, ?)")
    //         .bind(name)
    //         .bind(region)
    //         .execute(&pool)
    //         .await?;
    // }

    // 4) bulk-insert 1000 random events
    let mut rng = rand::thread_rng();
    let event_types = ["login", "logout", "purchase", "update"];

    let mut tx = pool.begin().await?;
    let time_start = Instant::now();
    for _ in 0..1_000 {
        let agent_id = rng.gen_range(1..=30);
        let ev_kind = event_types[rng.gen_range(0..event_types.len())];

        sqlx::query("INSERT INTO agent_events (agent_id, event_type) VALUES (?, ?)")
            .bind(agent_id)
            .bind(ev_kind)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let time_end = Instant::now();
    println!("=== insert events ===");
    println!(
        "inserted 1000 events in {:?}",
        time_end.duration_since(time_start)
    );

    // 5) count events per agent
    let time_start = Instant::now();
    let rows = sqlx::query(
        r#"
        SELECT a.name, COUNT(e.id) AS cnt
        FROM agents a
        LEFT JOIN agent_events e ON a.id = e.agent_id
        GROUP BY a.id, a.name
        ORDER BY a.id;
        "#,
    )
    .fetch_all(&pool)
    .await?;
    let time_end = Instant::now();
    println!("=== event totals ===");
    println!("fetched gents in {:?}", time_end.duration_since(time_start));

    let time_start = Instant::now();
    for row in rows {
        let time_start = Instant::now();
        let name: String = row.try_get("name")?;
        let time_start2 = Instant::now();
        let cnt: i64 = row.try_get("cnt")?;
        let time_end = Instant::now();
        println!(
            "{:<5} -> {} in {:?} {:?}",
            name,
            cnt,
            time_end.duration_since(time_start),
            time_end.duration_since(time_start2)
        );
    }

    Ok(())
}
