#[macro_use]
extern crate rocket;

use migration::MigratorTrait;

use rocket::{ Build, Request, Rocket, response::status, http::Status, fairing::{ self, AdHoc } };
use rocket_db_pools::Database as RedisDatabase;
use sea_orm_rocket::Database;

mod api;

mod auth;
use auth::{ login, register };

mod pool;
use pool::{ Db, RedisPool };

mod jwtuser;

#[get("/api")]
fn index() -> &'static str {
    "Hello, world!"
}

#[catch(404)]
fn not_found(req: &Request) -> String {
    format!("Sorry, '{}' is not a valid path.", req.uri())
}

#[catch(default)]
fn default_catcher(status: Status, req: &Request<'_>) -> status::Custom<String> {
    let msg = format!("{} ({})", status, req.uri());
    status::Custom(status, msg)
}

async fn run_migrations(rocket: Rocket<Build>) -> fairing::Result {
    let conn = &Db::fetch(&rocket).unwrap().conn;
    let _ = migration::Migrator::up(conn, Some(10)).await;
    Ok(rocket)
}

#[launch]
fn rocket() -> _ {
    rocket
        ::build()
        .attach(RedisPool::init())
        .attach(Db::init())
        .attach(AdHoc::try_on_ignite("Migrations", run_migrations))
        .mount("/", routes![index, login, register])
        .mount("/api", api::api_routes())
        .register("/", catchers![not_found, default_catcher])
}
