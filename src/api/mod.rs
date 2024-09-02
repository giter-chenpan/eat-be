use rocket::Route;
mod category;

pub fn api_routes() -> Vec<Route> {
    routes![category::import_category]
}
