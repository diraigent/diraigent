use std::sync::Arc;

pub struct AppState<R: ?Sized + Send + Sync + 'static> {
    pub repository: Arc<R>,
}

impl<R: ?Sized + Send + Sync + 'static> AppState<R> {
    pub fn new(repo: Arc<R>) -> Self {
        Self { repository: repo }
    }
}

impl<R: ?Sized + Send + Sync + 'static> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            repository: self.repository.clone(),
        }
    }
}

#[macro_export]
macro_rules! load_descriptor_bytes {
    () => {
        include_bytes!(concat!(env!("OUT_DIR"), "/proto_descriptor.bin"))
    };
}

#[macro_export]
macro_rules! define_app_state {
    ($repo_struct:ty, $repo_trait:path) => {
        pub use shared_utils::db::DbPool;
        use std::sync::Arc;

        pub type RepositoryDyn = dyn $repo_trait + Send + Sync;
        pub type AppState = shared_utils::state::AppState<RepositoryDyn>;

        pub fn new(pool: DbPool) -> AppState {
            let sql_repo = <$repo_struct>::new(pool);
            shared_utils::state::AppState::new(Arc::new(sql_repo))
        }
    };
}
