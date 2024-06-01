#[derive(Clone)]
pub struct Config {
    hostname: String,
    port: String,
}

impl Config {
    pub fn new(hostname: String, port: String) -> Config {
        Config { hostname, port }
    }

    pub fn check(&self) -> Result<(), &'static str> {
        // Vérifie que l'adresse soit bien une adresse IP valide.
        let host: Vec<_> = self.hostname.split(".").collect();

        if host.len() != 4 {
            return Err("invalid number of decimal in hostname");
        }

        for dec in host {
            if let Err(_) = dec.parse::<i32>() {
                return Err("invalid format in hostname");
            }
        }

        // Vérifie que le port soit dans le bon format.
        if let Err(_) = self.port.parse::<i32>() {
            return Err("invalid port format");
        }

        Ok(())
    }

    pub fn get_hostname(&self) -> String {
        self.hostname.clone()
    }

    pub fn get_port(&self) -> String {
        self.port.clone()
    }
}
