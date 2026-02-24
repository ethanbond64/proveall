use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

// Embed all migrations from the migrations directory at compile time
// This means the database schema is bundled with the application binary
// No external files or environment variables are needed at runtime
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Establishes a connection to the SQLite database at the given path
/// This path is determined at runtime based on the OS (see lib.rs)
/// No environment variables are used - this is a desktop app, not a server
pub fn establish_connection(db_path: &str) -> SqliteConnection {
    let mut conn = SqliteConnection::establish(db_path)
        .unwrap_or_else(|e| panic!("Error connecting to {}: {}", db_path, e));

    // Enable foreign keys and WAL mode for better performance
    diesel::sql_query("PRAGMA foreign_keys = ON")
        .execute(&mut conn)
        .expect("Failed to enable foreign keys");

    diesel::sql_query("PRAGMA journal_mode = WAL")
        .execute(&mut conn)
        .expect("Failed to set journal mode");

    // Run pending migrations
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    conn
}
