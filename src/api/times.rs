use ::entity::times::{self, Entity as Times};
use chrono::Utc;
use rocket::serde::json::Json;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
use sea_orm_rocket::Connection;
use serde::{Deserialize, Serialize};

use crate::common::{data_structure::*, enums::Code::*};
use crate::pool::Db;

use crate::jwtuser::Claims;

#[openapi(tag = "times", ignore = "db")]
#[get("/api/times")]
pub async fn set_times(_claims: Claims, db: Connection<'_, Db>) -> Json<Rep<Option<String>>> {
    let db = db.into_inner();
    let now = Utc::now();
    let time = format!("{}", now.format("%Y-%m-%d %H:%M:%S"));
    let day = format!("{}", now.format("%Y-%m-%d"));

    let user_id = _claims.sub.parse::<i32>().unwrap_or(0).to_string();

    let result = (times::ActiveModel {
        time: Set(day),
        all_time: Set(time.clone()),
        created_user: Set(user_id),
        created_time: Set(time),
        ..Default::default()
    })
    .save(db)
    .await;

    match result {
        Ok(_) => Rep::<Option<String>>::new(Success.self_code(), "成功", None),
        Err(e) => {
            println!("save error: {}", e);
            Rep::<Option<String>>::new(BusinessError.self_code(), "失败", None)
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindPageRepItem {
    all_time: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct FindPageRep {
    page: u64,
    total: u64,
    list: Vec<FindPageRepItem>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub page: u64,
    pub page_size: u64,
}

#[openapi(tag = "times", ignore = "db", ignore = "_claims")]
#[post("/api/get_times_page", data = "<data>", format = "json")]
pub async fn get_times_page(
    _claims: Claims,
    db: Connection<'_, Db>,
    data: Json<Params>,
) -> Json<Rep<FindPageRep>> {
    let db = db.into_inner();

    let result = Times::find().paginate(db, data.page_size);
    let current_page = data.page - 1;

    let result_list = result
        .fetch_page(current_page)
        .await
        .unwrap()
        .into_iter()
        .map(|item| FindPageRepItem {
            all_time: item.all_time,
        })
        .collect::<Vec<FindPageRepItem>>();

    let rep = FindPageRep {
        page: data.page,
        total: match result.num_items().await {
            Ok(total) => total,
            Err(error) => {
                println!("{}", error);
                return Rep::<FindPageRep>::new(BusinessError.self_code(), "查询错误", None);
            }
        },
        list: result_list,
    };
    Rep::<FindPageRep>::new(Success.self_code(), "成功", Some(rep))
}
