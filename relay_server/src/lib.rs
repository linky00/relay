// use std::collections::HashSet;

// use ed25519_dalek::{SigningKey, VerifyingKey};

// pub struct RelayServer {
//     signing_key: SigningKey,
//     trusted_keys: HashSet<VerifyingKey>,
//     message_handler: Mailroom,
// }

// impl RelayServer {
//     pub fn new<S, I>(name: S, poem: I) -> Self
//     where
//         S: Into<String>,
//         I: IntoIterator,
//         I::Item: AsRef<str>,
//     {
//         let signing_key: SigningKey = SigningKey::generate(&mut OsRng);
//         let message_handler = Mailroom::new(name, poem);

//         Self {
//             signing_key,
//             trusted_keys: HashSet::new(),
//             message_handler,
//         }
//     }

//     pub fn get_public_key(&self) -> [u8; PUBLIC_KEY_LENGTH] {
//         self.signing_key.verifying_key().to_bytes()
//     }

//     pub fn trust_public_key(
//         &mut self,
//         public_key: &[u8; PUBLIC_KEY_LENGTH],
//     ) -> Result<(), PublicKeyError> {
//         let verifying_key =
//             VerifyingKey::from_bytes(public_key).map_err(|_| PublicKeyError::CannotReadKey)?;

//         self.trusted_keys.insert(verifying_key);
//         Ok(())
//     }

//     pub fn forget_public_key(
//         &mut self,
//         public_key: &[u8; PUBLIC_KEY_LENGTH],
//     ) -> Result<(), PublicKeyError> {
//         let verifying_key =
//             VerifyingKey::from_bytes(public_key).map_err(|_| PublicKeyError::CannotReadKey)?;

//         self.trusted_keys.remove(&verifying_key);
//         Ok(())
//     }
// }
