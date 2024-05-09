use rocket::serde::{Deserialize, Serialize};

mod ty {
    #[derive(Deserialize, Serialize)]
    pub struct Res<'r> {
        code: u32,
        message: &'r str,
    }
}
