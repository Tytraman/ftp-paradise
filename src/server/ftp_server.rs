use std::{
    cell::RefCell,
    error::Error,
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use crate::{
    commands::CommandReturnType, server::ftp_client::FtpClient, thread_pool::ThreadPool, CONFIG,
};

pub struct FtpServer {
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
}

impl FtpServer {
    /// Try building an FTP server with the options specified in the `config`.
    ///
    /// # Return
    /// If no error occured it will return the `FtpServer`, otherwise the error.
    pub fn build() -> Result<FtpServer, Box<dyn Error>> {
        let listener = TcpListener::bind(format!(
            "{}:{}",
            CONFIG.get().unwrap().get_hostname(),
            CONFIG.get().unwrap().get_port()
        ))?;

        Ok(FtpServer {
            listener,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start the FTP server based on the options stated in the `config`.
    pub fn start(&mut self) -> Result<(), String> {
        // On impose une limite de threads pour éviter une faille dans laquelle énormément de
        // threads sont crées pour saturer la mémoire du serveur.
        // TODO: Définir le nombre de threads dans la config au lieu d'écrire en dur.
        let pool = ThreadPool::build(10)?;

        let server_shutdown = Arc::clone(&self.shutdown);
        let server = match self.listener.try_clone() {
            Ok(s) => s,
            Err(err) => {
                return Err(format!("cannot clone server listener: {err}"));
            }
        };

        // Thread du serveur qui s'occupe d'accepter et traiter les requêtes clients.
        let server_thread = thread::spawn(move || {
            // Boucle qui récupère un client à chaque demande de connexion,
            // la boucle s'arrête quand le serveur est coupé.
            for client in server.incoming() {
                if server_shutdown.load(Ordering::Relaxed) {
                    return;
                }

                // S'assure qu'aucune erreur n'est survenue pendant la connexion avec le client.
                // Utiliser 'match' permet de dé-structurer le résultat.
                let stream = match client {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("Error establishing connection: {err}.");
                        continue;
                    }
                };

                pool.execute(|| {
                    handle_connection(stream).unwrap_or_else(|err| {
                        eprintln!("Error occured when handling connection: {err}.")
                    })
                });
            }
        });

        server_thread.join().unwrap();

        Ok(())
    }

    pub fn get_shutdown_rc(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown)
    }
}

/// Function called just after a client has been connected into the server.
fn handle_connection(stream: TcpStream) -> Result<(), String> {
    let mut ftp_client = match FtpClient::build(stream) {
        Ok(client) => client,
        Err(err) => return Err(err.to_string()),
    };

    // Initialise la connexion.
    // Souvent appelé 'Greetings' ou 'Welcome message'.
    println!("Sending greetings...");
    match ftp_client.write(b"220 ready\r\n") {
        Ok(_) => (),
        Err(err) => return Err(err.to_string()),
    }

    // Boucle qui reçoit les requêtes de contrôles du client jusqu'à ce que la connexion soit
    // interrompu.
    loop {
        let request = match ftp_client.read_line() {
            Ok(line) => {
                if !line.is_empty() {
                    line
                } else {
                    return Err("EOF reached".to_string());
                }
            }
            Err(err) => {
                return Err(format!("cannot read client request: {err}"));
            }
        };

        println!("Request: {request}");

        let args = request.split(' ');
        let args: Vec<String> = args.map(|arg| arg.to_string()).collect();

        let mut it_args = args.into_iter();

        // Le protocole indique que la requête est insensible à la casse.
        // Donc pour simplifier le traitement, met la valeur en majuscule.
        let command = it_args.next().unwrap().to_uppercase();

        let (code, message);
        let mut multilines = false;

        match &command[..] {
            "USER" => match ftp_client.exec_user_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;

                    let options = ftp_client.get_options();
                    let opt = RefCell::borrow(&options);

                    match &opt.session {
                        Some(sess) => {
                            println!("Session changed: {:?}", sess);
                        }
                        None => (),
                    }
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            /*
            "PASS" => match ftp_client.exec_pass_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);

                    success = false;
                }
            },
            */
            "SYST" => match ftp_client.exec_syst_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "FEAT" => match ftp_client.exec_feat_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "OPTS" => match ftp_client.exec_opts_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "PWD" => match ftp_client.exec_pwd_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "TYPE" => match ftp_client.exec_type_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;

                    let options = ftp_client.get_options();
                    let opt = RefCell::borrow(&options);

                    println!("Data type changed: {:?}", opt.data_representation);
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "PASV" => match ftp_client.exec_pasv_command(Box::new(it_args)) {
                Ok((c, m, l, listener)) => {
                    (code, message) = (c, m);
                    multilines = l;

                    // Normalement il n'est pas censé avoir une autre variant de cette énum.
                    if let CommandReturnType::TcpListener(ls) = listener {
                        let mut data_listener = RefCell::borrow_mut(&ftp_client.data_listener);
                        *data_listener = Some(ls);
                    }
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "LIST" => match ftp_client.exec_list_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "CWD" => match ftp_client.exec_cwd_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            "CDUP" => match ftp_client.exec_cdup_command(Box::new(it_args)) {
                Ok((c, m, l, _)) => {
                    (code, message) = (c, m);
                    multilines = l;
                }
                Err((c, m)) => {
                    (code, message) = (c, m);
                }
            },
            _ => {
                (code, message) = (502, "no implementation".to_string());
            }
        }

        let reply = match multilines {
            true => format!("{code}-{message}\r\n{code} End\r\n"),
            false => format!("{code} {message}\r\n"),
        };

        // Envoie la réponse de contrôle finale au client.
        match ftp_client.write(reply.as_bytes()) {
            Ok(_) => (),
            Err(err) => eprintln!("Error when sending reply: {err}."),
        }
    }
}
