use weechat::{weechat_test, Weechat};

#[weechat_test]
fn get_home(weechat: &Weechat) {
    let home_dir = Weechat::home_dir();
    assert!(false, "HELLO WORLD");
}
