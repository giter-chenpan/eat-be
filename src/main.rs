#[macro_use]
extern crate rocket;

use rocket::{ Request, response::status, http::Status };
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

#[launch]
fn rocket() -> _ {
    rocket
        ::build()
        .attach(RedisPool::init())
        .attach(Db::init())
        .mount(
            "/",
            openapi_get_routes![
                home,
                auth::login,
                auth::register,
                auth::get_user_info,
                api::category::import_category,
                api::category::get_category,
                api::category::delete_category,
                api::dishes::save_dishes,
                api::file::upload_file,
                api::file::get_image
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
