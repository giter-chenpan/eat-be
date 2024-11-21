use ::entity::user::{ self, Entity as User };
use rocket::{
    request::{ self, FromRequest, Request },
    serde::{ Deserialize, Serialize, json::{ json, Json, Value } },
    outcome::Outcome,
    http::Status,
};
use rocket_db_pools::deadpool_redis::redis::AsyncCommands;
use rocket_okapi::{
    gen::OpenApiGenerator,
    okapi::openapi3::{ SecurityScheme, SecuritySchemeData, SecurityRequirement, Object },
    openapi,
    request::{ OpenApiFromRequest, RequestHeaderInput },
};
use schemars::JsonSchema;
use sea_orm::*;
use sea_orm_rocket::Connection;
use rocket_db_pools::Connection as RedisConnection;

use crate::pool::{ Db, RedisPool };
use crate::jwtuser::{ encode_token, Claims, SECRET };
use jsonwebtoken::{ decode, DecodingKey, Validation, Algorithm };
use chrono::Local;
use rocket::{ self, error };

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Params<'r> {
    name: &'r str,
    pwd: &'r str,
}

#[openapi(tag = "auth", ignore = "rsdb", ignore = "conn")]
#[post("/login", format = "json", data = "<input>")]
pub async fn login(
    mut rsdb: RedisConnection<RedisPool>,
    conn: Connection<'_, Db>,
    input: Json<Params<'_>>
) -> Value {
    let db = conn.into_inner();

    match find_user_by_name(db, input.name.to_string()).await {
        Ok(Some(user)) => {
            let id = user.id.to_string();
            let pwd = user.password.to_string();
            if pwd != input.pwd.to_string() {
                return json!({ "msg": "密码错误", "code": "business_error" });
            }
            match rsdb.get::<&str, String>(&id).await {
                Ok(redis_token) => {
                    json!({ "msg": "登陆成功！", "code": "success", "token": redis_token })
                }
                Err(_) => {
                    let new_token = encode_token(&id);

                    // 分两步操作：先 set，再设置过期时间
                    match
                        rsdb.set_ex::<&str, &str, String>(&id, &new_token, 30 * 24 * 60 * 60).await
                    {
                        Ok(_) => {
                            // 单独设置过期时间
                            if let Err(e) = rsdb.expire::<&str, u64>(&id, 30 * 24 * 60 * 60).await {
                                error!("Redis expire error: {:?}", e);
                            }
                            json!({ "msg": "登陆成功！", "code": "success", "token": new_token })
                        }
                        Err(e) => {
                            error!("Redis set error: {:?}", e);
                            json!({ "msg": "服务器错误", "code": "server_error" })
                        }
                    }
                }
            }
        }
        Ok(None) => json!({ "msg": "用户未注册", "code": "business_error" }),
        Err(e) => {
            error!("Database error: {:?}", e);
            json!({ "msg": "服务器错误", "code": "server_error" })
        }
    }
}

#[openapi(tag = "auth", ignore = "conn")]
#[post("/register", data = "<input>")]
pub async fn register(conn: Connection<'_, Db>, input: Json<Params<'_>>) -> Value {
    let db = conn.into_inner();
    match find_user_by_name(db, input.name.to_string()).await {
        Ok(None) => {
            if let Err(e) = insert_user(db, input).await {
                error!("Error inserting user: {:?}", e);
                return json!({ "msg": "服务器错误", "code": "server_error" });
            }
            json!({ "msg": "注册成功", "code": "success" })
        }
        Ok(Some(_)) => json!({ "msg": "用户已注册", "code": "business_error" }),
        Err(e) => {
            error!("Database error: {:?}", e);
            json!({ "msg": "服务器错误", "code": "server_error" })
        }
    }
}

#[openapi(tag = "auth", ignore = "conn")]
#[get("/getuserinfo")]
pub async fn get_user_info(_claims: Claims, conn: Connection<'_, Db>) -> Value {
    let db = conn.into_inner();
    let id = _claims.sub.parse::<i32>().unwrap_or(0);
    match User::find_by_id(id).one(db).await {
        Ok(user) => json!({ "msg": "获取用户信息成功", "code": "success", "data": user }),
        Err(_) => json!({ "msg": "服务器错误", "code": "server_error" }),
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
        let token = match request.headers().get_one("Authorization") {
            Some(token) => token.to_string(),
            None => {
                return Outcome::Error((Status::Unauthorized, ()));
            }
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        match decode::<Claims>(&token, &DecodingKey::from_secret(SECRET.as_ref()), &validation) {
            Ok(token_data) => Outcome::Success(token_data.claims),
            Err(e) => {
                error!("Error decoding token: {:?}", e);
                Outcome::Error((Status::Unauthorized, ()))
            }
        }
    }
}

// swagger token配置
impl<'a> OpenApiFromRequest<'a> for Claims {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        // Setup global requirement for Security scheme
        let security_scheme = SecurityScheme {
            description: Some(
                "Requires an Bearer token to access, token is: `Authorization`.".to_owned()
            ),
            // Setup data requirements.
            // In this case the header `Authorization: mytoken` needs to be set.
            data: SecuritySchemeData::ApiKey {
                name: "Authorization".to_owned(),
                location: "header".to_owned(),
            },
            extensions: Object::default(),
        };
        // Add the requirement for this route/endpoint
        // This can change between routes.
        let mut security_req = SecurityRequirement::new();
        // Each security requirement needs to be met before access is allowed.
        security_req.insert("HttpAuth".to_owned(), Vec::new());
        // These vvvvvvv-----^^^^^^^^ values need to match exactly!
        Ok(RequestHeaderInput::Security("HttpAuth".to_owned(), security_scheme, security_req))
    }
}
