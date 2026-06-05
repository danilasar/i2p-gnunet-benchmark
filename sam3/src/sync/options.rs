#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionOptions {
    inbound_length: Option<u8>,
    outbound_length: Option<u8>,
    inbound_length_variance: Option<i8>,
    outbound_length_variance: Option<i8>,
    inbound_quantity: Option<u8>,
    outbound_quantity: Option<u8>,
    inbound_backup_quantity: Option<u8>,
    outbound_backup_quantity: Option<u8>,
}

impl SessionOptions {
    pub fn inbound_length(mut self, n: u8) -> Self {
        self.inbound_length = Some(n);
        self
    }

    pub fn outbound_length(mut self, n: u8) -> Self {
        self.outbound_length = Some(n);
        self
    }

    pub fn inbound_length_variance(mut self, n: i8) -> Self {
        self.inbound_length_variance = Some(n);
        self
    }

    pub fn outbound_length_variance(mut self, n: i8) -> Self {
        self.outbound_length_variance = Some(n);
        self
    }

    pub fn inbound_quantity(mut self, n: u8) -> Self {
        self.inbound_quantity = Some(n);
        self
    }

    pub fn outbound_quantity(mut self, n: u8) -> Self {
        self.outbound_quantity = Some(n);
        self
    }

    pub fn inbound_backup_quantity(mut self, n: u8) -> Self {
        self.inbound_backup_quantity = Some(n);
        self
    }

    pub fn outbound_backup_quantity(mut self, n: u8) -> Self {
        self.outbound_backup_quantity = Some(n);
        self
    }

    pub fn zero_hop() -> Self {
        Self::default()
            .inbound_length(0)
            .outbound_length(0)
            .inbound_length_variance(0)
            .outbound_length_variance(0)
            .inbound_quantity(2)
            .outbound_quantity(2)
            .inbound_backup_quantity(0)
            .outbound_backup_quantity(0)
    }

    pub fn to_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(v) = self.inbound_length {
            pairs.push(("inbound.length".to_string(), v.to_string()));
        }
        if let Some(v) = self.outbound_length {
            pairs.push(("outbound.length".to_string(), v.to_string()));
        }
        if let Some(v) = self.inbound_length_variance {
            pairs.push(("inbound.lengthVariance".to_string(), v.to_string()));
        }
        if let Some(v) = self.outbound_length_variance {
            pairs.push(("outbound.lengthVariance".to_string(), v.to_string()));
        }
        if let Some(v) = self.inbound_quantity {
            pairs.push(("inbound.quantity".to_string(), v.to_string()));
        }
        if let Some(v) = self.outbound_quantity {
            pairs.push(("outbound.quantity".to_string(), v.to_string()));
        }
        if let Some(v) = self.inbound_backup_quantity {
            pairs.push(("inbound.backupQuantity".to_string(), v.to_string()));
        }
        if let Some(v) = self.outbound_backup_quantity {
            pairs.push(("outbound.backupQuantity".to_string(), v.to_string()));
        }
        pairs
    }

    pub fn to_opts_str(&self) -> String {
        self.to_pairs()
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamConnectOptions {
    pub from_port: u16,
    pub to_port: u16,
    pub silent: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RawSessionOptions {
    pub protocol: Option<u8>,
    pub header: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_opts_str_empty() {
        assert_eq!(SessionOptions::default().to_opts_str(), "");
    }

    #[test]
    fn to_opts_str_zero_hop() {
        let s = SessionOptions::zero_hop().to_opts_str();
        assert!(s.contains("inbound.length=0"));
        assert!(s.contains("outbound.length=0"));
    }

    #[test]
    fn stream_connect_options_default() {
        let opts = StreamConnectOptions::default();
        assert_eq!(opts.from_port, 0);
        assert_eq!(opts.to_port, 0);
        assert!(!opts.silent);
    }

    #[test]
    fn raw_session_options_default() {
        let opts = RawSessionOptions::default();
        assert!(opts.protocol.is_none());
        assert!(!opts.header);
    }
}
