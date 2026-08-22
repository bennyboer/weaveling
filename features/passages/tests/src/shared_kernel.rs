#[test]
fn the_server_and_the_client_agree_on_the_fragment_name() {
    assert_eq!(
        passages_core::FRAGMENT,
        passages_contract::FRAGMENT,
        "the server projects one XmlFragment and the client writes another, \
         so prose would silently vanish"
    );
}
