use std::sync::atomic::AtomicUsize;

lazy_static! {
    [pub] static ref TEST_INT: AtomicUsize = AtomicUsize::new(0);
}

#[test]
fn fail() {
    panic!("FAILFIALFAILFAILFAILFAILFIAL");
}
