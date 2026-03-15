fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon/ibkr_porez.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
