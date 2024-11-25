## 生成migration文件
```
# E.g. to generate 'migration/src/m20220101_000001_create_table.rs shown below
sea-orm-cli migrate generate create_table  //Generate a new migration file
sea-orm-cli migrate up   //Apply all pending migrations

```
## 数据库生成entity
```
# Show how to use `generate entity` subcommand
sea-orm-cli generate entity -u mysql://{xxxx} -o {dir}
```