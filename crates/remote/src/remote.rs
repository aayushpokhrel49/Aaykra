pub mod command;
pub mod json_log;
pub mod protocol;
pub mod proxy;
pub mod remote_client;
pub mod remote_identity;
mod transport;

#[cfg(target_os = "windows")]
pub use remote_client::OpenWslPath;
pub use remote_client::{
    CommandTemplate, ConnectionIdentifier, ConnectionState, Interactive, RemoteArch, RemoteClient,
    RemoteClientDelegate, RemoteClientEvent, RemoteConnection, RemoteConnectionOptions, RemoteOs,
    RemotePlatform, connect, has_active_connection,
};
<<<<<<< f50a707fcbfb535e683f575cebdb1580f8d6d233
pub use remote_identity::{
    RemoteConnectionIdentity, remote_connection_identity, same_remote_connection_identity,
};
=======
pub use remote_identity::RemoteConnectionIdentity;
pub use transport::docker::DockerConnectionOptions;
>>>>>>> c1c7e719cf8f6c805a800b7929218416c9e10f9f
pub use transport::ssh::{SshConnectionOptions, SshPortForwardOption};
pub use transport::wsl::WslConnectionOptions;
#[cfg(target_os = "windows")]
pub use transport::wsl::wsl_path_to_windows_path;

#[cfg(any(test, feature = "test-support"))]
pub use transport::mock::{
    MOCK_STDIO_FRAME_HEX, MockConnection, MockConnectionOptions, MockConnectionRegistry,
    MockDelegate,
};
