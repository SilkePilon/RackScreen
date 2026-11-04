//! glibc malloc tuning for a small board. By default glibc gives every
//! thread that allocates its own arena (up to eight per core) and grows the
//! mmap threshold after big frees, so freed buffers (a GIF decode, a k8s
//! list) stay as fragmented heap in many arenas instead of going back to the
//! OS. On the Pi that was most of the service's RSS.

/// Cap the arenas and fix the mmap threshold. Call first thing in `main`,
/// before any other thread starts.
pub fn tune() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: mallopt only sets allocator parameters.
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
        // Buffers of 256 KB and up (decode canvases, big responses) get
        // their own mapping and go back to the OS when freed. A fixed value
        // also turns off glibc's threshold growth. Frames (230 KB) stay on
        // the heap and are reused.
        libc::mallopt(libc::M_MMAP_THRESHOLD, 256 * 1024);
        libc::mallopt(libc::M_TRIM_THRESHOLD, 512 * 1024);
    }
}

/// Hand free heap pages back to the OS, after a burst of big allocations.
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim only releases free memory.
    unsafe {
        libc::malloc_trim(0);
    }
}
