use std::{env, error::Error, process};

use ftp_paradise::config::Config;

static VERSION: &str = "0.0.1";

fn main() -> Result<(), Box<dyn Error>> {
    // Récupère la configuration de l'application depuis la ligne de commande passée.
    let config = parse_args(env::args()).unwrap_or_else(|err| {
        eprintln!("Error parsing arguments: {err}.");

        process::exit(1);
    });

    // Vérifie que la configuration est valide.
    if let Err(err) = config.check() {
        eprintln!("Error in config: {err}.");

        process::exit(1);
    }

    // Démarre l'exécution de l'application.
    if let Err(err) = ftp_paradise::run(config) {
        eprintln!("Application error: {err}.");

        process::exit(1);
    }

    Ok(())
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Config, &'static str> {
    // Saute le 1er argument car c'est normalement le nom du programme.
    args.next();

    let mut hostname = String::new();
    let mut port = String::new();

    while let Some(arg) = args.next() {
        match &arg[..] {
            // Récupère l'adresse à utiliser pour héberger le serveur.
            "--hostname" | "-h" => {
                // Il faut qu'il y ai un argument après celui-ci qui contient l'adresse en
                // question.
                if let Some(h) = args.next() {
                    hostname = h;
                } else {
                    // S'il n'y a pas d'adresse spécifiée mais qu'une adresse avait déjà été
                    // spécifiée auparavant, alors il n'y a pas d'erreur.
                    if hostname.is_empty() {
                        return Err("no hostname specified after --hostname argument");
                    }
                }
            }
            // Récupère le port à utiliser pour héberger le serveur.
            "--port" | "-p" => {
                // Il faut qu'il y ai un argument après celui-ci qui contient le port en question.
                if let Some(p) = args.next() {
                    port = p;
                } else {
                    // S'il n'y a pas de port spécifié mais qu'un port avait déjà été spécifié
                    // auparavant, alors il n'y a pas d'erreur.
                    if port.is_empty() {
                        return Err("no port number specified after --port argument");
                    }
                }
            }
            "--version" | "-v" => {
                eprintln!("FTP Paradise v{VERSION}");
                process::exit(0);
            }
            _ => (),
        }
    }

    if hostname.is_empty() {
        return Err("no hostname specified");
    }

    if port.is_empty() {
        return Err("no port specified");
    }

    Ok(Config::new(hostname, port))
}
