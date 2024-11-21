#[macro_use]
extern crate rocket;

use migration::MigratorTrait;

use rocket::{ Build, Request, Rocket, response::status, http::Status, fairing::{ self, AdHoc } };
use rocket_db_pools::Database as RedisDatabase;
use sea_orm_rocket::Database;
use rocket_okapi::{ openapi, openapi_get_routes, swagger_ui::{ make_swagger_ui, SwaggerUIConfig } };

mod api;

mod auth;

mod pool;

use pool::{ Db, RedisPool };

mod jwtuser;

#[openapi(tag = "index")]
#[get("/")]
fn home() -> &'static str {
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
        .mount(
            "/",
            openapi_get_routes![
                home,
                auth::login,
                auth::register,
                auth::get_user_info,
                api::category::import_category,
                api::category::get_category,
                api::category::delete_category
            ]
        )
        .mount(
            "/swagger-ui/",
            make_swagger_ui(
                &(SwaggerUIConfig {
                    url: "/openapi.json".to_string(),
                    ..Default::default()
                })
            )
        )
        .register("/", catchers![not_found, default_catcher])
}
