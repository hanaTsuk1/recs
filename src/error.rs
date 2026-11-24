#[derive(Debug)]
pub enum RecsError {
    NoSuchEntity,
    AccessConflict { name: &'static str },
    NoSuchSystem { name: &'static str },
    CircularDependency,
}

pub type RecsResult<T> = Result<T, RecsError>;
