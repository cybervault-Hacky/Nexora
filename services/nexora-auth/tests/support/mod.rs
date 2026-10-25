//! Shared safety checks for PostgreSQL integration tests.

/// Refuse to apply migrations or run fixture cleanup unless the connected
/// database name is explicitly marked as a test database.
pub async fn assert_test_database(pool: &sqlx::PgPool) {
    let database_name = sqlx::query_scalar::<_, String>("SELECT current_database()")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| {
            panic!("could not verify the PostgreSQL test database; refusing database changes")
        });

    assert!(
        database_name.ends_with("_test"),
        "PostgreSQL integration tests require a database name ending in `_test`; refusing migrations or cleanup"
    );
}
