use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SECRET: &str = "secret";

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Claims {
    pub iat: usize,
    pub sub: String,
    pub exp: usize,
}

pub fn encode_token(aud: &str) -> String {
    let time = Utc::now().timestamp() as usize;
    let my_claims = Claims {
        iat: time,
        sub: aud.to_string(),
        exp: Utc::now()
            .checked_add_signed(Duration::days(30))
            .unwrap()
            .timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &my_claims,
        &EncodingKey::from_secret(SECRET.as_ref()),
    )
    .expect("encode token fail!");
    token
}
