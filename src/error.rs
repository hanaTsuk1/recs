#[derive(Debug)]
pub enum RecsError {
    NoSuchEntity,
    AccessConflict { name: &'static str },
}

pub type RecsResult<T> = Result<T, RecsError>;
