use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库操作失败: {0}")]
    Database(#[from] sqlx::Error),
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("配置格式错误: {0}")]
    Config(String),
    #[error("输入无效: {0}")]
    InvalidInput(String),
    #[error("操作失败: {0}")]
    Operation(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
