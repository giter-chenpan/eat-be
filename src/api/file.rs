use chrono::Local;
use rocket_okapi::{ openapi, JsonSchema };
use crate::jwtuser::Claims;

use sea_orm_rocket::Connection;
use sea_orm::{ ActiveModelTrait, Set };
use ::entity::dishes_images::{ self };
use crate::pool::Db;
use rocket::{ form::Form, fs::TempFile, serde::json::{ json, Value }, tokio::io::AsyncReadExt };

#[derive(FromForm, JsonSchema)]
pub struct FileUpload<'r> {
    #[schemars(skip)]
    file: TempFile<'r>,
}

/// # Upload image
///
/// Upload image to server
#[openapi(tag = "file", ignore = "db")]
#[post("/file/uploadimage", data = "<form_data>")]
pub async fn upload_file(
    _claims: Claims,
    db: Connection<'_, Db>,
    form_data: Form<FileUpload<'_>>
) -> Value {
    let db = db.into_inner();
    // 获取文件名和数据
    let filename = form_data.file.name().unwrap().to_string();
    let mut file = form_data.file.open().await.unwrap();
    let mut data = Vec::new();
    file.read_to_end(&mut data).await.unwrap();
    let result = (dishes_images::ActiveModel {
        name: Set(filename.to_owned()),
        image_data: Set(Some(data)),
        create_time: Set(Local::now().to_utc()),
        update_time: Set(Local::now().to_utc()),
        ..Default::default()
    }).insert(db).await;
    match result {
        Ok(_) => json!({ "msg": "上传成功", "code": "success" }),
        Err(e) => json!({ "msg": format!("{}", e), "code": "error" }),
    }
}
