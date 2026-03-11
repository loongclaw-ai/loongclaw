use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) struct ScopedEnvVar {
    key: &'static str,
    original: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl ScopedEnvVar {
    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let guard = env_lock().lock().expect("env lock should not be poisoned");
        let original = std::env::var_os(key);
        std::env::set_var(key, value);
        Self {
            key,
            original,
            _guard: guard,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn remove(key: &'static str) -> Self {
        let guard = env_lock().lock().expect("env lock should not be poisoned");
        let original = std::env::var_os(key);
        std::env::remove_var(key);
        Self {
            key,
            original,
            _guard: guard,
        }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}
