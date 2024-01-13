use std::net::IpAddr;

pub trait IpAddrOctets {
    fn octets(&self) -> Vec<u8>;
}

impl IpAddrOctets for IpAddr {
    fn octets(&self) -> Vec<u8> {
        match self {
            IpAddr::V4(a) => a.octets().to_vec(),
            IpAddr::V6(a) => a.octets().to_vec()
        }
    }
}
