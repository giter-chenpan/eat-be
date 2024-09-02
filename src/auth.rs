use ::entity::user::{ self, Entity as User };
use rocket::{
    request::{ self, FromRequest, Request },
    serde::{ Deserialize, Serialize, json::{ json, Json, Value } },
    outcome::Outcome,
};
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
use crate::jwtuser::{ encode_token, Claims, SECRET };
use jsonwebtoken::{ decode, DecodingKey, Validation, Algorithm };
use chrono::Local;

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
                .with_expiration(SetExpiry::EX(2592000));
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
    let time = Local::now();
    (user::ActiveModel {
        name: Set(data.name.to_owned()),
        password: Set(data.pwd.to_owned()),
        create_time: Set(time),
        ..Default::default()
    }).save(db).await
}

// token 拦截器
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        // 获取请求头中的 Authorization 字段
        let token = match request.headers().get_one("Authorization") {
            Some(token) => token.to_string(),
            None => {
                return Outcome::Error((rocket::http::Status::Unauthorized, ()));
            }
        };

        println!("{}", token);

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        match decode::<Claims>(&token, &DecodingKey::from_secret(SECRET.as_ref()), &validation) {
            Ok(token_data) => Outcome::Success(token_data.claims),
            Err(_) => Outcome::Error((rocket::http::Status::Unauthorized, ())),
        }
    }
}
