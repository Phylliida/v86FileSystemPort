
// TODO: STATUS_ON_STORAGE STUFF (local filestorage? can't in pure wasm)
// TODO: clone_me rename to clone once I'm done with porting (For Uint8Array)
// TODO: Is it ok to clone FSLockRegion vec?

mod filesystem;
mod marshall;
mod v9p;
mod wasi;


/*

use crate::v9p::Virtio9p;
use crate::wasi::{set_panic_hook, wasi_print_internal};
use crate::wasi::Pipe::Stdout;

#[no_mangle]
pub extern "C" fn main() {
    set_panic_hook();
    /*
    wasi_print!("hi!");
    let mut fs = Virtio9p::new(None);
    let path = "bees.bepis";
    let beesptr = fs.fs.create_text_file(path, fs.fs.root_id, "applebeeeeeees");
    wasi_print!("got file {}", beesptr);
    if let Some(result_text) = fs.fs.read_text_file(path) {
        wasi_print!("got text {}", result_text);
    }
    else {
        wasi_print!("got no text");
    }

    let directory = fs.fs.create_directory("applebees wow", Some(fs.fs.root_id));
    let beesptr2 = fs.fs.create_text_file(path, directory, "applebeeeeeees2");
    wasi_print!("got file 2 {} with full path {}", beesptr2, fs.fs.get_full_path(directory));
    if let Some(result_text) = fs.fs.read_text_file("/applebees wow/bees.bepis") {
        wasi_print!("got text 2 {}", result_text);
    }
    else {
        println!("got no text 2");
    }
    */
} 
*/
