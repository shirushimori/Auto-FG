use fitgirl_auto_downloader_mori::{choose_input_mode, clear_screen, write_input_file};

fn main() {
    clear_screen();
    let links = choose_input_mode();
    write_input_file(&links);
}
