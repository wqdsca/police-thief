pub struct GrpcSuccessResponse<T> {
    pub success: i32,
    pub data: T,
}

impl<T> GrpcSuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            success: 1,
            data,
        }
    }
}