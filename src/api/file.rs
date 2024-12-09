use chrono::Local;
use rocket_okapi::{ openapi, JsonSchema };
use crate::jwtuser::Claims;

use sea_orm_rocket::Connection;
use sea_orm::{ ActiveModelTrait, EntityTrait, Set };
use ::entity::dishes_images::{ self };
use crate::pool::Db;
use rocket::{
    form::Form,
    fs::TempFile,
    serde::json::{ json, Value },
    tokio::io::AsyncReadExt,
    http::ContentType,
};

#[derive(FromForm, JsonSchema)]
pub struct FileUpload<'r> {
    #[schemars(skip)]
    file: TempFile<'r>,
}

/// # Upload image
///
/// Upload image to server
#[openapi(tag = "file", ignore = "db")]
#[post("/api/file/uploadimage", data = "<form_data>")]
pub async fn upload_file(
    _claims: Claims,
    db: Connection<'_, Db>,
    form_data: Form<FileUpload<'_>>
) -> Value {
    let db = db.into_inner();
    // 获取文件名和数据
    let filename = form_data.file.name().unwrap().to_string();

    let content_type = form_data.file.content_type().unwrap();

    let allowed_type = vec!["image/png", "image/jpeg", "image/jpg"];
    if !allowed_type.contains(&content_type.to_string().as_str()) {
        return json!({ "msg": "不允许的文件类型", "code": "error" });
    }

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
        Ok(image) =>
            json!({ "msg": "上传成功", "code": "success", "data": json!({"id": image.id, "url": format!("/api/file/getimage?id={}", image.id)})}),
        Err(e) => json!({ "msg": format!("{}", e), "code": "error" }),
    }
}

/// # Get image
///
/// Get image by id
#[openapi(tag = "file", ignore = "db")]
#[get("/api/file/getimage?<id>")]
pub async fn get_image(
    db: Connection<'_, Db>,
    id: String
) -> Result<(ContentType, Vec<u8>), Value> {
    let db = db.into_inner();
    let result = dishes_images::Entity::find_by_id(id.parse::<i32>().unwrap_or(0)).one(db).await;
    match result {
        Ok(Some(image)) => {
            if let Some(image_data) = image.image_data {
                Ok((ContentType::PNG, image_data))
            } else {
                Err(
                    json!({
                        "msg": "图片数据为空",
                        "code": "error"
                    })
                )
            }
        }
        _ =>
            Err(
                json!({
                "msg": "未找到图片",
                "code": "error"
            })
            ),
    }
}
