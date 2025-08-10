use super::error::Error;

#[derive(Debug)]
pub enum Groth16Error {
    ProofVerificationFailed,
    PrepareInputsFailed,
    GeneralError(Error),
    Groth16VkeyHashMismatch,
}

impl From<Error> for Groth16Error {
    fn from(error: Error) -> Self {
        Groth16Error::GeneralError(error)
    }
}