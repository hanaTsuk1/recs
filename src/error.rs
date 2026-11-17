#[derive(Debug)]
pub enum RecsError {
    NoSuchEntity,
}

pub type RecsResult<T> = Result<T, RecsError>;
