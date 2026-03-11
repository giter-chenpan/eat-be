#[macro_use]
extern crate rocket;

use migration::MigratorTrait;
use rocket::{
    Build, Request, Rocket,
    fairing::{self, AdHoc},
    http::Status,
    response::status,
};
use rocket_db_pools::Database as RedisDatabase;
use rocket_okapi::{
    openapi, openapi_get_routes,
    swagger_ui::{SwaggerUIConfig, make_swagger_ui},
};
use sea_orm_rocket::Database;
mod api;
mod auth;
mod common;
mod config;

mod pool;

use pool::{Db, RedisPool};

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
    let _ = migration::Migrator::up(conn, None).await;
    Ok(rocket)
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(rocket::fairing::AdHoc::try_on_ignite(
            "MCP Server",
            |rocket| async {
                // 在独立 tokio task 中启动 MCP 服务，不阻塞 Rocket 主线程
                // MCP 监听 8081 端口，Rocket 主服务监听 Rocket.toml 中配置的端口
                rocket::tokio::spawn(async {
                    if let Err(e) = howtocook_mcp_server::start("0.0.0.0:8081").await {
                        eprintln!("❌ MCP Server error: {e}");
                    }
                });
                Ok(rocket)
            },
        ))
        .attach(RedisPool::init())
        .attach(Db::init())
        .attach(AdHoc::try_on_ignite("Migrations", run_migrations))
        .mount(
            "/",
            openapi_get_routes![
                home,
                auth::login,
                auth::logout,
                auth::register,
                auth::get_user_info,
                api::category::import_category,
                api::category::get_category,
                api::category::delete_category,
                api::dishes::save_dishes,
                api::dishes::find_page,
                api::dishes::get_random_dishes,
                api::file::upload_file,
                api::file::get_image,
                api::translation::handle_translation,
                api::translation::get_words,
                api::translation::find_page,
                api::times::set_times,
                api::times::get_times_page,
                api::times::delete_times,
            ],
        )
        .mount(
            "/swagger-ui/",
            make_swagger_ui(
                &(SwaggerUIConfig {
                    url: "/openapi.json".to_string(),
                    ..Default::default()
                }),
            ),
        )
        .register("/", catchers![not_found, default_catcher])
}
