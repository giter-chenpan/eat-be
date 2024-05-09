#[macro_use]
extern crate rocket;

use migration::MigratorTrait;
use rocket::fairing::{ self, AdHoc };
use rocket::http::Status;
use rocket::response::status;
use rocket::{ Build, Request, Rocket };
use sea_orm_rocket::Database;
mod auth;
use auth::{ login, register };

mod pool;
use pool::Db;

pub use entity::post;
pub use entity::post::Entity as Post;

#[get("/")]
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
    let _ = migration::Migrator::fresh(conn).await;
    Ok(rocket)
}

#[launch]
fn rocket() -> _ {
    rocket
        ::build()
        .attach(Db::init())
        .attach(AdHoc::try_on_ignite("Migrations", run_migrations))
        .mount("/", routes![index, login, register])
        .register("/", catchers![not_found, default_catcher])
}
