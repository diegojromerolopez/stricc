use std::collections::HashMap;
use std::sync::{Mutex, LazyLock, OnceLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::os::raw::{c_char, c_void};
use std::ffi::CStr;

// Pointer Metadata: (base, size, key)
type Metadata = (usize, usize, u64);

// Global Shadow Table: maps address of a pointer variable in memory -> Metadata of that pointer
static SHADOW_TABLE: LazyLock<Mutex<HashMap<usize, Metadata>>> = LazyLock::new(|| {
    Mutex::new(HashMap::new())
});

// Version Key tracking: maps allocation base address -> allocation version key
static KEY_TABLE: LazyLock<Mutex<HashMap<usize, u64>>> = LazyLock::new(|| {
    Mutex::new(HashMap::new())
});

// Next unique temporal key generator
static NEXT_KEY: LazyLock<Mutex<u64>> = LazyLock::new(|| {
    Mutex::new(1)
});

// Dynamic linking lookup for real system allocator functions to prevent recursion
extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}
const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;

static REAL_MALLOC: OnceLock<unsafe extern "C" fn(usize) -> *mut c_void> = OnceLock::new();
static REAL_FREE: OnceLock<unsafe extern "C" fn(*mut c_void)> = OnceLock::new();
static REAL_REALLOC: OnceLock<unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void> = OnceLock::new();

unsafe fn init_real_functions() {
    let malloc_sym = dlsym(RTLD_NEXT, b"malloc\0".as_ptr() as *const c_char);
    let free_sym = dlsym(RTLD_NEXT, b"free\0".as_ptr() as *const c_char);
    let realloc_sym = dlsym(RTLD_NEXT, b"realloc\0".as_ptr() as *const c_char);
    assert!(!malloc_sym.is_null(), "Failed to find real malloc via dlsym");
    assert!(!free_sym.is_null(), "Failed to find real free via dlsym");
    assert!(!realloc_sym.is_null(), "Failed to find real realloc via dlsym");
    let _ = REAL_MALLOC.set(std::mem::transmute(malloc_sym));
    let _ = REAL_FREE.set(std::mem::transmute(free_sym));
    let _ = REAL_REALLOC.set(std::mem::transmute(realloc_sym));
}

static TLS_KEY: AtomicUsize = AtomicUsize::new(usize::MAX);

unsafe fn get_in_malloc() -> bool {
    let key = TLS_KEY.load(Ordering::Relaxed);
    if key == usize::MAX {
        return false;
    }
    let ptr = libc::pthread_getspecific(key as libc::pthread_key_t);
    !ptr.is_null()
}

unsafe fn set_in_malloc(val: bool) {
    let mut key = TLS_KEY.load(Ordering::Relaxed);
    if key == usize::MAX {
        let mut new_key: libc::pthread_key_t = 0;
        let r = libc::pthread_key_create(&mut new_key, None);
        assert_eq!(r, 0, "Failed to create pthread key");
        match TLS_KEY.compare_exchange(
            usize::MAX,
            new_key as usize,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                key = new_key as usize;
            }
            Err(existing) => {
                libc::pthread_key_delete(new_key);
                key = existing;
            }
        }
    }
    let ptr = if val { 1 as *mut c_void } else { std::ptr::null_mut() };
    libc::pthread_setspecific(key as libc::pthread_key_t, ptr);
}

// Static bootstrap buffer for allocations during dlsym bootstrap
static mut BOOTSTRAP_BUFFER: [u8; 131072] = [0; 131072]; // 128 KB
static mut BOOTSTRAP_INDEX: usize = 0;

unsafe fn bootstrap_malloc(size: usize) -> *mut c_void {
    let align = 16;
    let offset = (BOOTSTRAP_INDEX + align - 1) & !(align - 1);
    if offset + size > BOOTSTRAP_BUFFER.len() {
        return std::ptr::null_mut();
    }
    BOOTSTRAP_INDEX = offset + size;
    BOOTSTRAP_BUFFER.as_mut_ptr().add(offset) as *mut c_void
}

unsafe fn is_bootstrap_ptr(ptr: *mut c_void) -> bool {
    let addr = ptr as usize;
    let buf_start = BOOTSTRAP_BUFFER.as_ptr() as usize;
    let buf_end = buf_start + BOOTSTRAP_BUFFER.len();
    addr >= buf_start && addr < buf_end
}

unsafe fn bootstrap_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return bootstrap_malloc(size);
    }
    if !is_bootstrap_ptr(ptr) {
        return std::ptr::null_mut();
    }
    let new_ptr = bootstrap_malloc(size);
    if !new_ptr.is_null() {
        let buf_start = BOOTSTRAP_BUFFER.as_ptr() as usize;
        let ptr_offset = ptr as usize - buf_start;
        let max_old_size = BOOTSTRAP_INDEX - ptr_offset;
        let copy_size = std::cmp::min(max_old_size, size);
        std::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
    }
    new_ptr
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_shadow_store(
    ptr_addr: *mut c_void,
    base: *mut c_void,
    size: usize,
    key: u64,
) {
    let mut table = SHADOW_TABLE.lock().unwrap();
    table.insert(ptr_addr as usize, (base as usize, size, key));
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_shadow_load(
    ptr_addr: *mut c_void,
    base_out: *mut *mut c_void,
    size_out: *mut usize,
    key_out: *mut u64,
) {
    let table = SHADOW_TABLE.lock().unwrap();
    if let Some(&(base, size, key)) = table.get(&(ptr_addr as usize)) {
        *base_out = base as *mut c_void;
        *size_out = size;
        *key_out = key;
    } else {
        // Fallback for un-instrumented pointers: infinite size, wildcard key
        *base_out = std::ptr::null_mut();
        *size_out = usize::MAX;
        *key_out = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_check_bounds(
    ptr: *mut c_void,
    base: *mut c_void,
    size: usize,
    key: u64,
    access_size: usize,
    file: *const c_char,
    line: i32,
) {
    let ptr_val = ptr as usize;
    let base_val = base as usize;

    // Check for wildcard / infinite size fallback (un-instrumented pointer)
    if base.is_null() && size == usize::MAX {
        return;
    }

    let file_str = if file.is_null() {
        "unknown"
    } else {
        CStr::from_ptr(file).to_str().unwrap_or("unknown")
    };

    // 1. Spatial check
    if ptr_val < base_val || ptr_val + access_size > base_val + size {
        eprintln!(
            "stricc dynamic check failure: Out-of-bounds pointer access\n\
             -> Attempted to access address {:p} with size {}\n\
             -> Valid allocation base: {:p}, size: {}\n\
             -> Location: {}:{}",
            ptr, access_size, base, size, file_str, line
        );
        print_backtrace();
        libc::abort();
    }

    // 2. Temporal check (CETS)
    let is_uaf = if key != 0 {
        let key_table = KEY_TABLE.lock().unwrap();
        if let Some(&current_key) = key_table.get(&base_val) {
            if current_key != key {
                Some(Some(current_key))
            } else {
                None
            }
        } else {
            Some(None)
        }
    } else {
        None
    };

    if let Some(uaf_info) = is_uaf {
        match uaf_info {
            Some(current_key) => {
                eprintln!(
                    "stricc dynamic check failure: Use-after-free detected\n\
                     -> Pointer key: {}, Active allocation key: {}\n\
                     -> Location: {}:{}",
                    key, current_key, file_str, line
                );
            }
            None => {
                eprintln!(
                    "stricc dynamic check failure: Use-after-free detected (dangling pointer)\n\
                     -> Pointer key: {} (freed)\n\
                     -> Location: {}:{}",
                    key, file_str, line
                );
            }
        }
        print_backtrace();
        libc::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_abort(
    msg: *const c_char,
    file: *const c_char,
    line: i32,
) {
    let msg_str = if msg.is_null() {
        "aborted"
    } else {
        CStr::from_ptr(msg).to_str().unwrap_or("aborted")
    };
    let file_str = if file.is_null() {
        "unknown"
    } else {
        CStr::from_ptr(file).to_str().unwrap_or("unknown")
    };

    eprintln!(
        "stricc runtime abort: {} at {}:{}",
        msg_str, file_str, line
    );
    print_backtrace();
    libc::abort();
}

fn print_backtrace() {
    eprintln!("Backtrace:");
    backtrace::trace(|frame| {
        let ip = frame.ip();

        backtrace::resolve_frame(frame, |symbol| {
            if let Some(name) = symbol.name() {
                let name_str = name.to_string();
                // Filter out runtime library internals from backtrace for cleaner output
                if !name_str.contains("__stricc_rt") && !name_str.contains("backtrace::") {
                    if let (Some(filename), Some(line)) = (symbol.filename(), symbol.lineno()) {
                        eprintln!("  at {}:{} ({})", filename.display(), line, name_str);
                    } else {
                        eprintln!("  at {:p} ({})", ip, name_str);
                    }
                }
            }
        });
        true
    });
}

// Wrapper for malloc
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    if REAL_MALLOC.get().is_none() {
        static mut INITIALIZING: bool = false;
        if INITIALIZING {
            return bootstrap_malloc(size);
        }
        INITIALIZING = true;
        init_real_functions();
        INITIALIZING = false;
    }

    let recursed = get_in_malloc();
    if recursed {
        return bootstrap_malloc(size);
    }

    set_in_malloc(true);

    let result = (|| {
        let real_malloc = *REAL_MALLOC.get().unwrap();
        let ptr = real_malloc(size);
        if ptr.is_null() {
            return ptr;
        }

        let mut next_key = NEXT_KEY.lock().unwrap();
        let key = *next_key;
        *next_key += 1;

        let addr = ptr as usize;

        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.insert(addr, key);

        let mut shadow_table = SHADOW_TABLE.lock().unwrap();
        shadow_table.insert(addr, (addr, size, key));

        ptr
    })();

    set_in_malloc(false);
    result
}

// Wrapper for free
#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }

    if is_bootstrap_ptr(ptr) {
        return;
    }

    if REAL_FREE.get().is_none() {
        static mut INITIALIZING: bool = false;
        if INITIALIZING {
            return;
        }
        INITIALIZING = true;
        init_real_functions();
        INITIALIZING = false;
    }

    let recursed = get_in_malloc();
    if recursed {
        let real_free = *REAL_FREE.get().unwrap();
        real_free(ptr);
        return;
    }

    set_in_malloc(true);

    let addr = ptr as usize;

    let is_double_free = {
        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.remove(&addr).is_none()
    };

    if is_double_free {
        eprintln!(
            "stricc dynamic check failure: Double-free or invalid free of address {:p}",
            ptr
        );
        print_backtrace();
        libc::abort();
    }

    let mut shadow_table = SHADOW_TABLE.lock().unwrap();
    shadow_table.remove(&addr);

    let real_free = *REAL_FREE.get().unwrap();
    real_free(ptr);

    set_in_malloc(false);
}

// Wrapper for calloc
#[no_mangle]
pub unsafe extern "C" fn calloc(num: usize, size: usize) -> *mut c_void {
    if num == 0 || size == 0 {
        return std::ptr::null_mut();
    }
    let (total_size, overflow) = num.overflowing_mul(size);
    if overflow {
        return std::ptr::null_mut();
    }
    let ptr = malloc(total_size);
    if !ptr.is_null() {
        std::ptr::write_bytes(ptr, 0, total_size);
    }
    ptr
}

// Wrapper for realloc
#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return malloc(size);
    }
    if size == 0 {
        free(ptr);
        return std::ptr::null_mut();
    }

    if is_bootstrap_ptr(ptr) {
        return bootstrap_realloc(ptr, size);
    }

    if REAL_REALLOC.get().is_none() {
        static mut INITIALIZING: bool = false;
        if INITIALIZING {
            return bootstrap_realloc(ptr, size);
        }
        INITIALIZING = true;
        init_real_functions();
        INITIALIZING = false;
    }

    let recursed = get_in_malloc();
    if recursed {
        let real_realloc = *REAL_REALLOC.get().unwrap();
        return real_realloc(ptr, size);
    }

    set_in_malloc(true);

    let addr = ptr as usize;

    let old_key = {
        let key_table = KEY_TABLE.lock().unwrap();
        key_table.get(&addr).cloned()
    };

    let old_key = if let Some(key) = old_key {
        key
    } else {
        eprintln!(
            "stricc dynamic check failure: Invalid realloc of address {:p}",
            ptr
        );
        print_backtrace();
        libc::abort();
    };

    {
        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.remove(&addr);
    }
    let mut shadow_table = SHADOW_TABLE.lock().unwrap();
    shadow_table.remove(&addr);

    let real_realloc = *REAL_REALLOC.get().unwrap();
    let new_ptr = real_realloc(ptr, size);

    if !new_ptr.is_null() {
        let new_addr = new_ptr as usize;
        let mut next_key = NEXT_KEY.lock().unwrap();
        let new_key = *next_key;
        *next_key += 1;

        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.insert(new_addr, new_key);
        shadow_table.insert(new_addr, (new_addr, size, new_key));
    } else {
        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.insert(addr, old_key);
        shadow_table.insert(addr, (addr, size, old_key));
    }

    set_in_malloc(false);

    new_ptr
}
