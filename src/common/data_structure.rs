use rocket::serde::{ json::{ json, Value }, Deserialize, Serialize };

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rep<T> {
    pub code: i32,
    pub msg: String,
    pub data: Option<T>,
}
impl<T: Serialize> Rep<T> {
    pub fn new(code: i32, msg: &str, data: Option<T>) -> Value {
        json!(Self {
            code,
            msg: msg.to_string(),
            data,
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct WordType {
    name: String,
    description: String,
}

pub fn mapping_word_type(s: &str) -> Option<WordType> {
    match s {
        "noun" =>
            Some(WordType {
                name: s.to_string(),
                description: "名词".to_string(),
            }),
        "verb" =>
            Some(WordType {
                name: s.to_string(),
                description: "动词".to_string(),
            }),
        "suffix" =>
            Some(WordType {
                name: s.to_string(),
                description: "后缀".to_string(),
            }),
        _ => None,
    }
}
