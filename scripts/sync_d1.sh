#!/bin/sh

set -e
db_name="dats"
db_path="./data/index.sqlite"
sql_path="data/index.sql"

if [ ! -f "$db_path" ]; then
  echo "Database not found at path $db_path. Create first by running:"
  echo ""
  ecoh "  cargo run --bin create_index --features=index -- client_portal.dat"
  echo ""

  exit 1
fi

echo "Preparing .sql file..."
echo "DROP TABLE IF EXISTS database_types;" > "$sql_path"
echo "DROP TABLE IF EXISTS file_types;" >> "$sql_path"
echo "DROP TABLE IF EXISTS file_subtypes;" >> "$sql_path"
echo "DROP TABLE IF EXISTS files;" >> "$sql_path"
echo "...done."

echo "Using database $db_path."
echo "Dumping database to $sql_path..."
sqlite3 "$db_path" ".dump" | grep -v "^PRAGMA" | grep -v "BEGIN TRANSACTION" | grep -v "COMMIT" >> "$sql_path"
echo "...done."

echo "Executing on CloudFlare.."
npx wrangler d1 execute "$db_name" --file "$sql_path" --remote
echo "...done."
