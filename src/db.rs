use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Deserialize, Serialize)]
pub struct File {
    pub id: i64,
    pub database_type: i64,
    pub file_type: i64,
    pub file_subtype: i64,
    pub file_offset: i64,
    pub file_size: i64,
}
