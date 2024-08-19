use ::entity::user::{ self, Entity as User };
use rocket::serde::json::{ json, Json, Value };
use rocket::serde::{ Deserialize, Serialize };
use rocket_db_pools::deadpool_redis::redis::{
    AsyncCommands,
    ExistenceCheck,
    SetExpiry,
    SetOptions,
};
use sea_orm::*;
use sea_orm_rocket::Connection;
use rocket_db_pools::Connection as RedisConnection;

use crate::pool::{ Db, RedisPool };
use crate::jwtuser::{ encode_token, decode_token };
use chrono::Utc;

#[derive(Debug, Deserialize, Serialize)]
pub struct Params<'r> {
    name: &'r str,
    pwd: &'r str,
}

#[post("/api/login", format = "json", data = "<input>")]
pub async fn login(
    mut rsdb: RedisConnection<RedisPool>,
    conn: Connection<'_, Db>,
    input: Json<Params<'_>>
) -> Value {
    let db = conn.into_inner();
    let obj: Option<user::Model> = find_user_by_name(db, input.name.to_string()).await.unwrap();
    if obj.is_none() {
        json!({ "msg": "用户未注册", "code": "BUSINESS_ERROR" })
    } else {
        let id = &obj.unwrap().id.to_string();
        println!("id: {}", id);
        let redis_token = rsdb.get::<&str, String>(id).await;

        if redis_token.is_err() {
            let opts = SetOptions::default()
                .conditional_set(ExistenceCheck::NX)
                .get(true)
                .with_expiration(SetExpiry::EX(86400000));
            let new_token = encode_token(id);
            println!("new token: {}", new_token);
            let _ = rsdb.set_options::<&str, &str, String>(id, &new_token, opts).await;
            json!({ "msg": "登陆成功！", "code": "SUCCESS",  "token": new_token})
        } else {
            json!({ "msg": "登陆成功！", "code": "SUCCESS",  "token": redis_token.unwrap()})
        }
    }
}

#[post("/api/register", data = "<input>")]
pub async fn register(conn: Connection<'_, Db>, input: Json<Params<'_>>) -> Value {
    let db = conn.into_inner();
    let username: Option<user::Model> = find_user_by_name(db, input.name.to_string()).await.expect(
        "error"
    );
    if username.is_none() {
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
    let time = Utc::now().timestamp_millis();

    (user::ActiveModel {
        name: Set(data.name.to_owned()),
        password: Set(data.pwd.to_owned()),
        create_time: Set(time),
        ..Default::default()
    }).save(db).await
}
