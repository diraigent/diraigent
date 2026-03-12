use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Poll until the shutdown flag is set, checking every 250 ms.
pub async fn wait_shutdown(flag: &AtomicBool) {
    while !flag.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Load `.env` files from the standard orchestra locations.
///
/// Load order (first loaded wins; pre-set env vars are never overridden):
/// 1. Walk up from cwd looking for `apps/orchestra/.env`
/// 2. `{cwd}/.env`
/// 3. Standard `dotenvy` fallback (nearest `.env` in the filesystem)
pub fn load_dotenv() {
    let cwd = std::env::current_dir().unwrap_or_default();

    // Walk up from cwd looking for apps/orchestra/.env — local dev fallback
    {
        let mut dir = cwd.clone();
        loop {
            let env_path = dir.join("apps/orchestra/.env");
            if env_path.exists() {
                dotenvy::from_path(&env_path).ok();
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    // cwd/.env — works in containers where .env is mounted at /app/.env
    dotenvy::from_path(cwd.join(".env")).ok();

    // Standard dotenv fallback
    dotenvy::dotenv().ok();
}
