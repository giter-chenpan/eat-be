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

sea-orm-cli generate entity -u mysql://{user:password@host:port/database} -o ./entity/src --with-serde both --ignore-tables dishes_images
```

## dev
reload program via [cargo watch](https://github.com/watchexec/cargo-watch) when files change 
```
cargo watch -x run 
``` 