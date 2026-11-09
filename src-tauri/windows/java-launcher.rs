use std::{
    env,
    ffi::{c_char, c_void, CStr},
    io,
    os::windows::ffi::OsStrExt,
    ptr,
};

type Module = *mut c_void;
const CP_UTF8: u32 = 65001;
const WC_ERR_INVALID_CHARS: u32 = 0x80;
const WC_NO_BEST_FIT_CHARS: u32 = 0x400;
const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x100;
const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x1000;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> Module;
    fn GetProcAddress(module: Module, name: *const c_char) -> *mut c_void;
    fn GetCommandLineW() -> *const u16;
    fn GetACP() -> u32;
    fn WideCharToMultiByte(
        code_page: u32,
        flags: u32,
        wide: *const u16,
        wide_length: i32,
        narrow: *mut c_char,
        narrow_length: i32,
        default_char: *const c_char,
        used_default: *mut i32,
    ) -> i32;
}

#[repr(C)]
struct JavaArgument {
    value: *mut c_char,
    has_wildcard: u8,
}

type Launch = unsafe extern "system" fn(
    i32,
    *mut *mut c_char,
    i32,
    *const *const c_char,
    i32,
    *const *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    u8,
    u8,
    u8,
    i32,
) -> i32;

unsafe fn symbol<T: Copy>(module: Module, name: &CStr) -> io::Result<T> {
    let address = GetProcAddress(module, name.as_ptr());
    if address.is_null() {
        return Err(io::Error::last_os_error());
    }
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&address));
    Ok(std::mem::transmute_copy(&address))
}

unsafe fn command_line() -> io::Result<Vec<u8>> {
    let wide = GetCommandLineW();
    let page = GetACP();
    let flags = if page == CP_UTF8 {
        WC_ERR_INVALID_CHARS
    } else {
        WC_NO_BEST_FIT_CHARS
    };
    let length = WideCharToMultiByte(
        page,
        flags,
        wide,
        -1,
        ptr::null_mut(),
        0,
        ptr::null(),
        ptr::null_mut(),
    );
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut line = vec![0u8; length as usize];
    let mut substituted = 0;
    let used_default = if page == CP_UTF8 {
        ptr::null_mut()
    } else {
        &mut substituted
    };
    let written = WideCharToMultiByte(
        page,
        flags,
        wide,
        -1,
        line.as_mut_ptr().cast(),
        length,
        ptr::null(),
        used_default,
    );
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    if substituted != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Java arguments are not supported by the Windows code page",
        ));
    }
    Ok(line)
}

fn run() -> io::Result<i32> {
    let path = env::current_exe()?.with_file_name("jli.dll");
    let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    // The JDK and its launcher DLL are siblings in the verified runtime archive.
    unsafe {
        let library = LoadLibraryExW(
            path.as_ptr(),
            ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        );
        if library.is_null() {
            return Err(io::Error::last_os_error());
        }
        let init: unsafe extern "system" fn(u8, u8) = symbol(library, c"JLI_InitArgProcessing")?;
        let parse: unsafe extern "system" fn(*mut c_char) = symbol(library, c"JLI_CmdToArgs")?;
        let count: unsafe extern "system" fn() -> i32 = symbol(library, c"JLI_GetStdArgc")?;
        let arguments: unsafe extern "system" fn() -> *const JavaArgument =
            symbol(library, c"JLI_GetStdArgs")?;
        let launch: Launch = symbol(library, c"JLI_Launch")?;
        let mut line = command_line()?;
        init(0, 0);
        parse(line.as_mut_ptr().cast());
        let count = count();
        let arguments = arguments();
        if count <= 0 || arguments.is_null() {
            return Err(io::Error::other("Java launcher arguments are missing"));
        }
        let mut argv: Vec<_> = std::slice::from_raw_parts(arguments, count as usize)
            .iter()
            .map(|argument| argument.value)
            .collect();
        argv.push(ptr::null_mut());
        Ok(launch(
            count,
            argv.as_mut_ptr(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            c"25".as_ptr(),
            c"25".as_ptr(),
            c"java".as_ptr(),
            c"java".as_ptr(),
            0,
            1,
            0,
            0,
        ))
    }
}

fn main() {
    match run() {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            eprintln!("Could not start Java: {error}");
            std::process::exit(1);
        }
    }
}
