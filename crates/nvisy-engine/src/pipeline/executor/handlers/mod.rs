//! Per-phase node handler implementations.

pub(crate) mod context;
pub(crate) mod extraction;
pub(crate) mod recognition;
pub(crate) mod refinement;

/// Call a closure with optional retry policy.
pub(super) mod retry {
    use nvisy_core::Error;

    use crate::pipeline::policy::CompiledRetryPolicy;

    pub async fn call<T, F, Fut>(retry: Option<&CompiledRetryPolicy>, mut f: F) -> Result<T, Error>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        match retry {
            Some(policy) => policy.with_retry(f).await,
            None => f().await,
        }
    }
}
