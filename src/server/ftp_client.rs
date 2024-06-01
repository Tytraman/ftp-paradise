use std::{
    cell::RefCell,
    error::Error,
    ffi::CStr,
    fs,
    io::{self, BufRead, BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    rc::Rc,
};

// Indique que la ligne du dessous ne sera incluse que sur des plateformes 'Linux'.
#[cfg(target_os = "linux")]
use std::os::{linux::fs::MetadataExt as _, unix::fs::MetadataExt};

use chrono::{DateTime, Local};

use crate::{
    commands::{CommandResult, CommandReturnType},
    options::{
        data_representation::DataType, listen_mode::ListenMode, session::SessionInformations,
        ClientOptions,
    },
    CONFIG,
};

pub struct FtpClient {
    stream_writer: TcpStream,
    stream_reader: BufReader<TcpStream>,
    // TODO: Se renseigner sur comment utiliser une référence au lieu d'un RC.
    options: Rc<RefCell<ClientOptions>>,
    pub data_listener: Rc<RefCell<Option<TcpListener>>>,
}

impl FtpClient {
    pub fn build(stream: TcpStream) -> Result<FtpClient, Box<dyn Error>> {
        // 'stream_writer' permet d'écrire dans le stream du client.
        // 'try_clone' fait une copie de la référence vers le stream.
        //
        // Lors de l'envoi des réponses avec 'BufWriter', probablement à cause du fonctionnement avec
        // un buffer interne, les clients ne recevaient pas la réponse.
        //
        // Donc je passe directement par le stream lui-même pour éviter les problèmes de buffers.
        let stream_copy = stream.try_clone()?;

        Ok(FtpClient {
            stream_writer: stream,
            // Afin de faciliter la lecture des requêtes, 'BufReader' est utilisée pour lire des lignes
            // complètes vu que le protocole FTP utilise le même format de requêtes que Telnet.
            // A savoir des lignes finissant par <CRLF>.
            //
            // Il y a cependant un risque en utilisant un 'BufReader' et en lisant une ligne :
            // si le client envoie une chaîne de caractères extrêmement longue sans <CRLF>, il se peut
            // qu'il sature la mémoire du serveur.
            // TODO: Se renseigner sur ce potentiel problème.
            stream_reader: BufReader::new(stream_copy),
            options: Rc::new(RefCell::new(ClientOptions {
                session: None,
                working_directory: "/".to_string(),
                // Le protocole indique que le type par défaut est ASCII.
                data_representation: DataType::ASCII,
                local_bytes: 0,
                listen_mode: ListenMode::Active,
            })),
            data_listener: Rc::new(RefCell::new(None)),
        })
    }

    pub fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream_writer.write(buffer)
    }

    pub fn read_line(&mut self) -> Result<String, String> {
        let mut line = String::new();

        match self.stream_reader.read_line(&mut line) {
            Ok(_) => Ok(line.trim().to_string()),
            Err(err) => Err(err.to_string()),
        }
    }

    /// Execute the FTP command USER.
    pub fn exec_user_command(&self, args: Box<dyn Iterator<Item = String>>) -> CommandResult {
        let mut username = String::new();
        let options = self.get_options();

        // Récupère tous les arguments pour en faire un nom d'utilisateur.
        args.for_each(|arg| username.push_str(&format!("{arg} ")));
        username = username.trim().to_string();

        let session = SessionInformations::new(username, None);

        let mut opt = RefCell::borrow_mut(&options);
        opt.session = Some(session);

        Ok((
            230,
            "user connected".to_string(),
            false,
            CommandReturnType::None,
        ))
    }

    /// Execute the FTP command SYST.
    pub fn exec_syst_command(&self, _: Box<dyn Iterator<Item = String>>) -> CommandResult {
        Ok((
            215,
            "UNIX Type: L8".to_string(),
            false,
            CommandReturnType::None,
        ))
    }

    /// Execute the FTP command FEAT.
    pub fn exec_feat_command(&self, _: Box<dyn Iterator<Item = String>>) -> CommandResult {
        Ok((
            211,
            "-Features\r\nUTF8".to_string(),
            true,
            CommandReturnType::None,
        ))
    }

    /// Execute the FTP command OPTS.
    pub fn exec_opts_command(&self, mut args: Box<dyn Iterator<Item = String>>) -> CommandResult {
        let arg = match args.next() {
            Some(a) => a,
            None => return Err((501, "Syntax error in arguments".to_string())),
        };

        match &arg[..] {
            "UTF8" => {
                return Ok((
                    202,
                    "UTF8 mode is always ON".to_string(),
                    false,
                    CommandReturnType::None,
                ))
            }
            _ => return Err((504, "command not implemented for this option".to_string())),
        }
    }

    /// Execute the FTP command PWD.
    pub fn exec_pwd_command(&self, _: Box<dyn Iterator<Item = String>>) -> CommandResult {
        let options = self.get_options();

        let options = RefCell::borrow(&options);

        Ok((
            257,
            format!("\"{}\"", options.working_directory),
            false,
            CommandReturnType::None,
        ))
    }

    /// Execute the FTP command TYPE.
    pub fn exec_type_command(&self, mut args: Box<dyn Iterator<Item = String>>) -> CommandResult {
        let options = self.get_options();

        let typee = match args.next() {
            Some(t) => t,
            None => return Err((501, "Syntax error in arguments".to_string())),
        };

        let mut options = RefCell::borrow_mut(&options);

        match &typee[..] {
            "A" => options.data_representation = DataType::ASCII,
            "E" => options.data_representation = DataType::EBCDIC,
            "I" => options.data_representation = DataType::Image,
            "L" => {
                if let Some(byte_size) = args.next() {
                    options.data_representation = DataType::Local;
                    options.local_bytes = match byte_size.parse() {
                        Ok(size) => size,
                        Err(_) => return Err((501, "Syntax error in arguments".to_string())),
                    }
                } else {
                    return Err((501, "Syntax error in arguments".to_string()));
                }
            }
            _ => return Err((504, "command not implemented for this option".to_string())),
        }

        Ok((
            200,
            "command OK".to_string(),
            false,
            CommandReturnType::None,
        ))
    }

    /// Execute the FTP command PASV.
    pub fn exec_pasv_command(&self, _: Box<dyn Iterator<Item = String>>) -> CommandResult {
        let options = self.get_options();
        let mut options = RefCell::borrow_mut(&options);

        // TODO: Pour le moment cela ne fonctionne que dans un réseau local, faire en sorte que cela
        // fonctionne aussi avec l'adresse IP publique.
        let hostname = CONFIG.get().unwrap().get_hostname();

        options.listen_mode = ListenMode::Passive;

        let mut port = 0;

        let mut data_listener = None;

        for p in 7000..65535 {
            if let Ok(listener) = TcpListener::bind(format!("{hostname}:{p}")) {
                data_listener = Some(listener);

                port = p;

                break;
            }
        }

        if data_listener.is_some() {
            let p1 = port / 256;
            let p2 = port - (p1 * 256);

            Ok((
                227,
                format!(
                    "Entering passive mode ({},{p1},{p2})",
                    hostname.replace(".", ","),
                ),
                false,
                CommandReturnType::TcpListener(data_listener.unwrap()),
            ))
        } else {
            Err((425, "cannot open data connection".to_string()))
        }
    }

    /// Execute the FTP command LIST.
    pub fn exec_list_command(&mut self, _args: Box<dyn Iterator<Item = String>>) -> CommandResult {
        // TODO: Gérer les arguments de la commande LIST.
        let data_listener = Rc::clone(&self.data_listener);
        let data_listener = RefCell::borrow_mut(&data_listener);
        let data_listener = data_listener.as_ref().unwrap();

        let options = self.get_options();

        let options = RefCell::borrow_mut(&options);

        let pwd = &options.working_directory;

        let paths = match fs::read_dir(pwd) {
            Ok(p) => p,
            Err(_) => return Err((550, "cannot access directory".to_string())),
        };

        let _ = self.write("150 ok\r\n".as_bytes());

        let connection = match data_listener.accept() {
            Ok((stream, _)) => stream,
            Err(_) => return Err((425, "cannot open data connection".to_string())),
        };

        let mut writer = BufWriter::new(&connection);

        // Itère à travers le dossier pour envoyer au client la liste des fichiers / dossiers
        // présents.
        for path in paths {
            match path {
                Ok(entry) => {
                    let absolute_path = entry.path();

                    if let Ok(path) = entry.file_name().into_string() {
                        if let Ok(metadata) = fs::metadata(absolute_path) {
                            if let Ok(modified) = metadata.modified() {
                                let date_time: DateTime<Local> = modified.into();

                                let perms = metadata.mode();

                                let user_read = if (perms & 0o400) > 0 { 'r' } else { '-' };
                                let user_write = if (perms & 0o200) > 0 { 'w' } else { '-' };
                                let user_execute = if (perms & 0o100) > 0 { 'x' } else { '-' };

                                let group_read = if (perms & 0o40) > 0 { 'r' } else { '-' };
                                let group_write = if (perms & 0o20) > 0 { 'w' } else { '-' };
                                let group_execute = if (perms & 0o10) > 0 { 'x' } else { '-' };

                                let others_read = if (perms & 0o4) > 0 { 'r' } else { '-' };
                                let others_write = if (perms & 0o2) > 0 { 'w' } else { '-' };
                                let others_execute = if (perms & 0o1) > 0 { 'x' } else { '-' };

                                // Récupère le nom d'utilisateur et le nom du groupe auquel le fichier
                                // appartient.
                                let (username, group) = unsafe {
                                    // TODO: Faire une structure cross-plateforme pour récupérer ces infos.
                                    let passwd = libc::getpwuid(metadata.st_uid());
                                    let grp = libc::getgrgid(metadata.st_gid());

                                    (
                                        CStr::from_ptr((*passwd).pw_name).to_str().unwrap(),
                                        CStr::from_ptr((*grp).gr_name).to_str().unwrap(),
                                    )
                                };

                                let response = format!("{}{user_read}{user_write}{user_execute}{group_read}{group_write}{group_execute}{others_read}{others_write}{others_execute} {username} {group} {} {:>5} {path}\r\n",
                                    if metadata.is_dir() { "d" } else { "-" },
                                    metadata.len(),
                                    date_time.format("%b %d %H:%M")
                                );

                                // Envoie au client la ligne contenant les informations du fichiers.
                                match writer.write(response.as_bytes()) {
                                    Ok(_) => (),
                                    Err(err) => {
                                        eprintln!("Error when writting to data connection: {err}.")
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => (),
            }
        }

        let _ = self.write("226 closing data connection\r\n".as_bytes());

        Ok((250, "ok".to_string(), false, CommandReturnType::None))
    }

    pub fn exec_cwd_command(
        &mut self,
        mut args: Box<dyn Iterator<Item = String>>,
    ) -> CommandResult {
        let mut path = match args.next() {
            Some(p) => p,
            None => return Err((501, "missing pathname".to_string())),
        };

        let options = self.get_options();
        let mut options = RefCell::borrow_mut(&options);

        // Si le client n'envoie pas de chemin absolu, alors il faut partir du dossier actuel.
        if !path.starts_with("/") {
            let wd = options.working_directory.trim_end_matches("/").to_string();

            // Si le client veut aller dans le dossier parent.
            if path == ".." {
                path = match wd.rfind("/") {
                    Some(idx) => {
                        if idx > 0 {
                            wd[..idx].to_string()
                        } else {
                            "/".to_string()
                        }
                    }
                    None => "/".to_string(),
                }
            } else {
                path.insert_str(0, &format!("{wd}/"));

                path = path.trim_end_matches("/").to_string();
            }
        }

        let folder = Path::new(&path);

        match folder.try_exists() {
            Ok(res) => {
                if !res {
                    return Err((550, format!("{path} inexistant path")));
                }
            }
            Err(_) => return Err((450, "error".to_string())),
        }

        options.working_directory = path;

        Ok((250, "ok".to_string(), false, CommandReturnType::None))
    }

    pub fn exec_cdup_command(&mut self, _: Box<dyn Iterator<Item = String>>) -> CommandResult {
        let args = vec!["..".to_string()];

        self.exec_cwd_command(Box::new(args.into_iter()))
    }

    pub fn get_options(&self) -> Rc<RefCell<ClientOptions>> {
        Rc::clone(&self.options)
    }

    pub fn set_session(&mut self, session: SessionInformations) {
        let options = Rc::clone(&self.options);

        let mut opt = RefCell::borrow_mut(&options);

        opt.session = Some(session);
    }
}
