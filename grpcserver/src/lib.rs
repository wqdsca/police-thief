pub mod controller;
pub mod service;
pub mod server;
pub mod room { tonic::include_proto!("room"); }
pub mod user { tonic::include_proto!("user"); }


pub use controller::*;
pub use service::*;
pub use server::*;

