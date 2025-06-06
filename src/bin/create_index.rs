use sqlite::{self, Connection};
use std::fs;

fn setup() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("./data")?;

    Ok(())
}
fn migrate(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    connection.execute("DROP TABLE IF EXISTS files;")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER,
            type TEXT NOT NULL,
            subtype TEXT,
            offset INTEGER NOT NULL
        )",
    )?;

    Ok(())
}
fn insert_test_data(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let test_data = [
        (0, "texture", "icon", 0),
        (0, "texture", "icon", 1024),
        (0, "texture", "icon", 2048),
        (0, "texture", "icon", 4096),
        (0, "texture", "icon", 8192),
        (0, "texture", "icon", 16384),
        (0, "texture", "icon", 32768),
        (0, "texture", "icon", 65536),
    ];

    for (id, file_type, subtype, offset) in test_data {
        let mut statement = connection
            .prepare("INSERT INTO files (id, type, subtype, offset) VALUES (?, ?, ?, ?)")?;

        statement.bind((1, id))?;
        statement.bind((2, file_type))?;
        statement.bind((3, subtype))?;
        statement.bind((4, offset as i64))?;

        statement.next()?;
    }

    Ok(())
}

fn show_data(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut statement = connection.prepare("SELECT * FROM files")?;
    while let sqlite::State::Row = statement.next()? {
        let id: i64 = statement.read(0)?;
        let file_type: String = statement.read(1)?;
        let subtype: Option<String> = statement.read(2)?;
        let offset: i64 = statement.read(3)?;

        println!(
            "ID: {}, Type: {}, Subtype: {:?}, Offset: {}",
            id, file_type, subtype, offset
        );
    }

    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "./data/index.sqlite";
    let connection = sqlite::open(db_path)?;

    setup()?;
    migrate(&connection)?;

    // TODO: Replace with real data
    insert_test_data(&connection)?;
    show_data(&connection)?;

    Ok(())
}
