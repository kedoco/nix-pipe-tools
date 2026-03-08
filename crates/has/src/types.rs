/// A single file/resource entry associated with a process.
#[derive(Debug, Clone)]
pub struct Entry {
    pub pid: String,
    pub command: String,
    pub user: String,
    pub fd: String,
    pub file_type: String,
    pub access: String,
    pub name: String,
}
