error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    errors {
        Connection(msg: String) {
            description("Connection error")
            display("Connection error: {}", msg)
        }

        RpcError(code: i64, error: String, method: String) {
            description("RPC error")
            display("{} RPC error {}: {}", method, code, error)
        }

        Interrupt(sig: i32) {
            description("Interruption by external signal")
            display("Iterrupted by signal {}", sig)
        }

        TooPopular {
            description("Too many history entries")
            display("Too many history entries")
        }

        #[cfg(feature = "electrum-discovery")]
        ElectrumClient(e: electrum_client::Error) {
            description("Electrum client error")
            display("Electrum client error: {:?}", e)
        }

    }
}

#[cfg(feature = "electrum-discovery")]
impl From<electrum_client::Error> for Error {
    fn from(e: electrum_client::Error) -> Self {
        Error::from(ErrorKind::ElectrumClient(e))
    }
}
