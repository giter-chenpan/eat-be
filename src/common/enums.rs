pub enum Code {
    Success,
    BadRequest,
}

impl Code {
    pub fn self_code(&self) -> i32 {
        match self {
            Code::Success => 200,
            Code::BadRequest => 400,
        }
    }
}
