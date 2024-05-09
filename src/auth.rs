use ::entity::user::{ self, Entity as User };
use rocket::serde::json::{ json, Json, Value };
use rocket::serde::{ Deserialize, Serialize };
use sea_orm::*;
use sea_orm_rocket::Connection;
use uuid::Uuid;

use crate::pool::Db;

#[derive(Debug, Deserialize, Serialize)]
pub struct Params<'r> {
    name: &'r str,
    pwd: &'r str,
}

#[post("/login", format = "json", data = "<input>")]
pub fn login(input: Json<Params<'_>>) -> Value {
    // json!({"code": 200, "token": "wwewe", "message": "success"})
    let id = Uuid::new_v4();

    json!({ "id": id.to_string(), "name": input.name })
}

#[post("/register", data = "<input>")]
pub async fn register(conn: Connection<'_, Db>, input: Json<Params<'_>>) -> Value {
    let db = conn.into_inner();
    let username: Option<user::Model> = find_user_by_name(db, input.name.to_string()).await.expect(
        "error"
    );
    if username == None {
        insert_user(db, input).await.expect("could not insert post");
        json!({ "msg": "注册成功", "code": "SUCCESS" })
    } else {
        json!({ "msg": "用户已注册", "code": "BUSINESS_ERROR" })
    }
}

async fn find_user_by_name(db: &DbConn, name: String) -> Result<Option<user::Model>, DbErr> {
    User::find().filter(user::Column::Name.eq(name)).one(db).await
}

async fn insert_user(db: &DbConn, data: Json<Params<'_>>) -> Result<user::ActiveModel, DbErr> {
    // let id = Uuid::new_v4().to_string();
    (user::ActiveModel {
        name: Set(data.name.to_owned()),
        password: Set(data.pwd.to_owned()),
        ..Default::default()
    }).save(db).await
}
