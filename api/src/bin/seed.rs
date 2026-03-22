use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup connection pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:672643@localhost:5432/dataguard".to_string());
    
    let pool = shared::db::create_pool(&database_url).await?;

    let secret = "supersecret123";

    println!("Création d'un compte de test...");
    sqlx::query!(
        "INSERT INTO accounts (email, plan) VALUES ($1, 'pro') ON CONFLICT (email) DO NOTHING",
        "test@dataguard.local"
    )
    .execute(&pool)
    .await?;

    let account = sqlx::query!(
        "SELECT id FROM accounts WHERE email = $1",
        "test@dataguard.local"
    )
    .fetch_one(&pool)
    .await?;

    let api_key_id = Uuid::new_v4();

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(secret.as_bytes(), &salt)
        .unwrap()
        .to_string();

    sqlx::query!(
        "INSERT INTO api_keys (id, account_id, key_hash, name) VALUES ($1, $2, $3, 'Test Key') ON CONFLICT DO NOTHING",
        api_key_id,
        account.id,
        password_hash
    )
    .execute(&pool)
    .await?;

    let full_key = format!("dg_live_{}_{}", api_key_id, secret);
    
    println!("========================================");
    println!("✓ Compte et clé d'API créés avec succès !");
    println!("========================================");
    println!("🗝️  VOTRE CLÉ D'API (X-Api-Key) :");
    println!("{}", full_key);
    println!("========================================");
    println!("➡️  Utilisez cette clé dans le header 'X-Api-Key' de vos requêtes Postman/Bruno/cURL.");

    Ok(())
}
