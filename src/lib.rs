pub mod commands;
pub mod config;
pub mod options;
pub mod server;
pub mod thread_pool;
pub mod platform;

use std::sync::OnceLock;

use crate::server::ftp_server::FtpServer;
use config::Config;

// Indique que la ligne du dessous ne sera incluse que sur des plateformes 'Linux'.
#[cfg(target_os = "linux")]
use std::{net::TcpStream, thread};
#[cfg(target_os = "linux")]
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn run(config: Config) -> Result<(), String> {
    match CONFIG.set(config) {
        Ok(()) => (),
        Err(_) => return Err("cannot create singleton config".to_string()),
    }

    let mut ftp_server = match FtpServer::build() {
        Ok(server) => server,
        Err(err) => {
            return Err(format!("cannot build FTP server: {err}"));
        }
    };

    // Indique que l'on veut intercepter les signaux SIGINT et SIGTERM.
    // Uniquement sur les plateformes 'Linux'.
    #[cfg(target_os = "linux")]
    {
        let server_shutdown = ftp_server.get_shutdown_rc();

        let mut signals = match Signals::new(&[SIGINT, SIGTERM]) {
            Ok(sig) => sig,
            Err(err) => {
                return Err(format!("cannot create signals handler: {err}"));
            }
        };

        // Lance un thread qui intercepte les signaux envoyés par le kernel.
        thread::spawn(move || {
            for _ in signals.forever() {
                println!("Interrupt signal received, cleaning up...");

                server_shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = TcpStream::connect(format!(
                    "{}:{}",
                    CONFIG.get().unwrap().get_hostname(),
                    CONFIG.get().unwrap().get_port()
                ));

                println!("Server stopped.");
            }
        });
    }

    let _ = ftp_server.start();

    Ok(())
}

#[cfg(test)]
mod tests {}
