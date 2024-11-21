use ::entity::category::{ self, Entity as Category };
use rocket_okapi::openapi;
use schemars::JsonSchema;
use sea_orm::{ ActiveModelTrait, DbConn, DbErr, EntityTrait, Set };
use crate::jwtuser::Claims;
use rocket::{ form::Form, fs::TempFile, serde::json::{ json, Value } };
use crate::pool::Db;
use calamine::{ open_workbook_auto, Reader };
use sea_orm_rocket::Connection;

#[derive(FromForm, JsonSchema)]
pub struct FileUpload<'r> {
    #[schemars(skip)]
    file: TempFile<'r>,
}
#[openapi(tag = "category", ignore = "conn")]
#[post("/category/import", data = "<form_data>")]
pub async fn import_category(
    _claims: Claims,
    conn: Connection<'_, Db>,
    form_data: Form<FileUpload<'_>>
) -> Value {
    let db = conn.into_inner();
    let path = form_data.file.path().unwrap();

    let mut workbook = open_workbook_auto(path).expect("open file error!");

    let range = workbook.worksheet_range("Sheet1").expect("err");

    let iter = range.rows();

    for val in iter {
        let _ = insert_category(db, val[0].to_string(), val[1].to_string()).await;
    }

    json!({ "msg": "导入成功", "code": "success" })
}

async fn insert_category(
    db: &DbConn,
    name: String,
    desc: String
) -> Result<category::ActiveModel, DbErr> {
    (category::ActiveModel {
        name: Set(name.to_owned()),
        desc: Set(desc.to_owned()),
        ..Default::default()
    }).save(db).await
}

#[openapi(tag = "category", ignore = "db")]
#[get("/getcategory?<id>")]
pub async fn get_category(_claims: Claims, db: Connection<'_, Db>, id: Option<String>) -> Value {
    let db = db.into_inner();
    let result = if id.is_none() {
        Category::find().all(db).await
    } else {
        Category::find_by_id(id.unwrap().parse::<i32>().unwrap_or(0))
            .one(db).await
            .map(|r| vec![r.unwrap()])
    };

    match result {
        Ok(categories) =>
            json!({
            "code": "success",
            "data": categories
        }),
        Err(_) => json!({
            "code": "error", 
            "msg": "查询失败"
        }),
    }
}

#[openapi(tag = "category", ignore = "db")]
#[delete("/category/delete?<id>")]
pub async fn delete_category(_claims: Claims, db: Connection<'_, Db>, id: String) -> Value {
    let result = Category::delete_by_id(id.parse::<i32>().unwrap_or(0)).exec(db.into_inner()).await;
    match result {
        Ok(_) => json!({ "msg": "删除成功", "code": "success" }),
        Err(_) => json!({ "msg": "删除失败", "code": "error" }),
    }
}
