/// Defines methods of transforming entropy into a result
pub enum Method {
    MaskSquareOfTwo,
    RemainderBelowLimit(u8),
}

/// Precalculated params for random result generation
pub struct Params {
    bound: u8,
    method: Method,
}

/// Precalculate params for random result generation
pub fn params_for(bound: u8) -> Params {
    Params {
        bound,
        method: if (bound - 1) & bound == 0 {
            Method::MaskSquareOfTwo
        } else {
            Method::RemainderBelowLimit(u8::MAX - u8::MAX % bound)
        },
    }
}

/// Try to generate a random number from the given precalculated params and entropy.
pub fn generate(params: &Params, entropy: u8) -> Option<u8> {
    match params.method {
        Method::MaskSquareOfTwo =>
            Some(entropy % params.bound),
        Method::RemainderBelowLimit(limit) =>
            if entropy <= limit {
                Some(entropy % params.bound)
            } else {
                None
            }
    }
}
