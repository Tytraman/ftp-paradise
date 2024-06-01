use std::{cell::RefCell, net::TcpListener, rc::Rc};

use crate::options::ClientOptions;

pub enum CommandReturnType {
    None,
    Bool(bool),
    String(String),
    TcpListener(TcpListener),
}

pub type CommandResult = Result<(i32, String, bool, CommandReturnType), (i32, String)>;

pub type CommandJob =
    Box<dyn Fn(Rc<RefCell<ClientOptions>>, Box<dyn Iterator<Item = String>>) -> CommandResult>;

pub type DataCommandJob = Box<
    dyn Fn(Rc<RefCell<Option<TcpListener>>>, Box<dyn Iterator<Item = String>>) -> CommandResult,
>;
