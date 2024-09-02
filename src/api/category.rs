use ::entity::category::{ self, Entity as Category, Model };
use sea_orm::{ ActiveModelTrait, DbConn, DbErr, EntityTrait, Set };
use crate::jwtuser::Claims;
use rocket::{ form::Form, fs::TempFile, serde::json::{ json, Value } };
use crate::pool::Db;
use calamine::{ open_workbook_auto, Reader };
use sea_orm_rocket::Connection;

#[derive(FromForm)]
pub struct FileUpload<'r> {
    file: TempFile<'r>,
}

#[post("/category/import", data = "<form_data>", format = "multipart")]
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

async fn get_category<T>(db: &DbConn, id: String) -> Result<Vec<Model>, DbErr> {
    if id.is_empty() {
        Category::find().all(db).await
    } else {
        Category::find_by_id(id.parse().unwrap_or(0)).all(db).await
    }
}
