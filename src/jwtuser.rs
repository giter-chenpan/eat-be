use chrono::{ Utc, Duration };

use serde::{ Serialize, Deserialize };
use jsonwebtoken::{ encode, EncodingKey, Header };

pub const SECRET: &str = "secret";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub iat: i64,
    pub sub: String,
    pub exp: usize,
}

pub fn encode_token(aud: &str) -> String {
    let time = Utc::now().timestamp();

    let my_claims = Claims {
        iat: time,
        sub: aud.to_string(),
        exp: Utc::now().checked_add_signed(Duration::days(30)).unwrap().timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &my_claims,
        &EncodingKey::from_secret(SECRET.as_ref())
    ).expect("encode token fail!");
    token
}
