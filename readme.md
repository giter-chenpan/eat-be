## generate migration files
```
# E.g. to generate 'migration/src/m20220101_000001_create_table.rs shown below  

sea-orm-cli migrate generate create_table  //Generate a new migration file
sea-orm-cli migrate up   //Apply all pending migrations

```
## generate entity files
[sea-orm](https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/)
```
# Show how to use `generate entity` subcommand  

sea-orm-cli generate entity -u sqlite://data/eatbe.db -o ./entity/src --with-serde both --ignore-tables dishes_images
```

## MySQL → SQLite data migration (one-time)

This project uses SQLite. To migrate existing MySQL data:

1. Copy `Rocket.toml.example` to `Rocket.toml` and adjust Redis / paths as needed.
2. Run the migration tool (requires access to the source MySQL database):

```bash
MYSQL_URL='mysql://user:pass@host:3306/eatbe' \
SQLITE_URL='sqlite://data/eatbe.db?mode=rwc' \
cargo run -p mysql-to-sqlite
```

Use `FORCE=1` to overwrite an existing SQLite file.

3. Update `Rocket.toml` `databases.sea_orm.url` to the SQLite URL if not already set.
4. Restart the application.

## dev
reload program via [cargo watch](https://github.com/watchexec/cargo-watch) when files change 
```
cargo watch -x run 
``` 