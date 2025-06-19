mod entity;

use entity::events::Entity as Events;
use entity::list::Entity as List;

use rand::Rng;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, QuerySelect, Set};
use std::env;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // 1) Connect.  The DSN needs ?ssl-mode=VERIFY_IDENTITY for TiDB Cloud.
    let url = env::var("DATABASE_URL").expect("set DATABASE_URL");
    let db = Database::connect(&url).await?;

    // 2) Create the tables with raw SQL (simplest for a demo).
    // Drop tables first to ensure schema changes take effect
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS events".to_string(),
    ))
    .await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        "DROP TABLE IF EXISTS list".to_string(),
    ))
    .await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE list (
            id      INT AUTO_INCREMENT PRIMARY KEY,
            name    VARCHAR(100) NOT NULL,
            region  VARCHAR(50)  NOT NULL
        )"#
        .to_string(),
    ))
    .await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE events (
            id         INT AUTO_INCREMENT PRIMARY KEY,
            agent_id   INT  NOT NULL,
            event_type VARCHAR(40),
            list_id    INT  NOT NULL,
            FOREIGN KEY (list_id) REFERENCES list(id)
        )"#
        .to_string(),
    ))
    .await?;

    // 3) Seed 10 agents — only if table is empty (id = 1 absent).
    if List::find_by_id(1).one(&db).await?.is_none() {
        let names_regions = [
            ("Alice", "North"),
            ("Bob", "South"),
            ("Carol", "East"),
            ("Dave", "West"),
            ("Eve", "North"),
            ("Frank", "South"),
            ("Grace", "East"),
            ("Heidi", "West"),
            ("Ivan", "North"),
            ("Judy", "South"),
        ];
        for (name, region) in names_regions {
            entity::list::ActiveModel {
                name: Set(name.to_string()),
                region: Set(region.to_string()),
                ..Default::default()
            }
            .insert(&db)
            .await?;
        }
    }

    // 4) Insert 1 000 random events in a single transaction.
    let mut rng = rand::thread_rng();
    let event_types = ["login", "logout", "purchase", "update"];
    let mut to_insert = Vec::with_capacity(1_000);

    let start = Instant::now();
    for _ in 0..1_000 {
        let agent_id = rng.gen_range(1..=10);
        let list_id = rng.gen_range(1..=10);
        let ev_kind = event_types[rng.gen_range(0..event_types.len())];

        to_insert.push(entity::events::ActiveModel {
            agent_id: Set(agent_id),
            event_type: Set(ev_kind.to_string()),
            list_id: Set(list_id),
            ..Default::default()
        });
    }
    Events::insert_many(to_insert).exec(&db).await?;
    let duration = start.elapsed();
    println!("Inserted 1000 events in {:?}", duration);

    // 5) Aggregate counts per agent with SeaORM’s query builder.
    let start = Instant::now();
    let results = List::find()
        .left_join(Events)
        .select_only()
        .column(entity::list::Column::Name)
        .expr_as(
            sea_orm::sea_query::Expr::col((Events, entity::events::Column::Id)).count(),
            "cnt",
        )
        .group_by(entity::list::Column::Id)
        .into_tuple::<(String, i64)>()
        .all(&db)
        .await?;
    let duration = start.elapsed();
    println!("Aggregated events in {:?}", duration);

    println!("=== event totals ===");
    for (name, cnt) in results {
        println!("{:<5} -> {}", name, cnt);
    }

    Ok(())
}
