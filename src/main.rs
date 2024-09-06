use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber;
mod config;
mod plots; // Import the plots module

#[tokio::main]
async fn main() {
    // Set up tracing/logging
    tracing_subscriber::fmt::init();

    // Load configuration
    let cfg = config::Config::from_env();

    // Create a PostgreSQL connection pool
    let pool = PgPoolOptions::new()
        .max_connections(25)
        .connect(&cfg.db_url.as_ref().unwrap())
        .await
        .expect("Could not connect to the database");

    // Build the router with routes from the plots module
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/plots", get(plots::views::get_plots)) // Route to plots views
        .with_state(pool);

    // Bind to an address and serve the application
    let addr: std::net::SocketAddr = "0.0.0.0:3000".parse().unwrap();
    println!("Listening on {}", addr);

    // Run the server (correct axum usage without `hyper::Server`)
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
