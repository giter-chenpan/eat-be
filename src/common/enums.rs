pub enum Code {
    Success,
    BadRequest,
    BusinessError,
}

impl Code {
    pub fn self_code(&self) -> i32 {
        match self {
            Code::Success => 200,
            Code::BadRequest => 400,
            Code::BusinessError => 500,
        }
    }
}
