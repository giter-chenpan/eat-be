use chrono::{ Utc, Duration };

use serde::{ Serialize, Deserialize };
use jsonwebtoken::{ encode, EncodingKey, Header };

pub const SECRET: &str = "secret";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub iat: i64,
    pub id: String,
    pub exp: i64,
}

pub fn encode_token(aud: &str) -> String {
    let time = Utc::now().timestamp();

    let my_claims = Claims {
        id: aud.to_owned(),
        iat: time,
        exp: (Utc::now() + Duration::days(30)).timestamp(),
    };
    let token = encode(
        &Header::default(),
        &my_claims,
        &EncodingKey::from_secret(SECRET.as_ref())
    ).expect("encode token fail!");
    token
}
