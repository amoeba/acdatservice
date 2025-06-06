use libac_rs::dat::reader::dat_database::DatDatabase;
use sqlite::{self, Connection};
use std::{
    env,
    fs::{self, File},
    path::Path,
};

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
            offset INTEGER NOT NULL,
            extra_info JSON
        )",
    )?;

    Ok(())
}

fn show_data(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut statement = connection.prepare("SELECT count(1) FROM files;")?;

    while let sqlite::State::Row = statement.next()? {
        let count: i64 = statement.read(0)?;
        println!("Count: {}", count);
    }

    Ok(())
}

fn create_index(connection: &Connection, dat_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut db_file = File::open(dat_path)?;
    let db: DatDatabase = DatDatabase::read(&mut db_file)?;
    let files = db.list_files(true)?;

    for file in files {
        println!("Processing file: {:?}", file);

        let dat_file_type = file.file_type();

        // TODO: Eventually handle more types
        if dat_file_type != libac_rs::dat::enums::dat_file_type::DatFileType::Texture {
            continue;
        }

        let mut statement = connection
            .prepare("INSERT INTO files (id, type, subtype, offset) VALUES (?, ?, ?, ?)")?;

        statement.bind((1, file.object_id as i64))?;
        statement.bind((2, dat_file_type.to_string().as_str()))?;
        statement.bind((3, "subtype"))?;
        statement.bind((4, file.file_offset as i64))?;

        statement.next()?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        return Err(Box::from("Must specify path to dat file to index."));
    }

    let dat_path = &args[1];

    if !Path::new(dat_path).exists() {
        return Err(Box::from(format!(
            "Provided dat file path doesn't exist: {}",
            dat_path
        )));
    }

    let db_path = "./data/index.sqlite";
    let connection = sqlite::open(db_path)?;

    setup()?;
    migrate(&connection)?;
    create_index(&connection, dat_path)?;
    show_data(&connection)?;

    Ok(())
}
