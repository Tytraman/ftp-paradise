#[derive(Debug)]
pub struct SessionInformations {
    username: String,
    password: Option<String>,
}

impl SessionInformations {
    pub fn new(username: String, password: Option<String>) -> SessionInformations {
        SessionInformations { username, password }
    }

    pub fn get_username(&self) -> &str {
        &self.username
    }

    pub fn get_password(&self) -> Option<&str> {
        match &self.password {
            Some(pass) => Some(&pass),
            None => None,
        }
    }

    pub fn set_password(&mut self, password: String) {
        self.password = Some(password);
    }
}
