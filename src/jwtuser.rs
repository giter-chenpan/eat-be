use serde::{ Serialize, Deserialize };
use jsonwebtoken::{
    decode,
    encode,
    errors::Error,
    Algorithm,
    DecodingKey,
    EncodingKey,
    Header,
    TokenData,
    Validation,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub aud: String,
    pub exp: usize,
}

pub fn encode_token(aud: &str) -> String {
    let my_claims = Claims {
        aud: aud.to_owned(),
        exp: 86400000,
    };
    let token = encode(
        &Header::default(),
        &my_claims,
        &EncodingKey::from_secret("secret".as_ref())
    ).expect("encode token fail!");
    token
}

pub fn decode_token(token: &str, aud: &str) -> Result<TokenData<Claims>, Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&[aud]);
    validation.validate_exp = false;
    let decode_val = decode::<Claims>(
        token,
        &DecodingKey::from_secret("secret".as_ref()),
        &validation
    );
    decode_val
}
