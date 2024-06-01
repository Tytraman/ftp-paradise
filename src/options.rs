pub mod data_representation;
pub mod listen_mode;
pub mod session;

use session::SessionInformations;

use self::{data_representation::DataType, listen_mode::ListenMode};

pub struct ClientOptions {
    pub session: Option<SessionInformations>,
    pub working_directory: String,
    pub data_representation: DataType,
    pub local_bytes: i32,
    pub listen_mode: ListenMode,
}
