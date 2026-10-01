//! Runs the petstore mock API on http://localhost:4010 for `examples/petstore`.
//!
//!     cargo run --example petstore_mock

#[path = "../tests/common/petstore.rs"]
#[allow(dead_code)]
mod petstore;

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "4010".into());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .expect("bind");
    println!(
        "petstore mock on http://localhost:{port} (password: {})",
        petstore::PASSWORD
    );
    axum::serve(listener, petstore::router()).await.unwrap();
}
