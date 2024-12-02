use chrono::Utc;
use rocket_okapi::{ openapi, JsonSchema };
use sea_orm::{ ActiveModelTrait, Set };
use crate::jwtuser::Claims;
use crate::pool::Db;
use sea_orm_rocket::Connection;
use rocket::serde::{ Deserialize, Serialize, json::{ json, Json, Value } };
use ::entity::dishes::{ self };

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ImportDishes {
    name: String,
    desc: String,
    category_id: String,
    view_id: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct FindPage {
    page: i32,
    page_size: i32,
}

/// #save dishes
///
/// save dishes
#[openapi(tag = "dishes", ignore = "db")]
#[post("/api/dishes/save", data = "<data>", format = "json")]
pub async fn save_dishes(
    _claims: Claims,
    db: Connection<'_, Db>,
    data: Json<ImportDishes>
) -> Value {
    let db = db.into_inner();
    let result = (dishes::ActiveModel {
        name: Set(data.name.to_owned()),
        desc: Set(data.desc.to_owned()),
        category_id: Set(data.category_id.parse::<i32>().unwrap()),
        view_id: Set(data.view_id.to_owned()),
        create_time: Set(Utc::now()),
        update_time: Set(Utc::now()),
        status: Set("1".to_owned()), // 1: 正常 0: 停用
        create_user: Set(_claims.sub.to_owned()),
        ..Default::default()
    }).save(db).await;

    match result {
        Ok(_) => json!({ "code": "success", "msg": "保存成功" }),
        Err(_) => json!({ "code": "error", "msg": "保存失败" }),
    }
}

#[openapi(tag = "dishes", ignore = "db")]
#[post("/api/dishes/findpage", data = "<data>")]
pub fn find_page(_claims: Claims, db: Connection<'_, Db>, data: Option<Json<FindPage>>) -> Value {
    json!({ "code": "success", "msg": "获取成功" })
}

#[openapi(tag = "dishes", ignore = "db")]
#[get("/api/dishes/random")]
pub async fn get_random_dishes(_claims: Claims, db: Connection<'_, Db>) -> Value {
    json!({ "code": "success", "msg": "获取成功" })
}
