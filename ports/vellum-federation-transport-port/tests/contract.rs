use vellum_federation_transport_port::FederationTransportLifecyclePortV1;

struct Transport {
    closed: bool,
}

impl FederationTransportLifecyclePortV1 for Transport {
    type Error = ();

    fn close(&mut self) -> Result<(), Self::Error> {
        self.closed = true;
        Ok(())
    }
}

#[test]
fn transport_lifecycle_port_carries_only_close_completion() {
    let mut transport = Transport { closed: false };
    assert_eq!(transport.close(), Ok(()));
    assert!(transport.closed);
}
