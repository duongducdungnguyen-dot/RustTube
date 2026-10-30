use anyhow::{anyhow, Result};
use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use std::sync::Arc;

pub type MpvHandle = *mut c_void;

type MpvCreate = unsafe extern "C" fn() -> MpvHandle;
type MpvInitialize = unsafe extern "C" fn(ctx: MpvHandle) -> c_int;
type MpvDestroy = unsafe extern "C" fn(ctx: MpvHandle);
type MpvTerminateDestroy = unsafe extern "C" fn(ctx: MpvHandle);
type MpvSetOptionString = unsafe extern "C" fn(ctx: MpvHandle, name: *const c_char, data: *const c_char) -> c_int;
type MpvSetPropertyString = unsafe extern "C" fn(ctx: MpvHandle, name: *const c_char, data: *const c_char) -> c_int;
type MpvGetPropertyString = unsafe extern "C" fn(ctx: MpvHandle, name: *const c_char) -> *mut c_char;
type MpvFree = unsafe extern "C" fn(data: *mut c_void);
type MpvCommand = unsafe extern "C" fn(ctx: MpvHandle, args: *mut *const c_char) -> c_int;

pub struct MpvLibrary {
    _lib: Library,
    create: MpvCreate,
    initialize: MpvInitialize,
    terminate_destroy: MpvTerminateDestroy,
    set_option_string: MpvSetOptionString,
    set_property_string: MpvSetPropertyString,
    get_property_string: MpvGetPropertyString,
    free: MpvFree,
    command: MpvCommand,
}

impl MpvLibrary {
    pub fn load() -> Result<Arc<Self>> {
        let candidates = ["bin/libmpv-2.dll", "bin/mpv-2.dll", "libmpv-2.dll", "mpv-2.dll", "mpv-1.dll"];
        let mut loaded = None;
        for path in &candidates {
            if Path::new(path).exists() {
                if let Ok(lib) = unsafe { Library::new(path) } {
                    loaded = Some(lib);
                    break;
                }
            }
        }

        let lib = match loaded {
            Some(l) => l,
            None => unsafe {
                Library::new("libmpv-2.dll")
                    .or_else(|_| Library::new("mpv-2.dll"))
                    .map_err(|e| anyhow!("Could not load libmpv DLL. Ensure libmpv-2.dll is in bin/. Error: {}", e))?
            },
        };

        // SAFETY: Dynamically locating exported C symbols from libmpv DLL.
        // Symbols are standard libmpv API definitions and signatures match.
        unsafe {
            let create: Symbol<MpvCreate> = lib.get(b"mpv_create")?;
            let initialize: Symbol<MpvInitialize> = lib.get(b"mpv_initialize")?;
            let terminate_destroy: Symbol<MpvTerminateDestroy> = lib.get(b"mpv_terminate_destroy")?;
            let set_option_string: Symbol<MpvSetOptionString> = lib.get(b"mpv_set_option_string")?;
            let set_property_string: Symbol<MpvSetPropertyString> = lib.get(b"mpv_set_property_string")?;
            let get_property_string: Symbol<MpvGetPropertyString> = lib.get(b"mpv_get_property_string")?;
            let free: Symbol<MpvFree> = lib.get(b"mpv_free")?;
            let command: Symbol<MpvCommand> = lib.get(b"mpv_command")?;

            Ok(Arc::new(Self {
                create: *create,
                initialize: *initialize,
                terminate_destroy: *terminate_destroy,
                set_option_string: *set_option_string,
                set_property_string: *set_property_string,
                get_property_string: *get_property_string,
                free: *free,
                command: *command,
                _lib: lib,
            }))
        }
    }
}

pub struct MpvInstance {
    lib: Arc<MpvLibrary>,
    handle: MpvHandle,
}

// SAFETY: mpv handles are thread-safe for basic synchronized commands as per libmpv docs.
unsafe impl Send for MpvInstance {}
unsafe impl Sync for MpvInstance {}

impl MpvInstance {
    pub fn new(lib: Arc<MpvLibrary>, hwdec: &str) -> Result<Self> {
        // SAFETY: Calling mpv_create creates an uninitialized mpv instance pointer.
        let handle = unsafe { (lib.create)() };
        if handle.is_null() {
            return Err(anyhow!("Failed to create mpv handle"));
        }

        let instance = Self { lib, handle };

        instance.set_option("hwdec", hwdec)?;
        instance.set_option("vo", "gpu")?;
        instance.set_option("gpu-context", "d3d11")?;
        instance.set_option("keep-open", "yes")?;
        instance.set_option("idle", "yes")?;
        instance.set_option("ytdl", "no")?;

        // SAFETY: Initialize the configured instance.
        let code = unsafe { (instance.lib.initialize)(instance.handle) };
        if code < 0 {
            return Err(anyhow!("Failed to initialize mpv, error code {}", code));
        }

        Ok(instance)
    }

    pub fn set_option(&self, name: &str, value: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let c_val = CString::new(value)?;
        // SAFETY: Pointers are valid null-terminated strings for the duration of the call.
        let ret = unsafe { (self.lib.set_option_string)(self.handle, c_name.as_ptr(), c_val.as_ptr()) };
        if ret < 0 {
            Err(anyhow!("Failed to set option {}={}: code {}", name, value, ret))
        } else {
            Ok(())
        }
    }

    pub fn set_property(&self, name: &str, value: &str) -> Result<()> {
        let c_name = CString::new(name)?;
        let c_val = CString::new(value)?;
        // SAFETY: Valid null-terminated C string pointers passed into libmpv.
        let ret = unsafe { (self.lib.set_property_string)(self.handle, c_name.as_ptr(), c_val.as_ptr()) };
        if ret < 0 {
            Err(anyhow!("Failed to set property {}={}: code {}", name, value, ret))
        } else {
            Ok(())
        }
    }

    pub fn get_property(&self, name: &str) -> Option<String> {
        let c_name = CString::new(name).ok()?;
        // SAFETY: Returning allocated string must be freed via mpv_free.
        unsafe {
            let ptr = (self.lib.get_property_string)(self.handle, c_name.as_ptr());
            if ptr.is_null() {
                None
            } else {
                let res = CStr::from_ptr(ptr).to_string_lossy().to_string();
                (self.lib.free)(ptr as *mut c_void);
                Some(res)
            }
        }
    }

    pub fn command(&self, args: &[&str]) -> Result<()> {
        let cstrings: Vec<CString> = args.iter().map(|&s| CString::new(s).unwrap()).collect();
        let mut pointers: Vec<*const c_char> = cstrings.iter().map(|cs| cs.as_ptr()).collect();
        pointers.push(std::ptr::null());

        // SAFETY: Null-terminated array of null-terminated C strings.
        let ret = unsafe { (self.lib.command)(self.handle, pointers.as_mut_ptr()) };
        if ret < 0 {
            Err(anyhow!("Mpv command failed with code {}", ret))
        } else {
            Ok(())
        }
    }

    pub fn load_file(&self, url: &str) -> Result<()> {
        self.command(&["loadfile", url, "replace"])
    }

    pub fn play(&self) -> Result<()> {
        self.set_property("pause", "no")
    }

    pub fn pause(&self) -> Result<()> {
        self.set_property("pause", "yes")
    }

    pub fn toggle_pause(&self) -> Result<()> {
        self.command(&["cycle", "pause"])
    }

    pub fn seek_relative(&self, seconds: f64) -> Result<()> {
        self.command(&["seek", &format!("{:+}", seconds), "relative"])
    }

    pub fn seek_absolute(&self, seconds: f64) -> Result<()> {
        self.command(&["seek", &format!("{}", seconds), "absolute"])
    }

    pub fn set_volume(&self, volume: u32) -> Result<()> {
        self.set_property("volume", &volume.to_string())
    }

    pub fn set_speed(&self, speed: f32) -> Result<()> {
        self.set_property("speed", &format!("{:.2}", speed))
    }

    pub fn stop(&self) -> Result<()> {
        self.command(&["stop"])
    }
}

impl Drop for MpvInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: Safely terminates and destroys the mpv instance on shutdown.
            unsafe {
                (self.lib.terminate_destroy)(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}
