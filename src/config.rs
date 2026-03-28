use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub telegram_api_base: String,
    pub database_url: String,
    pub codex_bin: String,
    pub poll_timeout_seconds: u64,
    pub max_directory_entries: usize,
    pub workspace_additional_writable_dirs: Vec<PathBuf>,
}

impl Config {
    pub fn load() -> AppResult<Self> {
        let telegram_bot_token = load_telegram_bot_token()?;
        let telegram_api_base = env::var("ATLAS2_TELEGRAM_API_BASE")
            .unwrap_or_else(|_| "https://api.telegram.org".to_string());
        let database_path =
            env::var("ATLAS2_DATABASE_PATH").unwrap_or_else(|_| "./data/atlas2.sqlite".to_string());
        let codex_bin = env::var("ATLAS2_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
        let poll_timeout_seconds = env_u64("ATLAS2_POLL_TIMEOUT_SECONDS", 30)?;
        let max_directory_entries = env_usize("ATLAS2_MAX_DIRECTORY_ENTRIES", 20)?;
        let workspace_additional_writable_dirs =
            env::var("ATLAS2_CODEX_ADD_DIRS").unwrap_or_default();

        let database_url = if database_path.starts_with("sqlite:") {
            database_path
        } else {
            format!("sqlite://{database_path}")
        };

        let additional_dirs = workspace_additional_writable_dirs
            .split(':')
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .collect();

        Ok(Self {
            telegram_bot_token,
            telegram_api_base,
            database_url,
            codex_bin,
            poll_timeout_seconds,
            max_directory_entries,
            workspace_additional_writable_dirs: additional_dirs,
        })
    }
}

fn load_telegram_bot_token() -> AppResult<String> {
    if let Ok(value) = env::var("ATLAS2_TELEGRAM_BOT_TOKEN") {
        let token = value.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    let token_path = telegram_bot_token_path()?;
    if let Some(token) = read_token_from_file(&token_path)? {
        return Ok(token);
    }

    let token = prompt_for_token()?;
    persist_token(&token_path, &token)?;
    Ok(token)
}

fn prompt_for_token() -> AppResult<String> {
    print!("Telegram bot token: ");
    io::stdout()
        .flush()
        .map_err(|error| AppError::Config(format!("failed to flush stdout: {error}")))?;

    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .map_err(|error| AppError::Config(format!("failed to read token from stdin: {error}")))?;

    let token = buffer.trim().to_string();
    if token.is_empty() {
        return Err(AppError::Config(
            "telegram bot token cannot be empty".into(),
        ));
    }
    Ok(token)
}

fn read_token_from_file(path: &Path) -> AppResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let token = contents.trim().to_string();
            if token.is_empty() {
                Ok(None)
            } else {
                Ok(Some(token))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Config(format!(
            "failed to read Telegram bot token from {}: {error}",
            path.display()
        ))),
    }
}

fn persist_token(path: &Path, token: &str) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Config(format!(
            "invalid Telegram bot token storage path: {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::Config(format!(
            "failed to create Telegram bot token directory {}: {error}",
            parent.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| {
                AppError::Config(format!(
                    "failed to persist Telegram bot token to {}: {error}",
                    path.display()
                ))
            })?;
        file.write_all(token.as_bytes()).map_err(|error| {
            AppError::Config(format!(
                "failed to write Telegram bot token to {}: {error}",
                path.display()
            ))
        })?;
        file.write_all(b"\n").map_err(|error| {
            AppError::Config(format!(
                "failed to finalize Telegram bot token file {}: {error}",
                path.display()
            ))
        })?;
    }

    #[cfg(not(unix))]
    {
        fs::write(path, format!("{token}\n")).map_err(|error| {
            AppError::Config(format!(
                "failed to persist Telegram bot token to {}: {error}",
                path.display()
            ))
        })?;
    }

    Ok(())
}

fn telegram_bot_token_path() -> AppResult<PathBuf> {
    if let Ok(value) = env::var("ATLAS2_TELEGRAM_BOT_TOKEN_FILE") {
        let path = PathBuf::from(value);
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }

    if let Ok(state_home) = env::var("XDG_STATE_HOME") {
        let path = PathBuf::from(state_home);
        if !path.as_os_str().is_empty() {
            return Ok(path.join("atlas2").join("telegram_bot_token"));
        }
    }

    let home = env::var("HOME").map_err(|_| {
        AppError::Config("HOME is not set; cannot determine token storage path".into())
    })?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("atlas2")
        .join("telegram_bot_token"))
}

fn env_u64(key: &str, default: u64) -> AppResult<u64> {
    match env::var(key) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| AppError::Config(format!("{key} must be an integer"))),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::read_token_from_file;

    #[test]
    fn reads_token_from_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("token");
        fs::write(&path, "secret-token\n").unwrap();

        let token = read_token_from_file(&path).unwrap();
        assert_eq!(token.as_deref(), Some("secret-token"));
    }
}

fn env_usize(key: &str, default: usize) -> AppResult<usize> {
    match env::var(key) {
        Ok(value) => value
            .parse::<usize>()
            .map_err(|_| AppError::Config(format!("{key} must be an integer"))),
        Err(_) => Ok(default),
    }
}
