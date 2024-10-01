use axum::routing::Router;
use chrono::NaiveDateTime;
use rstest::fixture;
use sea_orm::sea_query::TableCreateStatement;
use sea_orm::Database;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, Schema};
use soil_api_rust::soil::types::db::Entity;
use soil_api_rust::soil::types::views::router;
use uuid::Uuid;

pub async fn setup_database() -> DatabaseConnection {
    // Use an in-memory SQLite database for testing.
    let db: DatabaseConnection = Database::connect("sqlite::memory:").await.unwrap();

    // // Drop the table if it exists.
    // db.execute(sea_orm::Statement::from_string(
    //     db.get_database_backend(),
    //     "DROP TABLE IF EXISTS soiltype;",
    // ))
    // .await
    // .unwrap();

    // Create the table schema.
    let schema = Schema::new(db.get_database_backend());
    let stmt: TableCreateStatement = schema.create_table_from_entity(Entity).to_owned();

    db.execute(db.get_database_backend().build(&stmt))
        .await
        .unwrap();

    db
}

pub async fn insert_mock_data(db: &DatabaseConnection) {
    let now = NaiveDateTime::from_timestamp_opt(chrono::Utc::now().timestamp(), 0).unwrap();

    let soil_type_1 = soil_api_rust::soil::types::db::ActiveModel {
        id: sea_orm::ActiveValue::Set(Uuid::new_v4()),
        iterator: sea_orm::ActiveValue::Set(1),
        last_updated: sea_orm::ActiveValue::Set(now),
        name: sea_orm::ActiveValue::Set("Clay".to_string()),
        description: sea_orm::ActiveValue::Set("Clay soil type".to_string()),
        image: sea_orm::ActiveValue::Set(Some("clay.png".to_string())),
    };
    // ...
    soil_type_1.insert(db).await.unwrap();

    let soil_type_2 = soil_api_rust::soil::types::db::ActiveModel {
        id: sea_orm::ActiveValue::Set(Uuid::new_v4()),
        iterator: sea_orm::ActiveValue::Set(2),
        last_updated: sea_orm::ActiveValue::Set(now),
        name: sea_orm::ActiveValue::Set("Sand".to_string()),
        description: sea_orm::ActiveValue::Set("Sandy soil type".to_string()),
        image: sea_orm::ActiveValue::Set(None),
    };
    soil_type_2.insert(db).await.unwrap();
}

#[fixture]
pub async fn mock_api() -> Router {
    // Use a unique in-memory SQLite database per test run to avoid shared state.
    let db = setup_database().await;
    insert_mock_data(&db).await;

    // Step 4: Set up the router.
    //  let app = router(db);
    router(db)

    // let db = setup_database().await;
    // insert_mock_data(&db).await;
    // db
}
