use std::collections::HashMap;
use std::sync::{Mutex, LazyLock, OnceLock};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::os::raw::{c_char, c_void};
use std::ffi::CStr;

// ============================================================================
// Raw stderr I/O helpers — avoids Rust's std I/O which isn't initialized
// in C binaries linked against this static library.
// ============================================================================

unsafe fn write_stderr(s: &[u8]) {
    libc::write(2, s.as_ptr() as *const c_void, s.len());
}

unsafe fn write_stderr_str(s: &str) {
    write_stderr(s.as_bytes());
}

unsafe fn write_stderr_cstr(p: *const c_char) {
    if p.is_null() {
        write_stderr(b"(null)");
    } else {
        // Manually compute length to avoid calling our own strlen wrapper
        let mut len = 0usize;
        while *p.add(len) != 0 {
            len += 1;
        }
        libc::write(2, p as *const c_void, len);
    }
}

unsafe fn write_stderr_usize(mut n: usize) {
    if n == 0 {
        write_stderr(b"0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 20;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    write_stderr(&buf[i..]);
}

unsafe fn write_stderr_i32(n: i32) {
    if n < 0 {
        write_stderr(b"-");
        write_stderr_usize(-(n as i64) as usize);
    } else {
        write_stderr_usize(n as usize);
    }
}

unsafe fn write_stderr_u64(mut n: u64) {
    if n == 0 {
        write_stderr(b"0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 20;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    write_stderr(&buf[i..]);
}

unsafe fn write_stderr_ptr(p: usize) {
    write_stderr(b"0x");
    if p == 0 {
        write_stderr(b"0");
        return;
    }
    let mut buf = [0u8; 16];
    let mut n = p;
    let mut i = 16;
    while n > 0 {
        i -= 1;
        let digit = (n & 0xf) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        n >>= 4;
    }
    write_stderr(&buf[i..]);
}

// ============================================================================
// Pointer Metadata: (base, size, key)
// ============================================================================

type Metadata = (usize, usize, u64);

static SHADOW_TABLE_INITIALIZED: AtomicBool = AtomicBool::new(false);

// Global Shadow Table: maps address of a pointer variable in memory -> Metadata of that pointer
static SHADOW_TABLE: LazyLock<Mutex<HashMap<usize, Metadata>>> = LazyLock::new(|| {
    Mutex::new(HashMap::new())
});

unsafe fn get_shadow_table() -> std::sync::MutexGuard<'static, HashMap<usize, Metadata>> {
    let g = SHADOW_TABLE.lock().unwrap();
    SHADOW_TABLE_INITIALIZED.store(true, Ordering::SeqCst);
    g
}

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

unsafe fn get_in_runtime() -> bool {
    let key = TLS_KEY.load(Ordering::Relaxed);
    if key == usize::MAX {
        return false;
    }
    let ptr = libc::pthread_getspecific(key as libc::pthread_key_t);
    !ptr.is_null()
}

unsafe fn set_in_runtime(val: bool) {
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

struct RuntimeGuard {
    was_in_runtime: bool,
}

impl RuntimeGuard {
    unsafe fn enter() -> Self {
        let was_in_runtime = get_in_runtime();
        if !was_in_runtime {
            set_in_runtime(true);
        }
        RuntimeGuard { was_in_runtime }
    }
}

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        if !self.was_in_runtime {
            unsafe {
                set_in_runtime(false);
            }
        }
    }
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
    let _guard = RuntimeGuard::enter();
    let mut table = SHADOW_TABLE.lock().unwrap();
    SHADOW_TABLE_INITIALIZED.store(true, Ordering::SeqCst);
    if base.is_null() && size == 0 {
        table.remove(&(ptr_addr as usize));
    } else {
        table.insert(ptr_addr as usize, (base as usize, size, key));
    }
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_shadow_load(
    ptr_addr: *mut c_void,
    base_out: *mut *mut c_void,
    size_out: *mut usize,
    key_out: *mut u64,
) {
    let _guard = RuntimeGuard::enter();
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

    // 1. Spatial check
    if ptr_val < base_val || ptr_val + access_size > base_val + size {
        write_stderr(b"stricc dynamic check failure: Out-of-bounds pointer access\n");
        write_stderr(b"-> Attempted to access address ");
        write_stderr_ptr(ptr_val);
        write_stderr(b" with size ");
        write_stderr_usize(access_size);
        write_stderr(b"\n-> Valid allocation base: ");
        write_stderr_ptr(base_val);
        write_stderr(b", size: ");
        write_stderr_usize(size);
        write_stderr(b"\n-> Location: ");
        write_stderr_cstr(file);
        write_stderr(b":");
        write_stderr_i32(line);
        write_stderr(b"\n");
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
                write_stderr(b"stricc dynamic check failure: Use-after-free detected\n");
                write_stderr(b"-> Pointer key: ");
                write_stderr_u64(key);
                write_stderr(b", Active allocation key: ");
                write_stderr_u64(current_key);
                write_stderr(b"\n-> Location: ");
                write_stderr_cstr(file);
                write_stderr(b":");
                write_stderr_i32(line);
                write_stderr(b"\n");
            }
            None => {
                write_stderr(b"stricc dynamic check failure: Use-after-free detected (dangling pointer)\n");
                write_stderr(b"-> Pointer key: ");
                write_stderr_u64(key);
                write_stderr(b" (freed)\n-> Location: ");
                write_stderr_cstr(file);
                write_stderr(b":");
                write_stderr_i32(line);
                write_stderr(b"\n");
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
    write_stderr(b"stricc runtime abort: ");
    if msg.is_null() {
        write_stderr(b"aborted");
    } else {
        write_stderr_cstr(msg);
    }
    write_stderr(b" at ");
    if file.is_null() {
        write_stderr(b"unknown");
    } else {
        write_stderr_cstr(file);
    }
    write_stderr(b":");
    write_stderr_i32(line);
    write_stderr(b"\n");
    print_backtrace();
    libc::abort();
}

fn print_backtrace() {
    unsafe { write_stderr(b"Backtrace:\n"); }
    let mut depth = 0u32;
    backtrace::trace(|frame| {
        let ip = frame.ip();
        unsafe {
            write_stderr(b"  #");
            write_stderr_usize(depth as usize);
            write_stderr(b" ");
            write_stderr_ptr(ip as usize);
            write_stderr(b"\n");
        }
        depth += 1;
        depth < 32 // limit depth
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

    let recursed = get_in_runtime();
    if recursed {
        if let Some(real_malloc) = REAL_MALLOC.get() {
            return real_malloc(size);
        } else {
            return bootstrap_malloc(size);
        }
    }

    let _guard = RuntimeGuard::enter();

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
    SHADOW_TABLE_INITIALIZED.store(true, Ordering::SeqCst);
    shadow_table.insert(addr, (addr, size, key));

    ptr
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

    let recursed = get_in_runtime();
    if recursed {
        if let Some(real_free) = REAL_FREE.get() {
            real_free(ptr);
        }
        return;
    }

    let _guard = RuntimeGuard::enter();

    let addr = ptr as usize;

    let is_double_free = {
        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.remove(&addr).is_none()
    };

    if is_double_free {
        write_stderr(b"stricc dynamic check failure: Double-free or invalid free of address ");
        write_stderr_ptr(addr);
        write_stderr(b"\n");
        print_backtrace();
        libc::abort();
    }

    let mut shadow_table = SHADOW_TABLE.lock().unwrap();
    shadow_table.remove(&addr);

    let real_free = *REAL_FREE.get().unwrap();
    real_free(ptr);
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

    let recursed = get_in_runtime();
    if recursed {
        if let Some(real_realloc) = REAL_REALLOC.get() {
            return real_realloc(ptr, size);
        } else {
            return bootstrap_realloc(ptr, size);
        }
    }

    let _guard = RuntimeGuard::enter();

    let addr = ptr as usize;

    let old_key = {
        let key_table = KEY_TABLE.lock().unwrap();
        key_table.get(&addr).cloned()
    };

    let old_key = if let Some(key) = old_key {
        key
    } else {
        write_stderr(b"stricc dynamic check failure: Invalid realloc of address ");
        write_stderr_ptr(addr);
        write_stderr(b"\n");
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
        SHADOW_TABLE_INITIALIZED.store(true, Ordering::SeqCst);
        shadow_table.insert(new_addr, (new_addr, size, new_key));
    } else {
        let mut key_table = KEY_TABLE.lock().unwrap();
        key_table.insert(addr, old_key);
        SHADOW_TABLE_INITIALIZED.store(true, Ordering::SeqCst);
        shadow_table.insert(addr, (addr, size, old_key));
    }

    new_ptr
}

// Global CFI Table: function pointer -> signature hash
static CFI_TABLE: LazyLock<Mutex<HashMap<usize, u64>>> = LazyLock::new(|| {
    Mutex::new(HashMap::new())
});

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_cfi_register(
    func_ptr: *mut c_void,
    signature_hash: u64,
) {
    let _guard = RuntimeGuard::enter();
    let mut table = CFI_TABLE.lock().unwrap();
    table.insert(func_ptr as usize, signature_hash);
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_cfi_check(
    func_ptr: *mut c_void,
    expected_hash: u64,
) {
    let _guard = RuntimeGuard::enter();
    if func_ptr.is_null() {
        write_stderr(b"stricc dynamic check failure: Indirect call to NULL pointer\n");
        print_backtrace();
        libc::abort();
    }

    let table = CFI_TABLE.lock().unwrap();
    if let Some(&hash) = table.get(&(func_ptr as usize)) {
        if hash != expected_hash {
            write_stderr(b"stricc dynamic check failure: CFI violation\n");
            write_stderr(b"-> Attempted to call function pointer ");
            write_stderr_ptr(func_ptr as usize);
            write_stderr(b" with mismatched signature\n");
            write_stderr(b"-> Expected hash: ");
            write_stderr_u64(expected_hash);
            write_stderr(b", Registered hash: ");
            write_stderr_u64(hash);
            write_stderr(b"\n");
            print_backtrace();
            libc::abort();
        }
    } else {
        write_stderr(b"stricc dynamic check failure: CFI violation\n");
        write_stderr(b"-> Attempted to call unregistered function pointer ");
        write_stderr_ptr(func_ptr as usize);
        write_stderr(b"\n");
        print_backtrace();
        libc::abort();
    }
}

#[no_mangle]
pub unsafe extern "C" fn __stricc_rt_find_metadata(
    ptr: *const c_void,
    base_out: *mut *mut c_void,
    size_out: *mut usize,
    key_out: *mut u64,
) -> i32 {
    if !SHADOW_TABLE_INITIALIZED.load(Ordering::Relaxed) {
        *base_out = std::ptr::null_mut();
        *size_out = usize::MAX;
        *key_out = 0;
        return 0;
    }
    let addr = ptr as usize;
    let table = SHADOW_TABLE.lock().unwrap();
    for (_, &(base, size, key)) in table.iter() {
        if base != 0 && size != usize::MAX && size != 0 {
            if addr >= base && addr < base + size {
                *base_out = base as *mut c_void;
                *size_out = size;
                *key_out = key;
                return 1;
            }
        }
    }
    *base_out = std::ptr::null_mut();
    *size_out = usize::MAX;
    *key_out = 0;
    0
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if n == 0 {
        return dest;
    }
    if get_in_runtime() {
        let d = dest as *mut u8;
        let s = src as *const u8;
        for i in 0..n {
            *d.add(i) = *s.add(i);
        }
        return dest;
    }
    let _guard = RuntimeGuard::enter();
    if dest.is_null() || src.is_null() {
        write_stderr(b"stricc dynamic check failure: Null pointer passed to memcpy\n");
        print_backtrace();
        libc::abort();
    }

    let mut dest_base = std::ptr::null_mut();
    let mut dest_size = usize::MAX;
    let mut dest_key = 0;
    __stricc_rt_find_metadata(dest, &mut dest_base, &mut dest_size, &mut dest_key);
    if !(dest_base.is_null() && dest_size == usize::MAX) {
        if (dest as usize) < (dest_base as usize) || (dest as usize) + n > (dest_base as usize) + dest_size {
            write_stderr(b"stricc dynamic check failure: memcpy dest out of bounds\n");
            print_backtrace();
            libc::abort();
        }
    }

    let mut src_base = std::ptr::null_mut();
    let mut src_size = usize::MAX;
    let mut src_key = 0;
    __stricc_rt_find_metadata(src, &mut src_base, &mut src_size, &mut src_key);
    if !(src_base.is_null() && src_size == usize::MAX) {
        if (src as usize) < (src_base as usize) || (src as usize) + n > (src_base as usize) + src_size {
            write_stderr(b"stricc dynamic check failure: memcpy src out of bounds\n");
            print_backtrace();
            libc::abort();
        }
    }

    libc::memmove(dest, src, n)
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        write_stderr(b"stricc dynamic check failure: Null pointer passed to strlen\n");
        print_backtrace();
        libc::abort();
    }
    if get_in_runtime() {
        let mut len = 0;
        while *s.add(len) != 0 {
            len += 1;
        }
        return len;
    }
    let _guard = RuntimeGuard::enter();

    let mut base = std::ptr::null_mut();
    let mut size = usize::MAX;
    let mut key = 0;
    __stricc_rt_find_metadata(s as *const c_void, &mut base, &mut size, &mut key);

    let s_addr = s as usize;
    if !base.is_null() || size != usize::MAX {
        let base_addr = base as usize;
        let max_len = (base_addr + size) - s_addr;
        let mut len = 0;
        loop {
            if len >= max_len {
                write_stderr(b"stricc dynamic check failure: String not null-terminated within bounds in strlen\n");
                print_backtrace();
                libc::abort();
            }
            if *s.add(len) == 0 {
                return len;
            }
            len += 1;
        }
    } else {
        libc::strlen(s)
    }
}

#[no_mangle]
pub unsafe extern "C" fn strcpy(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    if dest.is_null() || src.is_null() {
        write_stderr(b"stricc dynamic check failure: Null pointer passed to strcpy\n");
        print_backtrace();
        libc::abort();
    }
    if get_in_runtime() {
        let mut i = 0;
        loop {
            let c = *src.add(i);
            *dest.add(i) = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
        return dest;
    }
    let _guard = RuntimeGuard::enter();

    let len = strlen(src);

    let mut dest_base = std::ptr::null_mut();
    let mut dest_size = usize::MAX;
    let mut dest_key = 0;
    __stricc_rt_find_metadata(dest as *const c_void, &mut dest_base, &mut dest_size, &mut dest_key);
    if !(dest_base.is_null() && dest_size == usize::MAX) {
        if (dest as usize) < (dest_base as usize) || (dest as usize) + len + 1 > (dest_base as usize) + dest_size {
            write_stderr(b"stricc dynamic check failure: strcpy dest out of bounds\n");
            print_backtrace();
            libc::abort();
        }
    }

    libc::memmove(dest as *mut c_void, src as *const c_void, len + 1);
    dest
}

#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> i32 {
    if s1.is_null() || s2.is_null() {
        write_stderr(b"stricc dynamic check failure: Null pointer passed to strcmp\n");
        print_backtrace();
        libc::abort();
    }
    if get_in_runtime() {
        let mut i = 0;
        loop {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            if c1 != c2 {
                return (c1 as i32) - (c2 as i32);
            }
            if c1 == 0 {
                return 0;
            }
            i += 1;
        }
    }
    let _guard = RuntimeGuard::enter();
    let _ = strlen(s1);
    let _ = strlen(s2);
    libc::strcmp(s1, s2)
}

#[no_mangle]
pub unsafe extern "C" fn isalpha(c: i32) -> i32 {
    if c < -1 || c > 255 {
        write_stderr(b"stricc dynamic check failure: ctype.h argument out of range (");
        write_stderr_i32(c);
        write_stderr(b")\n");
        print_backtrace();
        libc::abort();
    }
    libc::isalpha(c)
}

#[no_mangle]
pub unsafe extern "C" fn isdigit(c: i32) -> i32 {
    if c < -1 || c > 255 {
        write_stderr(b"stricc dynamic check failure: ctype.h argument out of range (");
        write_stderr_i32(c);
        write_stderr(b")\n");
        print_backtrace();
        libc::abort();
    }
    libc::isdigit(c)
}

#[no_mangle]
pub unsafe extern "C" fn isspace(c: i32) -> i32 {
    if c < -1 || c > 255 {
        write_stderr(b"stricc dynamic check failure: ctype.h argument out of range (");
        write_stderr_i32(c);
        write_stderr(b")\n");
        print_backtrace();
        libc::abort();
    }
    libc::isspace(c)
}

#[no_mangle]
pub unsafe extern "C" fn tolower(c: i32) -> i32 {
    if c < -1 || c > 255 {
        write_stderr(b"stricc dynamic check failure: ctype.h argument out of range (");
        write_stderr_i32(c);
        write_stderr(b")\n");
        print_backtrace();
        libc::abort();
    }
    libc::tolower(c)
}

#[no_mangle]
pub unsafe extern "C" fn toupper(c: i32) -> i32 {
    if c < -1 || c > 255 {
        write_stderr(b"stricc dynamic check failure: ctype.h argument out of range (");
        write_stderr_i32(c);
        write_stderr(b")\n");
        print_backtrace();
        libc::abort();
    }
    libc::toupper(c)
}
